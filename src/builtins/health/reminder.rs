use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Reminder {
    pub last_action: SystemTime,
    pub last_notification: Option<SystemTime>,
}

impl Reminder {
    pub fn new(now: SystemTime) -> Self {
        Self {
            last_action: now,
            last_notification: None,
        }
    }

    pub fn next_due(&self, interval: Duration) -> SystemTime {
        let completed = self.last_notification.map_or(0, |last| {
            last.duration_since(self.last_action)
                .unwrap_or_default()
                .as_secs()
                / interval.as_secs()
                + 1
        });
        self.last_action + interval * completed.max(1).min(u32::MAX as u64) as u32
    }

    pub fn remaining(&self, now: SystemTime, interval: Duration) -> Duration {
        self.next_due(interval)
            .duration_since(now)
            .unwrap_or_default()
    }

    pub fn notify_if_due(&mut self, now: SystemTime, interval: Duration) -> bool {
        if now < self.next_due(interval) {
            return false;
        }
        // A delayed wake emits one reminder and advances to the next interval.
        self.last_notification = Some(now);
        true
    }

    pub fn clamp_future_times(&mut self, now: SystemTime) {
        self.last_action = self.last_action.min(now);
        self.last_notification = self.last_notification.map(|time| time.min(now));
    }
}

pub(super) fn local_time(now: SystemTime) -> time::OffsetDateTime {
    let utc = time::OffsetDateTime::from(now);
    utc.to_offset(time::UtcOffset::local_offset_at(utc).unwrap_or(time::UtcOffset::UTC))
}

pub(super) fn next_midnight(now: SystemTime) -> SystemTime {
    let local = local_time(now);
    let next = local
        .date()
        .next_day()
        .unwrap()
        .midnight()
        .assume_offset(local.offset());
    // Re-evaluate the offset at midnight for daylight-saving transitions.
    let offset = time::UtcOffset::local_offset_at(next).unwrap_or(local.offset());
    next.replace_offset(offset).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminders_follow_the_last_action_and_skip_missed_intervals() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let interval = Duration::from_secs(1800);
        let mut reminder = Reminder::new(start);
        assert!(!reminder.notify_if_due(start + interval - Duration::from_secs(1), interval));
        assert!(reminder.notify_if_due(start + interval, interval));
        assert!(!reminder.notify_if_due(start + interval, interval));
        assert!(reminder.notify_if_due(start + interval * 5, interval));
        assert_eq!(reminder.next_due(interval), start + interval * 6);
        let sip = start + interval * 5 + Duration::from_secs(30);
        reminder = Reminder::new(sip);
        assert_eq!(reminder.next_due(interval), sip + interval);
        assert_eq!(
            reminder.next_due(Duration::from_secs(600)),
            sip + Duration::from_secs(600)
        );
    }
}
