use crate::{app::settings::CompactPreferences, metrics::Snapshot};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

mod compact;

use super::{
    mascot::Mascot,
    widgets::{paint_metrics, pomodoro, IslandTab},
};

const CAPSULE_HEIGHT: f32 = 52.0;
const EXPANDED_HEIGHT: f32 = 126.0;
const BLACK: Color32 = Color32::from_rgb(15, 16, 19);
const MUTED: Color32 = Color32::from_rgb(139, 145, 157);

pub(super) struct CapsuleWidgets<'a> {
    pub metrics: &'a Snapshot,
    pub mascot: &'a mut Mascot,
    pub preferences: &'a CompactPreferences,
    pub active_tab: &'a mut IslandTab,
    pub timer: &'a mut pomodoro::Pomodoro,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    context: &egui::Context,
    width: f32,
    progress: f32,
    content: CapsuleWidgets<'_>,
) -> (Rect, bool) {
    let height = CAPSULE_HEIGHT + EXPANDED_HEIGHT * progress;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let capsule = ui.interact(
        rect,
        egui::Id::new("island-capsule"),
        Sense::click_and_drag(),
    );
    if progress > 0.9 && capsule.drag_stopped() && capsule.drag_delta().x.abs() > 35.0 {
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
        )
    };
    capsule.context_menu(|ui| {
        if ui.button("退出灵动岛").clicked() {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
    (rect, settings)
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
    paint_tabs(ui, rect, active_tab);
    settings
}

fn paint_tabs(ui: &mut egui::Ui, rect: Rect, active_tab: &mut IslandTab) {
    let center = Pos2::new(rect.center().x, rect.bottom() - 8.0);
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
            },
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
    use super::{corner_radius, show, CapsuleWidgets};
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
                                CapsuleWidgets {
                                    metrics: &Snapshot::default(),
                                    mascot: &mut mascot,
                                    preferences: &CompactPreferences::default(),
                                    active_tab: &mut tab,
                                    timer: &mut timer,
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
}
