use super::{
    forms,
    model::Section,
    quiet_view::{self, QuietDialog},
    theme::{WidgetTheme, ACCENT_SELECTED, BORDER, PANEL, SECONDARY_TEXT, TEXT},
    CompactPreferences,
};
use crate::extension::{ContributionKind, ExtensionId};
use crate::sdk::secrets::{SecretBackend, SecretStore};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke};
use std::{collections::HashMap, time::Instant};

const NAV_BACKGROUND: Color32 = Color32::from_rgb(23, 25, 30);
const SIDEBAR_WIDTH: f32 = 104.0;
const PLUGIN_WIDTH: f32 = 134.0;
const COLUMN_GAP: f32 = 9.0;
const PANEL_INSET: f32 = 10.0;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
enum CapsuleSection {
    #[default]
    Motion,
    Mascot,
    Compact,
    Widgets,
}
impl CapsuleSection {
    const ALL: [Self; 4] = [Self::Motion, Self::Mascot, Self::Compact, Self::Widgets];
    fn label(self) -> &'static str {
        match self {
            Self::Motion => "动态效果",
            Self::Mascot => "Mascot",
            Self::Compact => "Compact 顺序",
            Self::Widgets => "Widget 顺序",
        }
    }
}

#[derive(Debug, Default)]
pub struct SettingsView {
    section: Section,
    selected: Option<ExtensionId>,
    capsule_section: CapsuleSection,
    quiet_dialog: Option<QuietDialog>,
    searches: HashMap<(ExtensionId, String), forms::SearchFieldState>,
    secret_store: Option<SecretStore>,
    secret_keys: Vec<String>,
    secret_key: String,
    secret_value: String,
    secret_status: String,
    secret_index_path: Option<std::path::PathBuf>,
    secret_revealed: bool,
    secret_dialog_open: bool,
}

#[derive(Clone, Copy)]
struct Panels {
    navigation: Rect,
    plugins: Option<Rect>,
    form: Rect,
}
impl Panels {
    fn new(capsule: Rect, extensions: bool) -> Self {
        let body = Rect::from_min_max(
            capsule.min + egui::vec2(12.0, 41.0),
            capsule.max - egui::vec2(12.0, 8.0),
        );
        // Keep three columns side by side while the capsule animates; clip the
        // reveal at the shell instead of reflowing the form under its navigation.
        let sidebar_width = SIDEBAR_WIDTH.min(body.width() * 0.24);
        let navigation = Rect::from_min_size(body.min, egui::vec2(sidebar_width, body.height()));
        let remaining = Rect::from_min_max(
            egui::pos2(navigation.right() + COLUMN_GAP, body.top()),
            body.max,
        );
        let plugins = extensions.then(|| {
            Rect::from_min_size(
                remaining.min,
                egui::vec2(PLUGIN_WIDTH.min(remaining.width() * 0.31), body.height()),
            )
        });
        let form = Rect::from_min_max(
            egui::pos2(
                plugins.map_or(remaining.left(), |p| p.right() + COLUMN_GAP),
                body.top(),
            ),
            body.max,
        );
        Self {
            navigation,
            plugins,
            form,
        }
    }
}

impl SettingsView {
    pub fn with_secret_store(
        mut self,
        store: SecretStore,
        index_path: std::path::PathBuf,
        index: &[String],
    ) -> Self {
        self.secret_store = Some(store);
        self.secret_index_path = Some(index_path);
        self.secret_keys = index.to_vec();
        self
    }
    // A fixed canvas is used only by existing interaction fixtures.
    #[cfg(test)]
    pub const WIDTH: f32 = 680.0;

