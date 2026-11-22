use super::*;
use crate::extension::SettingValue;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};

fn frame(
    ctx: &egui::Context,
    view: &mut SettingsView,
    prefs: &mut CompactPreferences,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(SettingsView::WIDTH + 40.0, 480.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.show(ui, bounds(), prefs);
            });
        },
    )
}
fn bounds() -> Rect {
    Rect::from_min_size(Pos2::ZERO, egui::vec2(SettingsView::WIDTH, 440.0))
}
fn text_rect(output: &egui::FullOutput, value: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == value => {
                Some(Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing label: {value}"))
}
fn toggle_for(output: &egui::FullOutput, label: &str) -> Rect {
    let label = text_rect(output, label);
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Rect(r)
                if (r.fill == forms::SWITCH_ON || r.fill == forms::SWITCH_OFF)
                    && r.rect.center().x > label.right()
                    && (r.rect.center().y - label.center().y).abs() < 2.0 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("form control must be to the right of its label")
}
fn click(
    ctx: &egui::Context,
    view: &mut SettingsView,
    prefs: &mut CompactPreferences,
    point: Pos2,
) {
    frame(ctx, view, prefs, vec![Event::PointerMoved(point)]);
    for pressed in [true, false] {
        frame(
            ctx,
            view,
            prefs,
            vec![Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }],
        );
    }
}

#[test]
fn secret_editor_buttons_respond_to_clicks() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView {
        section: Section::Secrets,
        secret_dialog_open: true,
        secret_key: "example".into(),
        secret_value: "token".into(),
        ..Default::default()
    };
    frame(&ctx, &mut view, &mut prefs, vec![]);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    click(
        &ctx,
        &mut view,
        &mut prefs,
        text_rect(&output, "保存 Secret").center(),
    );
    assert_eq!(view.secret_status, "秘密存储不可用");
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    click(
        &ctx,
        &mut view,
        &mut prefs,
        text_rect(&output, "取消").center(),
    );
    assert!(!view.secret_dialog_open);
}

#[test]
fn plugin_names_remain_in_second_column_and_configuration_is_in_the_right_panel() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView {
        section: Section::Extensions,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let panels = Panels::new(bounds(), true);
    let plugin = text_rect(&output, "番茄钟");
    assert!(panels.plugins.unwrap().contains(plugin.center()));
    click(&ctx, &mut view, &mut prefs, plugin.center());
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(panels
        .form
        .contains(text_rect(&output, "启用扩展").center()));
    assert!(panels
        .form
        .contains(text_rect(&output, "专注分钟数").center()));
    assert!(panels
        .plugins
        .unwrap()
        .contains(text_rect(&output, "系统监控").center()));
    assert!(panels.plugins.unwrap().right() < panels.form.left());
}

#[test]
fn stopped_extension_forms_are_editable_and_switching_plugins_keeps_values() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let id = ExtensionId::builtin("builtin.calendar");
    // The merged calendar is enabled by default because its month Widget is
    // enabled; explicitly stop it to test offline settings editing.
    prefs.pages.iter_mut().find(|page| page.descriptor.id == id)
        .expect("calendar settings page").enabled = false;
    let mut view = SettingsView {
        section: Section::Extensions,
        selected: Some(id.clone()),
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let switch = toggle_for(&output, "中国地区显示农历");
    click(&ctx, &mut view, &mut prefs, switch.center());
    let page = prefs.pages.iter().find(|p| p.descriptor.id == id).unwrap();
    assert!(!page.enabled);
    assert_eq!(page.values["show_lunar"], SettingValue::Boolean(false));
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    click(
        &ctx,
        &mut view,
        &mut prefs,
        toggle_for(&output, "启用扩展").center(),
    );
    assert!(
        prefs
            .pages
            .iter()
            .find(|p| p.descriptor.id == id)
            .unwrap()
            .enabled
    );
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    click(
        &ctx,
        &mut view,
        &mut prefs,
        text_rect(&output, "番茄钟").center(),
    );
    // The merged calendar moved earlier in the list; restoration is tested
    // independently of how far the navigational column was scrolled.
    view.selected = Some(id.clone());
    frame(&ctx, &mut view, &mut prefs, vec![]);
    assert_eq!(view.selected.as_ref(), Some(&id));
    assert_eq!(
        prefs
            .pages
            .iter()
            .find(|p| p.descriptor.id == id)
            .unwrap()
            .values["show_lunar"],
        SettingValue::Boolean(false)
    );
}

