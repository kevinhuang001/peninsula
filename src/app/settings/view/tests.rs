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
    let id = ExtensionId::builtin("builtin.clock");
    let mut view = SettingsView {
        section: Section::Extensions,
        selected: Some(id.clone()),
        ..Default::default()
    };
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    let switch = toggle_for(&output, "12 小时制");
    click(&ctx, &mut view, &mut prefs, switch.center());
    let page = prefs.pages.iter().find(|p| p.descriptor.id == id).unwrap();
    assert!(!page.enabled);
    assert_eq!(page.values["twelve_hour"], SettingValue::Boolean(true));
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
    let plugin_panel = Panels::new(bounds(), true).plugins.unwrap();
    for _ in 0..8 {
        frame(
            &ctx,
            &mut view,
            &mut prefs,
            vec![
                Event::PointerMoved(plugin_panel.center()),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -60.0),
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    let output = frame(&ctx, &mut view, &mut prefs, vec![]);
    click(
        &ctx,
        &mut view,
        &mut prefs,
        text_rect(&output, "时间和日期").center(),
    );
    assert_eq!(view.selected.as_ref(), Some(&id));
    assert_eq!(
        prefs
            .pages
            .iter()
            .find(|p| p.descriptor.id == id)
            .unwrap()
            .values["twelve_hour"],
        SettingValue::Boolean(true)
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
            matches!(section, Section::Extensions | Section::Capsule),
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
    view.capsule_section = CapsuleSection::Compact;
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
            let id = ExtensionId::builtin("builtin.clock");
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
            assert!(view.width(ctx, &longer) > normal);
            assert!(view.width(ctx, &longer) <= ctx.screen_rect().width() * 0.75);
        },
    );
}
