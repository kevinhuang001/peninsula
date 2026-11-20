use crate::{
    app::settings::CompactPreferences,
    core::{
        datasource::{self, DataSourceManager, Definition, InputType, SourceSnapshot},
        notifications::NotificationManager,
    },
    sdk::secrets::SecretStore,
};
use eframe::egui::{self, Align, Layout, Vec2};
use serde_json::Value;
use std::{collections::HashMap, fs};

const VIEWPORT_ID: &str = "peninsula-datasources";
const OPEN_REQUEST_ID: &str = "datasource-open-request";
const LEFT_WIDTH: f32 = 254.0;

#[derive(Default)]
pub struct DataSourceWindow {
    open: bool,
    selected: Option<String>,
    search: String,
    candidate: Option<Definition>,
    allow_private_network: bool,
    allow_plaintext_secrets: bool,
    status: String,
    delete_candidate: Option<String>,
    delete_secrets: bool,
    delete_history: bool,
    text_drafts: HashMap<String, String>,
    secret_drafts: HashMap<String, String>,
    secret_ref_drafts: HashMap<String, String>,
    secret_status: HashMap<String, Result<bool, String>>,
}

impl DataSourceWindow {
    pub fn show(
        &mut self,
        context: &egui::Context,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
        secrets: &SecretStore,
        notifications: &mut NotificationManager,
    ) {
        let requested = context
            .data_mut(|data| data.remove_temp::<bool>(egui::Id::new(OPEN_REQUEST_ID)))
            .unwrap_or(false);
        if requested {
            self.open = true;
        }
        if !self.open {
            return;
        }
        let builder = egui::ViewportBuilder::default()
            .with_title("Peninsula 数据源")
            .with_inner_size([1040.0, 700.0])
            .with_min_inner_size([720.0, 480.0]);
        let close = context.show_viewport_immediate(
            egui::ViewportId::from_hash_of(VIEWPORT_ID),
            builder,
            |context, class| {
                let close_requested = context.input(|input| {
                    (class != egui::ViewportClass::Embedded && input.viewport().close_requested())
                        || input.key_pressed(egui::Key::Escape)
                });
                if close_requested {
                    return true;
                }
                if class == egui::ViewportClass::Embedded {
                    let mut open = true;
                    egui::Window::new("Peninsula 数据源")
                        .id(egui::Id::new("datasource-window-embedded"))
                        .open(&mut open)
                        .collapsible(false)
                        .default_size(Vec2::new(900.0, 640.0))
                        .show(context, |ui| {
                            self.content(ui, manager, preferences, secrets, notifications);
                        });
                    !open
                } else {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::central_panel(&context.style()).inner_margin(16.0))
                        .show(context, |ui| {
                            self.content(ui, manager, preferences, secrets, notifications);
                        });
                    false
                }
            },
        );
        if close {
            self.open = false;
        }
    }

    fn content(
        &mut self,
        ui: &mut egui::Ui,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
        secrets: &SecretStore,
        notifications: &mut NotificationManager,
    ) {
        let sources = manager.snapshots();
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !sources.iter().any(|source| &source.id == id))
        {
            self.selected = sources.first().map(|source| source.id.clone());
        }
        ui.horizontal(|ui| {
            ui.heading("数据源");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("＋ 导入 .pds.json").clicked() {
                    self.pick_definition();
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            let height = ui.available_height();
            ui.allocate_ui_with_layout(
                Vec2::new(LEFT_WIDTH, height),
                Layout::top_down(Align::Min),
                |ui| {
                    self.source_list(ui, &sources, manager);
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                Vec2::new((ui.available_width() - 8.0).max(1.0), height),
                Layout::top_down(Align::Min),
                |ui| {
                    if let Some(selected) = self
                        .selected
                        .as_ref()
                        .and_then(|id| sources.iter().find(|source| &source.id == id))
                        .cloned()
                    {
                        self.source_detail(ui, &selected, manager, preferences, secrets);
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(80.0);
                            ui.heading("还没有数据源");
                            ui.label("导入一个 .pds.json 文件以开始。 ");
                        });
                    }
                },
            );
        });
        self.import_dialog(ui.ctx(), manager, preferences, secrets);
        self.delete_dialog(ui.ctx(), manager, preferences, secrets, notifications);
        if !self.status.is_empty() {
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    egui::Id::new("datasource-window-status"),
                    self.status.clone(),
                );
            });
        }
    }

    fn source_list(
        &mut self,
        ui: &mut egui::Ui,
        sources: &[SourceSnapshot],
        manager: &DataSourceManager,
    ) {
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("搜索数据源"));
        ui.add_space(5.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for source in sources.iter().filter(|source| {
                let search = self.search.trim().to_lowercase();
                search.is_empty()
                    || source.name.to_lowercase().contains(&search)
                    || source.id.to_lowercase().contains(&search)
            }) {
                let selected = self.selected.as_deref() == Some(&source.id);
                let response = ui
                    .allocate_response(Vec2::new(ui.available_width(), 56.0), egui::Sense::click());
                if response.clicked() {
                    self.selected = Some(source.id.clone());
                }
                let rect = response.rect;
                if selected || response.hovered() {
                    ui.painter().rect_filled(
                        rect,
                        8.0,
                        ui.visuals().widgets.noninteractive.bg_fill,
                    );
                }
                if let Some(texture) = manager.icon_texture(&source.id) {
                    ui.painter().image(
                        texture.id(),
                        egui::Rect::from_min_size(
                            rect.left_top() + Vec2::new(10.0, 16.0),
                            Vec2::splat(24.0),
                        ),
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                        ui.visuals().text_color(),
                    );
                } else {
                    ui.painter().text(
                        rect.left_top() + Vec2::new(14.0, 19.0),
                        egui::Align2::LEFT_TOP,
                        "◈",
                        egui::FontId::proportional(20.0),
                        ui.visuals().text_color(),
                    );
                }
                ui.painter().text(
                    rect.left_top() + Vec2::new(46.0, 10.0),
                    egui::Align2::LEFT_TOP,
                    &source.name,
                    egui::FontId::proportional(14.0),
                    ui.visuals().text_color(),
                );
                let status = source
                    .formatted
                    .as_deref()
                    .map(|value| format!("{value}  ·  {}", status_text(source)))
                    .unwrap_or_else(|| status_text(source).to_owned());
                ui.painter().text(
                    rect.left_top() + Vec2::new(46.0, 32.0),
                    egui::Align2::LEFT_TOP,
                    status,
                    egui::FontId::proportional(11.0),
                    ui.visuals().weak_text_color(),
                );
            }
        });
    }

    fn source_detail(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
        secrets: &SecretStore,
    ) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(&source.name);
                        ui.label(egui::RichText::new(&source.id).small().weak().monospace());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("删除").clicked() {
                            self.delete_candidate = Some(source.id.clone());
                            self.delete_secrets = false;
                            self.delete_history = false;
                        }
                        let mut enabled = source.enabled;
                        if ui.checkbox(&mut enabled, "启用").changed() {
                            self.set_status(manager.set_enabled(&source.id, enabled));
                        }
                    });
                });
                if !source.description.is_empty() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(&source.description).weak());
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    let mut compact = source.compact;
                    if ui.checkbox(&mut compact, "显示在 Compact").changed() {
                        self.set_status(manager.set_compact(&source.id, compact));
                        let id = compact_item_id(&source.id);
                        preferences.set_compact_item_enabled(&id, compact);
                        self.save_preferences(preferences);
                    }
                    if source.alert_enabled {
                        let mut alerts = source.alerts;
                        if ui.checkbox(&mut alerts, "启用提醒").changed() {
                            self.set_status(manager.set_alerts(&source.id, alerts));
                        }
                    }
                    if ui
                        .button(if source.in_flight {
                            "请求中…"
                        } else {
                            "立即刷新"
                        })
                        .clicked()
                        && !source.in_flight
                    {
                        self.set_status(manager.request_now(&source.id));
                    }
                });
                if source
                    .input_schema
                    .values()
                    .any(|input| input.kind == InputType::Secret)
                {
                    let mut allow_fallback = source.allow_plaintext_secrets;
                    if ui
                        .checkbox(
                            &mut allow_fallback,
                            "允许系统 Keyring 不可用时使用权限受限的本地 Secret 回退存储",
                        )
                        .changed()
                    {
                        self.set_status(
                            manager.set_plaintext_secret_fallback(&source.id, allow_fallback),
                        );
                    }
                }
            });
        ui.add_space(10.0);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt(("datasource-detail", &source.id))
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.inputs_form(ui, source, manager, secrets);
                ui.add_space(10.0);
                self.status_panel(ui, source, manager, secrets);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label("最后一次成功");
                    ui.label(source.formatted.as_deref().unwrap_or("—"));
                    if let Some(updated) = source.updated_ms {
                        ui.label(
                            egui::RichText::new(format!("· {}", timestamp_text(updated)))
                                .small()
                                .weak(),
                        );
                    }
                });
                if let Some(error) = &source.last_error {
                    ui.colored_label(egui::Color32::from_rgb(239, 132, 112), error);
                }
                if let Some(status) = source.http_status {
                    ui.label(format!("HTTP 状态：{status}"));
                }
                if !self.status.is_empty() {
                    ui.colored_label(ui.visuals().warn_fg_color, &self.status);
                }
            });
    }

    fn inputs_form(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
        secrets: &SecretStore,
    ) {
        ui.heading("参数");
        if source.input_schema.is_empty() {
            ui.label("此数据源没有用户参数。");
            return;
        }
        let columns_count = if ui.available_width() < 700.0 {
            1
        } else if ui.available_width() < 1100.0 {
            2
        } else {
            3
        }
        .min(source.input_schema.len());

        ui.columns(columns_count, |columns| {
            for (index, (key, schema)) in source.input_schema.iter().enumerate() {
                let column = &mut columns[index % columns_count];
                let current = source
                    .inputs
                    .get(key)
                    .cloned()
                    .or_else(|| schema.default.clone())
                    .unwrap_or(Value::Null);
                let draft_id = format!("{}::{key}", source.id);
                let secret_exists = (schema.kind == InputType::Secret).then(|| {
                    self.secret_status
                        .get(&draft_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            let result = manager.secret_exists(secrets, &source.id, key);
                            self.secret_status.insert(draft_id.clone(), result.clone());
                            result
                        })
                });
                let mut input_update = None;
                let mut save_secret = None;
                let mut clear_secret = false;
                let mut set_secret_ref = None;
                let mut invalid_number = false;

                egui::Frame::group(column.style())
                    .inner_margin(egui::Margin::same(12))
                    .show(column, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&schema.label).strong());
                            if schema.required {
                                ui.label(egui::RichText::new("必填").small().weak());
                            }
                        });
                        if !schema.description.is_empty() {
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new(&schema.description).small().weak());
                        }
                        ui.add_space(8.0);

                        match schema.kind {
                            InputType::Secret => {
                                let exists = secret_exists.as_ref().expect("Secret state was read");
                                ui.label(
                                    egui::RichText::new(match exists {
                                        Ok(true) => "已保存到系统凭证存储",
                                        Ok(false) => "尚未填写",
                                        Err(_) => "凭证存储暂不可用",
                                    })
                                    .small()
                                    .weak(),
                                );
                                let draft = self.secret_drafts.entry(draft_id.clone()).or_default();
                                ui.add_sized(
                                    [ui.available_width(), 30.0],
                                    egui::TextEdit::singleline(draft)
                                        .password(true)
                                        .hint_text("输入后保存"),
                                );
                                ui.horizontal_wrapped(|ui| {
                                    if ui.small_button("保存 Secret").clicked() {
                                        save_secret = Some(draft.clone());
                                    }
                                    if matches!(exists, Ok(true))
                                        && ui.small_button("清除").clicked()
                                    {
                                        clear_secret = true;
                                    }
                                });

                                let reference = source.secret_refs.get(key).cloned();
                                let ref_draft = self
                                    .secret_ref_drafts
                                    .entry(draft_id.clone())
                                    .or_insert_with(|| reference.clone().unwrap_or_default());
                                ui.add_sized(
                                    [ui.available_width(), 30.0],
                                    egui::TextEdit::singleline(ref_draft)
                                        .hint_text("已保存的 Secret 名称"),
                                );
                                if reference.as_deref().is_some_and(|value| value == ref_draft) {
                                    if ui.small_button("解除引用").clicked() {
                                        set_secret_ref = Some(None);
                                    }
                                } else if ui.small_button("引用此 Secret").clicked() {
                                    set_secret_ref = Some(Some(ref_draft.trim().to_owned()));
                                }
                            }
                            InputType::Boolean => {
                                let mut value = current.as_bool().unwrap_or(false);
                                if ui.checkbox(&mut value, "开启").changed() {
                                    input_update = Some(Value::Bool(value));
                                }
                            }
                            InputType::Enum => {
                                let mut value = current.as_str().unwrap_or("").to_owned();
                                egui::ComboBox::from_id_salt((&source.id, key))
                                    .selected_text(if value.is_empty() {
                                        "选择…"
                                    } else {
                                        &value
                                    })
                                    .show_ui(ui, |ui| {
                                        for option in &schema.options {
                                            ui.selectable_value(&mut value, option.clone(), option);
                                        }
                                    });
                                if value != current.as_str().unwrap_or("") {
                                    input_update = Some(Value::String(value));
                                }
                            }
                            InputType::String
                            | InputType::Number
                            | InputType::Integer
                            | InputType::Duration => {
                                let initial = match schema.kind {
                                    InputType::String | InputType::Duration => {
                                        current.as_str().map(str::to_owned).unwrap_or_default()
                                    }
                                    _ => current
                                        .as_number()
                                        .map(ToString::to_string)
                                        .unwrap_or_default(),
                                };
                                let draft = self
                                    .text_drafts
                                    .entry(draft_id.clone())
                                    .or_insert_with(|| initial.clone());
                                let response = ui.add_sized(
                                    [ui.available_width(), 30.0],
                                    egui::TextEdit::singleline(draft),
                                );
                                if response.changed() && schema.kind == InputType::String {
                                    input_update = Some(Value::String(draft.clone()));
                                }
                                let commit = response.lost_focus()
                                    || (response.has_focus()
                                        && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                                if commit && draft != &initial && schema.kind != InputType::String {
                                    let parsed = match schema.kind {
                                        InputType::Number => draft
                                            .parse::<f64>()
                                            .ok()
                                            .filter(|value| value.is_finite())
                                            .map(Value::from),
                                        InputType::Integer => {
                                            draft.parse::<i64>().ok().map(Value::from)
                                        }
                                        InputType::Duration
                                            if datasource::parse_duration(draft).is_ok() =>
                                        {
                                            Some(Value::String(draft.clone()))
                                        }
                                        _ => None,
                                    };
                                    if let Some(value) = parsed {
                                        input_update = Some(value);
                                    } else {
                                        invalid_number = true;
                                    }
                                }
                            }
                        }
                    });

                if let Some(value) = input_update {
                    self.set_status(manager.set_input(&source.id, key, value));
                }
                if let Some(value) = save_secret {
                    match manager.set_secret(secrets, &source.id, key, &value) {
                        Ok(()) => {
                            if let Some(draft) = self.secret_drafts.get_mut(&draft_id) {
                                draft.clear();
                            }
                            self.secret_status.insert(draft_id.clone(), Ok(true));
                            self.status = "Secret 已保存到 Peninsula 凭证存储。".into();
                            let _ = manager.request_now(&source.id);
                        }
                        Err(error) => self.status = error,
                    }
                }
                if clear_secret {
                    match manager.delete_secret(secrets, &source.id, key) {
                        Ok(()) => {
                            self.secret_status.insert(draft_id.clone(), Ok(false));
                            self.status = "Secret 已清除。".into();
                        }
                        Err(error) => self.status = error,
                    }
                }
                if let Some(reference) = set_secret_ref {
                    let result = manager.set_secret_reference(
                        secrets,
                        &source.id,
                        key,
                        reference.as_deref(),
                    );
                    let saved = result.is_ok();
                    self.set_status(result);
                    self.secret_status.remove(&draft_id);
                    if saved {
                        let _ = manager.request_now(&source.id);
                    }
                }
                if invalid_number {
                    self.status = format!("{} 的格式或范围无效。", schema.label);
                }
                column.add_space(8.0);
            }
        });
    }

    fn status_panel(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &DataSourceManager,
        secrets: &SecretStore,
    ) {
        ui.heading("Doctor");
        egui::Grid::new(("datasource-doctor", &source.id))
            .num_columns(2)
            .spacing([18.0, 5.0])
            .show(ui, |ui| {
                doctor_row(ui, "Schema", "Passed");
                doctor_row(
                    ui,
                    "Icon",
                    source
                        .icon_warning
                        .as_deref()
                        .unwrap_or("Passed / 使用默认图标"),
                );
                doctor_row(
                    ui,
                    "Inputs",
                    if required_inputs_ready(source) {
                        "Passed"
                    } else {
                        "Warning · 仍需填写"
                    },
                );
                let required_secrets = source
                    .input_schema
                    .iter()
                    .filter(|(_, input)| input.kind == InputType::Secret && input.required)
                    .map(|(key, _)| {
                        let cache_key = format!("{}::{key}", source.id);
                        self.secret_status
                            .get(&cache_key)
                            .cloned()
                            .unwrap_or_else(|| {
                                let value = manager.secret_exists(secrets, &source.id, key);
                                self.secret_status.insert(cache_key, value.clone());
                                value
                            })
                    })
                    .collect::<Vec<_>>();
                let secret_state = required_secrets
                    .iter()
                    .all(|value| matches!(value, Ok(true)));
                doctor_row(
                    ui,
                    "Secret",
                    if secret_state {
                        "Passed"
                    } else {
                        "Warning · 尚未保存"
                    },
                );
                doctor_row(ui, "Permissions", &source.origin);
                let http_status = source
                    .http_status
                    .map_or_else(|| "Unknown".to_owned(), |status| format!("HTTP {status}"));
                doctor_row(ui, "HTTP", &http_status);
                doctor_row(
                    ui,
                    "JSON / Value",
                    if source.value.is_some() {
                        "Passed"
                    } else {
                        "Unknown"
                    },
                );
                doctor_row(
                    ui,
                    "Alert",
                    if !source.alert_enabled {
                        "No rule"
                    } else if source.alerts {
                        "Enabled"
                    } else {
                        "Disabled"
                    },
                );
                doctor_row(
                    ui,
                    "Scheduler",
                    if source.enabled && (source.compact || source.alerts) {
                        "Running"
                    } else {
                        "Paused"
                    },
                );
            });
    }

    fn pick_definition(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            // Native dialogs generally match the final filename extension only. The import
            // handler below still checks the complete `.pds.json` suffix.
            .add_filter("Peninsula 数据源", &["json"])
            .set_title("导入 Peninsula 数据源")
            .pick_file()
        else {
            return;
        };
        if !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".pds.json"))
        {
            self.status = "请选择 .pds.json 文件。".into();
            return;
        }
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                self.status = "数据源文件不存在或不可读。".into();
                return;
            }
        };
        if metadata.len() > 2_000_000 {
            self.status = "数据源文件不存在或超过 2 MB。".into();
            return;
        }
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                self.status = format!("无法读取文件：{error}");
                return;
            }
        };
        match Definition::parse(&source) {
            Ok(definition) => {
                self.allow_private_network = false;
                self.allow_plaintext_secrets = false;
                self.status.clear();
                self.candidate = Some(definition);
            }
            Err(error) => self.status = format!("数据源 Schema 校验失败：{error}"),
        }
    }

    fn import_dialog(
        &mut self,
        context: &egui::Context,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
        secrets: &SecretStore,
    ) {
        let Some(candidate) = self.candidate.clone() else {
            return;
        };
        let origin = candidate.origin().unwrap_or_else(|_| "无效请求目标".into());
        let secret_inputs = candidate.secret_inputs();
        let mut open = true;
        egui::Window::new("授权并导入数据源")
            .id(egui::Id::new("datasource-import"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(context, |ui| {
                ui.set_min_width(430.0);
                ui.label(egui::RichText::new(&candidate.name).heading());
                ui.label(format!("ID：{}", candidate.id));
                ui.label(format!("请求目标：{}", origin));
                ui.label(format!(
                    "{} {}",
                    method_text(candidate.request.method),
                    candidate.request.url
                ));
                ui.add_space(8.0);
                ui.label("导入即授予此数据源向上述 Origin 发起请求的权限。");
                if secret_inputs.is_empty() {
                    ui.label("不需要 Secret。");
                } else {
                    ui.label(format!(
                        "HTTP 请求会读取并发送此数据源自己的 Secret：{}",
                        secret_inputs.join("、")
                    ));
                }
                ui.checkbox(
                    &mut self.allow_private_network,
                    "允许 HTTP 或 localhost / 私有网络请求目标",
                );
                if !secret_inputs.is_empty() {
                    ui.checkbox(
                        &mut self.allow_plaintext_secrets,
                        "若系统 Keyring 不可用，允许 Secret 写入权限受限的本地回退存储",
                    );
                }
                if origin.starts_with("http://") {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "目标使用未加密的 HTTP；只有勾选后才会运行请求。 ",
                    );
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.candidate = None;
                    }
                    if ui.button("授权并导入").clicked() {
                        match manager.import(
                            candidate.clone(),
                            self.allow_private_network,
                            self.allow_plaintext_secrets,
                            secrets,
                        ) {
                            Ok(_) => {
                                let ids = manager.compact_ids();
                                if preferences.sync_data_source_items(&ids) {
                                    self.save_preferences(preferences);
                                }
                                self.selected = Some(candidate.id.clone());
                                self.status = format!("已导入 {}。", candidate.name);
                                self.candidate = None;
                            }
                            Err(error) => self.status = format!("导入失败：{error}"),
                        }
                    }
                });
            });
        if !open {
            self.candidate = None;
        }
    }

    fn delete_dialog(
        &mut self,
        context: &egui::Context,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
        secrets: &SecretStore,
        notifications: &mut NotificationManager,
    ) {
        let Some(id) = self.delete_candidate.clone() else {
            return;
        };
        let name = manager
            .snapshots()
            .into_iter()
            .find(|source| source.id == id)
            .map(|source| source.name)
            .unwrap_or_else(|| id.clone());
        let mut open = true;
        egui::Window::new("删除数据源")
            .id(egui::Id::new("datasource-delete"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(context, |ui| {
                ui.label(format!("确定删除“{name}”吗？"));
                ui.checkbox(&mut self.delete_secrets, "同时清除此数据源的 Secret");
                ui.checkbox(&mut self.delete_history, "同时清除此数据源的通知历史");
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.delete_candidate = None;
                    }
                    if ui.button("删除").clicked() {
                        match manager.delete(&id) {
                            Ok(keys) => {
                                let cleanup_errors = if self.delete_secrets {
                                    keys.into_iter()
                                        .filter_map(|key| {
                                            secrets
                                                .delete(&format!("datasource::{id}::{key}"))
                                                .err()
                                                .map(|error| error.to_string())
                                        })
                                        .collect::<Vec<_>>()
                                } else {
                                    Vec::new()
                                };
                                if self.delete_history {
                                    if let Ok(source) = crate::extension::ExtensionId::new(format!(
                                        "datasource.{id}"
                                    )) {
                                        notifications.remove_source(&source);
                                    }
                                }
                                if let Ok(item_id) = compact_item_id_result(&id) {
                                    preferences.remove_compact_item(&item_id);
                                    self.save_preferences(preferences);
                                }
                                self.selected = None;
                                self.delete_candidate = None;
                                self.status = if cleanup_errors.is_empty() {
                                    format!("已删除数据源“{name}”。")
                                } else {
                                    format!(
                                        "已删除数据源“{name}”，但部分 Secret 未能清除：{}",
                                        cleanup_errors.join("；")
                                    )
                                };
                            }
                            Err(error) => self.status = format!("删除失败：{error}"),
                        }
                    }
                });
            });
        if !open {
            self.delete_candidate = None;
        }
    }

    fn set_status(&mut self, result: Result<(), String>) {
        self.status = result.map_or_else(|error| error, |_| String::new());
    }

    fn save_preferences(&mut self, preferences: &CompactPreferences) {
        if let Err(error) = preferences.save() {
            self.status = format!("无法保存 Compact 设置：{error}");
        }
    }
}

