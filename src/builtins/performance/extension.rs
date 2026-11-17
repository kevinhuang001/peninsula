use super::sampler::{MetricsSampler, Snapshot};
use crate::extension::{
    CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind, ExtensionOutput, ReloadPolicy,
    RuntimeExtension, SettingSchema, SettingValue, Settings, ViewNode, WidgetView,
};
use crate::ui::declarative::*;
use std::sync::mpsc::Receiver;

pub const ID: &str = "builtin.system-monitor/overview";

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(ID, ExtensionKind::Widget, "性能监控", "activity", true, 1)
            .with_size_hint(320.0, 76.0)
            .with_widget_bounds(WidgetBounds {
                min: [244.0, 76.0],
                max: [Some(380.0), Some(150.0)],
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
    show_download: bool,
    show_upload: bool,
}

impl crate::extension::NotificationPresentationProvider for PerformanceExtension {}

impl PerformanceExtension {
    pub fn new() -> Self {
        Self {
            snapshots: MetricsSampler::start(),
            snapshot: Snapshot::default(),
            show_download: true,
            show_upload: true,
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
        let network_color = ColorSpec::Info;
        let mut network = vec![super::super::compact_text("网络").color(ColorSpec::SecondaryText)];
        network.extend(self.network_rows(network_color));
        let network = card(ViewNode::column(
            network,
            Flow {
                gap: Some(2.0),
                justify: Justify::Center,
                ..Default::default()
            },
        ));
        let metric = |label: &str, value: String, color| {
            card(
                ViewNode::new(Node::Metric {
                    label: label.into(),
                    value,
                    unit: String::new(),
                })
                .color(color),
            )
        };
        let battery = card(ViewNode::column(
            vec![
                super::super::compact_text("电池").color(ColorSpec::SecondaryText),
                super::super::icon_text(
                    if metrics.charging {
                        "battery-charging"
                    } else {
                        "battery"
                    },
                    battery_label(metrics),
                    battery_color(metrics),
                    14.0,
                ),
            ],
            Flow {
                gap: Some(4.0),
                justify: Justify::Center,
                ..Default::default()
            },
        ));
        let mut grid = Grid::new(Columns::AutoFit {
            min_item_width: 65.0,
            max_columns: 4,
        });
        grid.column_gap = Some(6.0);
        grid.row_gap = Some(6.0);
        let view = WidgetView::new(
            ViewNode::new(Node::Grid(
                grid,
                vec![
                    metric(
                        "CPU",
                        format!("{:.0}%", metrics.cpu_percent),
                        ColorSpec::Success,
                    ),
                    metric(
                        "GPU",
                        metrics
                            .gpu_percent
                            .map_or_else(|| "不可用".into(), |value| format!("{value:.0}%")),
                        ColorSpec::Info,
                    ),
                    network,
                    battery,
                ],
            )),
            [320.0, 76.0],
        );
        Some(view)
    }

    fn compact_views(&self) -> Vec<CompactView> {
        use super::super::icon_text;
        let metrics = &self.snapshot;
        vec![
            compact_view(
                "builtin.system-monitor/battery",
                icon_text(
                    if metrics.charging {
                        "battery-charging"
                    } else {
                        "battery"
                    },
                    battery_label(metrics),
                    battery_color(metrics),
                    16.0,
                ),
                54.0,
            ),
            compact_view(
                "builtin.system-monitor/cpu",
                icon_text(
                    "cpu",
                    format!("{:.0}%", metrics.cpu_percent),
                    ColorSpec::Success,
                    16.0,
                ),
                56.0,
            ),
            compact_view(
                "builtin.system-monitor/gpu",
                icon_text(
                    "gpu",
                    metrics
                        .gpu_percent
                        .map_or_else(|| "--".into(), |v| format!("{v:.0}%")),
                    ColorSpec::Info,
                    16.0,
                ),
                56.0,
            ),
        ]
        .into_iter()
        .chain((self.show_download || self.show_upload).then(|| {
            compact_view(
                "builtin.system-monitor/network",
                ViewNode::column(
                    self.network_rows(ColorSpec::Info),
                    Flow {
                        gap: Some(0.0),
                        ..Default::default()
                    },
                ),
                80.0,
            )
        }))
        .collect()
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

impl PerformanceExtension {
    fn network_rows(&self, color: ColorSpec) -> Vec<ViewNode> {
        use super::super::icon_text;
        [
            (
                self.show_download,
                "arrow-down",
                self.snapshot.download_bytes_per_second,
            ),
            (
                self.show_upload,
                "arrow-up",
                self.snapshot.upload_bytes_per_second,
            ),
        ]
        .into_iter()
        .filter(|(enabled, _, _)| *enabled)
        .map(|(_, icon, speed)| icon_text(icon, format_speed(speed), color, 12.0))
        .collect()
    }
}

fn battery_label(metrics: &Snapshot) -> String {
    metrics
        .battery_percent
        .map_or_else(|| "--".into(), |v| format!("{v}%"))
}

fn battery_color(metrics: &Snapshot) -> ColorSpec {
    if metrics.battery_percent.is_some_and(|v| v < 20) {
        ColorSpec::Danger
    } else if metrics.charging {
        ColorSpec::Success
    } else {
        ColorSpec::PrimaryText
    }
}

fn card(mut node: ViewNode) -> ViewNode {
    node.style.normal.background = Some(ColorSpec::SurfaceVariant);
    node.style.normal.corner_radius = Some(10.0);
    node.style.normal.border_color = Some(ColorSpec::Border);
    node.style.normal.border_width = Some(1.0);
    node.style.padding = [4.0, 2.0, 4.0, 2.0];
    node.size.min_height = Some(58.0);
    node.clip = true;
    node
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
