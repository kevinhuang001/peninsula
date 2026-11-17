mod view;

use crate::{
    downloads::{Download, DownloadState},
    extension::{ExtensionId, Notification, NotificationAction, NotificationPriority},
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime},
};
pub use view::{alert_extra_height, show, show_center, AlertInteraction, CenterInteraction};

type NotificationKey = (ExtensionId, String);

struct NotificationRecord {
    notification: Notification,
    acknowledged: bool,
}

#[derive(Default)]
pub struct Notifications {
    records: VecDeque<NotificationRecord>,
    queued: VecDeque<NotificationKey>,
    current: Option<(NotificationKey, Instant)>,
}

impl Notifications {
    pub fn publish(&mut self, notification: Notification) {
        let key = (notification.source_id.clone(), notification.id.clone());
        if self.records.iter().any(|record| {
            record.notification.source_id == key.0 && record.notification.id == key.1
        }) {
            return;
        }
        self.queued.push_back(key);
        self.records.push_back(NotificationRecord {
            notification,
            acknowledged: false,
        });
    }

    pub fn push_downloads(&mut self, downloads: impl IntoIterator<Item = Download>) {
        for download in downloads {
            if download.state == DownloadState::Complete {
                self.publish(crate::builtins::download::completion_notification(&download));
            }
        }
    }

    pub fn update(&mut self, now: Instant, may_show: bool) -> bool {
        if self.current.as_ref().is_some_and(|(_, started)| {
            now.saturating_duration_since(*started) >= Duration::from_secs(8)
        }) {
            self.current = None;
        }
        if self.current.is_none() && may_show {
            while let Some(key) = self.queued.pop_front() {
                if self.is_pending(&key.0, &key.1) {
                    self.current = Some((key, now));
                    return true;
                }
            }
        }
        false
    }

    pub fn current(&self) -> Option<&Notification> {
        let key = &self.current.as_ref()?.0;
        self.records.iter().find_map(|record| {
            (record.notification.source_id == key.0 && record.notification.id == key.1
                && !record.acknowledged)
                .then_some(&record.notification)
        })
    }

    /// Closing or timing out an alert never removes it from the center.
    pub fn close_alert(&mut self) {
        self.current = None;
    }

    /// Confirm and every extension action permanently resolve the notification.
    pub fn acknowledge_current(&mut self) {
        if let Some((key, _)) = self.current.take() {
            self.acknowledge(&key.0, &key.1);
        }
    }

    pub fn acknowledge(&mut self, source: &ExtensionId, id: &str) {
        if let Some(record) = self.records.iter_mut().find(|record| {
            &record.notification.source_id == source && record.notification.id == id
        }) {
            record.acknowledged = true;
        }
        self.queued.retain(|key| &key.0 != source || key.1 != id);
        if self.current.as_ref().is_some_and(|(key, _)| {
            &key.0 == source && key.1 == id
        }) {
            self.current = None;
        }
    }

    pub fn clear_all(&mut self) {
        for record in &mut self.records {
            record.acknowledged = true;
        }
        self.queued.clear();
        self.current = None;
    }

    pub fn open_from_center(&mut self, source: &ExtensionId, id: &str, now: Instant) -> bool {
        if !self.is_pending(source, id) {
            return false;
        }
        self.queued.retain(|key| &key.0 != source || key.1 != id);
        self.current = Some(((source.clone(), id.to_owned()), now));
        true
    }

    pub fn suppress(&mut self) {
        // DND suppresses the popup, but preserves all pending center entries.
        self.current = None;
    }

    pub fn keep_visible(&mut self, now: Instant) {
        if let Some((_, shown_at)) = &mut self.current {
            *shown_at = now;
        }
    }

    pub fn is_pending(&self, source: &ExtensionId, id: &str) -> bool {
        self.records.iter().any(|record| {
            !record.acknowledged
                && &record.notification.source_id == source
                && record.notification.id == id
        })
    }

    pub fn pending_count(&self) -> usize {
        self.records.iter().filter(|record| !record.acknowledged).count()
    }

    pub fn pending(&self) -> impl Iterator<Item = &Notification> {
        self.records
            .iter()
            .filter(|record| !record.acknowledged)
            .map(|record| &record.notification)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn item(id: &str) -> Download {
        Download {
            id: id.into(),
            browser: "Chrome",
            path: PathBuf::from("/tmp/a"),
            received: 100,
            total: Some(100),
            bytes_per_second: 0.0,
            state: DownloadState::Complete,
        }
    }

    #[test]
    fn popup_timeout_or_close_keeps_center_item_until_confirmed() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push_downloads([item("1")]);
        assert!(notices.update(now, true));
        notices.close_alert();
        assert_eq!(notices.pending_count(), 1);
        assert!(notices.current().is_none());
        assert!(!notices.update(now + Duration::from_secs(9), true));
        let source = ExtensionId::builtin("builtin.downloads/notifications");
        assert!(notices.open_from_center(&source, "Chrome:1", now));
        notices.acknowledge_current();
        assert_eq!(notices.pending_count(), 0);
    }

    #[test]
    fn any_action_can_acknowledge_a_notification() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push_downloads([item("1"), item("2")]);
        assert!(notices.update(now, true));
        notices.acknowledge_current();
        assert_eq!(notices.pending_count(), 1);
        assert!(notices.update(now + Duration::from_secs(1), true));
        assert_eq!(notices.current().unwrap().id, "Chrome:2");
    }

    #[test]
    fn clear_all_empties_center_and_pending_popups() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push_downloads([item("1"), item("2")]);
        notices.update(now, true);
        notices.clear_all();
        assert_eq!(notices.pending_count(), 0);
        assert!(notices.current().is_none());
        assert!(notices.pending().next().is_none());
        assert!(!notices.update(now + Duration::from_secs(9), true));
    }

    #[test]
    fn do_not_disturb_suppresses_popups_without_deleting_center() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push_downloads([item("1"), item("2")]);
        assert!(notices.update(now, true));
        notices.suppress();
        assert_eq!(notices.pending_count(), 2);
        assert!(notices.update(now + Duration::from_secs(9), true));
        assert_eq!(notices.current().unwrap().id, "Chrome:2");
    }

    #[test]
    fn notifications_are_deduplicated_by_source_and_id() {
        let mut notices = Notifications::default();
        notices.push_downloads([item("same"), item("same")]);
        assert_eq!(notices.pending_count(), 1);
    }
}
