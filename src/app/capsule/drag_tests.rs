use super::{show, CapsuleWidgets};
use crate::app::settings::{AppPreferences, SettingsView};
use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect};

#[test]
fn dragging_the_page_right_reveals_the_previous_widget_on_release() {
    let ctx = egui::Context::default();
    let mut tab = crate::extension::ExtensionId::builtin("builtin.system-monitor/overview");
    let mut preferences = AppPreferences::default();
    let mut settings_view = SettingsView::default();
    let datasource_root = tempfile::tempdir().unwrap();
    let mut datasource_manager =
        crate::core::datasource::DataSourceManager::open(datasource_root.path());
    let mut datasource_view = crate::app::datasource::DataSourceView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let (widget_ids, widget_view) = widget_fixture();
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
                                capsule_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: false,
                                datasource_open: false,
                                datasource_size: egui::vec2(790.0, 570.0),
                                datasource_view: &mut datasource_view,
                                datasources: &mut datasource_manager,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
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
    assert_eq!(tab, crate::app::widgets::default_widget_id());
}

#[test]
fn clicking_widget_body_does_not_change_the_page() {
    let ctx = egui::Context::default();
    let mut tab = crate::extension::ExtensionId::builtin("builtin.system-monitor/overview");
    let mut preferences = AppPreferences::default();
    let mut settings_view = SettingsView::default();
    let datasource_root = tempfile::tempdir().unwrap();
    let mut datasource_manager =
        crate::core::datasource::DataSourceManager::open(datasource_root.path());
    let mut datasource_view = crate::app::datasource::DataSourceView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let (widget_ids, widget_view) = widget_fixture();
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
                                capsule_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: false,
                                datasource_open: false,
                                datasource_size: egui::vec2(790.0, 570.0),
                                datasource_view: &mut datasource_view,
                                datasources: &mut datasource_manager,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
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
    assert_eq!(
        tab,
        crate::extension::ExtensionId::builtin("builtin.system-monitor/overview")
    );
}

#[test]
fn dashboard_button_to_the_left_of_pin_acts_independently() {
    let ctx = egui::Context::default();
    let mut tab = crate::extension::ExtensionId::builtin("builtin.system-monitor/overview");
    let mut preferences = AppPreferences::default();
    let mut settings_view = SettingsView::default();
    let datasource_root = tempfile::tempdir().unwrap();
    let mut datasource_manager =
        crate::core::datasource::DataSourceManager::open(datasource_root.path());
    let mut datasource_view = crate::app::datasource::DataSourceView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let (widget_ids, widget_view) = widget_fixture();

    let mut frame = |events: Vec<Event>| {
        let mut capsule = Rect::NOTHING;
        let mut actions = (false, false, false);
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
                                capsule_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: false,
                                datasource_open: false,
                                datasource_size: egui::vec2(790.0, 570.0),
                                datasource_view: &mut datasource_view,
                                datasources: &mut datasource_manager,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
                                notification: None,
                                do_not_disturb: false,
                            },
                        );
                        capsule = result.0;
                        actions = (result.4, result.8, result.2);
                    });
                });
        });
        (capsule, actions)
    };
    let (capsule, _) = frame(Vec::new());
    let mut click = |from_right| {
        let point = pos2(capsule.right() - from_right, capsule.top() + 26.0);
        let button = |pressed| Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::default(),
        };
        frame(vec![Event::PointerMoved(point)]);
        frame(vec![button(true)]);
        frame(vec![button(false)]).1
    };
    assert_eq!(click(104.0), (false, true, false));
    assert_eq!(click(82.0), (true, false, false));
}

#[test]
fn moon_button_emits_quiet_mode_toggle_without_changing_the_widget() {
    let ctx = egui::Context::default();
    let mut tab = crate::extension::ExtensionId::builtin("builtin.system-monitor/overview");
    let mut preferences = AppPreferences::default();
    let mut settings_view = SettingsView::default();
    let datasource_root = tempfile::tempdir().unwrap();
    let mut datasource_manager =
        crate::core::datasource::DataSourceManager::open(datasource_root.path());
    let mut datasource_view = crate::app::datasource::DataSourceView::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let (widget_ids, widget_view) = widget_fixture();
    let point = pos2(301.0, 30.0);
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
                                capsule_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: false,
                                datasource_open: false,
                                datasource_size: egui::vec2(790.0, 570.0),
                                datasource_view: &mut datasource_view,
                                datasources: &mut datasource_manager,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
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
    assert_eq!(
        tab,
        crate::extension::ExtensionId::builtin("builtin.system-monitor/overview")
    );
}

fn widget_fixture() -> (
    Vec<crate::extension::ExtensionId>,
    crate::extension::WidgetView,
) {
    let widget_ids = crate::builtins::registry()
        .by_kind(crate::extension::ContributionKind::Widget)
        .into_iter()
        .map(|descriptor| descriptor.id.clone())
        .collect::<Vec<_>>();
    let widget_ids = std::iter::once(crate::extension::ExtensionId::builtin(
        crate::extension::NOTIFICATION_CENTER_ID,
    ))
    .chain(widget_ids)
    .collect();
    let widget_view = crate::extension::WidgetView::new(
        crate::extension::ViewNode::text("测试", Default::default()),
        [300.0, 76.0],
    );

    (widget_ids, widget_view)
}
