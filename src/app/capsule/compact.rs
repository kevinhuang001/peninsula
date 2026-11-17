use crate::{
    app::{
        compact_layout::{fitted_widths, CHIP_HEIGHT, ITEM_GAP},
        icons::Icons,
        mascot::Mascot,
        settings::CompactPreferences,
    },
    extension::{ActionInvocation, CompactView},
    ui::renderer,
};
use eframe::egui::{self, Pos2, Rect, Sense, Vec2};

use super::draw_compact_chip;

pub(super) struct CompactContent<'a> {
    pub(super) mascot: &'a mut Mascot,
    pub(super) prefs: &'a CompactPreferences,
    pub(super) views: &'a [&'a CompactView],
    pub(super) icons: &'a Icons,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    context: &egui::Context,
    rect: Rect,
    progress: f32,
    content: CompactContent<'_>,
) -> Option<ActionInvocation> {
    let CompactContent {
        mascot,
        prefs,
        views: available_views,
        icons,
    } = content;
    let opacity = 1.0 - progress.clamp(0.0, 1.0);
    let views: Vec<_> = prefs
        .ordered_compact_items()
        .into_iter()
        .filter(|item| prefs.is_enabled(&item.id))
        .filter_map(|item| available_views.iter().find(|view| view.id == item.id))
        .copied()
        .collect();
    let widths = fitted_widths(&views, rect.width());
    let total = widths.iter().sum::<f32>() + ITEM_GAP * views.len().saturating_sub(1) as f32;
    let mut x = rect.center().x - total * 0.5;
    let center_y = rect.center().y;

    let mut action = None;
    for (view, width) in views.into_iter().zip(widths) {
        let item_rect = Rect::from_center_size(
            Pos2::new(x + width * 0.5, center_y),
            Vec2::new(width, CHIP_HEIGHT),
        );
        if view.id.as_str() == "builtin.mascot/face" {
            render_sprite(ui, context, mascot, item_rect, opacity);
        } else {
            draw_compact_chip(ui, item_rect, opacity);
            if let Some(tooltip) = &view.tooltip {
                ui.interact(
                    item_rect,
                    egui::Id::new(("compact-item", &view.id)),
                    Sense::hover(),
                )
                .on_hover_text(tooltip);
            }
            if let Some(clicked) = renderer::show_compact(
                ui,
                item_rect.shrink2(egui::vec2(3.0, 0.0)),
                &view.id,
                &view.content,
                icons,
                opacity,
            ) {
                action = Some(ActionInvocation {
                    source_extension_id: view.source_extension_id.clone(),
                    action: clicked,
                });
            }
        }
        x += width + ITEM_GAP;
    }
    action
}

fn render_sprite(
    ui: &egui::Ui,
    context: &egui::Context,
    mascot: &mut Mascot,
    face: Rect,
    opacity: f32,
) {
    let response = ui.interact(face, egui::Id::new("island-mascot"), Sense::click());
    mascot.paint(context, ui.painter(), face, opacity);
    if response.clicked() {
        mascot.advance(0.0, true);
        context.request_repaint();
    }
}
