use super::{network::rate, platform};
use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::Instant,
};
use sysinfo::{Networks, System};

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub cpu_percent: f32,
    pub gpu_percent: Option<f32>,
    pub download_bytes_per_second: f64,
    pub upload_bytes_per_second: f64,
    pub battery_percent: Option<u8>,
    pub charging: bool,
}

pub struct MetricsSampler {
    system: System,
    networks: Networks,
    last_sample: Instant,
    last_received: u64,
    last_transmitted: u64,
    snapshot: Snapshot,
    sampled_once: bool,
}

impl Default for MetricsSampler {
    fn default() -> Self {
        Self {
            system: System::new(),
            networks: Networks::new_with_refreshed_list(),
            last_sample: Instant::now(),
            last_received: 0,
            last_transmitted: 0,
            snapshot: Snapshot::default(),
            sampled_once: false,
        }
    }
}

impl MetricsSampler {
    pub fn start() -> Receiver<Snapshot> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut sampler = Self::default();
            loop {
                let snapshot = sampler.sample();
                if sender.send(snapshot).is_err() {
                    break;
                }
                thread::sleep(std::time::Duration::from_secs(1));
            }
        });
        receiver
    }

    fn sample(&mut self) -> Snapshot {
        self.system.refresh_cpu_all();
        self.snapshot.cpu_percent = self.system.global_cpu_usage();
        self.networks.refresh(true);
        let (received, transmitted) = self
            .networks
            .iter()
            .filter(|(name, _)| !is_loopback(name))
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (
                    rx.saturating_add(data.total_received()),
                    tx.saturating_add(data.total_transmitted()),
                )
            });
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_sample).as_secs_f64();
        if self.sampled_once {
            self.snapshot.download_bytes_per_second = rate(self.last_received, received, elapsed);
            self.snapshot.upload_bytes_per_second =
                rate(self.last_transmitted, transmitted, elapsed);
        }
        self.last_received = received;
        self.last_transmitted = transmitted;
        self.last_sample = now;
        self.sampled_once = true;
        let (battery, charging) = battery_status();
        self.snapshot.battery_percent = battery;
        self.snapshot.charging = charging;
        self.snapshot.gpu_percent = platform::gpu_utilization();
        self.snapshot.clone()
    }
}

fn is_loopback(name: &str) -> bool {
    matches!(name, "lo" | "lo0" | "Loopback Pseudo-Interface 1")
}

fn battery_status() -> (Option<u8>, bool) {
    let Ok(manager) = battery::Manager::new() else {
        return (None, false);
    };
    let Ok(mut batteries) = manager.batteries() else {
        return (None, false);
    };
    let Some(Ok(battery)) = batteries.next() else {
        return (None, false);
    };
    let Some(percent) = battery_percent(battery.state_of_charge().value) else {
        return (None, false);
    };
    (Some(percent), battery.state() == battery::State::Charging)
}

fn battery_percent(ratio: f32) -> Option<u8> {
    if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
        return None;
    }
    Some((ratio * 100.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::{battery_percent, is_loopback};

    #[test]
    fn converts_valid_battery_ratios_to_percent() {
        assert_eq!(battery_percent(0.584), Some(58));
        assert_eq!(battery_percent(1.0), Some(100));
    }

    #[test]
    fn ignores_invalid_battery_ratios() {
        assert_eq!(battery_percent(-0.1), None);
        assert_eq!(battery_percent(1.1), None);
        assert_eq!(battery_percent(f32::NAN), None);
    }

    #[test]
    fn excludes_loopback_from_network_totals() {
        assert!(is_loopback("lo0"));
        assert!(is_loopback("lo"));
        assert!(!is_loopback("en0"));
    }

    #[test]
    fn live_sampler_returns_bounded_metrics_without_panicking() {
        let snapshot = super::MetricsSampler::default().sample();
        assert!((0.0..=100.0).contains(&snapshot.cpu_percent));
        assert!(snapshot
            .gpu_percent
            .is_none_or(|value| (0.0..=100.0).contains(&value)));
        assert!(snapshot.download_bytes_per_second.is_finite());
        assert!(snapshot.upload_bytes_per_second.is_finite());
        assert!(snapshot.battery_percent.is_none_or(|value| value <= 100));
    }
}
