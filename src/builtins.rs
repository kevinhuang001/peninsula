use crate::{
    downloads::Download,
    extension::{ColorRole, CompactNode, CompactView, ExtensionId},
    metrics::{format_speed, Snapshot},
};

pub fn compact_view(
    id: &ExtensionId,
    metrics: &Snapshot,
    download: Option<&Download>,
) -> Option<CompactView> {
    let (content, preferred_width) = match id.as_str() {
        "builtin.mascot/face" => (
            CompactNode::Sprite {
                asset: "mascot".into(),
                size: [38.0, 28.0],
            },
            38.0,
        ),
        "builtin.system-monitor/battery" => {
            let color = if metrics.battery_percent.is_some_and(|value| value < 20) {
                ColorRole::Warning
            } else if metrics.charging {
                ColorRole::Success
            } else {
                ColorRole::Primary
            };
            let icon = if metrics.charging {
                "battery-charging"
            } else {
                "battery"
            };
            (
                CompactNode::IconText {
                    icon: icon.into(),
                    text: metrics
                        .battery_percent
                        .map_or_else(|| "--".into(), |value| format!("{value}%")),
                    color,
                },
                54.0,
            )
        }
        "builtin.system-monitor/cpu" => (
            CompactNode::IconText {
                icon: "cpu".into(),
                text: format!("{:.0}%", metrics.cpu_percent),
                color: ColorRole::Success,
            },
            56.0,
        ),
        "builtin.system-monitor/gpu" => (
            CompactNode::IconText {
                icon: "gpu".into(),
                text: metrics
                    .gpu_percent
                    .map_or_else(|| "--".into(), |value| format!("{value:.0}%")),
                color: ColorRole::Accent,
            },
            56.0,
        ),
        "builtin.system-monitor/network" => (
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
        "builtin.downloads/status" => {
            let download = download?;
            let size = download.total.filter(|total| *total > 0).map_or_else(
                || compact_size(download.received),
                |total| compact_pair(download.received, total),
            );
            let fraction = download.progress().unwrap_or(0.0);
            (
                CompactNode::Row(vec![
                    CompactNode::Progress {
                        fraction,
                        label: download
                            .progress()
                            .map_or_else(|| "↓".into(), |value| format!("{:.0}%", value * 100.0)),
                    },
                    CompactNode::Column(vec![
                        CompactNode::Text {
                            text: size,
                            color: ColorRole::Primary,
                        },
                        CompactNode::Text {
                            text: format_speed(download.bytes_per_second),
                            color: ColorRole::Accent,
                        },
                    ]),
                ]),
                88.0,
            )
        }
        "builtin.clock/time" => {
            let now = time::OffsetDateTime::now_local()
                .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
            (
                CompactNode::IconText {
                    icon: "clock".into(),
                    text: format!("{:02}:{:02}", now.hour(), now.minute()),
                    color: ColorRole::Primary,
                },
                56.0,
            )
        }
        _ => return None,
    };
    Some(CompactView {
        content,
        min_width: preferred_width,
        preferred_width,
        priority: 0,
        tooltip: None,
    })
}

fn compact_size(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < 3 {
        value /= 1024.0;
        unit += 1;
    }
    let display = if value >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    format!("{display}{}", ["B", "K", "M", "G"][unit])
}

fn compact_pair(received: u64, total: u64) -> String {
    let unit = unit_index(received.max(total));
    let divisor = 1024_f64.powi(unit as i32);
    format!(
        "{}/{}{}",
        compact_number(received as f64 / divisor),
        compact_number(total as f64 / divisor),
        ["B", "K", "M", "G"][unit]
    )
}

fn unit_index(bytes: u64) -> usize {
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024 && unit < 3 {
        value /= 1024;
        unit += 1;
    }
    unit
}

fn compact_number(value: f64) -> String {
    if value >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::compact_view;
    use crate::{extension::ExtensionId, metrics::Snapshot};

    #[test]
    fn built_in_compact_views_are_data_only_and_downloads_can_be_absent() {
        let metrics = Snapshot::default();
        assert!(compact_view(
            &ExtensionId::builtin("builtin.system-monitor/cpu"),
            &metrics,
            None
        )
        .is_some());
        assert!(compact_view(
            &ExtensionId::builtin("builtin.downloads/status"),
            &metrics,
            None
        )
        .is_none());
    }
}
