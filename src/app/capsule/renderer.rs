use crate::{
    app::icons::{Icon, Icons},
    extension::{
        Action, ActionInvocation, ColorRole, ExtensionId, TextStyle, ViewNode, WidgetView,
    },
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::settings::theme::{POMODORO_BUTTON, WIDGET_BUTTON};

const CARD: Color32 = Color32::from_rgb(27, 29, 34);
const CHART_CARD: Color32 = Color32::from_rgb(23, 26, 32);
const MUTED: Color32 = Color32::from_rgb(177, 183, 194);
const CHART_HIGH: Color32 = Color32::from_rgb(255, 159, 109);
const CHART_LOW: Color32 = Color32::from_rgb(99, 179, 255);

pub(super) fn show_widget(
    ui: &mut egui::Ui,
    rect: Rect,
    source_id: &ExtensionId,
    view: &WidgetView,
    icons: &Icons,
    opacity: f32,
) -> Option<ActionInvocation> {
    paint_node(ui, rect, &view.root, source_id, icons, opacity)
}

fn paint_node(
    ui: &mut egui::Ui,
    rect: Rect,
    node: &ViewNode,
    source_id: &ExtensionId,
    icons: &Icons,
    opacity: f32,
) -> Option<ActionInvocation> {
    match node {
        ViewNode::Row(children) => {
            let gap = 8.0;
            let total_gap = gap * children.len().saturating_sub(1) as f32;
            let width = (rect.width() - total_gap).max(0.0) / children.len().max(1) as f32;
            for (index, child) in children.iter().enumerate() {
                let child_rect = Rect::from_min_size(
                    Pos2::new(rect.left() + index as f32 * (width + gap), rect.top()),
                    Vec2::new(width, rect.height()),
                );
                if let Some(action) = paint_node(ui, child_rect, child, source_id, icons, opacity) {
                    return Some(action);
                }
            }
        }
        ViewNode::Column(children) => {
            let heights: Vec<_> = children.iter().map(preferred_height).collect();
            let fixed: f32 = heights.iter().sum();
            let fill_count = heights.iter().filter(|height| **height == 0.0).count();
            let fill_height = (rect.height() - fixed).max(0.0) / fill_count.max(1) as f32;
            let mut top = rect.top();
            for (child, height) in children.iter().zip(heights) {
                let height = if height == 0.0 { fill_height } else { height };
                let child_rect = Rect::from_min_size(
                    Pos2::new(rect.left(), top),
                    Vec2::new(rect.width(), height.min(rect.bottom() - top).max(0.0)),
                );
                if let Some(action) = paint_node(ui, child_rect, child, source_id, icons, opacity) {
                    return Some(action);
                }
                top += height;
            }
        }
        ViewNode::Text { text, style } => {
            let (size, color, align) = match style {
                TextStyle::Display => (30.0, Color32::WHITE, Align2::CENTER_CENTER),
                TextStyle::Title => (17.0, Color32::WHITE, Align2::CENTER_CENTER),
                TextStyle::Heading => (14.0, Color32::WHITE, Align2::CENTER_CENTER),
                TextStyle::Muted => (11.0, MUTED, Align2::CENTER_CENTER),
                TextStyle::Small => (10.0, MUTED, Align2::CENTER_CENTER),
                TextStyle::Body => (12.0, Color32::WHITE, Align2::CENTER_CENTER),
            };
            ui.painter().text(
                rect.center(),
                align,
                text,
                FontId::proportional(size),
                color.linear_multiply(opacity),
            );
        }
        ViewNode::Icon { name, color } => {
            if let Some(icon) = Icon::from_name(name) {
                let side = (rect.width().min(rect.height()) - 8.0).max(1.0);
                icons.paint(
                    ui.painter(),
                    icon,
                    Rect::from_center_size(rect.center(), Vec2::splat(side)),
                    color_from_role(*color),
                    opacity,
                );
            }
        }
        ViewNode::Metric {
            label,
            value,
            color,
        } => {
            ui.painter().text(
                Pos2::new(rect.center().x, rect.top() + 5.0),
                Align2::CENTER_TOP,
                label,
                FontId::proportional(11.0),
                MUTED.linear_multiply(opacity),
            );
            ui.painter().text(
                Pos2::new(rect.center().x, rect.center().y + 4.0),
                Align2::CENTER_CENTER,
                value,
                FontId::proportional(16.0),
                color_from_role(*color).linear_multiply(opacity),
            );
        }
        ViewNode::MetricCard {
            label,
            value,
            subtitle,
            icon,
            color,
        } => {
            let painter = ui.painter();
            painter.rect_filled(rect, 9.0, CARD.linear_multiply(opacity));
            painter.text(
                Pos2::new(rect.center().x, rect.top() + 5.0),
                Align2::CENTER_TOP,
                label,
                FontId::proportional(11.0),
                Color32::from_rgb(248, 250, 253).linear_multiply(opacity),
            );
            let value_y = if subtitle.is_some() {
                rect.top() + 25.0
            } else {
                rect.top() + 28.0
            };
            if let Some(name) = icon.as_deref().and_then(Icon::from_name) {
                let icon_rect = Rect::from_center_size(
                    Pos2::new(rect.center().x - 18.0, value_y + 8.0),
                    Vec2::splat(16.0),
                );
                icons.paint(painter, name, icon_rect, color_from_role(*color), opacity);
                painter.text(
                    Pos2::new(rect.center().x + 9.0, value_y),
                    Align2::CENTER_TOP,
                    value,
                    FontId::proportional(14.0),
                    color_from_role(*color).linear_multiply(opacity),
                );
            } else {
                painter.text(
                    Pos2::new(rect.center().x, value_y),
                    Align2::CENTER_TOP,
                    value,
                    FontId::proportional(16.0),
                    color_from_role(*color).linear_multiply(opacity),
                );
            }
            if let Some(subtitle) = subtitle {
                painter.text(
                    Pos2::new(rect.center().x, rect.top() + 45.0),
                    Align2::CENTER_TOP,
                    subtitle,
                    FontId::proportional(10.0),
                    color_from_role(*color).linear_multiply(opacity),
                );
            }
        }
        ViewNode::Badge { text, icon, color } => {
            let painter = ui.painter();
            let role = color_from_role(*color);
            let text_width = painter
                .layout_no_wrap(text.clone(), FontId::proportional(9.5), role)
                .size()
                .x;
            let icon_width = if icon.is_some() { 16.0 } else { 0.0 };
            let width = (text_width + icon_width + 16.0).min(rect.width());
            let badge = Rect::from_center_size(rect.center(), Vec2::new(width, 18.0));
            painter.rect_filled(
                badge,
                9.0,
                Color32::from_rgba_unmultiplied(role.r(), role.g(), role.b(), 26)
                    .linear_multiply(opacity),
            );
            painter.rect_stroke(
                badge,
                9.0,
                Stroke::new(1.0_f32, role.linear_multiply(0.32 * opacity)),
                egui::StrokeKind::Inside,
            );
            let mut left = badge.center().x - (text_width + icon_width) * 0.5;
            if let Some(name) = icon.as_deref().and_then(Icon::from_name) {
                icons.paint(
                    painter,
                    name,
                    Rect::from_center_size(
                        Pos2::new(left + 5.0, badge.center().y),
                        Vec2::splat(10.0),
                    ),
                    role.linear_multiply(opacity),
                    opacity,
                );
                left += icon_width;
            }
            painter.text(
                Pos2::new(left, badge.center().y),
                Align2::LEFT_CENTER,
                text,
                FontId::proportional(9.5),
                role.linear_multiply(opacity),
            );
        }
        ViewNode::StatGrid { cells } => {
            if cells.is_empty() {
                return None;
            }
            let painter = ui.painter();
            let columns = 3;
            let rows = cells.len().div_ceil(columns);
            let gap = 4.0;
            let cell_width = (rect.width() - gap * (columns as f32 - 1.0)) / columns as f32;
            let cell_height = (rect.height() - gap * (rows as f32 - 1.0)) / rows as f32;
            for (index, cell) in cells.iter().enumerate() {
                let row = index / columns;
                let column = index % columns;
                let cell_rect = Rect::from_min_size(
                    Pos2::new(
                        rect.left() + column as f32 * (cell_width + gap),
                        rect.top() + row as f32 * (cell_height + gap),
                    ),
                    Vec2::new(cell_width, cell_height),
                );
                painter.rect_filled(cell_rect, 8.0, CARD.linear_multiply(opacity));
                let mut text_left = cell_rect.left() + 9.0;
                if let Some(name) = cell.icon.as_deref().and_then(Icon::from_name) {
                    icons.paint(
                        painter,
                        name,
                        Rect::from_center_size(
                            Pos2::new(cell_rect.left() + 15.0, cell_rect.center().y),
                            Vec2::splat(11.0),
                        ),
                        color_from_role(cell.color).linear_multiply(opacity),
                        opacity,
                    );
                    text_left = cell_rect.left() + 27.0;
                }
                painter.text(
                    Pos2::new(text_left, cell_rect.center().y),
                    Align2::LEFT_CENTER,
                    &cell.label,
                    FontId::proportional(8.6),
                    MUTED.linear_multiply(opacity),
                );
                painter.text(
                    Pos2::new(cell_rect.right() - 8.0, cell_rect.center().y),
                    Align2::RIGHT_CENTER,
                    &cell.value,
                    FontId::proportional(10.2),
                    color_from_role(cell.color).linear_multiply(opacity),
                );
            }
        }
        ViewNode::RangeChart { labels, high, low } => {
            let count = labels.len().min(high.len()).min(low.len());
            if count == 0 {
                return None;
            }
            let painter = ui.painter();
            painter.rect_filled(rect, 10.0, CHART_CARD.linear_multiply(opacity));
            let plot = Rect::from_min_max(
                Pos2::new(rect.left() + 14.0, rect.top() + 17.0),
                Pos2::new(rect.right() - 14.0, rect.bottom() - 17.0),
            );
            let mut min = low
                .iter()
                .take(count)
                .fold(f32::INFINITY, |min, value| min.min(*value));
            let mut max = high
                .iter()
                .take(count)
                .fold(f32::NEG_INFINITY, |max, value| max.max(*value));
            if !min.is_finite() || !max.is_finite() {
                return None;
            }
            let padding = (max - min).abs().max(1.0) * 0.18 + 1.0;
            min -= padding;
            max += padding;
            let span = (max - min).max(0.1);
            let x_at = |index: usize| {
                if count == 1 {
                    plot.center().x
                } else {
                    plot.left() + plot.width() * index as f32 / (count - 1) as f32
                }
            };
            let y_at = |value: f32| plot.bottom() - (value - min) / span * plot.height();
            for line in 0..=2 {
                let y = plot.top() + plot.height() * line as f32 / 2.0;
                painter.line_segment(
                    [
                        Pos2::new(plot.left(), y),
                        Pos2::new(plot.right(), y),
                    ],
                    Stroke::new(1.0_f32, Color32::from_white_alpha(9).linear_multiply(opacity)),
                );
            }
            let high_points: Vec<_> = (0..count)
                .map(|index| Pos2::new(x_at(index), y_at(high[index])))
                .collect();
            let low_points: Vec<_> = (0..count)
                .map(|index| Pos2::new(x_at(index), y_at(low[index])))
                .collect();
            let band = Color32::from_rgba_unmultiplied(99, 179, 255, 24).linear_multiply(opacity);
            for index in 0..count.saturating_sub(1) {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        high_points[index],
                        high_points[index + 1],
                        low_points[index + 1],
                        low_points[index],
                    ],
                    band,
                    Stroke::NONE,
                ));
            }
            painter.add(egui::Shape::line(
                high_points.clone(),
                Stroke::new(1.6_f32, CHART_HIGH.linear_multiply(opacity)),
            ));
            painter.add(egui::Shape::line(
                low_points.clone(),
                Stroke::new(1.6_f32, CHART_LOW.linear_multiply(opacity)),
            ));
            for index in 0..count {
                painter.circle_filled(high_points[index], 2.4, CHART_HIGH.linear_multiply(opacity));
                painter.circle_filled(low_points[index], 2.4, CHART_LOW.linear_multiply(opacity));
                painter.text(
                    high_points[index] + Vec2::new(0.0, -5.0),
                    Align2::CENTER_BOTTOM,
                    format!("{:.0}°", high[index]),
                    FontId::proportional(8.0),
                    CHART_HIGH.linear_multiply(opacity),
                );
                painter.text(
                    low_points[index] + Vec2::new(0.0, 5.0),
                    Align2::CENTER_TOP,
                    format!("{:.0}°", low[index]),
                    FontId::proportional(8.0),
                    CHART_LOW.linear_multiply(opacity),
                );
                painter.text(
                    Pos2::new(x_at(index), rect.bottom() - 4.0),
                    Align2::CENTER_BOTTOM,
                    &labels[index],
                    FontId::proportional(8.4),
                    MUTED.linear_multiply(opacity),
                );
            }
        }
        ViewNode::InfoBar { cells } => {
            if cells.is_empty() {
                return None;
            }
            let painter = ui.painter();
            painter.rect_filled(rect, 9.0, CARD.linear_multiply(opacity));
            let section_width = rect.width() / cells.len() as f32;
            for (index, cell) in cells.iter().enumerate() {
                let section = Rect::from_min_size(
                    Pos2::new(rect.left() + index as f32 * section_width, rect.top()),
                    Vec2::new(section_width, rect.height()),
                );
                if index > 0 {
                    painter.line_segment(
                        [
                            Pos2::new(section.left(), section.top() + 8.0),
                            Pos2::new(section.left(), section.bottom() - 8.0),
                        ],
                        Stroke::new(
                            1.0_f32,
                            Color32::from_rgb(48, 52, 60).linear_multiply(opacity),
                        ),
                    );
                }
                let mut text_left = section.left() + 10.0;
                if let Some(name) = cell.icon.as_deref().and_then(Icon::from_name) {
                    icons.paint(
                        painter,
                        name,
                        Rect::from_center_size(
                            Pos2::new(section.left() + 15.0, section.center().y),
                            Vec2::splat(12.0),
                        ),
                        color_from_role(cell.color).linear_multiply(opacity),
                        opacity,
                    );
                    text_left = section.left() + 27.0;
                }
                painter.text(
                    Pos2::new(text_left, section.center().y),
                    Align2::LEFT_CENTER,
                    &cell.label,
                    FontId::proportional(8.6),
                    MUTED.linear_multiply(opacity),
                );
                painter.text(
                    Pos2::new(section.right() - 8.0, section.center().y),
                    Align2::RIGHT_CENTER,
                    &cell.value,
                    FontId::proportional(10.6),
                    color_from_role(cell.color).linear_multiply(opacity),
                );
            }
        }
        ViewNode::Sized { height, child } => {
            let child_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width(), (*height).min(rect.height())),
            );
            return paint_node(ui, child_rect, child, source_id, icons, opacity);
        }
        ViewNode::Divider => {
            ui.painter().line_segment(
                [
                    Pos2::new(rect.left() + 4.0, rect.center().y),
                    Pos2::new(rect.right() - 4.0, rect.center().y),
                ],
                Stroke::new(1.0_f32, Color32::from_rgb(49, 53, 61).linear_multiply(opacity)),
            );
        }
        ViewNode::NotificationItem {
            stable_id,
            title,
            body,
            icon,
        } => {
            let clear_rect = Rect::from_center_size(
                Pos2::new(rect.right() - 15.0, rect.center().y),
                Vec2::new(22.0, 22.0),
            );
            let content_rect =
                Rect::from_min_max(rect.min, Pos2::new(clear_rect.left() - 2.0, rect.bottom()));
            let response = ui.interact(
                content_rect,
                egui::Id::new(("notification-item", stable_id)),
                Sense::click(),
            );
            ui.painter().rect_filled(
                rect,
                7.0,
                if response.hovered() {
                    Color32::from_rgb(47, 53, 64)
                } else {
                    Color32::from_rgb(35, 39, 47)
                }
                .linear_multiply(opacity),
            );
            let text_left = if icon.is_some() {
                rect.left() + 28.0
            } else {
                rect.left() + 9.0
            };
            if let Some(name) = icon.as_deref().and_then(Icon::from_name) {
                icons.paint(
                    ui.painter(),
                    name,
                    Rect::from_center_size(
                        Pos2::new(rect.left() + 17.0, rect.center().y),
                        Vec2::splat(16.0),
                    ),
                    Color32::from_rgb(91, 190, 255),
                    opacity,
                );
            }
            ui.painter().text(
                Pos2::new(text_left, rect.top() + 8.0),
                Align2::LEFT_TOP,
                truncate(title, 28),
                FontId::proportional(10.5),
                Color32::WHITE.linear_multiply(opacity),
            );
            ui.painter().text(
                Pos2::new(text_left, rect.top() + 21.0),
                Align2::LEFT_TOP,
                truncate(body, 36),
                FontId::proportional(9.5),
                MUTED.linear_multiply(opacity),
            );
            if ui
                .put(
                    clear_rect,
                    egui::Button::new(egui::RichText::new("×").color(Color32::WHITE))
                        .fill(WIDGET_BUTTON)
                        .corner_radius(7.0),
                )
                .clicked()
            {
                return Some(ActionInvocation {
                    source_extension_id: source_id.clone(),
                    action: Action::Custom(format!("core.notifications.acknowledge:{stable_id}")),
                });
            }
            if response.clicked() {
                return Some(ActionInvocation {
                    source_extension_id: source_id.clone(),
                    action: Action::Select(stable_id.clone()),
                });
            }
        }
        ViewNode::Button { label, action } => {
            let fill = if source_id.as_str() == "builtin.pomodoro/timer" {
                POMODORO_BUTTON
            } else {
                WIDGET_BUTTON
            };
            let response = ui.put(
                rect.shrink(1.0),
                egui::Button::new(egui::RichText::new(label).color(Color32::WHITE))
                    .fill(fill)
                    .corner_radius(8.0),
            );
            if response.clicked() {
                return Some(ActionInvocation {
                    source_extension_id: source_id.clone(),
                    action: action.clone(),
                });
            }
        }
        ViewNode::ProgressRing {
            fraction,
            label,
            color,
        } => {
            let radius = (rect.width().min(rect.height()) * 0.5 - 3.0).max(2.0);
            let center = Pos2::new(rect.center().x, rect.center().y);
            ui.painter().circle_stroke(
                center,
                radius,
                Stroke::new(3.0_f32, Color32::from_rgb(55, 59, 68)),
            );
            ui.painter().add(egui::Shape::line(
                progress_points(center, radius, *fraction),
                Stroke::new(3.0_f32, color_from_role(*color)),
            ));
            if let Some(label) = label {
                ui.painter().text(
                    center,
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(18.0),
                    Color32::WHITE,
                );
            }
        }
        ViewNode::Sprite { .. } | ViewNode::Spacer { .. } => {}
    }
    None
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        format!(
            "{}…",
            text.chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    }
}

