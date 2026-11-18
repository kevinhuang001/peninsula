//! Single-slot mailbox: consumers always read the newest observation.
use std::sync::{Arc, Mutex, Weak};
pub(crate) struct LatestReceiver<T>(Arc<Mutex<Option<T>>>);
pub(crate) struct LatestSender<T>(Weak<Mutex<Option<T>>>);
pub(crate) fn channel<T>() -> (LatestSender<T>, LatestReceiver<T>) {
    let slot = Arc::new(Mutex::new(None));
    (LatestSender(Arc::downgrade(&slot)), LatestReceiver(slot))
}
impl<T> LatestSender<T> {
    pub fn send(&self, value: T) -> bool {
        let Some(slot) = self.0.upgrade() else {
            return false;
        };
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
        true
    }
}
impl<T> LatestReceiver<T> {
    pub fn take(&self) -> Option<T> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn stalled_consumer_gets_latest_value_and_drop_stops_producer() {
        let (tx, rx) = super::channel();
        for i in 0..100 {
            assert!(tx.send(i));
        }
        assert_eq!(rx.take(), Some(99));
        assert_eq!(rx.take(), None);
        drop(rx);
        assert!(!tx.send(100));
    }
}
