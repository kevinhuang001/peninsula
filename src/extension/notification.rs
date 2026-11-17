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

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum NotificationPriority {
    Low,
    #[default]
    Normal,
    High,
}

/// The extension owns the contents of both views. Core owns the window and
/// injects the mandatory "Confirm" action into the alert presentation.
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

pub trait NotificationPresentation {
    fn alert_view(&self, notification: &Notification) -> NotificationAlertView;
    fn center_item_view(&self, notification: &Notification) -> NotificationCenterItemView;
}
