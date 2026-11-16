#[cfg(test)]
mod download;

use crate::{
    app::{
        compact_layout::{
            CHIP_HEIGHT, COMPACT_ICON_GAP, COMPACT_ICON_SIZE, COMPACT_TEXT_SIZE, ITEM_GAP,
        },
        icons::{Icon, Icons},
        mascot::Mascot,
        settings::CompactPreferences,
    },
    builtins,
    downloads::Download,
    extension::{ColorRole, CompactNode, CompactView},
    metrics::Snapshot,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use super::draw_compact_chip;

const DOWN: Color32 = Color32::from_rgb(80, 190, 255);

pub(super) struct CompactContent<'a> {
    pub(super) metrics: &'a Snapshot,
    pub(super) mascot: &'a mut Mascot,
    pub(super) prefs: &'a CompactPreferences,
    pub(super) download: Option<&'a Download>,
    pub(super) icons: &'a Icons,
}

pub(super) fn show(
    ui: &egui::Ui,
    context: &egui::Context,
    rect: Rect,
    progress: f32,
    content: CompactContent<'_>,
) -> bool {
    let CompactContent {
        metrics,
        mascot,
        prefs,
        download,
        icons,
    } = content;
    let opacity = 1.0 - progress.clamp(0.0, 1.0);
    let views: Vec<_> = prefs
        .ordered_compact_items()
        .into_iter()
        .filter(|item| prefs.is_enabled(&item.id))
        .filter_map(|item| {
            builtins::compact_view(&item.id, metrics, download).map(|view| (item.id.clone(), view))
        })
        .collect();
    let total = views
        .iter()
        .map(|(_, view)| view.preferred_width)
        .sum::<f32>()
        + ITEM_GAP * views.len().saturating_sub(1) as f32;
    let mut x = rect.center().x - total * 0.5;
    let center_y = rect.center().y;

    for (id, view) in views {
        let width = view.preferred_width;
        let item_rect = Rect::from_center_size(
            Pos2::new(x + width * 0.5, center_y),
            Vec2::new(width, CHIP_HEIGHT),
        );
        if let CompactNode::Sprite { size, .. } = &view.content {
            render_sprite(ui, context, mascot, x, center_y, *size, opacity);
        } else {
            draw_compact_chip(ui, item_rect, opacity);
            if let Some(tooltip) = &view.tooltip {
                ui.interact(
                    item_rect,
                    egui::Id::new(("compact-item", id)),
                    Sense::hover(),
                )
                .on_hover_text(tooltip);
            }
            render_node(ui, context, mascot, icons, item_rect, &view, opacity);
        }
        x += width + ITEM_GAP;
    }
    false
}

fn render_node(
    ui: &egui::Ui,
    context: &egui::Context,
    mascot: &mut Mascot,
    icons: &Icons,
    rect: Rect,
    view: &CompactView,
    opacity: f32,
) {
    match &view.content {
        CompactNode::Icon { name, color } => {
            paint_icon(ui, icons, rect, name, *color, opacity);
        }
        CompactNode::Text { text, color } => {
            paint_text(
                ui,
                Pos2::new(rect.left() + 3.0, rect.center().y),
                text,
                role_color(*color),
                opacity,
                COMPACT_TEXT_SIZE,
            );
        }
        CompactNode::IconText { icon, text, color } => {
            let text_width = ui
                .painter()
                .layout_no_wrap(
                    text.clone(),
                    FontId::proportional(COMPACT_TEXT_SIZE),
                    role_color(*color),
                )
                .size()
                .x;
            let content_width = COMPACT_ICON_SIZE + COMPACT_ICON_GAP + text_width;
            let left = rect.center().x - content_width * 0.5;
            let icon_rect = Rect::from_center_size(
                Pos2::new(left + COMPACT_ICON_SIZE * 0.5, rect.center().y),
                Vec2::splat(COMPACT_ICON_SIZE),
            );
            icons.paint(
                ui.painter(),
                icon_id(icon),
                icon_rect,
                role_color(*color),
                opacity,
            );
            paint_text(
                ui,
                Pos2::new(left + COMPACT_ICON_SIZE + COMPACT_ICON_GAP, rect.center().y),
                text,
                role_color(*color),
                opacity,
                COMPACT_TEXT_SIZE,
            );
        }
        CompactNode::Progress { fraction, label } => {
            paint_progress(ui, rect, *fraction, label, opacity);
        }
        CompactNode::Sprite { size, .. } => {
            render_sprite(
                ui,
                context,
                mascot,
                rect.left(),
                rect.center().y,
                *size,
                opacity,
            );
        }
        CompactNode::Row(children) => {
            let mut x = rect.left();
            let mut remaining_width = rect.width();
            for (index, child) in children.iter().enumerate() {
                let child_width = if index + 1 == children.len() {
                    remaining_width
                } else if matches!(child, CompactNode::Progress { .. }) {
                    26.0_f32.min(remaining_width)
                } else {
                    remaining_width / (children.len() - index) as f32
                };
                let child_rect = Rect::from_min_size(
                    Pos2::new(x, rect.top()),
                    Vec2::new(child_width, rect.height()),
                );
                render_child(ui, context, mascot, icons, child_rect, child, opacity);
                x += child_width;
                remaining_width -= child_width;
            }
        }
        CompactNode::Column(children) => {
            let row_height = rect.height() / children.len().max(1) as f32;
            for (index, child) in children.iter().enumerate() {
                let child_rect = Rect::from_min_size(
                    Pos2::new(rect.left(), rect.top() + row_height * index as f32),
                    Vec2::new(rect.width(), row_height),
                );
                render_child(ui, context, mascot, icons, child_rect, child, opacity);
            }
        }
    }
}

