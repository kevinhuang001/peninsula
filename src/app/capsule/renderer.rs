use crate::{
    app::icons::{Icon, Icons},
    extension::{
        Action, ActionInvocation, ColorRole, ExtensionId, TextStyle, ViewNode, WidgetView,
    },
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::settings::theme::{POMODORO_BUTTON, WIDGET_BUTTON};

const CARD: Color32 = Color32::from_rgb(27, 29, 34);
const MUTED: Color32 = Color32::from_rgb(177, 183, 194);

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
            if let Some(icon) = icon_from_name(name) {
                icons.paint(
                    ui.painter(),
                    icon,
                    rect.shrink(5.0),
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
            if let Some(name) = icon.as_deref().and_then(icon_from_name) {
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
            if let Some(name) = icon.as_deref().and_then(icon_from_name) {
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

fn icon_from_name(name: &str) -> Option<Icon> {
    Some(match name {
        "battery" => Icon::Battery,
        "battery-charging" => Icon::BatteryCharging,
        "cpu" => Icon::Cpu,
        "gpu" => Icon::Gpu,
        "arrow-down" => Icon::ArrowDown,
        "arrow-up" => Icon::ArrowUp,
        "clock" => Icon::Clock,
        _ => return None,
    })
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
