use crate::{
    app::{
        capsule_layout::{fitted_widths, CapsuleItem, CHIP_HEIGHT, ITEM_GAP},
        icons::Icons,
        settings::AppPreferences,
        unlock::{self, UnlockHandoff},
    },
    extension::{ActionInvocation, Prompt},
    ui::renderer,
};
use eframe::egui::{self, Pos2, Rect, Sense, Vec2};

use super::draw_capsule_chip;

#[derive(Clone)]
struct PromptEntrance {
    id: crate::extension::ExtensionId,
    content: crate::extension::ViewNode,
    started: f64,
}

const PROMPT_ENTRANCE_ID: &str = "capsule-prompt-entrance";

pub(super) struct CapsuleContent<'a> {
    pub(super) prefs: &'a AppPreferences,
    pub(super) views: &'a [CapsuleItem<'a>],
    pub(super) icons: &'a Icons,
    pub(super) handoff: Option<UnlockHandoff>,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    rect: Rect,
    progress: f32,
    content: CapsuleContent<'_>,
) -> Option<ActionInvocation> {
    let CapsuleContent {
        prefs,
        views: available_views,
        icons,
        handoff,
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
            .ordered_capsule_items()
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

    let appearance = renderer::CapsuleTextAppearance {
        size: prefs.capsule_text_size,
        color: prefs
            .capsule_text_color
            .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b)),
    };
    let mut action = None;
    for (item, width) in views.into_iter().zip(widths) {
        let view = item.view;
        let item_rect = Rect::from_center_size(
            Pos2::new(x + width * 0.5, center_y),
            Vec2::new(width, CHIP_HEIGHT),
        );
        let mascot = view.id.as_str() == crate::core::mascot::MASCOT_SLOT_ID;
        let item_opacity = if mascot {
            opacity
        } else {
            opacity * handoff.map_or(1.0, UnlockHandoff::items_opacity)
        };
        if item_opacity <= 0.0 {
            x += width + ITEM_GAP;
            continue;
        }
        if !mascot {
            draw_capsule_chip(ui, item_rect, item_opacity);
        }
        if let Some(tooltip) = view.tooltip.as_ref().filter(|_| handoff.is_none()) {
            ui.interact(
                item_rect,
                egui::Id::new(("capsule-item", &view.id)),
                Sense::hover(),
            )
            .on_hover_text(tooltip);
        }
        let content_rect = item_rect.shrink2(egui::vec2(3.0, 0.0));
        let content_rect = if let Some(handoff) = handoff.filter(|_| mascot) {
            let start = unlock::mascot_rect(Rect::from_center_size(
                rect.center(),
                Vec2::new(unlock::WIDTH, rect.height()),
            ));
            Rect::from_min_max(
                start.min.lerp(content_rect.min, handoff.travel()),
                start.max.lerp(content_rect.max, handoff.travel()),
            )
        } else {
            content_rect
        };
        if let Some(clicked) = renderer::show_capsule_with_appearance(
            ui,
            content_rect,
            &view.id,
            &view.content,
            icons,
            item_opacity,
            appearance,
        ) {
            action = Some(ActionInvocation {
                source_extension_id: view.id.clone(),
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
    prefs: &AppPreferences,
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
