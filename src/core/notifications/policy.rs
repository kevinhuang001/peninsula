use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct NotificationPolicy {
    pub alert_timeout: Duration,
    pub max_alert_actions: usize,
}

impl Default for NotificationPolicy {
    fn default() -> Self {
        Self {
            alert_timeout: Duration::from_secs(8),
            max_alert_actions: 4,
        }
    }
}
