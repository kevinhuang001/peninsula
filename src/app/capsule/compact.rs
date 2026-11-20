use crate::{
    app::{
        compact_layout::{fitted_widths, CompactItem, CHIP_HEIGHT, ITEM_GAP},
        icons::Icons,
        settings::CompactPreferences,
    },
    extension::{ActionInvocation, Prompt},
    ui::renderer,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};

use super::draw_compact_chip;

#[derive(Clone)]
struct PromptEntrance {
    id: crate::extension::ExtensionId,
    content: crate::extension::ViewNode,
    started: f64,
}

const PROMPT_ENTRANCE_ID: &str = "capsule-prompt-entrance";

pub(super) struct CompactContent<'a> {
    pub(super) prefs: &'a CompactPreferences,
    pub(super) views: &'a [CompactItem<'a>],
    pub(super) icons: &'a Icons,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    rect: Rect,
    progress: f32,
    content: CompactContent<'_>,
) -> Option<ActionInvocation> {
    let CompactContent {
        prefs,
        views: available_views,
        icons,
    } = content;
    let opacity = 1.0 - progress.clamp(0.0, 1.0);
    if let Some(prompt) = available_views.iter().find_map(|item| item.prompt.as_ref()) {
        return show_prompt(ui, rect, opacity, prompt, icons, prefs);
    }
    ui.ctx().data_mut(|data| {
        data.remove::<PromptEntrance>(egui::Id::new(PROMPT_ENTRANCE_ID));
    });
    let mut views: Vec<_> = if available_views.iter().any(|item| item.exclusive) {
        available_views
            .iter()
            .filter(|item| item.exclusive)
            .collect()
    } else {
        prefs
            .ordered_compact_items()
            .into_iter()
            .filter(|item| prefs.is_enabled(&item.id))
            .filter_map(|item| available_views.iter().find(|view| view.view.id == item.id))
            .collect()
    };
    if let Some(slot) = available_views
        .iter()
        .find(|v| prefs.mascot_enabled && v.view.id.as_str() == crate::core::mascot::MASCOT_SLOT_ID)
    {
        views.insert(0, slot);
    }
    let widths = fitted_widths(&views, rect.width());
    let total = widths.iter().sum::<f32>() + ITEM_GAP * views.len().saturating_sub(1) as f32;
    let mut x = rect.center().x - total * 0.5;
    let center_y = rect.center().y;

    let mut action = None;
    for (item, width) in views.into_iter().zip(widths) {
        let view = item.view;
        let item_rect = Rect::from_center_size(
            Pos2::new(x + width * 0.5, center_y),
            Vec2::new(width, CHIP_HEIGHT),
        );
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
        x += width + ITEM_GAP;
    }
    action
}

fn show_prompt(
    ui: &mut egui::Ui,
    rect: Rect,
    opacity: f32,
    prompt: &Prompt,
    icons: &Icons,
    prefs: &CompactPreferences,
) -> Option<ActionInvocation> {
    if opacity <= 0.0 {
        return None;
    }
    let reduced = prefs.reduced_motion || prefs.motion_off;
    let now = ui.input(|i| i.time);
    let started = ui.ctx().data_mut(|data| {
        let key = egui::Id::new(PROMPT_ENTRANCE_ID);
        if let Some(entrance) = data.get_temp::<PromptEntrance>(key) {
            if entrance.id == prompt.contribution_id && entrance.content == prompt.content {
                return entrance.started;
            }
        }
        data.insert_temp(
            key,
            PromptEntrance {
                id: prompt.contribution_id.clone(),
                content: prompt.content.clone(),
                started: now,
            },
        );
        now
    });
    let entrance = if reduced {
        1.0
    } else {
        ((now - started) as f32 / 0.38).clamp(0.0, 1.0)
    };
    let entrance = 1.0 - (1.0 - entrance).powi(3);
    let elapsed = (now - started) as f32;
    if !reduced && elapsed < 1.2 {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
    let accent = prompt
        .accent
        .map_or(ui.visuals().selection.bg_fill, |[r, g, b]| {
            Color32::from_rgb(r, g, b)
        });
    // A brief halo stays behind the leading icon and settles to transparent.
    // Keep the capsule silhouette quiet; the event content owns the animation.
    if !reduced && elapsed < 1.2 {
        let reveal = (elapsed / 0.2).clamp(0.0, 1.0);
        let fade = (1.0 - elapsed / 1.2).clamp(0.0, 1.0);
        let center = Pos2::new(rect.left() + rect.width() * 0.15, rect.center().y);
        let painter = ui.painter().with_clip_rect(rect.shrink(3.0));
        for ring in (1..=8).rev() {
            painter.circle_filled(
                center,
                rect.height() * (0.12 + ring as f32 * 0.026),
                accent.linear_multiply(opacity * reveal * fade * 0.025),
            );
        }
    }
    let mut action = None;
    if prompt.layers.is_empty() {
        action = renderer::show_prompt_layer(
            ui,
            rect.shrink2(egui::vec2(8.0, 0.0)),
            rect,
            &prompt.contribution_id,
            &prompt.content,
            icons,
            opacity * entrance,
        );
    } else {
        for (index, layer) in prompt.layers.iter().enumerate() {
            let [left, top, width, height] = layer.frame.map(|value| {
                if value.is_finite() {
                    value.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            });
            let layer_rect = Rect::from_min_size(
                rect.min + egui::vec2(rect.width() * left, rect.height() * top),
                egui::vec2(
                    rect.width() * width.min(1.0 - left),
                    rect.height() * height.min(1.0 - top),
                ),
            );
            let mut surface = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("prompt-layer", index))
                    .max_rect(layer_rect),
            );
            surface.set_clip_rect(ui.clip_rect().intersect(rect));
            if let Some(clicked) = renderer::show_prompt_layer(
                &mut surface,
                layer_rect,
                rect,
                &prompt.contribution_id,
                &layer.content,
                icons,
                opacity * entrance,
            ) {
                action = Some(clicked);
            }
        }
    }
    action.map(|action| ActionInvocation {
        source_extension_id: prompt.source_extension_id.clone(),
        action,
    })
}
