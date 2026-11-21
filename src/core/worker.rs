//! A worker's owner cancels and joins it before reporting Stopped.
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
    time::Duration,
};

pub struct Worker<T> {
    pub updates: Receiver<T>,
    latest: Option<crate::core::latest::LatestReceiver<T>>,
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}
impl<T: Send + 'static> Worker<T> {
    #[cfg(test)]
    pub fn from_receiver(updates: Receiver<T>) -> Self {
        let (stop, _) = mpsc::channel();
        Self {
            updates,
            latest: None,
            stop,
            thread: None,
        }
    }
    pub fn take(&self) -> Option<T> {
        self.latest
            .as_ref()
            .and_then(|rx| rx.take())
            .or_else(|| self.updates.try_iter().last())
    }
    pub fn try_recv(&self) -> Result<T, mpsc::TryRecvError> {
        self.updates.try_recv()
    }
    pub fn spawn_scoped(
        tasks: &crate::sdk::tasks::TaskScope,
        name: &str,
        interval: Duration,
        mut sample: impl FnMut() -> T + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, updates) = mpsc::channel();
        let (stop, stopped) = mpsc::channel();
        tasks.spawn(name, move |cancel| {
            while !cancel.is_cancelled() && stopped.try_recv() == Err(mpsc::TryRecvError::Empty) {
                if sender.send(sample()).is_err() || cancel.wait(interval) {
                    break;
                }
            }
        })?;
        Ok(Self {
            updates,
            latest: None,
            stop,
            thread: None,
        })
    }

    pub fn spawn_scoped_latest(
        tasks: &crate::sdk::tasks::TaskScope,
        name: &str,
        interval: Duration,
        mut sample: impl FnMut(&crate::sdk::tasks::CancellationToken) -> T + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, latest) = crate::core::latest::channel();
        let (_, updates) = mpsc::channel();
        let (stop, stopped) = mpsc::channel();
        tasks.spawn(name, move |cancel| {
            while !cancel.is_cancelled() && stopped.try_recv() == Err(mpsc::TryRecvError::Empty) {
                let started = std::time::Instant::now();
                if !sender.send(sample(&cancel))
                    || cancel.wait(interval.saturating_sub(started.elapsed()))
                {
                    break;
                }
            }
        })?;
        Ok(Self {
            updates,
            latest: Some(latest),
            stop,
            thread: None,
        })
    }

    #[cfg(test)]
    pub fn spawn(
        name: &str,
        interval: Duration,
        mut sample: impl FnMut() -> T + Send + 'static,
    ) -> Self {
        let (sender, updates) = mpsc::channel();
        let (stop, cancelled) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || loop {
                if cancelled.try_recv().is_ok() {
                    break;
                }
                if sender.send(sample()).is_err() {
                    break;
                }
                if cancelled.recv_timeout(interval) != Err(mpsc::RecvTimeoutError::Timeout) {
                    break;
                }
            })
            .expect("spawn extension worker");
        Self {
            updates,
            latest: None,
            stop,
            thread: Some(thread),
        }
    }
}
impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    #[test]
    fn cancelling_wakes_sleep_and_joins_worker() {
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        let worker = Worker::spawn("test", Duration::from_secs(60), move || {
            counter.fetch_add(1, Ordering::SeqCst)
        });
        worker.updates.recv_timeout(Duration::from_secs(1)).unwrap();
        let before = std::time::Instant::now();
        drop(worker);
        assert!(before.elapsed() < Duration::from_secs(1));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
