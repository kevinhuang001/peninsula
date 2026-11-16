use super::{show, CapsuleWidgets};
use crate::app::{
    mascot::Mascot,
    settings::CompactPreferences,
    widgets::{pomodoro::Pomodoro, IslandTab},
};
use crate::metrics::Snapshot;
use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect};

#[test]
fn horizontal_drag_changes_widget_once_on_release() {
    let ctx = egui::Context::default();
    let mut tab = IslandTab::Performance;
    let mut mascot = Mascot::default();
    let mut timer = Pomodoro::default();
    let mut frame = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 190.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        show(
                            ui,
                            ctx,
                            320.0,
                            1.0,
                            76.0,
                            CapsuleWidgets {
                                metrics: &Snapshot::default(),
                                mascot: &mut mascot,
                                preferences: &CompactPreferences::default(),
                                active_tab: &mut tab,
                                timer: &mut timer,
                                indicator_opacity: 1.0,
                            },
                        );
                    });
                });
        });
    };
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };
    frame(vec![Event::PointerMoved(pos2(100.0, 70.0))]);
    frame(vec![button(pos2(100.0, 70.0), true)]);
    frame(vec![Event::PointerMoved(pos2(120.0, 70.0))]);
    frame(vec![Event::PointerMoved(pos2(150.0, 70.0))]);
    frame(vec![button(pos2(150.0, 70.0), false)]);
    assert_eq!(tab, IslandTab::Pomodoro);
}

#[test]
fn top_pagination_dot_changes_widget_when_clicked() {
    let ctx = egui::Context::default();
    let mut tab = IslandTab::Performance;
    let mut mascot = Mascot::default();
    let mut timer = Pomodoro::default();
    let mut frame = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 190.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        show(
                            ui,
                            ctx,
                            320.0,
                            1.0,
                            76.0,
                            CapsuleWidgets {
                                metrics: &Snapshot::default(),
                                mascot: &mut mascot,
                                preferences: &CompactPreferences::default(),
                                active_tab: &mut tab,
                                timer: &mut timer,
                                indicator_opacity: 1.0,
                            },
                        );
                    });
                });
        });
    };
    let point = pos2(218.0, 13.0);
    let button = |pressed| Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };
    frame(vec![Event::PointerMoved(point)]);
    frame(vec![button(true)]);
    frame(vec![button(false)]);
    assert_eq!(tab, IslandTab::Pomodoro);
}