fn preferred_height(node: &ViewNode) -> f32 {
    match node {
        ViewNode::ProgressRing { .. } => 66.0,
        ViewNode::Text {
            style: TextStyle::Small,
            ..
        } => 18.0,
        ViewNode::Row(children)
            if children
                .iter()
                .all(|child| matches!(child, ViewNode::Button { .. })) =>
        {
            24.0
        }
        ViewNode::Button { .. } => 24.0,
        ViewNode::NotificationItem { .. } => 40.0,
        ViewNode::Badge { .. } => 20.0,
        ViewNode::StatGrid { cells } => {
            if cells.is_empty() {
                0.0
            } else {
                64.0
            }
        }
        ViewNode::RangeChart { .. } => 78.0,
        ViewNode::Sized { height, .. } => *height,
        ViewNode::Divider => 9.0,
        ViewNode::Spacer { size } => size[1],
        _ => 0.0,
    }
}

fn color_from_role(role: ColorRole) -> Color32 {
    match role {
        ColorRole::Primary => Color32::from_rgb(247, 249, 252),
        ColorRole::Muted => MUTED,
        ColorRole::Accent => Color32::from_rgb(73, 183, 255),
        ColorRole::Success => Color32::from_rgb(35, 255, 172),
        ColorRole::Warning => Color32::from_rgb(255, 117, 107),
    }
}


fn progress_points(center: Pos2, radius: f32, fraction: f32) -> Vec<Pos2> {
    let fraction = fraction.clamp(0.0, 1.0);
    let segments = (fraction * 48.0).ceil().max(1.0) as usize;
    (0..=segments)
        .map(|index| {
            let angle = -std::f32::consts::FRAC_PI_2
                + std::f32::consts::TAU * fraction * index as f32 / segments as f32;
            center + Vec2::new(angle.cos(), angle.sin()) * radius
        })
        .collect()
}