    pub fn width(&self, context: &egui::Context, prefs: &CompactPreferences) -> f32 {
        let extra_column = matches!(self.section, Section::Extensions | Section::Capsule);
        let chrome = 24.0
            + SIDEBAR_WIDTH
            + COLUMN_GAP
            + 2.0 * PANEL_INSET
            + if extra_column {
                PLUGIN_WIDTH + COLUMN_GAP
            } else {
                0.0
            };
        let limit = context.screen_rect().width() * 0.75;
        let available = (limit - chrome).max(1.0);
        let content_width = if self.section == Section::QuietHours {
            quiet_view::preferred_width(context, &prefs.quiet_hours).max(
                if self.quiet_dialog.is_some() {
                    300.0
                } else {
                    0.0
                },
            )
        } else {
            let mut measure = egui::Ui::new(
                context.clone(),
                egui::Id::new("settings-width-measure"),
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(available, 10_000.0),
                    ))
                    .sizing_pass()
                    .invisible(),
            );
            configure(&mut measure);
            let mut preview = Self {
                section: self.section,
                selected: self
                    .selected
                    .clone()
                    .or_else(|| prefs.pages.first().map(|p| p.descriptor.id.clone())),
                capsule_section: self.capsule_section,
                searches: self.searches.clone(),
                ..Default::default()
            };
            preview.content(&mut measure, &mut prefs.clone());
            measure.min_rect().width()
        };
        // Leave room for the shared title bar; larger forms follow their content.
        (chrome + content_width).ceil().max(380.0).min(limit)
    }

    pub fn extra_height(&self, context: &egui::Context, prefs: &CompactPreferences) -> f32 {
        let width = self.width(context, prefs);
        let panels = Panels::new(
            Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 10_000.0)),
            matches!(self.section, Section::Extensions | Section::Capsule),
        );
        let mut measure = egui::Ui::new(
            context.clone(),
            egui::Id::new("settings-content-measure"),
            egui::UiBuilder::new()
                .max_rect(panels.form.shrink(PANEL_INSET))
                .sizing_pass()
                .invisible(),
        );
        configure(&mut measure);
        let mut preview = Self {
            section: self.section,
            selected: self
                .selected
                .clone()
                .or_else(|| prefs.pages.first().map(|p| p.descriptor.id.clone())),
            capsule_section: self.capsule_section,
            quiet_dialog: None,
            searches: self.searches.clone(),
            ..Default::default()
        };
        let content_height = if self.section == Section::QuietHours {
            106.0 + 28.0 * prefs.quiet_hours.periods().len().max(1) as f32
        } else {
            preview.content(&mut measure, &mut prefs.clone());
            measure.min_rect().height()
        };
        let navigation_height = Section::ALL.len() as f32 * 40.0 + 10.0;
        let plugin_height = if self.section == Section::Extensions {
            prefs.pages.len() as f32 * 40.0 + 40.0
        } else {
            0.0
        };
        content_height.max(navigation_height).max(plugin_height) + 21.0
    }

    fn content(&mut self, ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
        match self.section {
            Section::General => general(ui, prefs),
            Section::Capsule => capsule_options(ui, prefs, self.capsule_section),
            Section::Extensions => self.extension_form(ui, prefs),
            Section::Secrets => self.secrets(ui),
            Section::About => {
                about(ui);
                false
            }
            Section::QuietHours => false,
        }
    }

    pub(in crate::app) fn show(
        &mut self,
        ui: &mut egui::Ui,
        capsule: Rect,
        prefs: &mut CompactPreferences,
    ) -> bool {
        if capsule.width() < 180.0 || capsule.height() < 60.0 {
            return false;
        }
        let mut surface = child(ui, capsule, "settings-surface");
        configure(&mut surface);
        // Pick a menu before computing columns, so navigation and content update
        // in the same pass and cannot show the previous menu's selection.
        let nav_rect = Panels::new(capsule, false).navigation;
        panel(&surface, nav_rect, NAV_BACKGROUND);
        self.navigation(&mut surface, nav_rect);
        let panels = Panels::new(
            capsule,
            matches!(self.section, Section::Extensions | Section::Capsule),
        );
        if let Some(plugins) = panels.plugins {
            panel(&surface, plugins, NAV_BACKGROUND);
            if self.section == Section::Capsule {
                self.capsule_navigation(&mut surface, plugins);
            } else {
                self.plugins(&mut surface, plugins, prefs);
            }
        }
        panel(&surface, panels.form, PANEL);
        let mut content = child(
            &mut surface,
            panels.form.shrink(PANEL_INSET),
            "settings-form",
        );
        if self.section == Section::QuietHours {
            return self.quiet(&mut content, panels.form.shrink(PANEL_INSET), prefs);
        }
        let mut changed = false;
        egui::ScrollArea::vertical()
            .id_salt((
                "settings-form-scroll",
                self.section,
                self.selected.clone(),
                self.capsule_section,
            ))
            .auto_shrink([false; 2])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(&mut content, |ui| {
                ui.set_width(ui.available_width());
                changed |= self.content(ui, prefs);
            });
        changed
    }

    fn navigation(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let mut navigation = child(ui, rect.shrink(5.0), "settings-navigation");
        for section in Section::ALL {
            if menu_row(
                &mut navigation,
                ("section", section),
                section.label(),
                self.section == section,
                None,
            ) {
                self.section = section;
            }
        }
    }

    fn secrets(&mut self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            ui.heading("秘密");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("＋ 新增秘密").clicked() {
                    self.secret_key.clear();
                    self.secret_value.clear();
                    self.secret_revealed = false;
                    self.secret_dialog_open = true;
                }
            });
        });
        ui.label("私密内容优先保存在系统 Keyring 中。");
        let mut changed = false;
        if self.secret_keys.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("还没有保存的秘密").size(14.0).strong());
                ui.label("点击“新增秘密”保存 API 密钥、令牌或其他私密内容。");
            });
        }
        let mut remove = None;
        for key in self.secret_keys.clone() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&key).strong());
                    ui.label(egui::RichText::new("Keyring").small().color(SECONDARY_TEXT));
                    if ui.small_button("编辑").clicked() {
                        self.secret_key = key.clone();
                        self.secret_value.clear();
                        self.secret_revealed = false;
                        self.secret_dialog_open = true;
                    }
                    if ui.small_button("删除").clicked() {
                        remove = Some(key.clone());
                    }
                });
            });
        }
        if let (Some(key), Some(store)) = (remove, &self.secret_store) {
            match store.delete(&key) {
                Ok(()) => {
                    self.secret_keys.retain(|entry| entry != &key);
                    self.persist_secret_index();
                    self.secret_status = "已删除".into();
                    changed = true;
                }
                Err(error) => self.secret_status = error.to_string(),
            }
        }
        if self.secret_dialog_open {
            let mut open = true;
            egui::Window::new(if self.secret_keys.contains(&self.secret_key) {
                "编辑秘密"
            } else {
                "新增秘密"
            })
            .id(egui::Id::new("secret-editor"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.set_min_width(300.0);
                ui.label("名称");
                ui.text_edit_singleline(&mut self.secret_key);
                ui.add_space(8.0);
                ui.label("内容");
                ui.add(
                    egui::TextEdit::singleline(&mut self.secret_value)
                        .password(!self.secret_revealed),
                );
                ui.checkbox(&mut self.secret_revealed, "显示内容");
                if !self.secret_status.is_empty() {
                    ui.label(&self.secret_status);
                }
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.secret_dialog_open = false;
                    }
                    if ui.button("保存").clicked() {
                        match self.save_secret() {
                            Ok(()) => {
                                changed = true;
                                self.secret_dialog_open = false;
                            }
                            Err(error) => self.secret_status = error,
                        }
                    }
                });
            });
            if !open {
                self.secret_dialog_open = false;
            }
        }
        changed
    }

    fn save_secret(&mut self) -> Result<(), String> {
        let key = self.secret_key.trim().to_owned();
        if key.is_empty() || self.secret_value.is_empty() {
            return Err("名称和内容都要填写".into());
        }
        let store = self.secret_store.as_ref().ok_or("秘密存储不可用")?;
        let backend = store
            .set(&key, &self.secret_value)
            .map_err(|error| error.to_string())?;
        if !self.secret_keys.contains(&key) {
            self.secret_keys.push(key.clone());
        }
        self.secret_keys.sort();
        self.persist_secret_index();
        self.secret_value.clear();
        self.secret_status = match backend {
            SecretBackend::Keyring => "已安全保存到系统 Keyring".into(),
            SecretBackend::PlaintextFallback => "Keyring 不可用，已保存到权限受限的回退存储".into(),
            SecretBackend::Unavailable => "秘密存储不可用".into(),
        };
        Ok(())
    }

    fn persist_secret_index(&self) {
        if let Some(path) = &self.secret_index_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(value) = serde_json::to_vec(&self.secret_keys) {
                let _ = std::fs::write(path, value);
            }
        }
    }

    fn capsule_navigation(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let mut list = child(ui, rect.shrink(5.0), "capsule-navigation");
        egui::ScrollArea::vertical()
            .id_salt("capsule-sections")
            .auto_shrink([false; 2])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(&mut list, |ui| {
                for section in CapsuleSection::ALL {
                    if menu_row(
                        ui,
                        ("capsule-section", section),
                        section.label(),
                        self.capsule_section == section,
                        None,
                    ) {
                        self.capsule_section = section;
                    }
                }
            });
    }

    fn plugins(&mut self, ui: &mut egui::Ui, rect: Rect, prefs: &CompactPreferences) {
        if !prefs
            .pages
            .iter()
            .any(|p| Some(&p.descriptor.id) == self.selected.as_ref())
        {
            self.selected = prefs.pages.first().map(|p| p.descriptor.id.clone());
        }
        let mut list = child(ui, rect.shrink(5.0), "settings-plugin-navigation");
        list.add_space(4.0);
        list.label(egui::RichText::new("扩展").size(11.0).color(SECONDARY_TEXT));
        list.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("plugin-names")
            .auto_shrink([false; 2])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(&mut list, |ui| {
                for page in &prefs.pages {
                    if menu_row(
                        ui,
                        &page.descriptor.id,
                        &page.descriptor.name,
                        self.selected.as_ref() == Some(&page.descriptor.id),
                        Some(page.enabled),
                    ) {
                        self.selected = Some(page.descriptor.id.clone());
                    }
                }
            });
    }

    fn extension_form(&mut self, ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
        let Some(id) = self.selected.as_ref() else {
            forms::empty(ui, "没有已注册的扩展");
            return false;
        };
        let Some(page) = prefs.pages.iter_mut().find(|p| &p.descriptor.id == id) else {
            return false;
        };
        forms::section(ui, "系统管理");
        let mut changed = forms::boolean(ui, "启用扩展", &mut page.enabled);
        forms::value(ui, "当前状态", &page.status);
        forms::section(ui, "Doctor 自检");
        forms::doctor(ui, &page.doctor);
        forms::section(ui, "插件设置");
        if page.descriptor.settings.is_empty() {
            forms::value(ui, "配置", "无需额外配置");
        }
        let extension_id = page.descriptor.id.clone();
        for field in &page.descriptor.settings {
            if let Some(value) = page.values.get_mut(&field.key) {
                ui.push_id((id, &field.key), |ui| {
                    if field.search.is_some() {
                        let state = self
                            .searches
                            .entry((extension_id.clone(), field.key.clone()))
                            .or_default();
                        changed |= forms::search_setting(ui, field, value, state);
                    } else {
                        changed |= forms::setting_control(ui, field, value);
                    }
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
        if !compact.is_empty() {
            forms::section(ui, "Compact 显示");
        }
        for (id, name) in compact {
            if let Some(item) = prefs.compact_items.iter_mut().find(|i| i.id == id) {
                ui.push_id(id, |ui| {
                    changed |= forms::boolean(ui, &name, &mut item.enabled);
                });
            }
        }
        changed
    }

    pub(in crate::app) fn take_due_searches(
        &mut self,
        prefs: &CompactPreferences,
        now: Instant,
    ) -> Vec<forms::SearchRequest> {
        let mut requests = Vec::new();
        for ((extension_id, setting_key), state) in &mut self.searches {
            let Some(page) = prefs
                .pages
                .iter()
                .find(|page| page.descriptor.id == *extension_id)
            else {
                continue;
            };
            let Some(search) = page
                .descriptor
                .settings
                .iter()
                .find(|field| field.key == *setting_key)
                .and_then(|field| field.search.as_ref())
            else {
                continue;
            };
            let Some(started) = state.pending_since else {
                continue;
            };
            if now.saturating_duration_since(started) < forms::SEARCH_DEBOUNCE
                || state.submitted_query.as_deref() == Some(state.query.as_str())
            {
                continue;
            }
            state.pending_since = None;
            state.error = None;
            state.results.clear();
            if state.query.trim().chars().count() < search.min_query_chars {
                state.searching = false;
                continue;
            }
            state.searching = true;
            state.submitted_query = Some(state.query.clone());
            requests.push(forms::SearchRequest {
                extension_id: extension_id.clone(),
                setting_key: setting_key.clone(),
                query: state.query.clone(),
            });
        }
        requests
    }

    pub(in crate::app) fn apply_search_result(
        &mut self,
        extension_id: &ExtensionId,
        setting_key: &str,
        query: &str,
        result: Result<Vec<crate::extension::SearchOption>, String>,
    ) {
        let Some(state) = self
            .searches
            .get_mut(&(extension_id.clone(), setting_key.to_owned()))
        else {
            return;
        };
        if state.query != query {
            return;
        }
        state.searching = false;
        state.pending_since = None;
        match result {
            Ok(results) => {
                state.error = None;
                state.results = results;
            }
            Err(error) => {
                state.results.clear();
                state.error = Some(error);
            }
        }
    }

    pub(in crate::app) fn has_searches_in_flight(&self) -> bool {
        self.searches.values().any(|state| state.searching)
    }

    pub(in crate::app) fn next_search_wake(&self) -> Option<Instant> {
        self.searches
            .values()
            .filter_map(|state| {
                state
                    .pending_since
                    .map(|started| started + forms::SEARCH_DEBOUNCE)
                    .or_else(|| {
                        state
                            .searching
                            .then(|| Instant::now() + std::time::Duration::from_millis(100))
                    })
            })
            .min()
    }

    fn quiet(&mut self, ui: &mut egui::Ui, rect: Rect, prefs: &mut CompactPreferences) -> bool {
        let mut changed = forms::boolean(ui, "开启免打扰", &mut prefs.quiet_hours.manual_enabled);
        forms::section(ui, "定时免打扰");
        let quiet = Rect::from_min_max(egui::pos2(rect.left(), ui.cursor().top() + 5.0), rect.max);
        let mut list = child(ui, quiet, "quiet-schedule-list");
        egui::ScrollArea::vertical()
            .id_salt("quiet-schedules")
            .auto_shrink([false; 2])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(&mut list, |ui| {
                let height = 28.0 * prefs.quiet_hours.periods().len().max(1) as f32 + 32.0;
                let (body, _) = ui
                    .allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
                changed |=
                    quiet_view::show(ui, body, &mut prefs.quiet_hours, &mut self.quiet_dialog);
            });
        if rect.height() >= quiet_view::DIALOG_MIN_HEIGHT {
            changed |=
                quiet_view::show_editor(ui, rect, &mut prefs.quiet_hours, &mut self.quiet_dialog);
        }
        changed
    }
}

fn general(ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
    let mut changed = forms::row(ui, "主题", |ui| {
        let mut changed = false;
        egui::ComboBox::from_id_salt("global-theme")
            .width(ui.available_width())
            .selected_text(theme_label(prefs.widget_theme))
            .show_ui(ui, |ui| {
                for theme in [WidgetTheme::System, WidgetTheme::Dark, WidgetTheme::Light] {
                    changed |= ui
                        .selectable_value(&mut prefs.widget_theme, theme, theme_label(theme))
                        .changed();
                }
            });
        changed
    });
    changed |= forms::row(ui, "强调色", |ui| {
        let mut changed = false;
        let colors = super::theme::ACCENT_PRESETS;
        let selected = colors
            .iter()
            .find(|(_, color)| *color == prefs.widget_accent)
            .map_or("蓝色", |(name, _)| *name);
        egui::ComboBox::from_id_salt("accent-preset")
            .width(ui.available_width())
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (name, color) in colors {
                    let label = egui::RichText::new(name)
                        .color(Color32::from_rgb(color[0], color[1], color[2]));
                    changed |= ui
                        .selectable_value(&mut prefs.widget_accent, color, label)
                        .changed();
                }
            });
        changed
    });
    changed
}
fn theme_label(theme: WidgetTheme) -> &'static str {
    match theme {
        WidgetTheme::System => "跟随系统",
        WidgetTheme::Dark => "深色",
        WidgetTheme::Light => "浅色",
    }
}

