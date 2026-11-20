use super::sampler::{MetricsSampler, Snapshot};
use crate::core::worker::Worker;
use crate::extension::{
    Action, CompactView, ContributionDescriptor, ContributionKind, ExtensionId, ExtensionOutput,
    ReloadPolicy, RuntimeExtension, SettingSchema, SettingValue, Settings, ViewNode, WidgetView,
};
use crate::ui::declarative::*;

pub const ID: &str = "builtin.system-monitor/overview";
const NETWORK_WIDTH: f32 = 106.0;

pub fn descriptors() -> Vec<ContributionDescriptor> {
    vec![
        ContributionDescriptor::builtin(
            ID,
            ContributionKind::Widget,
            "性能监控",
            "activity",
            true,
            1,
        )
        .with_size_hint(440.0, 410.0)
        .with_widget_bounds(WidgetBounds {
            min: [320.0, 200.0],
            max: [Some(620.0), Some(520.0)],
            resizable: true,
            fit_width: false,
            fit_height: true,
            overflow: Overflow::Scroll(ScrollAxes::Vertical),
        })
        .with_settings(
            1,
            vec![
                SettingSchema::new("show_download", SettingValue::Boolean(true), "显示下载速率"),
                SettingSchema::new("show_upload", SettingValue::Boolean(true), "显示上传速率"),
            ],
        )
        .with_capabilities(vec![
            crate::extension::Capability::View,
            crate::extension::Capability::Action,
            crate::extension::Capability::Event,
        ]),
        compact_descriptor("builtin.system-monitor/cpu", "CPU", "cpu", false, 2, 56.0),
        compact_descriptor("builtin.system-monitor/gpu", "GPU", "gpu", false, 3, 56.0),
        compact_descriptor(
            "builtin.system-monitor/memory",
            "内存",
            "memory",
            false,
            4,
            56.0,
        ),
        compact_descriptor(
            "builtin.system-monitor/network",
            "网络",
            "network",
            false,
            4,
            NETWORK_WIDTH,
        ),
    ]
}

fn compact_descriptor(
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    enabled: bool,
    order: u32,
    width: f32,
) -> ContributionDescriptor {
    ContributionDescriptor::builtin(id, ContributionKind::Compact, title, icon, enabled, order)
        .with_size_hint(width, 28.0)
}

pub struct PerformanceExtension {
    snapshots: Option<Worker<Snapshot>>,
    gpu_updates: Option<Worker<Option<f32>>>,
    sample_interval: std::time::Duration,
    gpu: Option<f32>,
    snapshot: Snapshot,
    show_download: bool,
    show_upload: bool,
    sort_memory: bool,
    termination_error: Option<String>,
    cpu_history: Vec<f32>,
    memory_history: Vec<f32>,
}

impl crate::extension::NotificationPresentationProvider for PerformanceExtension {}

impl PerformanceExtension {
    pub fn new() -> Self {
        Self {
            snapshots: None,
            gpu_updates: None,
            sample_interval: std::time::Duration::from_secs(1),
            gpu: None,
            snapshot: Snapshot::default(),
            show_download: true,
            show_upload: true,
            sort_memory: false,
            termination_error: None,
            cpu_history: Vec::new(),
            memory_history: Vec::new(),
        }
    }
}

impl Default for PerformanceExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeExtension for PerformanceExtension {
    fn descriptor(&self) -> crate::extension::ExtensionDescriptor {
        let mut descriptor = crate::extension::ExtensionDescriptor::builtin(
            "builtin.system-monitor",
            "系统监控",
            "CPU、GPU、内存、进程和网络",
            descriptors(),
        );
        descriptor.settings.push(
            SettingSchema::new("sample_seconds", SettingValue::Integer(1), "采样间隔（秒）")
                .range(1.0, 60.0),
        );
        descriptor
    }
    fn on_start(&mut self, _: &crate::extension::ExtensionContext) -> Result<(), String> {
        self.snapshots = Some(MetricsSampler::start(self.sample_interval));
        self.gpu_updates = Some(super::sampler::start_gpu(self.sample_interval));
        Ok(())
    }
    fn on_stop(&mut self, _: &crate::extension::ExtensionContext) -> Result<(), String> {
        self.snapshots = None;
        self.gpu_updates = None;
        Ok(())
    }

