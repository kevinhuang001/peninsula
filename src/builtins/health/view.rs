use super::{HealthExtension, STAND, WATER};
use crate::{extension::Action, ui::declarative::*};
use std::time::{Duration, SystemTime};

const WATER_COLOR: ColorSpec = ColorSpec::Info;
const STAND_COLOR: ColorSpec = ColorSpec::Success;

fn text(value: impl Into<String>, size: f32, color: ColorSpec) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Custom(size),
            ..Default::default()
        },
    )
    .color(color)
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

fn centered(node: ViewNode) -> ViewNode {
    ViewNode::row(
        vec![node],
        Flow {
            justify: Justify::Center,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)
}

fn time_label(duration: Duration) -> String {
    let seconds = duration.as_secs() + u64::from(duration.subsec_nanos() > 0);
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

fn ring(fraction: f32, label: String, caption: &str, color: ColorSpec) -> ViewNode {
    let mut ring = ViewNode::ring(fraction, label, 70.0, 4.0, 14.0).color(color);
    if let Node::ProgressRing {
        ring_style,
        text_style,
        ..
    } = &mut ring.kind
    {
        ring_style.track = ColorSpec::Border;
        text_style.monospace = true;
    }
    column(
        vec![
            centered(ring),
            centered(text(caption, 10.0, ColorSpec::SecondaryText)),
        ],
        7.0,
    )
}

fn countdown(
    health: &HealthExtension,
    index: usize,
    now: SystemTime,
    color: ColorSpec,
) -> ViewNode {
    let interval = health.intervals[index];
    let remaining = health.state.reminders[index].remaining(now, interval);
    let done = index == WATER && health.goal_met(now);
    ring(
        if done {
            1.0
        } else {
            (remaining.as_secs_f32() / interval.as_secs_f32()).clamp(0.0, 1.0)
        },
        if done {
            "完成".into()
        } else {
            time_label(remaining)
        },
        if done {
            "今日提醒已暂停"
        } else {
            "下次提醒"
        },
        color,
    )
}

fn card(title: &str, icon: &str, color: ColorSpec, children: Vec<ViewNode>) -> ViewNode {
    let header = ViewNode::row(
        vec![
            ViewNode::icon(icon, 18.0).color(color),
            ViewNode::text(
                title,
                TextStyle {
                    font_size: FontSize::Title,
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                },
            ),
        ],
        Flow {
            gap: Some(7.0),
            ..Default::default()
        },
    );
    let mut content = vec![header];
    content.extend(children);
    let mut card = column(content, 15.0);
    card.style.padding = [12.0, 14.0, 12.0, 14.0];
    card.style.normal.background = Some(ColorSpec::SurfaceVariant);
    card.style.normal.border_color = Some(ColorSpec::Border);
    card.style.normal.border_width = Some(1.0);
    card.style.normal.corner_radius = Some(15.0);
    card
}

fn button(label: &str, action: &str, color: ColorSpec) -> ViewNode {
    let mut button = ViewNode::subtle_button(label, Action::Custom(action.into()))
        .sized(Size::Fill(1.0), Size::Fixed(32.0));
    button.style.normal.foreground = Some(color);
    button.style.normal.background = Some(ColorSpec::Surface);
    button.style.normal.border_color = Some(ColorSpec::Border);
    button.style.normal.border_width = Some(1.0);
    button.style.normal.corner_radius = Some(9.0);
    button
}

pub(super) fn build(health: &HealthExtension, now: SystemTime) -> WidgetView {
    let water = health.state.water_today(now);
    let progress = (water as f32 / health.daily_goal_ml as f32).clamp(0.0, 1.0);
    let goal = ring(
        progress,
        format!("{:.0}%", progress * 100.0),
        "今日饮水",
        WATER_COLOR,
    );
    let mut water_grid = Grid::new(Columns::Tracks(vec![Track::Fraction(1.0); 2]));
    water_grid.column_gap = Some(8.0);
    let water_rings = ViewNode::new(Node::Grid(
        water_grid,
        vec![goal, countdown(health, WATER, now, WATER_COLOR)],
    ))
    .sized(Size::Fill(1.0), Size::Auto);
    let water_card = card(
        "喝水",
        "droplet",
        WATER_COLOR,
        vec![water_rings, button("喝一口水", "drink", WATER_COLOR)],
    );
    let stand_card = card(
        "久坐",
        "health/stand",
        STAND_COLOR,
        vec![
            countdown(health, STAND, now, STAND_COLOR),
            button("起身", "stand", STAND_COLOR),
        ],
    );
    let mut grid = Grid::new(Columns::AutoFit {
        min_item_width: 174.0,
        max_columns: 2,
    });
    grid.column_gap = Some(10.0);
    grid.row_gap = Some(10.0);
    let mut children = vec![
        ViewNode::new(Node::Grid(grid, vec![water_card, stand_card]))
            .sized(Size::Fill(1.0), Size::Auto),
    ];
    if let Some(error) = &health.storage_error {
        children.push(text(error, 10.0, ColorSpec::Danger));
    }
    let mut view = WidgetView::new(column(children, 8.0), [440.0, 280.0]);
    view.bounds.fit_height = true;
    view.bounds.overflow = Overflow::Scroll(ScrollAxes::Vertical);
    view.refresh_hint = RefreshHint::EverySecond;
    view
}
