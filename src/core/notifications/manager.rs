use super::{policy::NotificationPolicy, store::NotificationStore};
use crate::extension::{Notification, NotificationAction, NotificationKey, NotificationStatus};
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct NotificationPresentationState {
    current: Option<(NotificationKey, Instant)>,
}

impl NotificationPresentationState {
    pub fn current_key(&self) -> Option<&NotificationKey> {
        self.current.as_ref().map(|(key, _)| key)
    }

    pub fn is_showing(&self) -> bool {
        self.current.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedNotificationAction {
    pub key: NotificationKey,
    pub action: NotificationAction,
}

#[derive(Clone, Debug, Default)]
pub struct NotificationManager {
    store: NotificationStore,
    alert_queue: VecDeque<NotificationKey>,
    presentation: NotificationPresentationState,
    policy: NotificationPolicy,
    action_log: Vec<RecordedNotificationAction>,
}

impl NotificationManager {
    pub fn publish(&mut self, notification: Notification) -> bool {
        let key = notification.key();
        if !self.store.insert(notification) {
            return false;
        }
        self.alert_queue.push_back(key);
        true
    }

    pub fn update(&mut self, now: Instant, may_show: bool, quiet: bool) -> bool {
        if self
            .presentation
            .current
            .as_ref()
            .is_some_and(|(_, shown_at)| {
                now.saturating_duration_since(*shown_at) >= self.policy.alert_timeout
            })
        {
            self.presentation.current = None;
        }
        if quiet {
            self.suppress_alert();
            return false;
        }
        if !may_show || self.presentation.is_showing() {
            return false;
        }
        while let Some(key) = self.alert_queue.pop_front() {
            let Some(record) = self.store.get(&key) else {
                continue;
            };
            if record.status != NotificationStatus::Pending {
                continue;
            }
            self.presentation.current = Some((key, now));
            return true;
        }
        false
    }

    pub fn current(&self) -> Option<&Notification> {
        let key = self.presentation.current_key()?;
        self.store
            .get(key)
            .filter(|record| record.status == NotificationStatus::Pending)
            .map(|record| &record.notification)
    }

    pub fn pending(&self) -> impl Iterator<Item = &Notification> {
        self.store.pending()
    }

    pub fn pending_count(&self) -> usize {
        self.store.pending_count()
    }

    #[allow(dead_code)] // Exposes the unified Pending/Acknowledged state to host policy.
    pub fn status(&self, key: &NotificationKey) -> Option<NotificationStatus> {
        self.store.get(key).map(|record| record.status)
    }

    pub fn policy(&self) -> NotificationPolicy {
        self.policy
    }

    pub fn dismiss_alert(&mut self) {
        self.presentation.current = None;
    }

    pub fn show_again(&mut self, key: &NotificationKey, now: Instant) -> bool {
        let Some(record) = self.store.get(key) else {
            return false;
        };
        if record.status != NotificationStatus::Pending {
            return false;
        }
        self.alert_queue.retain(|queued| queued != key);
        self.presentation.current = Some((key.clone(), now));
        true
    }

    pub fn acknowledge(&mut self, key: &NotificationKey) -> bool {
        let changed = self.store.acknowledge(key);
        if changed {
            self.alert_queue.retain(|queued| queued != key);
            if self.presentation.current_key() == Some(key) {
                self.presentation.current = None;
            }
        }
        changed
    }

    pub fn acknowledge_all(&mut self) -> usize {
        self.alert_queue.clear();
        self.presentation.current = None;
        self.store.acknowledge_all()
    }

    pub fn record_action(&mut self, key: NotificationKey, action: NotificationAction) {
        self.action_log
            .push(RecordedNotificationAction { key, action });
    }

    #[allow(dead_code)] // Keeps an auditable record of extension alert operations.
    pub fn action_log(&self) -> &[RecordedNotificationAction] {
        &self.action_log
    }

    pub fn keep_alert_visible(&mut self, now: Instant) {
        if let Some((_, shown_at)) = &mut self.presentation.current {
            *shown_at = now;
        }
    }

    /// Quiet mode postpones the visible alert without changing notification
    /// status, so it can be presented after quiet mode ends.
    pub fn suppress_alert(&mut self) {
        if let Some((key, _)) = self.presentation.current.take() {
            if self
                .store
                .get(&key)
                .is_some_and(|record| record.status == NotificationStatus::Pending)
                && !self.alert_queue.contains(&key)
            {
                self.alert_queue.push_front(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NotificationManager;
    use crate::extension::{
        ExtensionId, Notification, NotificationAction, NotificationPriority, NotificationStatus,
    };
    use std::time::{Duration, Instant, SystemTime};

    fn item(id: &str) -> Notification {
        Notification {
            source_id: ExtensionId::builtin("builtin.test/notifications"),
            id: id.into(),
            title: format!("Notification {id}"),
            body: "body".into(),
            icon: None,
            priority: NotificationPriority::Normal,
            actions: vec![NotificationAction {
                id: "open".into(),
                label: "Open".into(),
                data: None,
            }],
            created_at: SystemTime::now(),
        }
    }

    #[test]
    fn close_and_timeout_only_change_presentation_state() {
        let now = Instant::now();
        let mut manager = NotificationManager::default();
        let notification = item("one");
        let key = notification.key();
        manager.publish(notification);
        assert!(manager.update(now, true, false));
        manager.dismiss_alert();
        assert_eq!(manager.pending_count(), 1);
        assert!(!manager.update(now + Duration::from_secs(9), true, false));
        assert!(manager.show_again(&key, now + Duration::from_secs(10)));
        assert_eq!(manager.pending_count(), 1);
    }

    #[test]
    fn action_acknowledgement_and_clear_all_remove_pending_queue_entries() {
        let now = Instant::now();
        let mut manager = NotificationManager::default();
        manager.publish(item("one"));
        manager.publish(item("two"));
        assert!(manager.update(now, true, false));
        assert_eq!(manager.acknowledge_all(), 2);
        assert_eq!(manager.pending_count(), 0);
        assert!(!manager.update(now + Duration::from_secs(20), true, false));
        assert_eq!(manager.pending_count(), 0);
        assert!(manager
            .store
            .get(&item("one").key())
            .is_some_and(|record| record.status == NotificationStatus::Acknowledged));
    }

    #[test]
    fn duplicate_publication_is_ignored_and_quiet_mode_preserves_queue() {
        let now = Instant::now();
        let mut manager = NotificationManager::default();
        let notification = item("same");
        manager.publish(notification.clone());
        assert!(!manager.publish(notification));
        assert!(!manager.update(now, true, true));
        assert_eq!(manager.pending_count(), 1);
        assert!(manager.update(now + Duration::from_secs(1), true, false));
        manager.suppress_alert();
        assert_eq!(manager.pending_count(), 1);
        assert!(manager.update(now + Duration::from_secs(2), true, false));
    }

    #[test]
    fn single_acknowledgement_removes_the_current_alert_and_its_queued_duplicate() {
        let now = Instant::now();
        let mut manager = NotificationManager::default();
        let first = item("first");
        let first_key = first.key();
        manager.publish(first.clone());
        manager.publish(item("second"));
        assert_eq!(
            manager.status(&first_key),
            Some(NotificationStatus::Pending)
        );
        assert!(manager.update(now, true, false));
        manager.record_action(
            first_key.clone(),
            NotificationAction {
                id: "open".into(),
                label: "Open".into(),
                data: None,
            },
        );
        assert!(manager.acknowledge(&first_key));
        assert_eq!(manager.current(), None);
        assert_eq!(
            manager.status(&first_key),
            Some(NotificationStatus::Acknowledged)
        );
        assert_eq!(manager.action_log().len(), 1);
        assert!(manager.update(now + Duration::from_secs(1), true, false));
        assert_eq!(
            manager
                .current()
                .map(|notification| notification.id.as_str()),
            Some("second")
        );
        assert_eq!(manager.pending_count(), 1);
    }
}
