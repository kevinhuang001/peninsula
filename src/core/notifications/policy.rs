use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct NotificationPolicy {
    pub alert_timeout: Duration,
    pub max_alert_actions: usize,
    pub max_center_items: usize,
}

impl Default for NotificationPolicy {
    fn default() -> Self {
        Self {
            alert_timeout: Duration::from_secs(8),
            max_alert_actions: 4,
            max_center_items: 3,
        }
    }
}
