mod view;

use crate::downloads::Download;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
pub use view::show;

#[derive(Default)]
pub struct Notifications {
    queue: VecDeque<Download>,
    current: Option<(Download, Instant)>,
}

impl Notifications {
    pub fn push(&mut self, downloads: impl IntoIterator<Item = Download>) {
        self.queue.extend(downloads);
    }
    pub fn update(&mut self, now: Instant, may_show: bool) -> bool {
        let expired = self
            .current
            .as_ref()
            .is_some_and(|(_, started)| now.duration_since(*started) >= Duration::from_secs(8));
        if expired {
            self.current = None;
        }
        if self.current.is_none() && may_show {
            if let Some(item) = self.queue.pop_front() {
                self.current = Some((item, now));
                return true;
            }
        }
        false
    }
    pub fn current(&self) -> Option<&Download> {
        self.current.as_ref().map(|(item, _)| item)
    }
    pub fn dismiss(&mut self) {
        self.current = None;
    }

    pub fn suppress(&mut self) {
        self.current = None;
        self.queue.clear();
    }

    pub fn keep_visible(&mut self, now: Instant) {
        if let Some((_, shown_at)) = &mut self.current {
            *shown_at = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::DownloadState;
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
    fn completion_queue_waits_for_settings_then_shows_each_notification() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push([item("1"), item("2")]);
        assert!(!notices.update(now, false));
        assert!(notices.update(now, true));
        assert_eq!(notices.current().unwrap().id, "1");
        assert!(notices.update(now + Duration::from_secs(9), true));
        assert_eq!(notices.current().unwrap().id, "2");
        notices.dismiss();
        assert!(notices.current().is_none());
    }

    #[test]
    fn interacting_with_a_notification_keeps_it_visible() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push([item("1")]);
        notices.update(now, true);
        notices.keep_visible(now + Duration::from_secs(7));
        notices.update(now + Duration::from_secs(9), true);
        assert!(notices.current().is_some());
    }

    #[test]
    fn do_not_disturb_discards_current_and_queued_completions() {
        let now = Instant::now();
        let mut notices = Notifications::default();
        notices.push([item("1"), item("2")]);
        assert!(notices.update(now, true));
        notices.suppress();
        assert!(notices.current().is_none());
        assert!(!notices.update(now + Duration::from_secs(10), true));
    }
}
