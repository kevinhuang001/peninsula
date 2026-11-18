use super::{
    model::Section,
    quiet_view::{self, QuietDialog},
    theme::WidgetTheme,
    CompactPreferences,
};
use crate::extension::{ContributionKind, ExtensionId, SettingSchema, SettingValue};
use eframe::egui::{self, Rect, Vec2};

#[derive(Debug, Default)]
pub struct SettingsView {
    section: Section,
    selected: Option<ExtensionId>,
    quiet_dialog: Option<QuietDialog>,
}
impl SettingsView {
    pub const WIDTH: f32 = 520.0;
    pub fn extra_height(&self, _: &egui::Context, _: &CompactPreferences) -> f32 {
        350.0
    }
    pub(in crate::app) fn show(
        &mut self,
        ui: &mut egui::Ui,
        capsule: Rect,
        prefs: &mut CompactPreferences,
    ) -> bool {
        let body = Rect::from_min_max(
            capsule.min + egui::vec2(12.0, 41.0),
            capsule.max - egui::vec2(12.0, 8.0),
        );
        if body.width() < 150.0 || body.height() < 1.0 {
            return false;
        }
        let sidebar = Rect::from_min_size(body.min, Vec2::new(82.0, body.height()));
        let panel = Rect::from_min_max(egui::pos2(sidebar.right() + 10.0, body.top()), body.max);
        let mut navigation = child(ui, sidebar, "settings-navigation");
        for section in Section::ALL {
            if navigation
                .selectable_label(section == self.section, section.label())
                .clicked()
            {
                self.section = section;
            }
            navigation.add_space(10.0);
        }
        let mut content = child(ui, panel, "settings-body");
        if self.section == Section::QuietHours {
            let mut changed = content
                .checkbox(&mut prefs.quiet_hours.manual_enabled, "开启免打扰")
                .changed();
            content.label("通知仍会保存在通知中心");
            let quiet = Rect::from_min_max(
                egui::pos2(panel.left(), content.cursor().top() + 6.0),
                panel.max,
            );
            changed |= quiet_view::show(
                &mut content,
                quiet,
                &mut prefs.quiet_hours,
                &mut self.quiet_dialog,
            );
            if quiet.height() >= quiet_view::DIALOG_MIN_HEIGHT {
                changed |= quiet_view::show_editor(
                    &mut content,
                    quiet,
                    &mut prefs.quiet_hours,
                    &mut self.quiet_dialog,
                );
            }
            return changed;
        }
        let mut changed = false;
        egui::ScrollArea::vertical()
            .id_salt(("settings", self.section, self.selected.clone()))
            .auto_shrink([false; 2])
            .show(&mut content, |ui| {
                ui.set_min_width(panel.width().max(0.0) - 12.0);
                match self.section {
                    Section::General => {
                        ui.heading("通用");
                        egui::ComboBox::from_label("主题")
                            .selected_text(prefs.widget_theme.name())
                            .show_ui(ui, |ui| {
                                for theme in
                                    [WidgetTheme::System, WidgetTheme::Dark, WidgetTheme::Light]
                                {
                                    changed |= ui
                                        .selectable_value(
                                            &mut prefs.widget_theme,
                                            theme,
                                            theme.name(),
                                        )
                                        .changed();
                                }
                            });
                        changed |= ui
                            .color_edit_button_srgb(&mut prefs.widget_accent)
                            .changed();
                    }
                    Section::Capsule => {
                        ui.heading("胶囊");
                        changed |= ui
                            .checkbox(&mut prefs.reduced_motion, "减少动态效果")
                            .changed();
                        changed |= ui.checkbox(&mut prefs.motion_off, "关闭动态效果").changed();
                        changed |= ui
                            .checkbox(&mut prefs.mascot_enabled, "显示 Mascot")
                            .changed();
                        ui.horizontal(|ui| {
                            ui.label("Mascot 宽度");
                            changed |= ui
                                .add(egui::DragValue::new(&mut prefs.mascot_size).range(24..=64))
                                .changed();
                        });
                        egui::ComboBox::from_label("Mascot 动画")
                            .selected_text(&prefs.mascot_motion)
                            .show_ui(ui, |ui| {
                                for mode in ["Full", "Reduced", "Off"] {
                                    changed |= ui
                                        .selectable_value(
                                            &mut prefs.mascot_motion,
                                            mode.into(),
                                            mode,
                                        )
                                        .changed();
                                }
                            });
                        ui.separator();
                        ui.label("Compact 顺序");
                        for item in prefs
                            .ordered_compact_items()
                            .into_iter()
                            .map(|i| i.id.clone())
                            .collect::<Vec<_>>()
                        {
                            let title = prefs
                                .pages
                                .iter()
                                .flat_map(|p| &p.descriptor.contributions)
                                .find(|c| c.id == item)
                                .map_or_else(|| item.to_string(), |c| c.title.clone());
                            ui.horizontal(|ui| {
                                ui.label(title);
                                if ui.small_button("↑").clicked() {
                                    changed |= prefs.move_compact_item(&item, -1);
                                }
                                if ui.small_button("↓").clicked() {
                                    changed |= prefs.move_compact_item(&item, 1);
                                }
                            });
                        }
                        ui.separator();
                        ui.label("Widget 顺序（通知中心始终第一位）");
                        let order = prefs.widget_order.clone();
                        for (index, id) in order.iter().enumerate() {
                            let name = prefs
                                .pages
                                .iter()
                                .flat_map(|p| &p.descriptor.contributions)
                                .find(|c| &c.id == id)
                                .map_or_else(|| id.to_string(), |c| c.title.clone());
                            ui.horizontal(|ui| {
                                ui.label(name);
                                if ui.small_button("↑").clicked() && index > 0 {
                                    prefs.widget_order.swap(index, index - 1);
                                    changed = true;
                                }
                                if ui.small_button("↓").clicked() && index + 1 < order.len() {
                                    prefs.widget_order.swap(index, index + 1);
                                    changed = true;
                                }
                            });
                        }
                    }
                    Section::Extensions => {
                        if let Some(id) = self.selected.clone() {
                            if ui.button("← 插件列表").clicked() {
                                self.selected = None;
                            }
                            if let Some(page) =
                                prefs.pages.iter_mut().find(|p| p.descriptor.id == id)
                            {
                                ui.heading(format!("{} 设置", page.descriptor.name));
                                ui.label(&page.descriptor.description);
                                ui.separator();
                                ui.strong("系统管理");
                                changed |= ui.checkbox(&mut page.enabled, "启用扩展").changed();
                                ui.label(format!("当前状态：{}", page.status));
                                ui.separator();
                                ui.strong("插件设置");
                                if page.descriptor.settings.is_empty() {
                                    ui.label("此扩展没有自定义配置");
                                }
                                for field in &page.descriptor.settings {
                                    if let Some(value) = page.values.get_mut(&field.key) {
                                        ui.push_id((&id, &field.key), |ui| {
                                            changed |= setting_control(ui, field, value)
                                        });
                                    }
                                }
                                let compact: Vec<_> = page
                                    .descriptor
                                    .contributions
                                    .iter()
                                    .filter(|c| c.kind == ContributionKind::Compact)
                                    .map(|c| (c.id.clone(), c.title.clone()))
                                    .collect();
                                for (id, name) in compact {
                                    if let Some(item) =
                                        prefs.compact_items.iter_mut().find(|i| i.id == id)
                                    {
                                        changed |= ui
                                            .checkbox(
                                                &mut item.enabled,
                                                format!("Compact · {name}"),
                                            )
                                            .changed();
                                    }
                                }
                                ui.separator();
                                ui.small(format!(
                                    "{} · v{}",
                                    page.descriptor.id, page.descriptor.version
                                ));
                            }
                        } else {
                            ui.heading("扩展 · 插件列表");
                            for page in &prefs.pages {
                                ui.push_id(&page.descriptor.id, |ui| {
                                    if ui
                                        .button(format!(
                                            "{}  {}",
                                            page.descriptor.icon, page.descriptor.name
                                        ))
                                        .clicked()
                                    {
                                        self.selected = Some(page.descriptor.id.clone());
                                    }
                                    ui.label(&page.descriptor.description);
                                    ui.small(format!(
                                        "{} · {}",
                                        if page.enabled {
                                            "已启用"
                                        } else {
                                            "已停用"
                                        },
                                        page.status
                                    ));
                                    ui.separator();
                                });
                            }
                        }
                    }
                    Section::About => {
                        ui.heading("Peninsula");
                        ui.label(concat!("版本 ", env!("CARGO_PKG_VERSION")));
                        ui.hyperlink_to("GitHub", "https://github.com/kevinhuang001/peninsula");
                        ui.label("许可证和更新信息见项目仓库");
                    }
                    Section::QuietHours => {}
                }
            });
        changed
    }
}
fn child(ui: &mut egui::Ui, rect: Rect, id: &str) -> egui::Ui {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child
}
/// All field types are rendered by Core; extensions supply only schemas and values.
fn setting_control(ui: &mut egui::Ui, field: &SettingSchema, value: &mut SettingValue) -> bool {
    let old = value.clone();
    ui.label(&field.description);
    match value {
        SettingValue::Boolean(v) => {
            ui.checkbox(v, "开启");
        }
        SettingValue::Integer(v) => {
            ui.add(egui::DragValue::new(v).range(
                field.constraints.min.unwrap_or(i32::MIN as f64) as i64
                    ..=field.constraints.max.unwrap_or(i32::MAX as f64) as i64,
            ));
        }
        SettingValue::Float(v) => {
            ui.add(egui::DragValue::new(v).speed(0.1).range(
                field.constraints.min.unwrap_or(-1e9)..=field.constraints.max.unwrap_or(1e9),
            ));
        }
        SettingValue::Duration(v) => {
            ui.add(egui::DragValue::new(v).suffix(" ms").range(
                field.constraints.min.unwrap_or(0.0) as u64
                    ..=field.constraints.max.unwrap_or(u32::MAX as f64) as u64,
            ));
        }
        SettingValue::String(v) => {
            ui.add(
                egui::TextEdit::singleline(v)
                    .char_limit(field.constraints.max_length.unwrap_or(4096)),
            );
        }
        SettingValue::Enum(v) => {
            egui::ComboBox::from_id_salt(&field.key)
                .selected_text(v.clone())
                .show_ui(ui, |ui| {
                    for option in &field.options {
                        ui.selectable_value(v, option.clone(), option);
                    }
                });
        }
        SettingValue::Color(v) => {
            ui.color_edit_button_srgba_unmultiplied(v);
        }
    }
    if field.validate(value).is_err() {
        *value = old;
        return false;
    }
    *value != old
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};
    fn frame(
        ctx: &egui::Context,
        view: &mut SettingsView,
        prefs: &mut CompactPreferences,
        events: Vec<Event>,
    ) -> egui::FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(560.0, 460.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    view.show(
                        ui,
                        Rect::from_min_size(Pos2::ZERO, egui::vec2(520.0, 440.0)),
                        prefs,
                    );
                });
            },
        )
    }
    fn text_center(output: &egui::FullOutput, value: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == value => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing rendered label: {value}"))
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
    fn core_controls_and_schema_fields_work_while_extension_is_stopped() {
        let ctx = egui::Context::default();
        let mut prefs = CompactPreferences::default();
        let id = ExtensionId::builtin("builtin.clock");
        let mut view = SettingsView {
            section: Section::Extensions,
            selected: Some(id.clone()),
            ..Default::default()
        };
        let output = frame(&ctx, &mut view, &mut prefs, vec![]);
        let field = text_center(&output, "开启");
        click(&ctx, &mut view, &mut prefs, field);
        let page = prefs.pages.iter().find(|p| p.descriptor.id == id).unwrap();
        assert!(!page.enabled);
        assert_eq!(page.values["twelve_hour"], SettingValue::Boolean(true));
        let output = frame(&ctx, &mut view, &mut prefs, vec![]);
        let toggle = text_center(&output, "启用扩展");
        click(&ctx, &mut view, &mut prefs, toggle);
        assert!(
            prefs
                .pages
                .iter()
                .find(|p| p.descriptor.id == id)
                .unwrap()
                .enabled
        );
        let output = frame(&ctx, &mut view, &mut prefs, vec![]);
        let back = text_center(&output, "← 插件列表");
        click(&ctx, &mut view, &mut prefs, back);
        assert!(view.selected.is_none());
        view.selected = Some(id.clone());
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
}
