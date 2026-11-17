use super::{icons::Icons, minimum_widget_size};
use crate::{
    extension::{ActionInvocation, ExtensionId, WidgetView},
    ui::{
        declarative::{Overflow, ScrollAxes},
        renderer,
    },
};
use eframe::egui::{self, pos2, vec2, Align2, FontId, Rect, Sense, Stroke, Vec2};
use std::sync::Arc;

const GAP: f32 = 12.0;
const PAD: f32 = 12.0;
const HEADER: f32 = 38.0;
const FOOTER: f32 = 10.0;

pub(super) struct Card {
    pub id: ExtensionId,
    pub title: String,
    pub view: Arc<WidgetView>,
}

#[derive(Default)]
pub(super) struct Outcome {
    pub close: bool,
    pub actions: Vec<ActionInvocation>,
}

pub(super) fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("peninsula-dashboard")
}

pub(super) fn show(
    context: &egui::Context,
    icons: &Icons,
    cards: &[Card],
    screen: Vec2,
) -> Outcome {
    let initial_size = vec2(screen.x * 0.92, screen.y * 0.86);
    let builder = egui::ViewportBuilder::default()
        .with_title("Peninsula 仪表盘")
        .with_inner_size(initial_size);
    context.show_viewport_immediate(viewport_id(), builder, |context, class| {
        let mut result = Outcome {
            close: context.input(|input| {
                (class != egui::ViewportClass::Embedded && input.viewport().close_requested())
                    || input.key_pressed(egui::Key::Escape)
            }),
            ..Outcome::default()
        };
        if result.close {
            return result;
        }
        if class == egui::ViewportClass::Embedded {
            let mut open = true;
            egui::Window::new("Peninsula 仪表盘")
                .open(&mut open)
                .collapsible(false)
                .default_size(context.screen_rect().size() * 0.90)
                .show(context, |ui| {
                    result.actions = show_cards(ui, icons, cards, minimum_widget_size(screen))
                });
            result.close = !open;
        } else {
            let monitor = context
                .input(|input| input.viewport().monitor_size)
                .unwrap_or(screen);
            egui::CentralPanel::default()
                .frame(egui::Frame::central_panel(&context.style()).inner_margin(PAD))
                .show(context, |ui| {
                    result.actions = show_cards(ui, icons, cards, minimum_widget_size(monitor));
                });
        }
        result
    })
}

fn column_count(width: f32, min_width: f32) -> usize {
    (((width + GAP) / (min_width + GAP)).floor() as usize).clamp(1, 3)
}

fn show_cards(
    ui: &mut egui::Ui,
    icons: &Icons,
    cards: &[Card],
    minimum: Vec2,
) -> Vec<ActionInvocation> {
    let columns = column_count(ui.available_width(), minimum.x);
    let rows = cards.len().div_ceil(columns).max(1);
    let row_limit =
        ((ui.available_height() - GAP * (rows - 1) as f32) / rows as f32).max(minimum.y);
    let mut actions = Vec::new();
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(GAP);
            for row in cards.chunks(columns) {
                let width =
                    (ui.available_width() - GAP * (row.len() - 1) as f32) / row.len() as f32;
                let content_width = (width - 2.0 * PAD).max(1.0);
                let heights: Vec<_> = row
                    .iter()
                    .map(|card| {
                        renderer::content_height(ui.ctx(), &card.view.root, content_width, icons)
                            .max(1.0)
                    })
                    .collect();
                let largest = heights.iter().copied().fold(0.0, f32::max);
                let height = (largest + HEADER + FOOTER).clamp(minimum.y, row_limit);
                ui.horizontal_top(|ui| {
                    for (card, content_height) in row.iter().zip(heights) {
                        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                        if let Some(action) = paint_card(ui, rect, card, icons, content_height) {
                            actions.push(action);
                        }
                    }
                });
            }
        });
    actions
}

fn paint_card(
    ui: &mut egui::Ui,
    rect: Rect,
    card: &Card,
    icons: &Icons,
    content_height: f32,
) -> Option<ActionInvocation> {
    let visuals = ui.visuals();
    let painter = ui.painter();
    painter.rect(
        rect,
        10.0,
        visuals.widgets.noninteractive.bg_fill,
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pos2(rect.left() + PAD, rect.top() + 19.0),
        Align2::LEFT_CENTER,
        &card.title,
        FontId::proportional(14.0),
        visuals.text_color(),
    );
    let available_height = (rect.height() - HEADER - FOOTER).max(0.0);
    let height = content_height.min(available_height);
    let body = Rect::from_min_size(
        pos2(
            rect.left() + PAD,
            rect.top() + HEADER + (available_height - height) * 0.5,
        ),
        vec2((rect.width() - 2.0 * PAD).max(0.0), height),
    );
    if !body.is_positive() {
        return None;
    }
    let overflow = if content_height > available_height + 0.5 {
        match card.view.bounds.overflow {
            Overflow::Scroll(ScrollAxes::Horizontal | ScrollAxes::Both) => {
                Overflow::Scroll(ScrollAxes::Both)
            }
            _ => Overflow::Scroll(ScrollAxes::Vertical),
        }
    } else {
        card.view.bounds.overflow
    };
    renderer::show_widget_with_overflow(ui, body, &card.id, &card.view, icons, 1.0, overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_preserves_the_screen_relative_tile_width_when_choosing_columns() {
        assert_eq!(column_count(1280.0, 360.0), 3);
        assert_eq!(column_count(800.0, 360.0), 2);
        assert_eq!(column_count(670.0, 360.0), 1);
    }
}