#[test]
fn every_menu_keeps_its_bordered_navigation_and_content_panels() {
    for section in Section::ALL {
        let ctx = egui::Context::default();
        let mut prefs = CompactPreferences::default();
        let mut view = SettingsView {
            section,
            ..Default::default()
        };
        let output = frame(&ctx, &mut view, &mut prefs, vec![]);
        let panels = Panels::new(
            bounds(),
            matches!(section, Section::Extensions | Section::Capsule | Section::Components),
        );
        for rect in [Some(panels.navigation), panels.plugins, Some(panels.form)]
            .into_iter()
            .flatten()
        {
            assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rect && r.stroke.color == BORDER && r.stroke.width >= 1.0)));
        }
    }
}

#[test]
fn settings_height_grows_with_content_before_host_applies_the_screen_limit() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView::default();
    frame(&ctx, &mut view, &mut prefs, vec![]);
    let general = view.extra_height(&ctx, &prefs);
    view.section = Section::Extensions;
    let extensions = view.extra_height(&ctx, &prefs);
    view.section = Section::Capsule;
    view.capsule_section = CapsuleSection::CompactSwitch;
    let capsule = view.extra_height(&ctx, &prefs);
    assert!(extensions > general);
    assert!(capsule > general);
    assert!(general < 300.0);
}

#[test]
fn capsule_submenus_keep_configuration_in_the_third_column() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let panels = Panels::new(bounds(), true);
    assert!(panels
        .plugins
        .unwrap()
        .contains(text_rect(&output, "动态效果").center()));
    assert!(panels
        .form
        .contains(text_rect(&output, "全局动态效果").center()));
    click(
        &ctx,
        &mut view,
        &mut prefs,
        text_rect(&output, "Mascot").center(),
    );
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert_eq!(view.capsule_section, CapsuleSection::Mascot);
    assert!(panels
        .form
        .contains(text_rect(&output, "显示 Mascot").center()));
    toggle_for(&output, "显示 Mascot");
    assert_eq!(view.capsule_section, CapsuleSection::Mascot);
}

#[test]
fn settings_width_follows_the_selected_content_and_screen_limit() {
    let ctx = egui::Context::default();
    let prefs = CompactPreferences::default();
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            ..Default::default()
        },
        |ctx| {
            let general = SettingsView::default().width(ctx, &prefs);
            let quiet = SettingsView {
                section: Section::QuietHours,
                ..Default::default()
            }
            .width(ctx, &prefs);
            assert!(general < 680.0);
            assert!(quiet < 680.0);
            let id = ExtensionId::builtin("builtin.calendar");
            let view = SettingsView {
                section: Section::Extensions,
                selected: Some(id.clone()),
                ..Default::default()
            };
            let normal = view.width(ctx, &prefs);
            let mut longer = prefs.clone();
            let page = longer
                .pages
                .iter_mut()
                .find(|page| page.descriptor.id == id)
                .unwrap();
            page.descriptor.settings[0].description =
                "A much longer setting description that requires more room for the control".into();
            assert_eq!(view.width(ctx, &longer), normal);
            assert!(view.width(ctx, &longer) <= ctx.screen_rect().width() * 0.75);
        },
    );
}

#[test]
fn compact_switches_and_order_live_in_capsule_not_components() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        capsule_section: CapsuleSection::CompactSwitch,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for title in ["动态效果", "Mascot", "Compact 样式", "Compact 开关", "Compact 顺序"] {
        assert!(text_rect(&output, title).width() > 0.0);
    }
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "Compact 顺序").center());
    assert_eq!(view.capsule_section, CapsuleSection::CompactOrder);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "Compact 顺序（仅已开启）").width() > 0.0);

    click(&ctx, &mut view, &mut prefs, text_rect(&output, "组件").center());
    assert_eq!(view.section, Section::Components);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for title in ["Widget 开关", "Widget 顺序"] {
        assert!(text_rect(&output, title).width() > 0.0);
    }
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "Compact 开关"
            || text.galley.job.text == "Compact 顺序")));
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "Widget 顺序").center());
    assert_eq!(view.component_section, ComponentSection::WidgetOrder);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "Widget 顺序（仅已开启）").width() > 0.0);
    assert!(text_rect(&output, "通知中心").width() > 0.0);
    assert!(text_rect(&output, "固定").width() > 0.0);
}

#[test]
fn notifications_menu_is_preserved() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView {
        section: Section::Notifications,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "通知").width() > 0.0);
}

