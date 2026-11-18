use super::power::PowerSnapshot;
use super::sampler::{MetricsSampler, Snapshot};
use crate::core::latest::LatestReceiver;
use crate::extension::{
    CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind, ExtensionOutput, ReloadPolicy,
    RuntimeExtension, SettingSchema, SettingValue, Settings, ViewNode, WidgetView,
};
use crate::ui::declarative::*;

pub const ID: &str = "builtin.system-monitor/overview";
const BATTERY_WIDTH: f32 = 64.0;
const NETWORK_WIDTH: f32 = 88.0;

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(ID, ExtensionKind::Widget, "性能监控", "activity", true, 1)
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
                    SettingSchema::new(
                        "show_download",
                        SettingValue::Boolean(true),
                        "显示下载速率",
                    ),
                    SettingSchema::new("show_upload", SettingValue::Boolean(true), "显示上传速率"),
                ],
            )
            .with_capabilities(vec![
                crate::extension::Capability::View,
                crate::extension::Capability::Action,
                crate::extension::Capability::Event,
            ]),
        compact_descriptor(
            "builtin.system-monitor/battery",
            "电池",
            "battery",
            true,
            1,
            BATTERY_WIDTH,
        ),
        compact_descriptor("builtin.system-monitor/cpu", "CPU", "cpu", false, 2, 56.0),
        compact_descriptor("builtin.system-monitor/gpu", "GPU", "gpu", false, 3, 56.0),
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
) -> ExtensionDescriptor {
    ExtensionDescriptor::builtin(id, ExtensionKind::Compact, title, icon, enabled, order)
        .with_size_hint(width, 28.0)
}

pub struct PerformanceExtension {
    snapshots: LatestReceiver<Snapshot>,
    power_updates: LatestReceiver<PowerSnapshot>,
    power: PowerSnapshot,
    gpu_updates: LatestReceiver<Option<f32>>,
    gpu: Option<f32>,
    snapshot: Snapshot,
    show_download: bool,
    show_upload: bool,
    sort_memory: bool,
    cpu_history: Vec<f32>,
    memory_history: Vec<f32>,
}

impl crate::extension::NotificationPresentationProvider for PerformanceExtension {}

impl PerformanceExtension {
    pub fn new() -> Self {
        Self {
            snapshots: MetricsSampler::start(),
            power_updates: super::power::start(),
            power: PowerSnapshot::default(),
            gpu_updates: super::sampler::start_gpu(),
            gpu: None,
            snapshot: Snapshot::default(),
            show_download: true,
            show_upload: true,
            sort_memory: false,
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
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        descriptors()
    }

    fn update(&mut self, _now: std::time::Instant) -> ExtensionOutput {
        let mut changed = false;
        while let Some(snapshot) = self.snapshots.take() {
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
        if let Some(power) = self.power_updates.take() {
            changed |= self.power != power;
            self.power = power;
        }
        if let Some(gpu) = self.gpu_updates.take() {
            changed |= self.gpu != gpu;
            self.gpu = gpu;
        }
        self.snapshot.gpu_percent = self.gpu;
        self.snapshot.battery_percent = self.power.percent;
        self.snapshot.charging = self.power.charging;
        self.snapshot.external_power = self.power.external;
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
        ))
    }

