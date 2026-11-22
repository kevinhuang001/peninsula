use super::{
    extension::{battery_color, battery_label},
    sampler::Snapshot,
};
use crate::{extension::Action, ui::declarative::*};

pub(super) fn memory_percent(s: &Snapshot) -> f32 {
    if s.memory_total == 0 {
        0.0
    } else {
        (s.memory_used as f64 / s.memory_total as f64 * 100.0).clamp(0.0, 100.0) as f32
    }
}

fn text(value: impl Into<String>, size: f32, color: ColorSpec) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Custom(size),
            align: Align::Start,
            max_lines: Some(1),
            ..Default::default()
        },
    )
    .color(color)
}
fn row(children: Vec<ViewNode>, gap: f32) -> ViewNode {
    ViewNode::row(
        children,
        Flow {
            gap: Some(gap),
            justify: Justify::SpaceBetween,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)
}
fn column(children: Vec<ViewNode>, gap: f32) -> ViewNode {
    ViewNode::column(
        children,
        Flow {
            gap: Some(gap),
            align: Align::Stretch,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)
}
fn panel(mut node: ViewNode) -> ViewNode {
    node.style.padding = [10.0; 4];
    node.style.normal.background = Some(ColorSpec::SurfaceVariant);
    node.style.normal.corner_radius = Some(12.0);
    node.clip = true;
    node
}
fn metric(
    label: &str,
    value: String,
    detail: String,
    percent: f32,
    history: &[f32],
    color: ColorSpec,
) -> ViewNode {
    panel(column(
        vec![
            row(
                vec![
                    text(label, 10.0, ColorSpec::SecondaryText),
                    text(value, 20.0, color),
                ],
                6.0,
            ),
            ViewNode::new(Node::Chart {
                kind: ChartKind::Line,
                values: history.to_vec(),
            })
            .sized(Size::Fill(1.0), Size::Fixed(24.0))
            .color(color),
            ViewNode::new(Node::ProgressBar {
                fraction: percent / 100.0,
                label: String::new(),
                marker: false,
            })
            .sized(Size::Fill(1.0), Size::Fixed(3.0))
            .color(color),
            text(detail, 9.0, ColorSpec::SecondaryText),
        ],
        4.0,
    ))
}
fn bytes(value: u64) -> String {
    if value >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", value as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.0} MB", value as f64 / (1024.0 * 1024.0))
    }
}

pub(super) fn build(
    s: &Snapshot,
    memory: bool,
    cpu_history: &[f32],
    memory_history: &[f32],
    download: bool,
    upload: bool,
) -> WidgetView {
    let cpu_color = ColorSpec::Success;
    let memory_color = ColorSpec::Info;
    let mut grid = Grid::new(Columns::Tracks(vec![
        Track::Fraction(1.0),
        Track::Fraction(1.0),
    ]));
    grid.column_gap = Some(8.0);
    let metrics = ViewNode::new(Node::Grid(
        grid,
        vec![
            metric(
                "CPU",
                if s.ready {
                    format!("{:.1}%", s.cpu_percent)
                } else {
                    "—".into()
                },
                "最近 60 次采样 · 整机占用".into(),
                s.cpu_percent,
                cpu_history,
                cpu_color,
            ),
            metric(
                "内存",
                if s.memory_total > 0 {
                    format!("{:.1}%", memory_percent(s))
                } else {
                    "—".into()
                },
                format!("{} / {}", bytes(s.memory_used), bytes(s.memory_total)),
                memory_percent(s),
                memory_history,
                memory_color,
            ),
        ],
    ))
    .sized(Size::Fill(1.0), Size::Auto);
    let mut system = vec![text(
        format!(
            "GPU  {}",
            s.gpu_percent
                .map_or_else(|| "—".into(), |p| format!("{p:.0}%"))
        ),
        10.0,
        ColorSpec::SecondaryText,
    )];
    if download {
        system.push(text(
            format!(
                "↓ {}",
                super::sampler::format_speed(s.download_bytes_per_second)
            ),
            10.0,
            ColorSpec::Info,
        ));
    }
    if upload {
        system.push(text(
            format!(
                "↑ {}",
                super::sampler::format_speed(s.upload_bytes_per_second)
            ),
            10.0,
            ColorSpec::Info,
        ));
    }
    system.push(text(
        if s.battery_percent.is_some() {
            format!(
                "{} {}",
                if s.charging {
                    "充电中"
                } else if s.external_power == Some(true) {
                    "已接电"
                } else {
                    "电池"
                },
                battery_label(s)
            )
        } else {
            "电池 —".into()
        },
        10.0,
        battery_color(s),
    ));
    let tab = |label: &str, key: &str, active: bool| {
        let mut button = ViewNode::subtle_button(label, Action::Select(key.into()));
        button.style.normal.corner_radius = Some(7.0);
        if !active {
            button.style.normal.background = Some(ColorSpec::Transparent);
            button.style.normal.foreground = Some(ColorSpec::SecondaryText);
        }
        button
    };
    let tabs = ViewNode::row(
        vec![
            tab("CPU 占用", "process-cpu", !memory),
            tab("内存占用", "process-memory", memory),
        ],
        Flow {
            gap: Some(3.0),
            ..Default::default()
        },
    );
    let heading = row(
        vec![text("进程 TOP 10", 11.0, ColorSpec::PrimaryText), tabs],
        8.0,
    );
    let processes = if memory { &s.top_memory } else { &s.top_cpu };
    let table = Table {
        columns: vec![
            TableColumn::new("进程 / PID", 2.8),
            TableColumn::new(if memory { "CPU" } else { "CPU ↓" }, 1.0),
            TableColumn::new(if memory { "内存 ↓" } else { "内存" }, 1.0),
        ],
        rows: processes
            .iter()
            .enumerate()
            .map(|(index, p)| {
                vec![
                    text(
                        format!("{:02}  {} · {}", index + 1, p.name, p.pid),
                        10.0,
                        ColorSpec::PrimaryText,
                    ),
                    text(
                        if s.ready {
                            format!("{:.1}%", p.cpu_percent)
                        } else {
                            "—".into()
                        },
                        10.0,
                        if memory {
                            ColorSpec::SecondaryText
                        } else {
                            cpu_color
                        },
                    ),
                    text(
                        bytes(p.memory_bytes),
                        10.0,
                        if memory {
                            memory_color
                        } else {
                            ColorSpec::SecondaryText
                        },
                    ),
                ]
            })
            .collect(),
    };
    let mut content = vec![metrics, panel(row(system, 6.0)), heading];
    if processes.is_empty() {
        content.push(text("正在读取进程…", 11.0, ColorSpec::SecondaryText));
    } else {
        content.push(ViewNode::new(Node::Table(table)).sized(Size::Fill(1.0), Size::Auto));
    }
    content.push(row(
        vec![
            text(
                format!("{} 个进程 · 每秒更新", s.process_count),
                9.0,
                ColorSpec::MutedText,
            ),
            text("CPU 按整机 100% 计", 9.0, ColorSpec::MutedText),
        ],
        6.0,
    ));
    WidgetView::new(column(content, 8.0), [440.0, 410.0])
}