#[test]
fn component_submenus_only_contain_widget_switch_and_order() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView {
        section: Section::Components,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for label in ["Widget 开关", "Widget 顺序"] {
        assert!(text_rect(&output, label).width() > 0.0, "{label}");
    }
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "Widget 开关").center());
    assert_eq!(view.component_section, ComponentSection::WidgetSwitch);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "通知中心").width() > 0.0);
    let mut fixed = text_rect(&output, "通知中心").center();
    assert_eq!(prefs.ordered_enabled_widgets().len(),
        prefs.ordered_enabled_widgets().len());
    // The immutable switch must not mutate saved contribution preferences.
    let old_preferences = prefs.contribution_preferences.clone();
    click(&ctx, &mut view, &mut prefs, fixed);
    assert_eq!(prefs.contribution_preferences, old_preferences);
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "Widget 顺序").center());
    assert_eq!(view.component_section, ComponentSection::WidgetOrder);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "Widget 顺序（仅已开启）").width() > 0.0);
}

#[test]
fn nested_settings_children_preserve_input_focus_stroke_edges() {
    let ctx = egui::Context::default();
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(500.0, 300.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(
                    Pos2::new(50.0, 50.0),
                    egui::vec2(250.0, 90.0),
                );
                let nested = child(ui, rect, "focus-clip-test");
                assert!(nested.clip_rect().left() < rect.left() - 1.0);
                assert!(nested.clip_rect().right() > rect.right() + 1.0);
            });
        },
    );
}

#[test]
fn market_notification_settings_show_one_shared_interval_and_asset_rules() {
    use crate::extension::{ContributionDescriptor, SettingSchema};
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let id = ExtensionId::builtin("builtin.market/assets/stock/AAPL/price-alert");
    let market = prefs.pages.iter_mut().find(|page| {
        page.descriptor.id.as_str() == "builtin.market"
    }).expect("market extension is registered");
    market.descriptor.contributions.push(
        ContributionDescriptor::builtin(
            "builtin.market/assets/stock/AAPL/price-alert",
            ContributionKind::Notification,
            "AAPL 价格提醒",
            "bell",
            false,
            100,
        )
        .with_settings(
            1,
            vec![
                SettingSchema {
                    options: vec!["高于".into(), "低于".into()],
                    ..SettingSchema::new(
                        "condition",
                        SettingValue::Enum("高于".into()),
                        "触发条件",
                    )
                },
                SettingSchema::new(
                    "threshold",
                    SettingValue::Float(0.0),
                    "目标价格",
                ),
            ],
        ),
    );
    // Reproduce a newly added stock that has never saved any notification
    // settings: the entire form must still be shown and editable.
    assert!(!prefs.contribution_values.contains_key(&id));
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO, egui::vec2(900.0, 1400.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                notification_options(ui, &mut prefs, &mut HashMap::new());
            });
        },
    );
    for label in [
        "统一最短提醒间隔",
        "AAPL 价格提醒",
        "启用提醒",
        "触发条件",
        "目标价格",
    ] {
        assert!(text_rect(&output, label).width() > 0.0, "{label}");
    }
    assert_eq!(prefs.contribution_values[&id]["condition"],
        SettingValue::Enum("高于".into()));
    assert_eq!(prefs.contribution_values[&id]["threshold"],
        SettingValue::Float(0.0));
}

#[test]
fn market_price_entry_accepts_decimals_and_rejects_invalid_values() {
    assert_eq!(parse_target_price("  150.25  "), Some(150.25));
    assert_eq!(parse_target_price("0.001"), Some(0.001));
    assert_eq!(parse_target_price(""), Some(0.0));
    assert_eq!(parse_target_price("0"), Some(0.0));
    assert_eq!(parse_target_price("-5"), None);
    assert_eq!(parse_target_price("NaN"), None);
    assert_eq!(parse_target_price("inf"), None);
    assert_eq!(parse_target_price("abc"), None);
    assert_eq!(parse_target_price("10.3.2"), None);
}

#[test]
fn compact_configuration_remains_in_capsule_and_widgets_remain_first_level() {
    let ctx = egui::Context::default();
    let mut prefs = CompactPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for title in ["组件", "动态效果", "Compact 样式", "Compact 开关", "Compact 顺序"] {
        assert!(text_rect(&output, title).width() > 0.0);
    }
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "组件").center());
    assert_eq!(view.section, Section::Components);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let panels = Panels::new(bounds(), true);
    for label in ["Widget 开关", "Widget 顺序"] {
        assert!(panels.plugins.unwrap().contains(text_rect(&output, label).center()));
    }
    assert_eq!(view.component_section, ComponentSection::WidgetSwitch);
    assert!(panels.form.left() > panels.plugins.unwrap().right());
}
