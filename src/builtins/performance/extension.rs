use super::sampler::{MetricsSampler, Snapshot};
use crate::extension::{
    ColorRole, CompactNode, CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind,
    ExtensionOutput, RefreshHint, RuntimeExtension, ViewNode, WidgetView,
};
use std::sync::mpsc::Receiver;

pub const ID: &str = "builtin.system-monitor/overview";

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(ID, ExtensionKind::Widget, "性能监控", "activity", true, 1)
            .with_size_hint(320.0, 76.0)
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
            54.0,
        ),
        compact_descriptor("builtin.system-monitor/cpu", "CPU", "cpu", false, 2, 56.0),
        compact_descriptor("builtin.system-monitor/gpu", "GPU", "gpu", false, 3, 56.0),
        compact_descriptor(
            "builtin.system-monitor/network",
            "网络",
            "network",
            false,
            4,
            80.0,
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
    snapshots: Receiver<Snapshot>,
    snapshot: Snapshot,
}

impl crate::extension::NotificationPresentationProvider for PerformanceExtension {}

impl PerformanceExtension {
    pub fn new() -> Self {
        Self {
            snapshots: MetricsSampler::start(),
            snapshot: Snapshot::default(),
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
        while let Ok(snapshot) = self.snapshots.try_recv() {
            self.snapshot = snapshot;
            changed = true;
        }
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
        let metrics = &self.snapshot;
        Some(WidgetView {
            root: ViewNode::Row(vec![
                ViewNode::MetricCard {
                    label: "CPU".into(),
                    value: format!("{:.0}%", metrics.cpu_percent),
                    subtitle: None,
                    icon: None,
                    color: ColorRole::Success,
                },
                ViewNode::MetricCard {
                    label: "GPU".into(),
                    value: metrics
                        .gpu_percent
                        .map_or_else(|| "不可用".into(), |value| format!("{value:.0}%")),
                    subtitle: None,
                    icon: None,
                    color: ColorRole::Accent,
                },
                ViewNode::MetricCard {
                    label: "网络".into(),
                    value: format_speed(metrics.download_bytes_per_second),
                    subtitle: Some(format!(
                        "↑ {}",
                        format_speed(metrics.upload_bytes_per_second)
                    )),
                    icon: Some("arrow-down".into()),
                    color: ColorRole::Accent,
                },
                ViewNode::MetricCard {
                    label: "电池".into(),
                    value: metrics
                        .battery_percent
                        .map_or_else(|| "--".into(), |value| format!("{value}%")),
                    subtitle: metrics.charging.then(|| "充电中".into()),
                    icon: Some(
                        if metrics.charging {
                            "battery-charging"
                        } else {
                            "battery"
                        }
                        .into(),
                    ),
                    color: if metrics.battery_percent.is_some_and(|value| value < 20) {
                        ColorRole::Warning
                    } else if metrics.charging {
                        ColorRole::Success
                    } else {
                        ColorRole::Primary
                    },
                },
            ]),
            preferred_size: [320.0, 76.0],
            refresh_hint: RefreshHint::EverySecond,
        })
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let metrics = &self.snapshot;
        let source = ExtensionId::builtin(ID);
        let battery_color = if metrics.battery_percent.is_some_and(|value| value < 20) {
            ColorRole::Warning
        } else if metrics.charging {
            ColorRole::Success
        } else {
            ColorRole::Primary
        };
        vec![
            compact_view(
                "builtin.system-monitor/battery",
                &source,
                CompactNode::IconText {
                    icon: if metrics.charging {
                        "battery-charging"
                    } else {
                        "battery"
                    }
                    .into(),
                    text: metrics
                        .battery_percent
                        .map_or_else(|| "--".into(), |v| format!("{v}%")),
                    color: battery_color,
                },
                54.0,
            ),
            compact_view(
                "builtin.system-monitor/cpu",
                &source,
                CompactNode::IconText {
                    icon: "cpu".into(),
                    text: format!("{:.0}%", metrics.cpu_percent),
                    color: ColorRole::Success,
                },
                56.0,
            ),
            compact_view(
                "builtin.system-monitor/gpu",
                &source,
                CompactNode::IconText {
                    icon: "gpu".into(),
                    text: metrics
                        .gpu_percent
                        .map_or_else(|| "--".into(), |v| format!("{v:.0}%")),
                    color: ColorRole::Accent,
                },
                56.0,
            ),
            compact_view(
                "builtin.system-monitor/network",
                &source,
                CompactNode::Column(vec![
                    CompactNode::IconText {
                        icon: "arrow-down".into(),
                        text: format_speed(metrics.download_bytes_per_second),
                        color: ColorRole::Accent,
                    },
                    CompactNode::IconText {
                        icon: "arrow-up".into(),
                        text: format_speed(metrics.upload_bytes_per_second),
                        color: ColorRole::Success,
                    },
                ]),
                80.0,
            ),
        ]
    }
}

fn compact_view(
    id: &'static str,
    source: &ExtensionId,
    content: CompactNode,
    width: f32,
) -> CompactView {
    CompactView {
        id: ExtensionId::builtin(id),
        source_extension_id: source.clone(),
        content,
        min_width: width,
        preferred_width: width,
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