    fn on_action(
        &mut self,
        action: crate::extension::Action,
        _now: std::time::Instant,
    ) -> ExtensionOutput {
        let next = match action {
            crate::extension::Action::Select(ref key) if key == "process-cpu" => false,
            crate::extension::Action::Select(ref key) if key == "process-memory" => true,
            _ => return ExtensionOutput::default(),
        };
        self.sort_memory = next;
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(ID)],
            ..Default::default()
        }
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let metrics = &self.snapshot;
        let mut views = vec![
            compact_view(
                "builtin.system-monitor/battery",
                battery_compact(metrics),
                BATTERY_WIDTH,
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
                processor_compact("gpu", metrics.gpu_percent, ColorSpec::Info),
                56.0,
            ),
        ];
        views[0].tooltip = Some(power_label(metrics).into());
        if self.show_download || self.show_upload {
            let rates = [
                (self.show_download, "↓", metrics.download_bytes_per_second),
                (self.show_upload, "↑", metrics.upload_bytes_per_second),
            ];
            let mut transmitting = 0.0_f64;
            let mut lines = Vec::new();
            for (visible, arrow, speed) in rates {
                if visible {
                    transmitting = transmitting.max(speed);
                    lines.push(
                        super::super::compact_detail(format!("{arrow} {}", format_speed(speed)))
                            .color(ColorSpec::PrimaryText),
                    );
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
                .color(ColorSpec::Info)
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

fn battery_compact(metrics: &Snapshot) -> ViewNode {
    let level = f32::from(metrics.battery_percent.unwrap_or(0)) / 100.0;
    let color = battery_color(metrics);
    let indicator = |segments, fraction, width, inactive, color| {
        ViewNode::new(Node::SegmentedIndicator {
            segments,
            fraction,
            gap: 0.9,
            direction: FillDirection::LeftToRight,
            inactive,
        })
        .sized(Size::Fixed(width), Size::Fixed(5.0))
        .color(color)
    };
    let leading = ViewNode::new(Node::Stack(
        [Align::Center; 2],
        vec![
            ViewNode::svg("compact/battery", 24.0).color(color),
            indicator(5, level, 17.0, ColorSpec::Border, color)
                .motion(Motion::progress(level, 200)),
            indicator(1, 1.0, 1.8, ColorSpec::Transparent, ColorSpec::OnAccent)
                .styled(Style {
                    normal: Paint {
                        foreground: Some(ColorSpec::OnAccent),
                        opacity: Some(if metrics.charging { 0.55 } else { 0.0 }),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .motion(Motion {
                    trigger: Trigger::While(metrics.charging),
                    duration_ms: 900,
                    gap_ms: 150,
                    repeat: Repeat::Infinite,
                    tracks: vec![
                        MotionTrack::new(MotionProperty::TranslateX, &[(0, -8.0), (900, 8.0)]),
                        MotionTrack::new(
                            MotionProperty::Opacity,
                            &[(0, 0.0), (150, 1.0), (750, 1.0), (900, 0.0)],
                        ),
                    ],
                    ..Default::default()
                }),
        ],
    ))
    .sized(Size::Fixed(25.0), Size::Fixed(24.0));
    ViewNode::compact(
        leading,
        super::super::compact_value(battery_label(metrics), true).color(color),
        CompactLayout {
            leading_size: [25.0, 24.0],
            gap: 4.0,
            ..Default::default()
        },
    )
}

fn power_label(metrics: &Snapshot) -> &'static str {
    if metrics.charging {
        "正在充电"
    } else if metrics.external_power == Some(true) && metrics.battery_percent.is_none() {
        "已接通电源 · 电池电量未知或无电池"
    } else if metrics.external_power == Some(true) {
        "已接通电源 · 未充电"
    } else if metrics.external_power == Some(false) {
        "使用电池供电"
    } else {
        "电源状态未知"
    }
}

pub(super) fn battery_label(metrics: &Snapshot) -> String {
    metrics
        .battery_percent
        .map_or_else(|| "--".into(), |v| format!("{v}%"))
}

pub(super) fn battery_color(metrics: &Snapshot) -> ColorSpec {
    if metrics.battery_percent.is_some_and(|v| v < 20) {
        ColorSpec::Danger
    } else if metrics.charging || metrics.external_power == Some(true) {
        ColorSpec::Success
    } else {
        ColorSpec::PrimaryText
    }
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
    fn switching_process_metric_invalidates_the_view_and_keeps_other_actions_unchanged() {
        let (_sender, snapshots) = crate::core::latest::channel();
        let mut extension = PerformanceExtension {
            snapshots,
            power_updates: crate::core::latest::channel().1,
            power: PowerSnapshot::default(),
            gpu_updates: crate::core::latest::channel().1,
            gpu: None,
            snapshot: Snapshot::default(),
            show_download: true,
            show_upload: true,
            sort_memory: false,
            cpu_history: vec![],
            memory_history: vec![],
        };
        let now = std::time::Instant::now();
        let output = extension.on_action(
            crate::extension::Action::Select("process-memory".into()),
            now,
        );
        assert!(extension.sort_memory);
        assert_eq!(output.invalidated, vec![ExtensionId::builtin(ID)]);
        assert!(extension
            .on_action(crate::extension::Action::Reset, now)
            .invalidated
            .is_empty());
        assert!(extension.sort_memory);
        extension.on_action(crate::extension::Action::Select("process-cpu".into()), now);
        assert!(!extension.sort_memory);
    }
}

#[cfg(test)]
mod freshness_tests {
    use super::*;
    #[test]
    fn power_changes_invalidate_compact_without_waiting_for_gpu_metrics() {
        let (metrics_tx, snapshots) = crate::core::latest::channel();
        let (power_tx, power_updates) = crate::core::latest::channel();
        let mut extension = PerformanceExtension {
            snapshots,
            power_updates,
            power: PowerSnapshot::default(),
            gpu_updates: crate::core::latest::channel().1,
            gpu: None,
            snapshot: Snapshot::default(),
            show_download: true,
            show_upload: true,
            sort_memory: false,
            cpu_history: vec![],
            memory_history: vec![],
        };
        let now = std::time::Instant::now();
        for (external, charging, label) in [
            (true, true, "正在充电"),
            (true, false, "已接通电源"),
            (false, false, "使用电池"),
        ] {
            power_tx.send(PowerSnapshot {
                percent: Some(80),
                external: Some(external),
                charging,
            });
            assert!(!extension.update(now).invalidated.is_empty());
            assert!(extension.compact_views()[0]
                .tooltip
                .as_ref()
                .unwrap()
                .starts_with(label));
            metrics_tx.send(Snapshot::default());
            extension.update(now);
            assert_eq!(extension.snapshot.charging, charging);
            assert_eq!(extension.snapshot.external_power, Some(external));
            assert_eq!(extension.snapshot.battery_percent, Some(80));
        }
        assert!(extension.update(now).invalidated.is_empty());
    }
}
