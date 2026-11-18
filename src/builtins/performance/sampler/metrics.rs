use super::{network::rate, platform};
use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::Instant,
};
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Clone, Debug, Default)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    /// Percentage of the whole machine, matching the overview CPU meter.
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub cpu_percent: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub process_count: usize,
    pub top_cpu: Vec<ProcessSnapshot>,
    pub top_memory: Vec<ProcessSnapshot>,
    pub ready: bool,
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
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut sampler = Self::default();
            loop {
                let snapshot = sampler.sample();
                match sender.try_send(snapshot) {
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                    _ => {}
                }
                thread::sleep(std::time::Duration::from_secs(1));
            }
        });
        receiver
    }

    fn sample(&mut self) -> Snapshot {
        self.system.refresh_cpu_all();
        self.snapshot.cpu_percent = self.system.global_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        self.snapshot.memory_used = self.system.used_memory();
        self.snapshot.memory_total = self.system.total_memory();
        let cores = self.system.cpus().len().max(1) as f32;
        let processes: Vec<_> = self
            .system
            .processes()
            .iter()
            // Linux also exposes individual tasks here. Their RSS is shared
            // with the parent; listing them would duplicate memory and CPU.
            .filter(|(_, process)| process.thread_kind().is_none())
            .map(|(pid, process)| ProcessSnapshot {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().into_owned(),
                cpu_percent: if self.sampled_once {
                    (process.cpu_usage() / cores).clamp(0.0, 100.0)
                } else {
                    0.0
                },
                memory_bytes: process.memory(),
            })
            .collect();
        self.snapshot.process_count = processes.len();
        self.snapshot.top_cpu = ranked(processes.clone(), true);
        self.snapshot.top_memory = ranked(processes, false);
        self.snapshot.ready = self.sampled_once;
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

pub(super) fn ranked(mut processes: Vec<ProcessSnapshot>, cpu: bool) -> Vec<ProcessSnapshot> {
    processes.sort_by(|a, b| {
        let order = if cpu {
            b.cpu_percent.total_cmp(&a.cpu_percent)
        } else {
            b.memory_bytes.cmp(&a.memory_bytes)
        };
        order.then_with(|| a.pid.cmp(&b.pid))
    });
    processes.truncate(10);
    processes
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

#[cfg(test)]
mod process_tests {
    use super::*;
    #[test]
    fn process_rankings_are_independent_bounded_and_stable_for_ties() {
        let data: Vec<_> = (1..=24)
            .map(|pid| ProcessSnapshot {
                pid,
                name: format!("process-{pid}"),
                cpu_percent: (pid / 2) as f32,
                memory_bytes: u64::from(25 - pid) * 1024,
            })
            .collect();
        let cpu = ranked(data.clone(), true);
        let memory = ranked(data, false);
        assert_eq!(cpu.len(), 10);
        assert_eq!(memory.len(), 10);
        assert_eq!(cpu[0].pid, 24);
        assert_eq!(memory[0].pid, 1);
        assert!(cpu.windows(2).all(|p| p[0].cpu_percent >= p[1].cpu_percent));
        assert!(memory
            .windows(2)
            .all(|p| p[0].memory_bytes >= p[1].memory_bytes));
        assert_eq!(cpu[1].pid, 22);
        assert_eq!(cpu[2].pid, 23);
    }
    #[test]
    fn first_sample_reads_memory_and_waits_for_cpu_baseline() {
        let mut sampler = MetricsSampler::default();
        let first = sampler.sample();
        assert!(!first.ready);
        assert!(first.top_cpu.iter().all(|p| p.cpu_percent == 0.0));
        assert!(first.memory_total > 0);
        assert!(first.memory_used <= first.memory_total);
        assert!(first.process_count > 0);
        #[cfg(target_os = "linux")]
        for p in first.top_memory.iter().chain(&first.top_cpu) {
            if let Ok(status) = std::fs::read_to_string(format!("/proc/{}/status", p.pid)) {
                let tgid = status
                    .lines()
                    .find_map(|line| line.strip_prefix("Tgid:"))
                    .unwrap()
                    .trim()
                    .parse::<u32>()
                    .unwrap();
                assert_eq!(
                    p.pid, tgid,
                    "individual threads must not duplicate process RSS"
                );
            }
        }
    }
}