    fn update(&mut self, _now: std::time::Instant) -> ExtensionOutput {
        let mut changed = false;
        while let Some(snapshot) = self.snapshots.as_ref().and_then(Worker::take) {
            if snapshot.ready {
                self.cpu_history.push(snapshot.cpu_percent);
                self.memory_history
                    .push(super::view::memory_percent(&snapshot));
                if self.cpu_history.len() > 60 {
                    self.cpu_history.remove(0);
                    self.memory_history.remove(0);
                }
            }
            self.snapshot = snapshot;
            changed = true;
        }
        if let Some(gpu) = self.gpu_updates.as_ref().and_then(Worker::take) {
            changed |= self.gpu != gpu;
            self.gpu = gpu;
        }
        self.snapshot.gpu_percent = self.gpu;
        ExtensionOutput {
            invalidated: changed
                .then(|| ExtensionId::builtin(ID))
                .into_iter()
                .collect(),
            ..ExtensionOutput::default()
        }
    }

    fn widget_view(&self, widget_id: &ExtensionId) -> Option<WidgetView> {
        if widget_id.as_str() != ID {
            return None;
        }
        Some(super::view::build(
            &self.snapshot,
            self.sort_memory,
            &self.cpu_history,
            &self.memory_history,
            self.show_download,
            self.show_upload,
            self.termination_error.as_deref(),
        ))
    }

    fn on_action(&mut self, action: Action, _now: std::time::Instant) -> ExtensionOutput {
        match action {
            Action::Select(key) if key == "process-cpu" => self.sort_memory = false,
            Action::Select(key) if key == "process-memory" => self.sort_memory = true,
            Action::Custom(key) if key.starts_with("terminate:") => {
                let target = self
                    .snapshot
                    .top_cpu
                    .iter()
                    .chain(&self.snapshot.top_memory)
                    .find(|app| app.termination_key().as_deref() == Some(key.as_str()));
                self.termination_error = target
                    .filter(|app| !app.terminate())
                    .map(|app| format!("无法结束 {} 的部分或全部进程", app.name));
            }
            _ => return ExtensionOutput::default(),
        }
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(ID)],
            ..Default::default()
        }
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let metrics = &self.snapshot;
        let mut views = vec![
            compact_view(
                "builtin.system-monitor/memory",
                processor_compact(
                    "cpu",
                    Some(super::view::memory_percent(metrics)),
                    ColorSpec::Info,
                ),
                56.0,
            ),
            compact_view(
                "builtin.system-monitor/cpu",
                processor_compact(
                    "cpu",
                    metrics.ready.then_some(metrics.cpu_percent),
                    ColorSpec::Success,
                ),
                56.0,
            ),
            compact_view(
                "builtin.system-monitor/gpu",
                processor_compact("gpu", metrics.gpu_percent, ColorSpec::Violet),
                56.0,
            ),
        ];
        if self.show_download || self.show_upload {
            let rates = [
                (
                    self.show_download,
                    "下行",
                    metrics.download_bytes_per_second,
                ),
                (self.show_upload, "上行", metrics.upload_bytes_per_second),
            ];
            let mut transmitting = 0.0_f64;
            let mut lines = Vec::new();
            for (visible, arrow, speed) in rates {
                if visible {
                    transmitting = transmitting.max(speed);
                    lines.push(super::super::compact_detail(format!(
                        "{arrow} {}",
                        format_speed(speed)
                    )));
                }
            }
            let mut pulse = MotionTrack::new(
                MotionProperty::Opacity,
                &[(0, 1.0), (300, 0.45), (600, 1.0)],
            );
            pulse.amplitude = Some(ValueMapping {
                value: transmitting as f32,
                input: [0.0, 1024.0 * 128.0],
                output: [0.25, 1.0],
            });
            let icon = ViewNode::svg("compact/network", 16.0)
                .color(ColorSpec::Cyan)
                .motion(Motion {
                    trigger: Trigger::While(transmitting > 0.0),
                    duration_ms: 600,
                    repeat: Repeat::Infinite,
                    tracks: vec![pulse],
                    ..Default::default()
                });
            views.push(compact_view(
                "builtin.system-monitor/network",
                ViewNode::compact(
                    icon,
                    ViewNode::column(
                        lines,
                        Flow {
                            gap: Some(0.0),
                            align: Align::Start,
                            ..Default::default()
                        },
                    ),
                    CompactLayout {
                        leading_size: [18.0, 22.0],
                        ..Default::default()
                    },
                ),
                NETWORK_WIDTH,
            ));
        }
        views
    }

    fn on_settings_changed(
        &mut self,
        _ctx: &crate::extension::ExtensionContext,
        _id: &ExtensionId,
        settings: &Settings,
        _policy: ReloadPolicy,
    ) -> ExtensionOutput {
        if let Some(SettingValue::Integer(seconds)) = settings.get("sample_seconds") {
            let interval = std::time::Duration::from_secs(*seconds as u64);
            if interval != self.sample_interval {
                self.sample_interval = interval;
                if self.snapshots.is_some() {
                    self.snapshots = None;
                    self.snapshots = Some(MetricsSampler::start(interval));
                }
                if self.gpu_updates.is_some() {
                    self.gpu_updates = None;
                    self.gpu_updates = Some(super::sampler::start_gpu(interval));
                }
            }
        }
        self.show_download = settings.get("show_download") != Some(&SettingValue::Boolean(false));
        self.show_upload = settings.get("show_upload") != Some(&SettingValue::Boolean(false));
        ExtensionOutput::default()
    }
}

