use super::ExtensionId;
use std::time::SystemTime;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationAction {
    pub id: String,
    pub label: String,
    pub data: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notification {
    pub source_id: ExtensionId,
    pub id: String,
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub priority: NotificationPriority,
    pub actions: Vec<NotificationAction>,
    pub created_at: SystemTime,
}

impl Notification {
    pub fn key(&self) -> NotificationKey {
        NotificationKey {
            source_extension_id: self.source_id.clone(),
            notification_id: self.id.clone(),
        }
    }
}

/// Stable identity shared by the alert queue and notification center.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NotificationKey {
    pub source_extension_id: ExtensionId,
    pub notification_id: String,
}

impl NotificationKey {
    pub fn stable_id(&self) -> String {
        format!("{}:{}", self.source_extension_id, self.notification_id)
    }

    pub fn from_stable_id(value: &str) -> Option<Self> {
        let (source, id) = value.split_once(':')?;
        if id.is_empty() {
            return None;
        }
        Some(Self {
            source_extension_id: ExtensionId::new(source.to_owned()).ok()?,
            notification_id: id.to_owned(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationStatus {
    Pending,
    Acknowledged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationAlertView {
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub actions: Vec<NotificationAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationCenterItemView {
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationPresentation {
    pub alert: NotificationAlertView,
    pub center_item: NotificationCenterItemView,
}

impl NotificationPresentation {
    pub fn from_notification(notification: &Notification) -> Self {
        Self {
            alert: NotificationAlertView {
                title: notification.title.clone(),
                body: notification.body.clone(),
                icon: notification.icon.clone(),
                actions: notification.actions.clone(),
            },
            center_item: NotificationCenterItemView {
                title: notification.title.clone(),
                body: notification.body.clone(),
                icon: notification.icon.clone(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum NotificationPriority {
    Low,
    #[default]
    Normal,
    High,
}

#[cfg(test)]
mod tests {
    use super::{ExtensionId, NotificationKey};

    #[test]
    fn stable_key_round_trips_and_scopes_local_ids_by_extension() {
        let first = NotificationKey {
            source_extension_id: ExtensionId::builtin("builtin.downloads/notifications"),
            notification_id: "download:42".into(),
        };
        let second = NotificationKey {
            source_extension_id: ExtensionId::builtin("builtin.pomodoro/notifications"),
            notification_id: "download:42".into(),
        };
        assert_ne!(first, second);
        assert_eq!(
            NotificationKey::from_stable_id(&first.stable_id()),
            Some(first)
        );
    }
}
