use super::{capsule_height, corner_radius, drag_handle_bounds, show, CapsuleWidgets};
use crate::app::settings::{CompactPreferences, SettingsView};
use eframe::egui::{self, Pos2, Rect, Vec2};

#[test]
fn capsule_paint_and_hit_bounds_follow_the_animated_width_and_height() {
    let context = egui::Context::default();
    let mut rect = Rect::NOTHING;
    let _ = context.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 214.0))),
            ..Default::default()
        },
        |context| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(context, |ui| {
                    ui.vertical_centered(|ui| {
                        let mut tab = crate::app::widgets::default_widget_id();
                        let mut preferences = CompactPreferences::default();
                        let mut settings_view = SettingsView::default();
                        let icons = crate::app::icons::Icons::load(context);
                        let (widget_ids, widget_view) = widget_fixture();
                        rect = show(
                            ui,
                            context,
                            220.0,
                            0.5,
                            126.0,
                            CapsuleWidgets {
                                compact_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: false,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
                                notification: None,
                                do_not_disturb: false,
                                bilibili_status: "",
                            },
                        )
                        .0;
                    });
                });
        },
    );
    assert_eq!(rect.width(), 220.0);
    assert_eq!(rect.height(), 120.0);
    assert!((rect.center().x - 180.0).abs() < 1.0);
}

#[test]
fn compact_capsule_is_shorter_without_changing_the_expanded_widget_height() {
    assert_eq!(capsule_height(0.0, 76.0), 44.0);
    assert_eq!(capsule_height(1.0, 76.0), 124.0);
}

#[test]
fn in_capsule_settings_toggle_updates_compact_preferences() {
    use eframe::egui::{Event, Modifiers, PointerButton};

    let context = egui::Context::default();
    let mut tab = crate::app::widgets::default_widget_id();
    let mut preferences = CompactPreferences::default();
    let mut settings_view = SettingsView::default();
    let icons = crate::app::icons::Icons::load(&context);
    let (widget_ids, widget_view) = widget_fixture();
    let point = Pos2::new(369.0, 105.0);
    let mut changed = false;
    let mut frame = |events: Vec<Event>| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(420.0, 340.0))),
            events,
            ..Default::default()
        };
        let _ = context.run(input, |context| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(context, |ui| {
                    ui.vertical_centered(|ui| {
                        let result = show(
                            ui,
                            context,
                            SettingsView::WIDTH,
                            1.0,
                            settings_view.extra_height(context, &preferences, ""),
                            CapsuleWidgets {
                                compact_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: true,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
                                notification: None,
                                do_not_disturb: false,
                                bilibili_status: "",
                            },
                        );
                        changed = result.6;
                    });
                });
        });
    };
    let button = |pressed| Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(vec![Event::PointerMoved(point)]);
    frame(vec![button(true)]);
    frame(vec![button(false)]);
    assert!(
        !preferences.is_enabled(&crate::extension::ExtensionId::builtin(
            "builtin.mascot/face"
        ))
    );
    assert!(changed);
}

#[test]
fn settings_close_button_returns_to_the_active_widget() {
    use eframe::egui::{Event, Modifiers, PointerButton};

    let context = egui::Context::default();
    let mut tab = crate::extension::ExtensionId::builtin("builtin.system-monitor/overview");
    let mut preferences = CompactPreferences::default();
    let mut settings_view = SettingsView::default();
    let icons = crate::app::icons::Icons::load(&context);
    let (widget_ids, widget_view) = widget_fixture();
    let point = Pos2::new(385.0, 26.0);
    let mut close_requested = false;
    let mut frame = |events: Vec<Event>| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(420.0, 340.0))),
            events,
            ..Default::default()
        };
        let _ = context.run(input, |context| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(context, |ui| {
                    ui.vertical_centered(|ui| {
                        close_requested = show(
                            ui,
                            context,
                            SettingsView::WIDTH,
                            1.0,
                            settings_view.extra_height(context, &preferences, ""),
                            CapsuleWidgets {
                                compact_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: true,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
                                notification: None,
                                do_not_disturb: false,
                                bilibili_status: "",
                            },
                        )
                        .1;
                    });
                });
        });
    };
    let button = |pressed| Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(vec![Event::PointerMoved(point)]);
    frame(vec![button(true)]);
    frame(vec![button(false)]);
    assert!(close_requested);
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
    assert!(!handle.contains(Pos2::new(260.0, 26.0)));
    assert!(!handle.contains(Pos2::new(295.0, 26.0)));
    assert!(!handle.contains(Pos2::new(160.0, 90.0)));
    assert_eq!(drag_handle_bounds(rect, 0.0), rect);
}

fn widget_fixture() -> (
    Vec<crate::extension::ExtensionId>,
    crate::extension::WidgetView,
) {
    let widget_ids = crate::builtins::registry()
        .widget_list()
        .into_iter()
        .map(|descriptor| descriptor.id.clone())
        .collect();
    let widget_view = crate::extension::WidgetView::new(
        crate::extension::ViewNode::text("测试", Default::default()),
        [300.0, 76.0],
    );

    (widget_ids, widget_view)
}