fn processor_compact(icon: &str, percent: Option<f32>, color: ColorSpec) -> ViewNode {
    let mut pulse = MotionTrack::new(MotionProperty::Scale, &[(0, 1.0), (400, 1.14), (800, 1.0)]);
    pulse.amplitude = Some(ValueMapping {
        value: percent.unwrap_or_default(),
        input: [25.0, 100.0],
        output: [0.0, 1.0],
    });
    let leading = ViewNode::icon(icon, 14.0).color(color).motion(Motion {
        trigger: Trigger::While(percent.is_some_and(|value| value > 25.0)),
        duration_ms: 800,
        repeat: Repeat::Infinite,
        easing: Easing::EaseInOut,
        tracks: vec![pulse],
        ..Default::default()
    });
    ViewNode::compact(
        leading,
        super::super::compact_value(
            percent.map_or_else(|| "—".into(), |value| format!("{value:.0}%")),
            true,
        ),
        CompactLayout {
            leading_size: [18.0, 22.0],
            gap: 3.0,
            ..Default::default()
        },
    )
}

fn compact_view(id: &'static str, content: ViewNode, width: f32) -> CompactView {
    CompactView {
        id: ExtensionId::builtin(id),
        source_extension_id: ExtensionId::builtin(ID),
        content,
        visibility: Default::default(),
        min_width: width,
        preferred_width: width,
        fit_width: false,
        priority: 0,
        tooltip: None,
    }
}

fn format_speed(bytes_per_second: f64) -> String {
    if bytes_per_second >= 1_000_000.0 {
        format!("{:.1} MB/s", bytes_per_second / 1_000_000.0)
    } else if bytes_per_second >= 1_000.0 {
        format!("{:.0} KB/s", bytes_per_second / 1_000.0)
    } else {
        format!("{:.0} B/s", bytes_per_second.max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_sort_is_independent_of_worker_lifecycle() {
        let mut e = PerformanceExtension::default();
        assert!(e.snapshots.is_none());
        let output = e.on_action(
            crate::extension::Action::Select("process-memory".into()),
            std::time::Instant::now(),
        );
        assert!(e.sort_memory);
        assert_eq!(output.invalidated, vec![ExtensionId::builtin(ID)]);
    }
}
