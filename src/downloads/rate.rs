use super::model::Download;
use std::{collections::HashMap, time::Instant};

#[derive(Default)]
pub struct RateTracker {
    previous: HashMap<String, (u64, Instant)>,
}

impl RateTracker {
    pub fn sample(&mut self, downloads: &mut [Download], now: Instant) {
        let mut current = HashMap::new();
        for item in downloads.iter_mut() {
            item.bytes_per_second = self
                .previous
                .get(&item.id)
                .and_then(|(bytes, sampled)| {
                    let elapsed = now.duration_since(*sampled).as_secs_f64();
                    if elapsed > 0.0 && item.received >= *bytes {
                        Some((item.received - bytes) as f64 / elapsed)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            current.insert(item.id.clone(), (item.received, now));
        }
        self.previous = current;
    }
}

#[cfg(test)]
mod tests {
    use super::RateTracker;
    use crate::downloads::{Download, DownloadState};
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    fn download(received: u64) -> Download {
        Download {
            id: "active-download".into(),
            browser: "Chrome",
            path: PathBuf::from("/tmp/file.zip"),
            received,
            total: Some(1000),
            bytes_per_second: 0.0,
            state: DownloadState::Active,
        }
    }

    #[test]
    fn samples_real_download_speed_without_making_up_the_initial_rate() {
        let start = Instant::now();
        let mut tracker = RateTracker::default();
        let mut first = [download(200)];
        tracker.sample(&mut first, start);
        assert_eq!(first[0].bytes_per_second, 0.0);

        let mut next = [download(700)];
        tracker.sample(&mut next, start + Duration::from_secs(2));
        assert_eq!(next[0].bytes_per_second, 250.0);
    }

    #[test]
    fn resets_speed_when_download_bytes_or_id_restart() {
        let start = Instant::now();
        let mut tracker = RateTracker::default();
        tracker.sample(&mut [download(700)], start);
        let mut restarted = [download(10)];
        tracker.sample(&mut restarted, start + Duration::from_secs(1));
        assert_eq!(restarted[0].bytes_per_second, 0.0);
        tracker.sample(&mut [], start + Duration::from_secs(2));
        let mut fresh = [download(900)];
        tracker.sample(&mut fresh, start + Duration::from_secs(3));
        assert_eq!(fresh[0].bytes_per_second, 0.0);
    }
}
