use super::icons::IconCache;
use super::network::rate;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};
use sysinfo::{Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProcessIdentity {
    pid: Pid,
    started_at: u64,
}

impl ProcessIdentity {
    fn can_terminate(self) -> bool {
        let pid = self.pid.as_u32();
        pid > 1 && pid != std::process::id() && self.started_at != 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct ApplicationSnapshot {
    pub name: String,
    path: Option<PathBuf>,
    processes: Vec<ProcessIdentity>,
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
    pub top_cpu: Vec<ApplicationSnapshot>,
    pub top_memory: Vec<ApplicationSnapshot>,
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
            .map(|(pid, process)| ApplicationSnapshot {
                name: process.name().to_string_lossy().into_owned(),
                path: process.exe().map(PathBuf::from),
                processes: vec![ProcessIdentity {
                    pid: *pid,
                    started_at: process.start_time(),
                }],
                icon: None,
                cpu_percent: if self.sampled_once {
                    (process.cpu_usage() / cores).clamp(0.0, 100.0)
                } else {
                    0.0
                },
                memory_bytes: process.memory(),
            })
            .collect();
        let applications = aggregate_applications(processes);
        self.snapshot.top_cpu = ranked(applications.clone(), true);
        self.snapshot.top_memory = ranked(applications, false);
        for process in self
            .snapshot
            .top_cpu
            .iter_mut()
            .chain(self.snapshot.top_memory.iter_mut())
        {
            if let Some(exe) = process.path.as_deref() {
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

impl ApplicationSnapshot {
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    pub fn termination_key(&self) -> Option<String> {
        if self.processes.iter().any(|target| !target.can_terminate()) {
            return None;
        }
        let first = self.processes.first()?;
        let application = self
            .path
            .as_deref()
            .map_or_else(|| self.name.as_str().into(), path_key);
        Some(format!(
            "terminate:{}:{}:{application}",
            first.pid, first.started_at
        ))
    }

    /// Recheck the sampled identities before signalling any of the app's processes.
    pub fn terminate(&self) -> bool {
        let pids: Vec<_> = self.processes.iter().map(|target| target.pid).collect();
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        let mut success = true;
        for target in self
            .processes
            .iter()
            .filter(|target| target.can_terminate())
        {
            let Some(process) = system.process(target.pid) else {
                continue;
            };
            let expected = self.path.as_deref();
            let actual = process.exe().map(application_path);
            let same_application = match (expected, actual) {
                (Some(expected), Some(actual)) => path_key(expected) == path_key(actual),
                (None, _) => process.name() == self.name.as_str(),
                _ => false,
            };
            if process.start_time() == target.started_at && same_application {
                success &= if cfg!(any(target_os = "macos", target_os = "windows")) {
                    process.kill()
                } else {
                    process.kill_with(Signal::Term).unwrap_or(false)
                };
            }
        }
        success
    }
}

fn application_path(executable: &Path) -> &Path {
    // A helper can have its own nested bundle. Keep the outer app for both
    // aggregation and icon lookup.
    executable
        .ancestors()
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .last()
        .unwrap_or(executable)
}

fn path_key(path: &Path) -> std::borrow::Cow<'_, str> {
    let key = path.to_string_lossy();
    if cfg!(target_os = "windows") {
        key.to_lowercase().into()
    } else {
        key
    }
}

fn aggregate_applications(processes: Vec<ApplicationSnapshot>) -> Vec<ApplicationSnapshot> {
    let mut groups: HashMap<String, ApplicationSnapshot> = HashMap::new();
    for mut process in processes {
        if let Some(path) = &mut process.path {
            let bundle = application_path(path);
            if bundle != path {
                *path = bundle.to_owned();
            }
            if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                process.name = name.to_owned();
            }
        }
        let key = match &process.path {
            Some(path) => path_key(path).into_owned(),
            None => format!("unknown:{}", process.name),
        };
        if let Some(app) = groups.get_mut(&key) {
            app.cpu_percent = (app.cpu_percent + process.cpu_percent).min(100.0);
            app.memory_bytes = app.memory_bytes.saturating_add(process.memory_bytes);
            app.processes.append(&mut process.processes);
        } else {
            groups.insert(key, process);
        }
    }
    groups
        .into_values()
        .map(|mut app| {
            app.processes.sort_unstable();
            app
        })
        .collect()
}

pub(super) fn ranked(
    mut processes: Vec<ApplicationSnapshot>,
    cpu: bool,
) -> Vec<ApplicationSnapshot> {
    processes.sort_by(|a, b| {
        let order = if cpu {
            b.cpu_percent.total_cmp(&a.cpu_percent)
        } else {
            b.memory_bytes.cmp(&a.memory_bytes)
        };
        order.then_with(|| a.processes.first().cmp(&b.processes.first()))
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
            .map(|pid| ApplicationSnapshot {
                name: format!("process-{pid}"),
                processes: vec![ProcessIdentity {
                    pid: Pid::from_u32(pid),
                    started_at: 1,
                }],
                cpu_percent: (pid / 2) as f32,
                memory_bytes: u64::from(25 - pid) * 1024,
                ..Default::default()
            })
            .collect();
        let cpu = ranked(data.clone(), true);
        let memory = ranked(data, false);
        assert_eq!(cpu.len(), 10);
        assert_eq!(memory.len(), 10);
        assert_eq!(cpu[0].processes[0].pid.as_u32(), 24);
        assert_eq!(memory[0].processes[0].pid.as_u32(), 1);
        assert!(cpu.windows(2).all(|p| p[0].cpu_percent >= p[1].cpu_percent));
        assert!(memory
            .windows(2)
            .all(|p| p[0].memory_bytes >= p[1].memory_bytes));
        assert_eq!(cpu[1].processes[0].pid.as_u32(), 22);
        assert_eq!(cpu[2].processes[0].pid.as_u32(), 23);
    }
    #[test]
    fn first_sample_reads_memory_and_waits_for_cpu_baseline() {
        let mut sampler = MetricsSampler::default();
        let first = sampler.sample();
        assert!(!first.ready);
        assert!(first.top_cpu.iter().all(|p| p.cpu_percent == 0.0));
        assert!(first.memory_total > 0);
        assert!(first.memory_used <= first.memory_total);
        assert!(!first.top_memory.is_empty());
        #[cfg(target_os = "linux")]
        for p in first.top_memory.iter().chain(&first.top_cpu) {
            if let Ok(status) =
                std::fs::read_to_string(format!("/proc/{}/status", p.processes[0].pid))
            {
                let tgid = status
                    .lines()
                    .find_map(|line| line.strip_prefix("Tgid:"))
                    .unwrap()
                    .trim()
                    .parse::<u32>()
                    .unwrap();
                assert_eq!(
                    p.processes[0].pid.as_u32(),
                    tgid,
                    "individual threads must not duplicate process RSS"
                );
            }
        }
    }
}

#[cfg(test)]
mod application_tests {
    use super::*;
    #[test]
    fn same_executable_is_aggregated_before_ranking_but_same_name_at_other_paths_is_separate() {
        let sample = |pid, path: &str, cpu| ApplicationSnapshot {
            name: "browser".into(),
            path: Some(path.into()),
            processes: vec![ProcessIdentity {
                pid: Pid::from_u32(pid),
                started_at: 1,
            }],
            cpu_percent: cpu,
            memory_bytes: 1024,
            ..Default::default()
        };
        let applications = aggregate_applications(vec![
            sample(1, "/apps/browser", 20.0),
            sample(2, "/apps/browser", 30.0),
            sample(3, "/other/browser", 40.0),
        ]);
        assert_eq!(applications.len(), 2);
        let ranked = ranked(applications, true);
        assert_eq!(ranked[0].cpu_percent, 50.0);
        assert_eq!(ranked[0].memory_bytes, 2048);
        assert_eq!(ranked[0].process_count(), 2);
        assert_eq!(ranked[1].process_count(), 1);
    }
    #[test]
    fn macos_bundle_helpers_are_one_application() {
        let samples = [
            "/Applications/Browser.app/Contents/MacOS/Browser",
            "/Applications/Browser.app/Contents/Frameworks/Helper.app/Contents/MacOS/Helper",
        ]
        .into_iter()
        .enumerate()
        .map(|(pid, path)| ApplicationSnapshot {
            name: path.into(),
            path: Some(path.into()),
            processes: vec![ProcessIdentity {
                pid: Pid::from_u32(pid as u32 + 2),
                started_at: 1,
            }],
            ..Default::default()
        })
        .collect();
        let applications = aggregate_applications(samples);
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].process_count(), 2);
        assert_eq!(applications[0].name, "Browser");
        assert_eq!(
            applications[0].path.as_deref(),
            Some(Path::new("/Applications/Browser.app"))
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_nested_helper_uses_the_system_icon_for_its_outer_bundle() {
        let bundle = Path::new("/System/Library/CoreServices/Finder.app");
        let helper = bundle.join("Contents/Helpers/Helper.app/Contents/MacOS/Helper");
        let mut cache = IconCache::default();
        let icon = cache.get(application_path(&helper)).unwrap();
        assert_eq!(icon.size, [32, 32]);
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
        assert_eq!(cache.get(bundle), Some(icon));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn terminating_a_group_skips_a_process_whose_start_time_changed() {
        use std::process::{Child, Command};
        use std::time::Duration;

        struct TestProcess(Child);
        impl Drop for TestProcess {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let spawn = || {
            let mut command = Command::new("/bin/sleep");
            command.arg("30");
            #[cfg(target_os = "macos")]
            {
                use std::os::unix::process::CommandExt;

                // exec preserves ignored signals set in the child.
                unsafe {
                    command.pre_exec(|| {
                        if libc::signal(libc::SIGTERM, libc::SIG_IGN) == libc::SIG_ERR {
                            return Err(std::io::Error::last_os_error());
                        }
                        Ok(())
                    });
                }
            }
            TestProcess(command.spawn().unwrap())
        };
        let mut original = spawn();
        let mut stale = spawn();
        let original_pid = Pid::from_u32(original.0.id());
        let stale_pid = Pid::from_u32(stale.0.id());
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[original_pid, stale_pid]),
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        let process = system.process(original_pid).unwrap();
        let app = ApplicationSnapshot {
            path: process.exe().map(PathBuf::from),
            name: process.name().to_string_lossy().into_owned(),
            processes: vec![
                ProcessIdentity {
                    pid: original_pid,
                    started_at: process.start_time(),
                },
                ProcessIdentity {
                    pid: stale_pid,
                    started_at: system.process(stale_pid).unwrap().start_time() + 1,
                },
            ],
            ..Default::default()
        };
        assert!(app.terminate());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = original.0.try_wait().unwrap() {
                #[cfg(target_os = "macos")]
                {
                    use std::os::unix::process::ExitStatusExt;

                    assert_eq!(status.signal(), Some(libc::SIGKILL));
                }
                assert!(!status.success());
                break;
            }
            assert!(Instant::now() < deadline, "target process did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(stale.0.try_wait().unwrap().is_none());
    }
}
