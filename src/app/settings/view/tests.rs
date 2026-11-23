use super::*;
use crate::extension::SettingValue;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};

fn frame(
    ctx: &egui::Context,
    view: &mut SettingsView,
    prefs: &mut AppPreferences,
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
                if (r.rect.width() - 32.0).abs() < 0.5
                    && (r.rect.height() - 18.0).abs() < 0.5
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
    prefs: &mut AppPreferences,
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
    let mut prefs = AppPreferences::default();
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
    let mut prefs = AppPreferences::default();
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
    let mut prefs = AppPreferences::default();
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
        let mut prefs = AppPreferences::default();
        let mut view = SettingsView {
            section,
            ..Default::default()
        };
        let output = frame(&ctx, &mut view, &mut prefs, vec![]);
        let panels = Panels::new(
            bounds(),
            matches!(section, Section::Extensions | Section::Capsule | Section::Components | Section::Notifications),
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
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView::default();
    frame(&ctx, &mut view, &mut prefs, vec![]);
    let general = view.extra_height(&ctx, &prefs);
    view.section = Section::Extensions;
    let extensions = view.extra_height(&ctx, &prefs);
    view.section = Section::Capsule;
    view.capsule_section = CapsuleSection::CapsuleSwitch;
    let capsule = view.extra_height(&ctx, &prefs);
    assert!(extensions >= general);
    assert!(capsule >= general);
    assert!(general >= 8.0 * 40.0 + 49.0);
    assert!(general < 600.0);
}

#[test]
fn capsule_submenus_keep_configuration_in_the_third_column() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let panels = Panels::new(bounds(), true);
    assert!(panels
        .plugins
        .unwrap()
        .contains(text_rect(&output, "Mascot").center()));
    assert!(panels
        .form
        .contains(text_rect(&output, "显示 Mascot").center()));
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "全局动态效果"
            || text.galley.job.text == "动态效果")));
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
    let prefs = AppPreferences::default();
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
fn capsule_switches_and_order_live_in_capsule_not_components() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        capsule_section: CapsuleSection::CapsuleSwitch,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for title in ["Mascot", "胶囊样式", "胶囊开关", "胶囊顺序"] {
        assert!(text_rect(&output, title).width() > 0.0);
    }
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "胶囊顺序").center());
    assert_eq!(view.capsule_section, CapsuleSection::CapsuleOrder);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "胶囊顺序（仅已开启）").width() > 0.0);

    click(&ctx, &mut view, &mut prefs, text_rect(&output, "组件").center());
    assert_eq!(view.section, Section::Components);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for title in ["组件开关", "组件顺序"] {
        assert!(text_rect(&output, title).width() > 0.0);
    }
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "胶囊开关"
            || text.galley.job.text == "胶囊顺序")));
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "组件顺序").center());
    assert_eq!(view.component_section, ComponentSection::WidgetOrder);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "组件顺序（仅已开启）").width() > 0.0);
    assert!(text_rect(&output, "通知中心").width() > 0.0);
    assert!(text_rect(&output, "固定").width() > 0.0);
}

#[test]
fn notification_owner_navigation_keeps_only_selected_extension_in_form() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Notifications,
        notification_selected: Some(ExtensionId::builtin("builtin.calendar")),
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let panels = Panels::new(bounds(), true);
    assert!(panels.plugins.unwrap().contains(text_rect(&output, "日历与时间").center()));
    assert!(panels.form.contains(text_rect(&output, "节日提醒").center()));
    assert!(notifications::owners(&prefs).iter()
        .all(|(id, _)| prefs.pages.iter().any(|p|
            p.descriptor.id == *id && p.descriptor.contributions.iter()
                .any(|item| item.kind == ContributionKind::Notification))));
}

#[test]
fn notifications_menu_is_preserved() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Notifications,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "通知").width() > 0.0);
    assert!(text_rect(&output, "关于").width() > 0.0);
}

