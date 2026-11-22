use super::{
    forms,
    model::Section,
    quiet_view::{self, QuietDialog},
    theme::{WidgetTheme, ACCENT, ACCENT_SELECTED, BORDER, PANEL, ROW, SECONDARY_TEXT, TEXT},
    CompactPreferences,
};
use crate::extension::{ContributionDescriptor, ContributionKind, ExtensionId};
use crate::sdk::secrets::SecretStore;
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke};
use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

const NAV_BACKGROUND: Color32 = Color32::from_rgb(23, 25, 30);
const SIDEBAR_WIDTH: f32 = 104.0;
const PLUGIN_WIDTH: f32 = 134.0;
const EXTENSION_FORM_WIDTH: f32 = 360.0;
const EXTENSION_FORM_HEIGHT: f32 = 380.0;
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
            Self::Compact => "Compact",
            Self::Widgets => "Widget",
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
    encryption_dialog: Option<bool>,
    encryption_password: String,
    encryption_confirmation: String,
    encryption_task: Option<Receiver<Result<(), String>>>,
    encryption_in_progress: bool,
    encryption_error_at: Option<Instant>,
    encryption_failed: bool,
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
        let content_width = if matches!(self.section, Section::Extensions | Section::Capsule) {
            EXTENSION_FORM_WIDTH
        } else if self.section == Section::Secrets {
            360.0
        } else if self.section == Section::QuietHours {
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
                secret_keys: self.secret_keys.clone(),
                secret_store: self.secret_store.clone(),
                encryption_dialog: self.encryption_dialog,
                ..Default::default()
            };
            preview.content(&mut measure, &mut prefs.clone());
            measure.min_rect().width()
        };
        // Leave room for the shared title bar; larger forms follow their content.
        (chrome + content_width).ceil().max(380.0).min(limit)
    }

    pub fn extra_height(&self, context: &egui::Context, prefs: &CompactPreferences) -> f32 {
        if self.section == Section::Extensions {
            return EXTENSION_FORM_HEIGHT + 21.0;
        }
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
            secret_keys: self.secret_keys.clone(),
            secret_store: self.secret_store.clone(),
            encryption_dialog: self.encryption_dialog,
            ..Default::default()
        };
        let content_height = if self.section == Section::QuietHours {
            106.0 + 28.0 * prefs.quiet_hours.periods().len().max(1) as f32
        } else {
            preview.content(&mut measure, &mut prefs.clone());
            measure.min_rect().height().max(
                if self.section == Section::Secrets && self.secret_dialog_open {
                    320.0
                } else {
                    0.0
                },
            )
        };
        let navigation_height = Section::ALL.len() as f32 * 36.0 + 10.0;
        content_height.max(navigation_height) + 21.0
    }

    fn content(&mut self, ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
        match self.section {
            Section::General => self.general(ui, prefs),
            Section::Capsule => capsule_options(ui, prefs, self.capsule_section),
            Section::Extensions => self.extension_form(ui, prefs),
            Section::Notifications => notification_options(ui, prefs),
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
        self.poll_encryption();
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
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(&mut content, |ui| {
                ui.set_width(ui.available_width());
                changed |= self.content(ui, prefs);
            });
        if self.section == Section::Secrets && self.secret_dialog_open {
            changed |= self.secret_editor(&mut surface, panels.form.shrink(PANEL_INSET));
        }
        if self.section == Section::General && self.encryption_dialog.is_some() {
            self.encryption_editor(&mut surface, panels.form.shrink(PANEL_INSET));
        }
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
        let content_width = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(content_width, 30.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                if ui
                    .add(egui::Button::new("＋  新增 Secret").min_size(egui::vec2(112.0, 28.0)))
                    .clicked()
                {
                    self.secret_key.clear();
                    self.secret_value.clear();
                    self.secret_status.clear();
                    self.secret_revealed = false;
                    self.secret_dialog_open = true;
                }
            },
        );
        ui.add_space(8.0);
        let mut changed = false;
        if self.secret_keys.is_empty() {
            egui::Frame::new()
                .fill(ROW)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .corner_radius(9.0)
                .inner_margin(egui::Margin::symmetric(16, 22))
                .show(ui, |ui| {
                    ui.set_width(content_width - 32.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("◇").size(26.0).color(ACCENT));
                        ui.label(
                            egui::RichText::new("还没有保存的 Secret")
                                .size(12.5)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            egui::RichText::new("添加 API 密钥、令牌或其他私密内容")
                                .size(10.5)
                                .color(SECONDARY_TEXT),
                        );
                    });
                });
        }
        let mut remove = None;
        for key in self.secret_keys.clone() {
            egui::Frame::new()
                .fill(ROW)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .corner_radius(9.0)
                .inner_margin(egui::Margin::symmetric(11, 8))
                .show(ui, |ui| {
                    ui.set_width(content_width - 22.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&key).size(11.5).strong().color(TEXT),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&key);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("删除").clicked() {
                                remove = Some(key.clone());
                            }
                            if ui.small_button("编辑").clicked() {
                                self.secret_key = key.clone();
                                self.secret_value.clear();
                                self.secret_status.clear();
                                self.secret_revealed = false;
                                self.secret_dialog_open = true;
                            }
                        });
                    });
                });
            ui.add_space(5.0);
        }
        if let (Some(key), Some(store)) = (remove, &self.secret_store) {
            match store.delete(&key) {
                Ok(()) => {
                    self.secret_keys.retain(|entry| entry != &key);
                    self.persist_secret_index();
                    self.secret_status.clear();
                    changed = true;
                }
                Err(error) => self.secret_status = error.to_string(),
            }
        }
        changed
    }

    fn general(&mut self, ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
        let Some(store) = self.secret_store.clone() else {
            return general(ui, prefs);
        };
        let mut changed = general_theme(ui, prefs);
        let encrypted = store.is_encrypted();
        let mut enabled = encrypted;
        if forms::boolean(ui, "加密保存 Secret", &mut enabled) {
            self.encryption_dialog = Some(enabled);
            self.encryption_password.clear();
            self.encryption_confirmation.clear();
            self.encryption_error_at = None;
            self.encryption_failed = false;
        } else {
            changed |= general_accent(ui, prefs);
        }
        changed
    }

    fn encryption_editor(&mut self, ui: &mut egui::Ui, panel: Rect) {
        let Some(enable) = self.encryption_dialog else {
            return;
        };
        let Some(store) = self.secret_store.clone() else {
            return;
        };
        let mut card = Rect::from_center_size(
            panel.center(),
            egui::vec2(panel.width().min(360.0), panel.height().min(190.0)),
        );
        let error_progress = self.encryption_error_at.map(|start| {
            (Instant::now().duration_since(start).as_secs_f32() / 0.6).clamp(0.0, 1.0)
        });
        if let Some(progress) = error_progress {
            if progress >= 1.0 {
                self.encryption_error_at = None;
            } else {
                let elapsed = Instant::now()
                    .duration_since(self.encryption_error_at.unwrap())
                    .as_secs_f32();
                card = card.translate(egui::vec2(
                    (1.0 - progress) * 9.0 * (elapsed * 42.0).sin(),
                    0.0,
                ));
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
        }
        let error_active = self.encryption_failed;
        let _ = ui.interact(
            panel,
            egui::Id::new("encryption-overlay-blocker"),
            Sense::click(),
        );
        ui.painter().rect_filled(panel, 9.0, PANEL);
        ui.painter().rect_filled(card, 12.0, ROW);
        ui.painter().rect_stroke(
            card,
            12.0,
            Stroke::new(1.0_f32, BORDER),
            egui::StrokeKind::Inside,
        );
        let mut editor = child(ui, card.shrink(14.0), "secret-encryption-editor");
        editor.visuals_mut().extreme_bg_color = ROW;
        editor.label(
            egui::RichText::new(if enable {
                "设置加密密码"
            } else {
                "关闭 Secret 加密"
            })
            .size(15.0)
            .strong()
            .color(TEXT),
        );
        editor.add_space(8.0);
        let input_stroke = if error_active {
            Stroke::new(2.5, Color32::from_rgb(194, 103, 109))
        } else {
            Stroke::NONE
        };
        let password_response = editor.add_sized(
            [editor.available_width(), 28.0],
            egui::TextEdit::singleline(&mut self.encryption_password)
                .password(true)
                .hint_text(if enable { "密码" } else { "当前密码" }),
        );
        if password_response.changed() {
            self.encryption_failed = false;
            self.encryption_error_at = None;
        }
        if error_active {
            editor.painter().rect_stroke(
                password_response.rect.expand(1.5),
                7.0,
                input_stroke,
                egui::StrokeKind::Inside,
            );
        }
        if enable {
            editor.add_space(7.0);
            let confirmation_response = editor.add_sized(
                [editor.available_width(), 28.0],
                egui::TextEdit::singleline(&mut self.encryption_confirmation)
                    .password(true)
                    .hint_text("确认密码"),
            );
            if confirmation_response.changed() {
                self.encryption_failed = false;
                self.encryption_error_at = None;
            }
            if error_active {
                editor.painter().rect_stroke(
                    confirmation_response.rect.expand(1.5),
                    7.0,
                    input_stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }
        editor.add_space(9.0);
        editor.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let confirm = ui.add_enabled(
                !self.encryption_in_progress,
                egui::Button::new(if self.encryption_in_progress {
                    ""
                } else if enable {
                    "确认"
                } else {
                    "解密"
                })
                .min_size(egui::vec2(52.0, 26.0)),
            );
            if self.encryption_in_progress {
                let mut spinner = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("encryption-spinner")
                        .max_rect(confirm.rect),
                );
                spinner.centered_and_justified(|ui| {
                    ui.add(egui::Spinner::new().size(15.0));
                });
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
            if confirm.clicked() {
                if enable && self.encryption_password != self.encryption_confirmation {
                    self.encryption_failed = true;
                    self.encryption_error_at = Some(Instant::now());
                } else {
                    self.encryption_failed = false;
                    self.encryption_error_at = None;
                    let password = self.encryption_password.clone();
                    let store = store.clone();
                    let (sender, receiver) = mpsc::channel();
                    self.encryption_task = Some(receiver);
                    self.encryption_in_progress = true;
                    std::thread::spawn(move || {
                        let result = if enable {
                            store.enable_encryption(&password)
                        } else {
                            store.disable_encryption(&password)
                        }
                        .map_err(|error| error.to_string());
                        let _ = sender.send(result);
                    });
                }
            }
            if ui
                .add_enabled(!self.encryption_in_progress, egui::Button::new("取消"))
                .clicked()
            {
                self.encryption_dialog = None;
                self.encryption_password.clear();
                self.encryption_confirmation.clear();
                self.encryption_error_at = None;
                self.encryption_failed = false;
            }
        });
    }

    fn poll_encryption(&mut self) {
        let Some(receiver) = self.encryption_task.as_ref() else {
            return;
        };
        let result = receiver.try_recv();
        match result {
            Ok(Ok(())) => {
                self.encryption_task = None;
                self.encryption_in_progress = false;
                self.encryption_dialog = None;
                self.encryption_password.clear();
                self.encryption_confirmation.clear();
                self.encryption_error_at = None;
                self.encryption_failed = false;
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.encryption_task = None;
                self.encryption_in_progress = false;
                self.encryption_error_at = Some(Instant::now());
                self.encryption_failed = true;
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn secret_editor(&mut self, ui: &mut egui::Ui, panel: Rect) -> bool {
        let mut changed = false;
        let dialog = Rect::from_center_size(
            panel.center(),
            egui::vec2(panel.width().min(410.0), panel.height().min(310.0)),
        );
        ui.painter().rect_filled(panel, 9.0, PANEL);
        ui.painter().rect_filled(dialog, 12.0, PANEL);
        ui.painter().rect_stroke(
            dialog,
            12.0,
            Stroke::new(1.0_f32, BORDER),
            egui::StrokeKind::Inside,
        );
        let mut editor = child(ui, dialog.shrink(16.0), "secret-editor");
        editor.visuals_mut().extreme_bg_color = ROW;
        let ui = &mut editor;
        {
            let editing = self.secret_keys.contains(&self.secret_key);
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(if editing {
                        "编辑 Secret"
                    } else {
                        "新增 Secret"
                    })
                    .size(17.0)
                    .strong()
                    .color(TEXT),
                );
                ui.add_space((ui.available_width() - 30.0).max(0.0));
                if ui.add(egui::Button::new("×").fill(PANEL)).clicked() {
                    self.secret_dialog_open = false;
                }
            });
            ui.add_space(8.0);
            ui.label(egui::RichText::new("名称").size(11.0).strong());
            ui.add_sized(
                [ui.available_width(), 28.0],
                egui::TextEdit::singleline(&mut self.secret_key).hint_text("例如 OPENAI_API_KEY"),
            );
            ui.add_space(10.0);
            ui.label(egui::RichText::new("私密内容").size(11.0).strong());
            ui.add_sized(
                [ui.available_width(), 28.0],
                egui::TextEdit::singleline(&mut self.secret_value)
                    .password(!self.secret_revealed)
                    .hint_text(if editing {
                        "输入新内容以替换"
                    } else {
                        "输入密钥或令牌"
                    }),
            );
            ui.add_space(3.0);
            ui.checkbox(&mut self.secret_revealed, "显示内容");
            if !self.secret_status.is_empty() {
                ui.colored_label(ui.visuals().error_fg_color, &self.secret_status);
            }
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 30.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(
                            egui::Button::new("保存 Secret")
                                .min_size(egui::vec2(100.0, 28.0))
                                .fill(ACCENT_SELECTED),
                        )
                        .clicked()
                    {
                        match self.save_secret() {
                            Ok(()) => {
                                changed = true;
                                self.secret_dialog_open = false;
                            }
                            Err(error) => self.secret_status = error,
                        }
                    }
                    if ui.add(egui::Button::new("取消").fill(ROW)).clicked() {
                        self.secret_dialog_open = false;
                    }
                },
            );
        }
        changed
    }

    fn save_secret(&mut self) -> Result<(), String> {
        let key = self.secret_key.trim().to_owned();
        if key.is_empty() || self.secret_value.is_empty() {
            return Err("名称和内容都要填写".into());
        }
        let store = self.secret_store.as_ref().ok_or("秘密存储不可用")?;
        store
            .set(&key, &self.secret_value)
            .map_err(|error| error.to_string())?;
        if !self.secret_keys.contains(&key) {
            self.secret_keys.push(key.clone());
        }
        self.secret_keys.sort();
        self.persist_secret_index();
        self.secret_value.clear();
        self.secret_status.clear();
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
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
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
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
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
        forms::section(ui, "Doctor 自检");
        forms::doctor(ui, &page.doctor);
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
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
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

fn contribution_groups(
    prefs: &CompactPreferences,
    kind: ContributionKind,
) -> Vec<(String, Vec<ContributionDescriptor>)> {
    prefs
        .pages
        .iter()
        .filter_map(|page| {
            let entries: Vec<_> = page
                .descriptor
                .contributions
                .iter()
                .filter(|entry| entry.kind == kind)
                .cloned()
                .collect();
            (!entries.is_empty()).then(|| (page.descriptor.name.clone(), entries))
        })
        .collect()
}

fn notification_options(ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
    let mut changed = false;
    for (name, entries) in contribution_groups(prefs, ContributionKind::Notification) {
        forms::section(ui, &name);
        for entry in entries {
            ui.push_id(&entry.id, |ui| {
                let mut enabled = prefs.contribution_enabled(&entry);
                if forms::boolean(ui, &entry.title, &mut enabled) {
                    prefs.set_contribution_enabled(entry.id.clone(), enabled);
                    changed = true;
                }
                if let Some(values) = prefs.contribution_values.get_mut(&entry.id) {
                    for field in &entry.settings {
                        if let Some(value) = values.get_mut(&field.key) {
                            ui.push_id(&field.key, |ui| {
                                changed |= forms::setting_control(ui, field, value);
                            });
                        }
                    }
                }
            });
        }
    }
    changed
}

fn general(ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
    let mut changed = general_theme(ui, prefs);
    changed |= general_accent(ui, prefs);
    changed
}

fn general_theme(ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
    forms::row(ui, "主题", |ui| {
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
    })
}

fn general_accent(ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
    forms::row(ui, "强调色", |ui| {
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
    })
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
        forms::section(ui, "Compact 文字样式");
        changed |= forms::row(ui, "字号", |ui| {
            ui.add(
                egui::DragValue::new(&mut prefs.compact_text_size)
                    .speed(0.25)
                    .range(8.0..=18.0)
                    .suffix(" pt"),
            )
            .changed()
        });
        changed |= forms::row(ui, "文字颜色", |ui| {
            let mut follow_theme = prefs.compact_text_color.is_none();
            let mut changed = forms::toggle(ui, &mut follow_theme, "跟随主题");
            if changed {
                prefs.compact_text_color = (!follow_theme).then_some([245, 248, 255]);
            }
            if let Some(color) = &mut prefs.compact_text_color {
                changed |= ui
                    .color_edit_button_srgb(color)
                    .on_hover_text("显示时会自动调整颜色，确保文字与背景有足够对比度")
                    .changed();
            }
            changed
        });
        let order: Vec<_> = prefs
            .ordered_compact_items()
            .into_iter()
            .map(|item| item.id.clone())
            .collect();
        for (group, mut entries) in contribution_groups(prefs, ContributionKind::Compact) {
            forms::section(ui, &group);
            entries.sort_by_key(|entry| order.iter().position(|id| id == &entry.id));
            for (index, entry) in entries.iter().enumerate() {
                let mut direction = 0;
                let mut enabled = prefs.is_enabled(&entry.id);
                ui.push_id(&entry.id, |ui| {
                    if forms::row(ui, &entry.title, |ui| {
                        let toggled = forms::toggle(ui, &mut enabled, &entry.title);
                        direction = forms::reorder(ui);
                        toggled
                    }) {
                        changed |= prefs.set_compact_item_enabled(&entry.id, enabled);
                    }
                });
                if let Some(target) = index
                    .checked_add_signed(direction as isize)
                    .and_then(|index| entries.get(index))
                {
                    if let (Some(from), Some(to)) = (
                        order.iter().position(|id| id == &entry.id),
                        order.iter().position(|id| id == &target.id),
                    ) {
                        changed |= prefs.move_compact_item(&entry.id, to as isize - from as isize);
                    }
                }
            }
        }
        return changed;
    }
    forms::section(ui, "通知中心固定第一位");
    let order = prefs.widget_order.clone();
    for (group, mut entries) in contribution_groups(prefs, ContributionKind::Widget) {
        forms::section(ui, &group);
        entries.sort_by_key(|entry| order.iter().position(|id| id == &entry.id));
        for (index, entry) in entries.iter().enumerate() {
            let mut direction = 0;
            let mut enabled = prefs.contribution_enabled(entry);
            ui.push_id(&entry.id, |ui| {
                if forms::row(ui, &entry.title, |ui| {
                    let toggled = forms::toggle(ui, &mut enabled, &entry.title);
                    direction = forms::reorder(ui);
                    toggled
                }) {
                    prefs.set_contribution_enabled(entry.id.clone(), enabled);
                    changed = true;
                }
            });
            if let Some(target) = index
                .checked_add_signed(direction as isize)
                .and_then(|index| entries.get(index))
            {
                if let (Some(from), Some(to)) = (
                    order.iter().position(|id| id == &entry.id),
                    order.iter().position(|id| id == &target.id),
                ) {
                    if from != to {
                        prefs.widget_order.swap(from, to);
                        changed = true;
                    }
                }
            }
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
