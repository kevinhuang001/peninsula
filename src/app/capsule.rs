use crate::{app::settings::CompactPreferences, metrics::Snapshot};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

mod compact;

use super::{
    mascot::Mascot,
    widgets::{paint_metrics, pomodoro, IslandTab},
};

const CAPSULE_HEIGHT: f32 = 52.0;
const BLACK: Color32 = Color32::from_rgb(15, 16, 19);
const MUTED: Color32 = Color32::from_rgb(139, 145, 157);

pub(super) struct CapsuleWidgets<'a> {
    pub metrics: &'a Snapshot,
    pub mascot: &'a mut Mascot,
    pub preferences: &'a CompactPreferences,
    pub active_tab: &'a mut IslandTab,
    pub timer: &'a mut pomodoro::Pomodoro,
    pub indicator_opacity: f32,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    context: &egui::Context,
    width: f32,
    progress: f32,
    expanded_height: f32,
    content: CapsuleWidgets<'_>,
) -> (Rect, bool, bool) {
    let height = CAPSULE_HEIGHT + expanded_height * progress;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let capsule = ui.interact(
        rect,
        egui::Id::new("island-capsule"),
        Sense::click_and_drag(),
    );
    let drag_area = drag_handle_bounds(rect, progress);
    let handle = ui.interact(
        drag_area,
        egui::Id::new("island-drag-handle"),
        Sense::drag(),
    );
    let drag_started = handle.drag_started();
    if progress > 0.9 && swipe_finished(context, &capsule) {
        *content.active_tab = content.active_tab.other();
        context.request_repaint();
    }
    paint_shell(ui, rect, progress);
    let settings = if progress < 0.9 {
        compact::show(
            ui,
            context,
            rect,
            content.metrics,
            content.mascot,
            progress,
            content.preferences,
        )
    } else {
        paint_expanded(
            ui,
            rect,
            content.metrics,
            progress,
            content.active_tab,
            content.timer,
            content.indicator_opacity,
        )
    };
    capsule.context_menu(|ui| {
        if ui.button("退出灵动岛").clicked() {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
    (rect, settings, drag_started)
}

fn swipe_finished(context: &egui::Context, response: &egui::Response) -> bool {
    let id = egui::Id::new("island-swipe-origin");
    if response.drag_started() {
        if let Some(origin) = context.input(|input| input.pointer.press_origin()) {
            context.data_mut(|data| data.insert_temp(id, origin));
        }
    }
    if response.drag_stopped() {
        let origin = context.data_mut(|data| data.remove_temp::<Pos2>(id));
        let end = context.input(|input| input.pointer.interact_pos());
        return origin.zip(end).is_some_and(|(start, end)| {
            let delta = end - start;
            delta.x.abs() > 35.0 && delta.x.abs() > delta.y.abs()
        });
    }
    false
}

#[cfg(test)]
mod drag_tests;

fn drag_handle_bounds(rect: Rect, progress: f32) -> Rect {
    if progress >= 0.9 {
        Rect::from_min_max(rect.min, Pos2::new(rect.right() - 48.0, rect.top() + 43.0))
    } else {
        rect
    }
}

fn paint_shell(ui: &egui::Ui, rect: Rect, progress: f32) {
    let painter = ui.painter();
    let corner_radius = corner_radius(progress);
    painter.rect_filled(rect, corner_radius, BLACK);
    painter.rect_stroke(
        rect,
        corner_radius,
        Stroke::new(1.0, Color32::from_rgb(44, 47, 54)),
        egui::StrokeKind::Inside,
    );
}

fn corner_radius(progress: f32) -> f32 {
    26.0 + 8.0 * progress.clamp(0.0, 1.0)
}

fn paint_expanded(
    ui: &mut egui::Ui,
    rect: Rect,
    metrics: &Snapshot,
    progress: f32,
    active_tab: &mut IslandTab,
    timer: &mut pomodoro::Pomodoro,
    indicator_opacity: f32,
) -> bool {
    let reveal = progress.clamp(0.0, 1.0);
    let title = if *active_tab == IslandTab::Performance {
        "性能监控"
    } else {
        "番茄钟"
    };
    ui.painter().text(
        Pos2::new(rect.left() + 22.0, rect.top() + 27.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        MUTED.linear_multiply(reveal),
    );
    let gear_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 25.0, rect.top() + 26.0),
        Vec2::splat(28.0),
    );
    let settings = ui
        .put(gear_rect, egui::Button::new("⚙").frame(false))
        .clicked();
    let body = Rect::from_min_max(
        Pos2::new(rect.left() + 10.0, rect.top() + 48.0),
        Pos2::new(rect.right() - 10.0, rect.bottom() - 15.0),
    );
    if *active_tab == IslandTab::Performance {
        let metrics_rect = paint_metrics(ui, body, metrics, reveal);
        let _ = metrics_rect;
    } else {
        pomodoro::show(ui, body, timer);
    }
    if indicator_opacity > 0.0 {
        paint_tabs(ui, rect, active_tab, indicator_opacity);
    }
    settings
}

fn paint_tabs(ui: &mut egui::Ui, rect: Rect, active_tab: &mut IslandTab, opacity: f32) {
    let center = Pos2::new(rect.center().x, rect.top() + 13.0);
    for (index, tab) in IslandTab::ALL.into_iter().enumerate() {
        let point = Pos2::new(center.x + (index as f32 - 0.5) * 16.0, center.y);
        let size = if *active_tab == tab {
            Vec2::new(12.0, 5.0)
        } else {
            Vec2::splat(5.0)
        };
        let dot = Rect::from_center_size(point, size);
        ui.painter().rect_filled(
            dot,
            3.0,
            if *active_tab == tab {
                Color32::WHITE
            } else {
                MUTED
            }
            .linear_multiply(opacity),
        );
        if ui
            .interact(
                dot.expand(5.0),
                egui::Id::new(("tab", index)),
                Sense::click(),
            )
            .clicked()
        {
            *active_tab = tab;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{corner_radius, drag_handle_bounds, show, CapsuleWidgets};
    use crate::{
        app::{
            mascot::Mascot,
            settings::CompactPreferences,
            widgets::{pomodoro::Pomodoro, IslandTab},
        },
        metrics::Snapshot,
    };
    use eframe::egui::{self, Pos2, Rect, Vec2};

    #[test]
    fn capsule_paint_and_hit_bounds_follow_the_animated_width_and_height() {
        let context = egui::Context::default();
        let mut rect = Rect::NOTHING;
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 204.0))),
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(context, |ui| {
                        ui.vertical_centered(|ui| {
                            let mut mascot = Mascot::default();
                            let mut tab = IslandTab::default();
                            let mut timer = Pomodoro::default();
                            rect = show(
                                ui,
                                context,
                                220.0,
                                0.5,
                                126.0,
                                CapsuleWidgets {
                                    metrics: &Snapshot::default(),
                                    mascot: &mut mascot,
                                    preferences: &CompactPreferences::default(),
                                    active_tab: &mut tab,
                                    timer: &mut timer,
                                    indicator_opacity: 1.0,
                                },
                            )
                            .0;
                        });
                    });
            },
        );
        assert_eq!(rect.width(), 220.0);
        assert_eq!(rect.height(), 115.0);
        assert!((rect.center().x - 180.0).abs() < 1.0);
    }

    #[test]
    fn expanded_capsule_keeps_rounded_rectangle_corners_instead_of_becoming_an_oval() {
        assert_eq!(corner_radius(0.0), 26.0);
        assert_eq!(corner_radius(1.0), 34.0);
        assert!(corner_radius(1.0) < 52.0);
    }

    #[test]
    fn expanded_drag_handle_excludes_settings_and_widget_contents() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 178.0));
        let handle = drag_handle_bounds(rect, 1.0);
        assert!(handle.contains(Pos2::new(100.0, 20.0)));
        assert!(!handle.contains(Pos2::new(295.0, 26.0)));
        assert!(!handle.contains(Pos2::new(160.0, 90.0)));
        assert_eq!(drag_handle_bounds(rect, 0.0), rect);
    }
}
