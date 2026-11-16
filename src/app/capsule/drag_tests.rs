use super::{show, CapsuleWidgets};
use crate::app::{
    mascot::Mascot,
    settings::{CompactPreferences, SettingsView},
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
    let mut preferences = CompactPreferences::default();
    let mut settings_view = SettingsView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let mut frame = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 214.0))),
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
                                preferences: &mut preferences,
                                settings_open: false,
                                settings_view: &mut settings_view,
                                active_tab: &mut tab,
                                timer: &mut timer,
                                pinned: false,
                                icons: &icons,
                                download: None,
                                notification: None,
                                do_not_disturb: false,
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
fn bottom_pagination_dots_are_always_passive() {
    let ctx = egui::Context::default();
    let mut tab = IslandTab::Performance;
    let mut mascot = Mascot::default();
    let mut timer = Pomodoro::default();
    let mut preferences = CompactPreferences::default();
    let mut settings_view = SettingsView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let mut frame = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 214.0))),
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
                                preferences: &mut preferences,
                                settings_open: false,
                                settings_view: &mut settings_view,
                                active_tab: &mut tab,
                                timer: &mut timer,
                                pinned: false,
                                icons: &icons,
                                download: None,
                                notification: None,
                                do_not_disturb: false,
                            },
                        );
                    });
                });
        });
    };
    let point = pos2(218.0, 122.0);
    let button = |pressed| Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };
    frame(vec![Event::PointerMoved(point)]);
    frame(vec![button(true)]);
    frame(vec![button(false)]);
    assert_eq!(tab, IslandTab::Performance);
}

#[test]
fn expanded_pin_button_emits_toggle_without_starting_a_drag() {
    let ctx = egui::Context::default();
    let mut tab = IslandTab::Performance;
    let mut mascot = Mascot::default();
    let mut timer = Pomodoro::default();
    let mut preferences = CompactPreferences::default();
    let mut settings_view = SettingsView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let point = pos2(294.0, 30.0);
    let mut toggled = false;
    let mut frame = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 214.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        let result = show(
                            ui,
                            ctx,
                            320.0,
                            1.0,
                            76.0,
                            CapsuleWidgets {
                                metrics: &Snapshot::default(),
                                mascot: &mut mascot,
                                preferences: &mut preferences,
                                settings_open: false,
                                settings_view: &mut settings_view,
                                active_tab: &mut tab,
                                timer: &mut timer,
                                pinned: false,
                                icons: &icons,
                                download: None,
                                notification: None,
                                do_not_disturb: false,
                            },
                        );
                        toggled = result.4;
                    });
                });
        });
    };
    let button = |pressed| Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };
    frame(vec![Event::PointerMoved(point)]);
    frame(vec![button(true)]);
    frame(vec![button(false)]);
    assert!(toggled);
}

#[test]
fn moon_button_emits_quiet_mode_toggle_without_changing_the_widget() {
    let ctx = egui::Context::default();
    let mut tab = IslandTab::Performance;
    let mut mascot = Mascot::default();
    let mut timer = Pomodoro::default();
    let mut preferences = CompactPreferences::default();
    let mut settings_view = SettingsView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let point = pos2(323.0, 30.0);
    let mut toggled = false;
    let mut frame = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 214.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        let result = show(
                            ui,
                            ctx,
                            320.0,
                            1.0,
                            76.0,
                            CapsuleWidgets {
                                metrics: &Snapshot::default(),
                                mascot: &mut mascot,
                                preferences: &mut preferences,
                                settings_open: false,
                                settings_view: &mut settings_view,
                                active_tab: &mut tab,
                                timer: &mut timer,
                                pinned: false,
                                icons: &icons,
                                download: None,
                                notification: None,
                                do_not_disturb: false,
                            },
                        );
                        toggled = result.5;
                    });
                });
        });
    };
    let button = |pressed| Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };
    frame(vec![Event::PointerMoved(point)]);
    frame(vec![button(true)]);
    frame(vec![button(false)]);
    assert!(toggled);
    assert_eq!(tab, IslandTab::Performance);
}
