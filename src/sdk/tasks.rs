use crate::extension::ExtensionId;
use std::{
    collections::HashMap,
    fmt, io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle, Thread},
    time::Duration,
};

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Waits for the duration or returns early when the owning extension stops.
    pub fn wait(&self, duration: Duration) -> bool {
        let deadline = std::time::Instant::now() + duration;
        while !self.is_cancelled() {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            thread::park_timeout(remaining.min(Duration::from_millis(100)));
        }
        true
    }
}

#[derive(Default)]
struct ScopeInner {
    token: CancellationToken,
    threads: Mutex<Vec<(Thread, JoinHandle<()>)>>,
}

#[derive(Clone, Default)]
pub struct TaskScope(Arc<ScopeInner>);

impl fmt::Debug for TaskScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskScope")
            .field("cancelled", &self.0.token.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl TaskScope {
    pub fn token(&self) -> CancellationToken {
        self.0.token.clone()
    }

    /// Starts extension-owned work. Every task receives the scope's shared
    /// cancellation token and is joined when the extension stops.
    pub fn spawn(
        &self,
        name: impl Into<String>,
        task: impl FnOnce(CancellationToken) + Send + 'static,
    ) -> io::Result<()> {
        let mut threads = self
            .0
            .threads
            .lock()
            .map_err(|_| io::Error::other("task scope lock poisoned"))?;
        if self.0.token.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "task scope stopped",
            ));
        }
        let token = self.0.token.clone();
        let handle = thread::Builder::new()
            .name(name.into())
            .spawn(move || task(token))?;
        threads.push((handle.thread().clone(), handle));
        drop(threads);
        self.reap();
        Ok(())
    }

    pub fn cancel(&self) {
        self.0.token.cancel();
        if let Ok(threads) = self.0.threads.lock() {
            for (thread, _) in threads.iter() {
                thread.unpark();
            }
        }
    }

    fn reap(&self) -> bool {
        let Ok(mut threads) = self.0.threads.lock() else {
            return false;
        };
        let mut index = 0;
        while index < threads.len() {
            if threads[index].1.is_finished() {
                let (_, finished) = threads.swap_remove(index);
                let _ = finished.join();
            } else {
                index += 1;
            }
        }
        threads.is_empty()
    }

    pub fn shutdown(&self) -> bool {
        self.cancel();
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        loop {
            if self.reap() {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

type Scopes = HashMap<(ExtensionId, u64), TaskScope>;

#[derive(Clone, Debug, Default)]
pub(crate) struct TaskRegistry {
    active: Arc<Mutex<Scopes>>,
    stopping: Arc<Mutex<Scopes>>,
}

impl TaskRegistry {
    pub(crate) fn scope(&self, id: &ExtensionId, generation: u64) -> TaskScope {
        self.active
            .lock()
            .map(|mut scopes| scopes.entry((id.clone(), generation)).or_default().clone())
            .unwrap_or_default()
    }

    pub(crate) fn cancel(&self, id: &ExtensionId, generation: u64) -> bool {
        let key = (id.clone(), generation);
        let scope = self
            .active
            .lock()
            .ok()
            .and_then(|mut scopes| scopes.remove(&key));
        if let Some(scope) = scope {
            if !scope.shutdown() {
                if let Ok(mut stopping) = self.stopping.lock() {
                    stopping.insert(key, scope);
                }
                return false;
            }
        }
        self.stopping
            .lock()
            .is_ok_and(|stopping| !stopping.contains_key(&key))
    }

    pub(crate) fn reap(&self) {
        if let Ok(mut stopping) = self.stopping.lock() {
            stopping.retain(|_, scope| !scope.reap());
        }
    }

    pub(crate) fn can_start(&self, id: &ExtensionId) -> bool {
        self.stopping
            .lock()
            .is_ok_and(|scopes| scopes.keys().all(|(owner, _)| owner != id))
    }
}

#[cfg(test)]
mod tests {
    use super::TaskScope;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::Duration;

    #[test]
    fn shutdown_cancels_and_joins_extension_tasks() {
        let scope = TaskScope::default();
        let finished = Arc::new(AtomicBool::new(false));
        let task_finished = finished.clone();
        scope
            .spawn("sdk-test", move |cancel| {
                while !cancel.is_cancelled() {
                    std::thread::park_timeout(Duration::from_millis(10));
                }
                task_finished.store(true, Ordering::Release);
            })
            .unwrap();
        assert!(scope.shutdown());
        assert!(finished.load(Ordering::Acquire));
        assert!(scope.token().is_cancelled());
    }
}
