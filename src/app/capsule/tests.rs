use super::{capsule_height, corner_radius, drag_handle_bounds, show, CapsuleWidgets};
use crate::app::settings::{AppPreferences, SettingsView};
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
                        let mut preferences = AppPreferences::default();
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
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
                                notification: None,
                                do_not_disturb: false,
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
fn separate_data_source_window_does_not_expand_or_replace_capsule() {
    fn bounds(source_window_open: bool) -> Rect {
        let context = egui::Context::default();
        let mut rect = Rect::NOTHING;
        let mut title_rendered = false;
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(460.0, 260.0))),
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(context, |ui| {
                        ui.vertical_centered(|ui| {
                            let mut tab = crate::app::widgets::default_widget_id();
                            let mut prefs = AppPreferences::default();
                            let mut settings = SettingsView::default();
                            let icons = crate::app::icons::Icons::load(context);
                            let (ids, widget) = widget_fixture();
                            rect = show(
                                ui,
                                context,
                                320.0,
                                1.0,
                                112.0,
                                CapsuleWidgets {
                                    capsule_views: &[],
                                    widget_ids: &ids,
                                    widget_titles: &[],
                                    widget_title: "测试",
                                    widget_view: &widget,
                                    widget_body_limit: egui::vec2(300.0, 76.0),
                                    alert_view: None,
                                    preferences: &mut prefs,
                                    settings_open: false,
                                    datasource_open: source_window_open,
                                    settings_view: &mut settings,
                                    active_widget: &mut tab,
                                    pinned: false,
                                    icons: &icons,
                                    notification: None,
                                    do_not_disturb: false,
                                },
                            ).0;
                        });
                    });
            },
        );
        title_rendered |= output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("测试"))
        });
        assert!(title_rendered, "widget title remains visible when data-source window is open");
        rect
    }
    assert_eq!(bounds(false).size(), bounds(true).size());
}

#[test]
fn capsule_capsule_is_shorter_without_changing_the_expanded_widget_height() {
    assert_eq!(capsule_height(0.0, 76.0), 44.0);
    assert_eq!(capsule_height(1.0, 76.0), 124.0);
}

#[test]
fn settings_close_button_returns_to_the_active_widget() {
    use eframe::egui::{Event, Modifiers, PointerButton};

    let context = egui::Context::default();
    let mut tab = crate::extension::ExtensionId::builtin("builtin.system-monitor/overview");
    let mut preferences = AppPreferences::default();
    let mut settings_view = SettingsView::default();
    let icons = crate::app::icons::Icons::load(&context);
    let (widget_ids, widget_view) = widget_fixture();
    let point = Pos2::new(SettingsView::WIDTH - 19.0 + 8.0, 26.0);
    let mut close_requested = false;
    let mut frame = |events: Vec<Event>| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(SettingsView::WIDTH + 16.0, 460.0),
            )),
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
                            settings_view.extra_height(context, &preferences),
                            CapsuleWidgets {
                                capsule_views: &[],
                                widget_ids: &widget_ids,
                                widget_titles: &[],
                                widget_title: "测试",
                                widget_view: &widget_view,
                                widget_body_limit: egui::vec2(300.0, 76.0),
                                alert_view: None,
                                preferences: &mut preferences,
                                settings_open: true,
                                datasource_open: false,
                                settings_view: &mut settings_view,
                                active_widget: &mut tab,
                                pinned: false,
                                icons: &icons,
                                notification: None,
                                do_not_disturb: false,
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

#[test]
fn table_text_keeps_its_destination_wrap_width_during_expansion() {
    use crate::ui::declarative::*;
    let ctx = egui::Context::default();
    let icons = crate::app::icons::Icons::load(&ctx);
    let mut preferences = AppPreferences::default();
    let mut settings = SettingsView::default();
    let mut active = crate::extension::ExtensionId::builtin("test/table");
    let ids = vec![active.clone()];
    let label =
        "A long parcel description that wraps across several lines at the destination width";
    let view = WidgetView::new(
        ViewNode::new(Node::Table(Table {
            row_menus: Vec::new(),
            columns: vec![TableColumn::new("Parcel", 1.0)],
            rows: vec![vec![ViewNode::text(
                label,
                TextStyle {
                    wrap: true,
                    ..Default::default()
                },
            )]],
        })),
        [440.0, 180.0],
    );
    let mut rows = vec![];
    for width in [280.0, 360.0, 440.0] {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 400.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    super::paint_expanded(
                        ui,
                        Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, 280.0)),
                        0.8,
                        super::ExpandedContent {
                            active_widget: &mut active,
                            widget_ids: &ids,
                            widget_titles: &[],
                            widget_title: "Test",
                            widget_view: &view,
                            widget_body_limit: egui::vec2(408.0, 180.0),
                            pinned: false,
                            do_not_disturb: false,
                            icons: &icons,
                            settings_open: false,
                            datasource_open: false,
                            settings_view: &mut settings,
                            preferences: &mut preferences,
                        },
                    );
                });
            },
        );
        let text = output
            .shapes
            .iter()
            .find_map(|item| match &item.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => Some(t),
                _ => None,
            })
            .unwrap();
        rows.push(text.galley.rows.len());
    }
    assert!(rows[0] > 1);
    assert_eq!(
        rows,
        vec![rows[0]; 3],
        "spring animation must not reflow the parcel table"
    );
}
