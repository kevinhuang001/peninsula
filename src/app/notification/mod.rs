mod view;

use crate::{
    downloads::{Download, DownloadState},
    extension::{ExtensionId, Notification, NotificationAction, NotificationPriority},
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime},
};
pub use view::{show, show_center};

struct NotificationRecord {
    notification: Notification,
    seen: bool,
}

#[derive(Default)]
pub struct Notifications {
    records: VecDeque<NotificationRecord>,
    queued: VecDeque<(ExtensionId, String)>,
    current: Option<((ExtensionId, String), Instant)>,
}

impl Notifications {
    pub fn publish(&mut self, notification: Notification) {
        let key = (notification.source_id.clone(), notification.id.clone());
        if self
            .records
            .iter()
            .any(|record| record.notification.source_id == key.0 && record.notification.id == key.1)
        {
            return;
        }
        self.queued.push_back(key);
        self.records.push_back(NotificationRecord {
            notification,
            seen: false,
        });
    }

    pub fn push_downloads(&mut self, downloads: impl IntoIterator<Item = Download>) {
        for download in downloads {
            if download.state != DownloadState::Complete {
                continue;
            }
            let path = download.path.to_string_lossy().into_owned();
            self.publish(Notification {
                source_id: ExtensionId::builtin("builtin.downloads/notifications"),
                id: format!("{}:{}", download.browser, download.id),
                title: "下载完成".into(),
                body: download.filename(),
                icon: Some("download".into()),
                priority: NotificationPriority::Normal,
                actions: vec![
                    NotificationAction {
                        id: "open".into(),
                        label: "打开文件".into(),
                        data: Some(path.clone()),
                    },
                    NotificationAction {
                        id: "reveal".into(),
                        label: "显示所在位置".into(),
                        data: Some(path),
                    },
                ],
                created_at: SystemTime::now(),
            });
        }
    }

    pub fn update(&mut self, now: Instant, may_show: bool) -> bool {
        let expired = self
            .current
            .as_ref()
            .is_some_and(|(_, shown_at)| now.duration_since(*shown_at) >= Duration::from_secs(8));
        if expired {
            self.current = None;
        }
        if self.current.is_none() && may_show {
            while let Some(key) = self.queued.pop_front() {
                let unread = self.records.iter().any(|record| {
                    !record.seen
                        && record.notification.source_id == key.0
                        && record.notification.id == key.1
                });
                if unread {
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
            (record.notification.source_id == key.0 && record.notification.id == key.1)
                .then_some(&record.notification)
        })
    }

    pub fn dismiss(&mut self) {
        if let Some((key, _)) = self.current.take() {
            if let Some(record) = self.records.iter_mut().find(|record| {
                record.notification.source_id == key.0 && record.notification.id == key.1
            }) {
                record.seen = true;
            }
        }
    }

    pub fn suppress(&mut self) {
        self.current = None;
    }

    pub fn keep_visible(&mut self, now: Instant) {
        if let Some((_, shown_at)) = &mut self.current {
            *shown_at = now;
        }
    }

    pub fn has_unread(&self) -> bool {
        self.records.iter().any(|record| !record.seen)
    }

    pub fn unread(&self) -> impl Iterator<Item = &Notification> {
        self.records
            .iter()
            .filter(|record| !record.seen)
            .map(|record| &record.notification)
    }

    pub fn mark_seen(&mut self, source: &ExtensionId, id: &str) {
        if let Some(record) = self
            .records
            .iter_mut()
            .find(|record| &record.notification.source_id == source && record.notification.id == id)
        {
            record.seen = true;
        }
        if self
            .current
            .as_ref()
            .is_some_and(|(key, _)| &key.0 == source && key.1 == id)
        {
            self.current = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> Download {
        Download {
            id: id.into(),
            browser: "Chrome",
            path: "/tmp/a".into(),
            received: 100,
            total: Some(100),
            bytes_per_second: 0.0,
            state: DownloadState::Complete,
        }
    }

    #[test]
    fn published_notifications_wait_for_policy_and_are_seen_only_after_dismissal() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push_downloads([item("1"), item("2")]);
        assert!(!notices.update(now, false));
        assert!(notices.has_unread());
        assert!(notices.update(now, true));
        assert_eq!(notices.current().unwrap().id, "Chrome:1");
        assert!(notices.has_unread());
        assert!(notices.update(now + Duration::from_secs(9), true));
        assert_eq!(notices.current().unwrap().id, "Chrome:2");
        notices.dismiss();
        assert_eq!(notices.unread().count(), 1);
        notices.mark_seen(
            &ExtensionId::builtin("builtin.downloads/notifications"),
            "Chrome:1",
        );
        assert!(!notices.has_unread());
        assert!(notices.current().is_none());
    }

    #[test]
    fn quiet_mode_hides_popups_without_discarding_unread_notifications() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push_downloads([item("1"), item("2")]);
        assert!(notices.update(now, true));
        notices.suppress();
        assert!(notices.current().is_none());
        assert!(notices.has_unread());
        assert!(!notices.update(now + Duration::from_secs(10), false));
        assert!(notices.update(now + Duration::from_secs(11), true));
        assert_eq!(notices.current().unwrap().id, "Chrome:2");
    }

    #[test]
    fn notifications_are_deduplicated_by_source_and_id() {
        let mut notices = Notifications::default();
        notices.push_downloads([item("same"), item("same")]);
        assert_eq!(notices.unread().count(), 1);
    }
}