fn compact_item_id(id: &str) -> crate::extension::ExtensionId {
    compact_item_id_result(id).expect("validated data-source IDs create valid Compact IDs")
}

fn compact_item_id_result(id: &str) -> Result<crate::extension::ExtensionId, String> {
    crate::extension::ExtensionId::new(format!("datasource/{id}/value"))
        .map_err(|_| "数据源 ID 无效".to_owned())
}

fn status_text(source: &SourceSnapshot) -> &str {
    if source.in_flight {
        "刷新中"
    } else if source.last_error.is_some() {
        "请求失败"
    } else if source.value.is_some() {
        "已更新"
    } else {
        "等待首次刷新"
    }
}

fn required_inputs_ready(source: &SourceSnapshot) -> bool {
    source.input_schema.iter().all(|(key, schema)| {
        if !schema.required || schema.kind == InputType::Secret {
            return true;
        }
        source
            .inputs
            .get(key)
            .or(schema.default.as_ref())
            .is_some_and(|value| !value.is_null())
    })
}

fn doctor_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).weak());
    ui.label(value);
    ui.end_row();
}

fn method_text(method: datasource::HttpMethod) -> &'static str {
    match method {
        datasource::HttpMethod::Get => "GET",
        datasource::HttpMethod::Post => "POST",
        datasource::HttpMethod::Put => "PUT",
        datasource::HttpMethod::Patch => "PATCH",
    }
}

fn timestamp_text(milliseconds: u64) -> String {
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
        .map(|value| value.to_offset(time::UtcOffset::UTC).to_string())
        .unwrap_or_else(|_| milliseconds.to_string())
}