fn render_child(
    ui: &egui::Ui,
    context: &egui::Context,
    mascot: &mut Mascot,
    icons: &Icons,
    rect: Rect,
    node: &CompactNode,
    opacity: f32,
) {
    let view = CompactView {
        content: node.clone(),
        min_width: rect.width(),
        preferred_width: rect.width(),
        priority: 0,
        tooltip: None,
    };
    render_node(ui, context, mascot, icons, rect, &view, opacity);
}

fn paint_icon(
    ui: &egui::Ui,
    icons: &Icons,
    rect: Rect,
    name: &str,
    color: ColorRole,
    opacity: f32,
) {
    icons.paint(
        ui.painter(),
        icon_id(name),
        Rect::from_center_size(rect.center(), Vec2::splat(COMPACT_ICON_SIZE)),
        role_color(color),
        opacity,
    );
}

fn icon_id(name: &str) -> Icon {
    match name {
        "battery" => Icon::Battery,
        "battery-charging" => Icon::BatteryCharging,
        "cpu" => Icon::Cpu,
        "gpu" => Icon::Gpu,
        "arrow-down" => Icon::ArrowDown,
        "arrow-up" => Icon::ArrowUp,
        "clock" => Icon::Clock,
        _ => Icon::Close,
    }
}

fn role_color(role: ColorRole) -> Color32 {
    match role {
        ColorRole::Primary => Color32::from_rgb(247, 249, 252),
        ColorRole::Muted => Color32::from_rgb(177, 183, 194),
        ColorRole::Accent => Color32::from_rgb(80, 190, 255),
        ColorRole::Success => Color32::from_rgb(35, 255, 172),
        ColorRole::Warning => Color32::from_rgb(255, 91, 91),
    }
}

fn paint_text(ui: &egui::Ui, position: Pos2, text: &str, color: Color32, opacity: f32, size: f32) {
    ui.painter().text(
        position,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        color.linear_multiply(opacity),
    );
}

fn paint_progress(ui: &egui::Ui, rect: Rect, fraction: f32, label: &str, opacity: f32) {
    let center = rect.center();
    let radius = (rect.height().min(rect.width()) * 0.5 - 2.0).max(2.0);
    ui.painter().circle_stroke(
        center,
        radius,
        Stroke::new(
            2.0_f32,
            Color32::from_rgb(61, 70, 84).linear_multiply(opacity),
        ),
    );
    if fraction > 0.0 {
        let segments = (fraction.clamp(0.0, 1.0) * 48.0).ceil().max(1.0) as usize;
        let points: Vec<_> = (0..=segments)
            .map(|index| {
                let angle = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * fraction.clamp(0.0, 1.0) * index as f32
                        / segments as f32;
                center + Vec2::new(angle.cos(), angle.sin()) * radius
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            points,
            Stroke::new(2.0_f32, DOWN.linear_multiply(opacity)),
        ));
    }
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(5.8),
        Color32::WHITE.linear_multiply(opacity),
    );
}

fn render_sprite(
    ui: &egui::Ui,
    context: &egui::Context,
    mascot: &mut Mascot,
    x: f32,
    center_y: f32,
    size: [f32; 2],
    opacity: f32,
) {
    let face = Rect::from_center_size(
        Pos2::new(x + size[0] * 0.5, center_y),
        Vec2::new(size[0], size[1]),
    );
    let response = ui.interact(face, egui::Id::new("island-mascot"), Sense::click());
    mascot.paint(context, ui.painter(), face, opacity);
    if response.clicked() {
        mascot.advance(0.0, true);
        context.request_repaint();
    }
}