fn capsule_options(
    ui: &mut egui::Ui,
    prefs: &mut CompactPreferences,
    section: CapsuleSection,
) -> bool {
    let mut changed = false;
    if section == CapsuleSection::Motion {
        changed |= forms::row(ui, "全局动态效果", |ui| {
            let before = (prefs.reduced_motion, prefs.motion_off);
            let mut motion = if prefs.motion_off {
                "Off"
            } else if prefs.reduced_motion {
                "Reduced"
            } else {
                "Full"
            };
            motion_picker(ui, "global-motion", &mut motion);
            prefs.reduced_motion = motion == "Reduced";
            prefs.motion_off = motion == "Off";
            before != (prefs.reduced_motion, prefs.motion_off)
        });
        return changed;
    }
    if section == CapsuleSection::Mascot {
        changed |= forms::boolean(ui, "显示 Mascot", &mut prefs.mascot_enabled);
        changed |= forms::row(ui, "Mascot 大小", |ui| {
            ui.add(
                egui::DragValue::new(&mut prefs.mascot_size)
                    .range(24..=64)
                    .suffix(" px"),
            )
            .changed()
        });
        changed |= forms::row(ui, "Mascot 动画", |ui| {
            let before = prefs.mascot_motion.clone();
            let mut value = prefs.mascot_motion.as_str();
            motion_picker(ui, "mascot-motion", &mut value);
            let value = value.to_owned();
            prefs.mascot_motion = value;
            prefs.mascot_motion != before
        });
        return changed;
    }
    if section == CapsuleSection::Compact {
        let order: Vec<_> = prefs
            .ordered_compact_items()
            .into_iter()
            .map(|i| i.id.clone())
            .collect();
        for id in order {
            let name = prefs
                .pages
                .iter()
                .flat_map(|p| &p.descriptor.contributions)
                .find(|c| c.id == id)
                .map_or_else(|| id.to_string(), |c| c.title.clone());
            let mut direction = 0;
            ui.push_id(&id, |ui| {
                forms::row(ui, &name, |ui| {
                    direction = forms::reorder(ui);
                    false
                });
            });
            if direction != 0 {
                changed |= prefs.move_compact_item(&id, direction as isize);
            }
        }
        return changed;
    }
    forms::section(ui, "通知中心固定第一位");
    for (index, id) in prefs.widget_order.clone().iter().enumerate() {
        let name = prefs
            .pages
            .iter()
            .flat_map(|p| &p.descriptor.contributions)
            .find(|c| &c.id == id)
            .map_or_else(|| id.to_string(), |c| c.title.clone());
        let mut direction = 0;
        ui.push_id(id, |ui| {
            forms::row(ui, &name, |ui| {
                direction = forms::reorder(ui);
                false
            });
        });
        let next = index
            .saturating_add_signed(direction as isize)
            .min(prefs.widget_order.len().saturating_sub(1));
        if next != index {
            prefs.widget_order.swap(index, next);
            changed = true;
        }
    }
    changed
}
fn motion_picker(ui: &mut egui::Ui, id: &str, value: &mut &str) {
    let label = |mode| match mode {
        "Reduced" => "减少",
        "Off" => "关闭",
        _ => "完整",
    };
    egui::ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .selected_text(label(*value))
        .show_ui(ui, |ui| {
            for mode in ["Full", "Reduced", "Off"] {
                ui.selectable_value(value, mode, label(mode));
            }
        });
}
fn about(ui: &mut egui::Ui) {
    forms::value(ui, "版本", env!("CARGO_PKG_VERSION"));
    forms::row(ui, "项目地址", |ui| {
        ui.hyperlink_to("GitHub ↗", "https://github.com/kevinhuang001/peninsula");
        false
    });
    forms::value(ui, "许可证和更新", "查看项目仓库");
}

