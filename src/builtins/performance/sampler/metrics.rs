use super::icons::IconCache;
use super::network::rate;
use std::time::Instant;
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Clone, Debug, Default)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    pub icon: Option<crate::ui::declarative::RasterImage>,
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
}

pub struct MetricsSampler {
    system: System,
    networks: Networks,
    last_sample: Instant,
    last_received: u64,
    last_transmitted: u64,
    snapshot: Snapshot,
    sampled_once: bool,
    icons: IconCache,
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
            icons: IconCache::default(),
        }
    }
}

impl MetricsSampler {
    pub fn start(interval: std::time::Duration) -> crate::core::worker::Worker<Snapshot> {
        let mut sampler = None;
        crate::core::worker::Worker::spawn_latest("system-monitor", interval, move || {
            sampler.get_or_insert_with(Self::default).sample()
        })
    }
    fn sample(&mut self) -> Snapshot {
        self.system.refresh_cpu_all();
        self.snapshot.cpu_percent = self.system.global_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_exe(UpdateKind::OnlyIfNotSet),
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
                icon: None,
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
        for process in self
            .snapshot
            .top_cpu
            .iter_mut()
            .chain(self.snapshot.top_memory.iter_mut())
        {
            if let Some(exe) = self
                .system
                .process(sysinfo::Pid::from_u32(process.pid))
                .and_then(|p| p.exe())
            {
                process.icon = self.icons.get(exe);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::is_loopback;

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
                icon: None,
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