#[test]
fn component_submenus_only_contain_widget_switch_and_order() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Components,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for label in ["组件开关", "组件顺序"] {
        assert!(text_rect(&output, label).width() > 0.0, "{label}");
    }
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "组件开关").center());
    assert_eq!(view.component_section, ComponentSection::WidgetSwitch);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "通知中心").width() > 0.0);
    // Clicking the disabled host-owned switch cannot mutate preferences.
    let fixed_label = text_rect(&output, "通知中心");
    let row_rect = output.shapes.iter().filter_map(|shape| match &shape.shape {
        egui::Shape::Rect(r)
            if r.rect.contains(fixed_label.center()) && r.rect.width() > 160.0 =>
        {
            Some(r.rect)
        }
        _ => None,
    }).min_by(|a, b| a.width().total_cmp(&b.width()))
        .expect("fixed Notification Center control has a settings row");
    let before = prefs.clone();
    click(&ctx, &mut view, &mut prefs,
        egui::pos2(row_rect.right() - 24.0, row_rect.center().y));
    assert_eq!(prefs, before);
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "组件顺序").center());
    assert_eq!(view.component_section, ComponentSection::WidgetOrder);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "组件顺序（仅已开启）").width() > 0.0);
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
    let mut prefs = AppPreferences::default();
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
        .with_group(vec!["股票".into()])
        .with_setup(true)
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
                ).range(0.0, 1_000_000.0)
                    .number_text("例如 150.50")
                    .activation_positive(),
            ],
        ),
    );
    // An alert may be enabled before its activation threshold is complete.
    prefs.set_contribution_enabled(id.clone(), true);
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
                let owner = ExtensionId::builtin("builtin.market");
                notifications::options(ui, &mut prefs, Some(&owner), &mut HashMap::new());
            });
        },
    );
    for label in [
        "统一最短提醒间隔",
        "股票",
        "AAPL 价格提醒",
        "启用提醒",
        "触发条件",
        "目标价格",
    ] {
        assert!(text_rect(&output, label).width() > 0.0, "{label}");
    }
    let group = text_rect(&output, "股票");
    let item = text_rect(&output, "AAPL 价格提醒");
    assert!(item.top() > group.bottom());
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "市场")));
    // The asset settings are grouped inside their own bordered card;
    // absolute text indents depend on font metrics and are not the hierarchy.
    assert!(output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Rect(rect) => {
            rect.stroke.color == BORDER
                && rect.rect.contains(item.center())
                && rect.rect.top() > group.bottom()
                && rect.rect.height() > 55.0
        }
        _ => false,
    }));
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text.contains(
            "所有市场价格通知共用上述间隔"
        )
    )));
    assert_eq!(prefs.contribution_values[&id]["condition"],
        SettingValue::Enum("高于".into()));
    assert_eq!(prefs.contribution_values[&id]["threshold"],
        SettingValue::Float(0.0));
    assert!(text_rect(&output, "目标价格：需填写大于 0 的数值").width() > 0.0);
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text.contains(
            "待配置：请完成必填配置后启用提醒"
        )
    )));
    prefs.contribution_values.get_mut(&id).unwrap()
        .insert("threshold".into(), SettingValue::Float(150.50));
    let configured = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO, egui::vec2(900.0, 1400.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let owner = ExtensionId::builtin("builtin.market");
                notifications::options(ui, &mut prefs, Some(&owner), &mut HashMap::new());
            });
        },
    );
    assert!(!configured.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text.contains(
            "需填写大于 0 的数值"
        )
    )));
    assert!(prefs.contribution_enabled(
        &prefs.pages.iter().flat_map(|p| &p.descriptor.contributions)
            .find(|entry| entry.id == id).unwrap().clone()
    ));
}

#[test]
fn generic_number_text_accepts_decimals_and_rejects_invalid_values() {
    use crate::extension::SettingSchema;
    let schema = SettingSchema::new("value", SettingValue::Float(0.0), "数值")
        .range(0.0, 1_000_000.0).number_text("例如 150.50");
    for (input, value) in [
        ("  150.25  ", 150.25), ("0.001", 0.001), ("", 0.0), ("0", 0.0),
    ] {
        assert_eq!(forms::parse_number_text(&schema, input), Some(SettingValue::Float(value)));
    }
    for invalid in ["-5", "NaN", "inf", "abc", "10.3.2"] {
        assert_eq!(forms::parse_number_text(&schema, invalid), None);
    }
}

#[test]
fn notification_price_edit_is_reported_as_dirty_to_the_app_save_pipeline() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let alert = ExtensionId::builtin("builtin.market/assets/stock/AAPL/price-alert");
    let owner = ExtensionId::builtin("builtin.market");
    let mut view = SettingsView {
        section: Section::Notifications,
        notification_selected: Some(owner),
        ..Default::default()
    };
    view.number_drafts.insert((alert.clone(), "threshold".into()), "120".into());
    let mut dirty = false;
    ctx.run(RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 1000.0))),
        ..Default::default()
    }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            dirty = view.show(ui, bounds(), &mut prefs);
        });
    });
    assert!(dirty, "valid number edits must reach the app save pipeline");
    assert_eq!(prefs.contribution_values[&alert]["threshold"], SettingValue::Float(120.0));
}