fn menu_row(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    name: &str,
    selected: bool,
    enabled: Option<bool>,
) -> bool {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::hover());
    let response = ui.interact(rect, ui.id().with(id), Sense::click());
    ui.painter().rect_filled(
        rect,
        8.0,
        if selected {
            Color32::from_rgb(42, 46, 54)
        } else {
            NAV_BACKGROUND
        },
    );
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(12.0),
        if selected { TEXT } else { SECONDARY_TEXT },
    );
    if let Some(enabled) = enabled {
        ui.painter().circle_filled(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            2.5,
            if enabled {
                Color32::from_rgb(97, 222, 174)
            } else {
                Color32::from_rgb(118, 126, 138)
            },
        );
    }
    ui.add_space(4.0);
    response.clicked()
}
fn panel(ui: &egui::Ui, rect: Rect, fill: Color32) {
    ui.painter().rect_filled(rect, 10.0, fill);
    ui.painter().rect_stroke(
        rect,
        10.0,
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );
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
fn configure(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = ACCENT_SELECTED;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(44, 49, 59);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.hovered.bg_fill = ACCENT_SELECTED;
    style.visuals.widgets.hovered.fg_stroke.color = TEXT;
    style.visuals.widgets.active.fg_stroke.color = TEXT;
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.interact_size.y = 24.0;
}

#[cfg(test)]
mod tests;
