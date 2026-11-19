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

#[derive(Clone, Default)]
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
        let thread = handle.thread().clone();
        let mut threads = self
            .0
            .threads
            .lock()
            .map_err(|_| io::Error::other("task scope lock poisoned"))?;
        let mut finished = Vec::new();
        let previous = std::mem::take(&mut *threads);
        for completed in previous {
            if completed.1.is_finished() {
                finished.push(completed);
            } else {
                threads.push(completed);
            }
        }
        if self.0.token.is_cancelled() {
            thread.unpark();
            drop(threads);
            let _ = handle.join();
            for (_, completed) in finished {
                let _ = completed.join();
            }
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "task scope stopped",
            ));
        }
        threads.push((thread, handle));
        drop(threads);
        for (_, completed) in finished {
            let _ = completed.join();
        }
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

    pub fn shutdown(&self) {
        self.cancel();
        let handles = self
            .0
            .threads
            .lock()
            .map(|mut threads| std::mem::take(&mut *threads))
            .unwrap_or_default();
        for (_, handle) in handles {
            let _ = handle.join();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TaskRegistry(Arc<Mutex<HashMap<(ExtensionId, u64), TaskScope>>>);

impl TaskRegistry {
    pub(crate) fn scope(&self, id: &ExtensionId, generation: u64) -> TaskScope {
        self.0
            .lock()
            .map(|mut scopes| scopes.entry((id.clone(), generation)).or_default().clone())
            .unwrap_or_default()
    }

    pub(crate) fn cancel(&self, id: &ExtensionId, generation: u64) {
        let scope = self
            .0
            .lock()
            .ok()
            .and_then(|mut scopes| scopes.remove(&(id.clone(), generation)));
        if let Some(scope) = scope {
            scope.shutdown();
        }
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
        scope.shutdown();
        assert!(finished.load(Ordering::Acquire));
        assert!(scope.token().is_cancelled());
    }
}