#[test]
fn number_text_drafts_keep_stored_values_in_sync_with_visible_valid_prices() {
    let ctx = egui::Context::default();
    let owner = ExtensionId::builtin("builtin.market/assets/stock/AAPL/price-alert");
    let field = crate::extension::SettingSchema::new(
        "threshold", SettingValue::Float(0.0), "目标价格",
    ).range(0.0, 1_000_000.0).number_text("例如 150.50").activation_positive();
    let mut value = SettingValue::Float(0.0);
    let mut drafts = HashMap::new();
    drafts.insert((owner.clone(), field.key.clone()), "120".into());
    let mut changed = false;
    ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            changed = forms::setting_control_with_draft(
                ui, &field, &mut value, &owner, &mut drafts,
            );
        });
    });
    assert!(changed);
    assert_eq!(value, SettingValue::Float(120.0));
    assert!(field.activation_ready(&value));
}

#[test]
fn notification_page_has_a_bounded_content_width() {
    let ctx = egui::Context::default();
    let prefs = AppPreferences::default();
    ctx.run(RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))),
        ..Default::default()
    }, |ctx| {
        let view = SettingsView { section: Section::Notifications, ..Default::default() };
        assert!(view.width(ctx, &prefs) < 700.0);
        assert!(view.width(ctx, &prefs) > 550.0);
    });
}

#[test]
fn notification_renderer_is_generic_for_calendar_contributions() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO, egui::vec2(900.0, 1400.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let owner = ExtensionId::builtin("builtin.calendar");
                notifications::options(ui, &mut prefs, Some(&owner), &mut HashMap::new());
            });
        },
    );
    for label in ["节日提醒", "提醒时间", "小时（24 小时制）"] {
        assert!(text_rect(&output, label).width() > 0.0, "{label}");
    }
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "日历与时间")));
}

#[test]
fn capsule_configuration_remains_in_capsule_and_widgets_remain_first_level() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    for title in ["组件", "Mascot", "胶囊样式", "胶囊开关", "胶囊顺序"] {
        assert!(text_rect(&output, title).width() > 0.0);
    }
    click(&ctx, &mut view, &mut prefs, text_rect(&output, "组件").center());
    assert_eq!(view.section, Section::Components);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let panels = Panels::new(bounds(), true);
    for label in ["组件开关", "组件顺序"] {
        assert!(panels.plugins.unwrap().contains(text_rect(&output, label).center()));
    }
    assert_eq!(view.component_section, ComponentSection::WidgetSwitch);
    assert!(panels.form.left() > panels.plugins.unwrap().right());
}

#[test]
fn capsule_font_size_is_a_dropdown_and_not_a_numeric_drag_field() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        capsule_section: CapsuleSection::CapsuleStyle,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "字号").width() > 0.0);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text)
            if text.galley.job.text.contains(
                "标准"
            )
    )), "font-size control must render the selected named size");
}

#[test]
fn capsule_text_color_is_a_named_dropdown_with_readable_presets() {
    let ctx = egui::Context::default();
    let mut prefs = AppPreferences::default();
    let mut view = SettingsView {
        section: Section::Capsule,
        capsule_section: CapsuleSection::CapsuleStyle,
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "文字颜色").width() > 0.0);
    assert!(text_rect(&output, "默认").width() > 0.0);

    prefs.capsule_text_color = Some([158, 214, 255]);
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    assert!(text_rect(&output, "浅蓝").width() > 0.0);
}

#[test]
fn all_settings_rows_use_a_consistent_core_card_frame() {
    let ctx = egui::Context::default();
    let mut input_rect = Rect::NOTHING;
    let mut toggle_rect = Rect::NOTHING;
    let mut text = String::from("example");
    let mut flag = false;
    let output = ctx.run(RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
        ..Default::default()
    }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            input_rect = ui.available_rect_before_wrap();
            forms::input_row(ui, "输入", |ui| {
                ui.add_sized([100.0, 22.0], egui::TextEdit::singleline(&mut text)).changed()
            });
            toggle_rect = ui.available_rect_before_wrap();
            forms::boolean(ui, "开关", &mut flag);
        });
    });
    let row_border = |r: Rect| output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Rect(rect)
            if rect.stroke.color == BORDER
                && (rect.rect.left() - r.left()).abs() <= 2.0
                && rect.rect.width() > 250.0
                && (rect.rect.top() - r.top()).abs() <= 1.0)
    });
    assert!(row_border(input_rect));
    assert!(row_border(toggle_rect));
}
