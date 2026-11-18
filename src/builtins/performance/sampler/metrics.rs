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
    #[cfg(target_os = "windows")]
    if let Some(status) = platform::windows_battery_status() {
        return status;
    }

    #[cfg(target_os = "macos")]
    let external_power = mac_external_power_connected().unwrap_or(false);
    #[cfg(not(target_os = "macos"))]
    let external_power = false;

    let Ok(manager) = battery::Manager::new() else {
        return (None, external_power);
    };
    let Ok(mut batteries) = manager.batteries() else {
        return (None, external_power);
    };
    let Some(Ok(battery)) = batteries.next() else {
        return (None, external_power);
    };
    let percent = battery_percent(battery.state_of_charge().value);
    let charging = battery.state() == battery::State::Charging || external_power;
    (percent, charging)
}

#[cfg(target_os = "macos")]
fn mac_external_power_connected() -> Option<bool> {
    let output = crate::core::external::process::command("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    parse_external_power_status(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn parse_external_power_status(output: &str) -> Option<bool> {
    if output.contains("AC Power") || output.contains("AC attached") {
        Some(true)
    } else if output.contains("Battery Power") || output.contains("No adapter") {
        Some(false)
    } else {
        None
    }
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

    #[cfg(target_os = "macos")]
    #[test]
    fn treats_an_attached_mac_power_adapter_as_charging_even_at_full_capacity() {
        assert_eq!(
            super::parse_external_power_status("Now drawing from 'AC Power'"),
            Some(true)
        );
        assert_eq!(
            super::parse_external_power_status("100%; charged; AC attached; not charging"),
            Some(true)
        );
        assert_eq!(
            super::parse_external_power_status("Now drawing from 'Battery Power'"),
            Some(false)
        );
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
