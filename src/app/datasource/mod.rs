use crate::{
    app::settings::CompactPreferences,
    core::{
        datasource::{self, DataSourceManager, Definition, InputType, SourceSnapshot},
        notifications::NotificationManager,
    },
};
use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use serde_json::Value;
use std::{collections::HashMap, fs, time::Duration};

const VIEWPORT_ID: &str = "peninsula-datasources";
const OPEN_REQUEST_ID: &str = "datasource-open-request";
const LEFT_WIDTH: f32 = 175.0;
const COLUMN_GAP: f32 = 28.0;
const CARD_CONTENT_WIDTH: f32 = 430.0;

fn surface(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    let fill = if visuals.dark_mode {
        Color32::from_rgb(25, 29, 36)
    } else {
        Color32::WHITE
    };
    let border = if visuals.dark_mode {
        Color32::from_rgb(48, 55, 66)
    } else {
        Color32::from_rgb(211, 217, 225)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::same(16))
}

fn form_surface(ui: &egui::Ui) -> egui::Frame {
    surface(ui).inner_margin(egui::Margin::symmetric(12, 7))
}

fn accent(ui: &egui::Ui) -> Color32 {
    ui.visuals().hyperlink_color
}

fn panel(ui: &egui::Ui) -> egui::Frame {
    surface(ui).inner_margin(egui::Margin::symmetric(14, 12))
}

fn import_badge(ui: &mut egui::Ui) {
    let color = accent(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        11.0,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 38),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "↓",
        egui::FontId::proportional(20.0),
        color,
    );
}

fn method_pill(ui: &mut egui::Ui, method: &str) {
    let font = egui::FontId::monospace(10.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(method.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(text_width + 14.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 5.0, accent(ui));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        method,
        font,
        Color32::WHITE,
    );
}

fn section_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(16.0).strong());
    if !subtitle.is_empty() {
        ui.label(
            RichText::new(subtitle)
                .size(11.0)
                .color(ui.visuals().weak_text_color()),
        );
    }
    ui.add_space(8.0);
}

fn switch(ui: &mut egui::Ui, value: &mut bool, label: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), egui::Sense::click());
        if response.clicked() {
            *value = !*value;
            changed = true;
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
        });
        ui.painter().rect_filled(
            rect,
            10.0,
            if *value {
                accent(ui)
            } else {
                ui.visuals().widgets.inactive.bg_fill
            },
        );
        ui.painter().circle_filled(
            egui::pos2(
                if *value {
                    rect.right() - 10.0
                } else {
                    rect.left() + 10.0
                },
                rect.center().y,
            ),
            7.0,
            Color32::WHITE,
        );
        ui.label(label);
    });
    changed
}

fn switch_row(ui: &mut egui::Ui, value: &mut bool, label: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(CARD_CONTENT_WIDTH), 28.0),
        egui::Sense::click(),
    );
    if response.clicked() {
        *value = !*value;
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    let track = egui::Rect::from_center_size(
        rect.right_center() - Vec2::new(18.0, 0.0),
        Vec2::new(36.0, 20.0),
    );
    ui.painter().rect_filled(
        track,
        10.0,
        if *value {
            accent(ui)
        } else {
            ui.visuals().widgets.inactive.bg_fill
        },
    );
    ui.painter().circle_filled(
        egui::pos2(
            if *value {
                track.right() - 10.0
            } else {
                track.left() + 10.0
            },
            track.center().y,
        ),
        7.0,
        Color32::WHITE,
    );
    response.clicked()
}

fn info_card(ui: &mut egui::Ui, label: &str, value: &str, value_size: f32) {
    form_surface(ui).show(ui, |ui| {
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().min(CARD_CONTENT_WIDTH), 28.0),
            egui::Sense::hover(),
        );
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
        ui.painter().text(
            rect.right_center(),
            egui::Align2::RIGHT_CENTER,
            value,
            egui::FontId::proportional(value_size),
            ui.visuals().text_color(),
        );
    });
}

#[derive(Default)]
pub struct DataSourceWindow {
    open: bool,
    selected: Option<String>,
    candidate: Option<Definition>,
    allow_private_network: bool,
    status: String,
    /// `None` means the message is window-wide (import, storage, deletion). Messages tied to
    /// one source are only rendered inside that source's detail panel.
    status_source: Option<String>,
    delete_candidate: Option<String>,
    delete_history: bool,
    text_drafts: HashMap<String, String>,
    runtime_drafts: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DurationUnit {
    Seconds,
    #[default]
    Minutes,
    Hours,
    Days,
}

impl DurationUnit {
    fn label(self) -> &'static str {
        match self {
            Self::Seconds => "秒",
            Self::Minutes => "分钟",
            Self::Hours => "小时",
            Self::Days => "天",
        }
    }

    fn multiplier(self) -> u64 {
        match self {
            Self::Seconds => 1_000,
            Self::Minutes => 60_000,
            Self::Hours => 3_600_000,
            Self::Days => 86_400_000,
        }
    }
}

impl DataSourceWindow {
    pub fn show(
        &mut self,
        context: &egui::Context,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
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
            .with_inner_size([720.0, 780.0])
            .with_min_inner_size([680.0, 480.0]);
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
                            self.content(ui, manager, preferences, notifications);
                        });
                    !open
                } else {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::central_panel(&context.style()).inner_margin(16.0))
                        .show(context, |ui| {
                            self.content(ui, manager, preferences, notifications);
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
        notifications: &mut NotificationManager,
    ) {
        if ui.visuals().dark_mode {
            let visuals = ui.visuals_mut();
            visuals.extreme_bg_color = Color32::from_rgb(31, 37, 46);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 47, 58);
            visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(40, 47, 58);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(51, 61, 74);
            visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(51, 61, 74);
            visuals.widgets.active.bg_fill = Color32::from_rgb(58, 70, 85);
        }
        let sources = manager.snapshots();
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !sources.iter().any(|source| &source.id == id))
        {
            self.selected = sources.first().map(|source| source.id.clone());
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("数据源").size(23.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let can_refresh = self.selected.as_ref().is_some_and(|id| {
                    sources
                        .iter()
                        .any(|source| &source.id == id && source.enabled)
                });
                let refreshing = self.selected.as_ref().is_some_and(|id| {
                    sources
                        .iter()
                        .any(|source| &source.id == id && source.in_flight)
                });
                if ui
                    .add_enabled(
                        can_refresh,
                        egui::Button::new(if refreshing {
                            "刷新中…"
                        } else {
                            "↻  刷新"
                        })
                        .min_size(Vec2::new(96.0, 32.0)),
                    )
                    .on_hover_text("立即刷新当前数据源")
                    .clicked()
                {
                    if let Some(id) = self.selected.clone() {
                        self.set_source_status(&id, manager.refresh_now(&id));
                    }
                }
            });
        });
        ui.add_space(12.0);
        if let Some(error) = manager.storage_error() {
            ui.colored_label(ui.visuals().error_fg_color, format!("存储诊断：{error}"));
            ui.add_space(6.0);
        }
        if self.status_source.is_none() && !self.status.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, &self.status);
            ui.add_space(6.0);
        }
        let height = ui.available_height().max(1.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(LEFT_WIDTH, height),
                Layout::top_down(Align::Min),
                |ui| {
                    let import_height = 32.0;
                    let gap = 8.0;
                    let list_height = (height - import_height - gap).max(1.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(LEFT_WIDTH, list_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.source_list(ui, &sources, manager);
                        },
                    );
                    ui.add_space(gap);
                    if ui
                        .add_sized(
                            [ui.available_width(), import_height],
                            egui::Button::new("＋  导入数据源")
                                .fill(Color32::from_rgb(28, 92, 154)),
                        )
                        .on_hover_text("选择 .pds.json 文件")
                        .clicked()
                    {
                        self.pick_definition();
                    }
                },
            );
            ui.add_space(COLUMN_GAP);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width().max(1.0), height),
                Layout::top_down(Align::Min),
                |ui| {
                    if let Some(selected) = self
                        .selected
                        .as_ref()
                        .and_then(|id| sources.iter().find(|source| &source.id == id))
                        .cloned()
                    {
                        egui::ScrollArea::vertical()
                            .id_salt(("datasource-detail", &selected.id))
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                self.source_detail(ui, &selected, manager, preferences);
                            });
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(100.0);
                            ui.label(RichText::new("◈").size(36.0).color(accent(ui)));
                            ui.add_space(8.0);
                            ui.heading("还没有数据源");
                            ui.label("导入 .pds.json 文件，开始获取和展示数据。 ");
                        });
                    }
                },
            );
        });
        self.import_dialog(ui.ctx(), manager, preferences);
        self.delete_dialog(ui.ctx(), manager, preferences, notifications);
    }

    fn source_list(
        &mut self,
        ui: &mut egui::Ui,
        sources: &[SourceSnapshot],
        manager: &DataSourceManager,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("datasource-list")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for source in sources {
                    let selected = self.selected.as_deref() == Some(&source.id);
                    let response = ui.allocate_response(
                        Vec2::new(ui.available_width(), 42.0),
                        egui::Sense::click(),
                    );
                    if response.clicked() {
                        self.selected = Some(source.id.clone());
                    }
                    let rect = response.rect;
                    let fill = if selected {
                        ui.visuals().widgets.inactive.bg_fill
                    } else if response.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, 8.0, fill);
                    ui.painter().rect_stroke(
                        rect,
                        8.0,
                        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                        egui::StrokeKind::Inside,
                    );
                    if let Some(icon) = manager.icon_texture(&source.id) {
                        ui.painter().image(
                            icon.id(),
                            egui::Rect::from_min_size(
                                rect.left_center() + Vec2::new(10.0, -10.0),
                                Vec2::splat(20.0),
                            ),
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                            ui.visuals().text_color(),
                        );
                    }
                    let painter = ui.painter().with_clip_rect(egui::Rect::from_min_max(
                        rect.min + Vec2::new(36.0, 0.0),
                        rect.max - Vec2::new(11.0, 0.0),
                    ));
                    painter.text(
                        rect.left_center() + Vec2::new(38.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &source.name,
                        egui::FontId::proportional(13.0),
                        ui.visuals().text_color(),
                    );
                    response.context_menu(|ui| {
                        ui.label(&source.name);
                        ui.separator();
                        if ui.button("删除数据源…").clicked() {
                            self.delete_candidate = Some(source.id.clone());
                            self.delete_history = false;
                            ui.close_menu();
                        }
                    });
                    ui.add_space(5.0);
                }
            });
    }

    fn source_detail(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
    ) {
        ui.horizontal(|ui| {
            if let Some(icon) = manager.icon_texture(&source.id) {
                ui.image((icon.id(), Vec2::splat(24.0)));
            }
            ui.label(RichText::new(&source.name).size(20.0).strong());
        });
        if !source.description.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&source.description).weak());
        }
        ui.add_space(14.0);
        info_card(
            ui,
            "当前值",
            source.formatted.as_deref().unwrap_or("—"),
            13.0,
        );
        ui.add_space(8.0);
        let updated = source
            .updated_ms
            .map(updated_time_text)
            .unwrap_or_else(|| "等待首次更新".into());
        info_card(ui, "更新时间", &updated, 12.0);
        ui.add_space(12.0);
        let mut enabled = source.enabled;
        form_surface(ui).show(ui, |ui| {
            if switch_row(ui, &mut enabled, "启用") {
                self.set_source_status(&source.id, manager.set_enabled(&source.id, enabled));
            }
        });
        ui.add_space(8.0);
        let mut compact = source.compact;
        form_surface(ui).show(ui, |ui| {
            if switch_row(ui, &mut compact, "显示在 Compact") {
                self.set_source_status(&source.id, manager.set_compact(&source.id, compact));
                let id = compact_item_id(&source.id);
                preferences.set_compact_item_enabled(&id, compact);
                self.save_preferences(preferences);
            }
        });
        if source.alert_enabled {
            ui.add_space(8.0);
            let mut alerts = source.alerts;
            form_surface(ui).show(ui, |ui| {
                if switch_row(ui, &mut alerts, "启用提醒") {
                    self.set_source_status(&source.id, manager.set_alerts(&source.id, alerts));
                }
            });
        }
        ui.add_space(12.0);
        self.inputs_form(ui, source, manager);
        ui.add_space(12.0);
        self.runtime_settings(ui, source, manager);
        ui.add_space(12.0);
        if let Some(error) = &source.last_error {
            ui.colored_label(egui::Color32::from_rgb(239, 132, 112), error);
        }
        if let Some(status) = source.http_status {
            ui.label(format!("HTTP 状态：{status}"));
        }
        if self.status_source.as_deref() == Some(source.id.as_str()) && !self.status.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, &self.status);
        }
        ui.add_space(10.0);
        egui::CollapsingHeader::new("诊断详情")
            .id_salt(("datasource-doctor-open", &source.id))
            .show(ui, |ui| {
                self.status_panel(ui, source);
            });
    }

    fn inputs_form(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
    ) {
        section_title(ui, "参数", "");
        if source.input_schema.is_empty() {
            ui.label("此数据源没有用户参数。");
            return;
        }
        let columns_count = 1;

        let spacing = 10.0;
        let card_width = ((ui.available_width().min(CARD_CONTENT_WIDTH + 24.0)
            - spacing * (columns_count - 1) as f32)
            / columns_count as f32)
            .max(220.0);
        egui::Grid::new(("datasource-input-cards", &source.id))
            .num_columns(columns_count)
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for (index, (key, schema)) in source.input_schema.iter().enumerate() {
                    let current = source
                        .inputs
                        .get(key)
                        .cloned()
                        .or_else(|| schema.default.clone())
                        .unwrap_or(Value::Null);
                    let draft_id = format!("{}::{key}", source.id);
                    let mut input_update = None;
                    let mut invalid_number = false;

                    form_surface(ui).show(ui, |ui| {
                        ui.set_width((card_width - 24.0).max(180.0));
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
                        ui.add_space(2.0);

                        match schema.kind {
                            InputType::Boolean => {
                                let mut value = current.as_bool().unwrap_or(false);
                                if switch(ui, &mut value, "开启") {
                                    input_update = Some(Value::Bool(value));
                                }
                            }
                            InputType::Enum => {
                                let mut value = current.as_str().unwrap_or("").to_owned();
                                ui.add_space((ui.available_width() - 220.0).max(0.0));
                                egui::ComboBox::from_id_salt((&source.id, key))
                                    .width(220.0)
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
                                let editor = egui::TextEdit::singleline(draft);
                                ui.add_space((ui.available_width() - 220.0).max(0.0));
                                let response = ui.add_sized(
                                    [ui.available_width().min(220.0), 28.0],
                                    editor.horizontal_align(Align::RIGHT),
                                );
                                let response = if schema.kind == InputType::String {
                                    response.on_hover_text("可填明文，或用 $名称$ 引用「秘密」条目")
                                } else {
                                    response
                                };
                                if response.changed() && schema.kind == InputType::String {
                                    input_update = Some(Value::String(draft.clone()));
                                }
                                let commit = response.lost_focus()
                                    || (response.has_focus()
                                        && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                                if commit && draft != &initial {
                                    let parsed = match schema.kind {
                                        InputType::String => Some(Value::String(draft.clone())),
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
                        self.set_source_status(
                            &source.id,
                            manager.set_input(&source.id, key, value),
                        );
                    }
                    if invalid_number {
                        self.set_source_message(
                            &source.id,
                            format!("{} 的格式或范围无效。", schema.label),
                        );
                    }
                    if (index + 1) % columns_count == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    fn runtime_settings(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
    ) {
        section_title(ui, "运行设置", "");
        egui::Grid::new(("datasource-runtime-settings", &source.id))
            .num_columns(1)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                self.duration_setting(
                    ui,
                    source,
                    manager,
                    "refresh",
                    "自动刷新间隔",
                    source.refresh_interval_ms,
                    Duration::from_secs(30),
                    Duration::from_secs(24 * 60 * 60),
                    true,
                );
                ui.end_row();
                if source.alert_enabled {
                    self.duration_setting(
                        ui,
                        source,
                        manager,
                        "cooldown",
                        "最短提醒间隔",
                        source.min_alert_interval_ms,
                        Duration::from_secs(60),
                        Duration::from_secs(30 * 24 * 60 * 60),
                        false,
                    );
                    ui.end_row();
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn duration_setting(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
        key: &str,
        label: &str,
        current_ms: u64,
        min: Duration,
        max: Duration,
        refresh: bool,
    ) {
        let draft_key = format!("{}::{key}", source.id);
        let draft = self
            .runtime_drafts
            .entry(draft_key.clone())
            .or_insert_with(|| current_ms.div_ceil(60_000).to_string());
        let mut changed = false;
        form_surface(ui).show(ui, |ui| {
            ui.set_width((ui.available_width() - 24.0).clamp(280.0, CARD_CONTENT_WIDTH));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).strong());
                ui.add_space((ui.available_width() - 120.0).max(0.0));
                changed = ui
                    .add_sized(
                        [72.0, 26.0],
                        egui::TextEdit::singleline(draft)
                            .desired_width(72.0)
                            .horizontal_align(Align::RIGHT),
                    )
                    .changed();
                ui.label("分钟");
            });
        });
        if changed {
            if let Some(minutes) = draft.trim().parse::<u64>().ok().filter(|value| *value > 0) {
                if let Some(seconds) = minutes.checked_mul(60) {
                    let interval = Duration::from_secs(seconds);
                    if interval >= min && interval <= max {
                        let result = if refresh {
                            manager.set_refresh_interval(&source.id, interval)
                        } else {
                            manager.set_min_alert_interval(&source.id, interval)
                        };
                        self.set_source_status(&source.id, result);
                    }
                }
            }
        }
    }

    fn status_panel(&mut self, ui: &mut egui::Ui, source: &SourceSnapshot) {
        ui.label(RichText::new("运行诊断").strong());
        let now_ms = current_timestamp_ms();
        egui::Grid::new(("datasource-doctor", &source.id))
            .num_columns(2)
            .spacing([18.0, 5.0])
            .show(ui, |ui| {
                doctor_row(ui, "Schema", "v1 · 已拒绝旧调度字段");
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
                doctor_row(ui, "Permissions", &source.origin);
                let http_status = source
                    .http_status
                    .map_or_else(|| "Unknown".to_owned(), |status| format!("HTTP {status}"));
                doctor_row(ui, "HTTP", &http_status);
                doctor_row(
                    ui,
                    "Value",
                    if source.value.is_some() {
                        "提取与数值转换成功"
                    } else {
                        "尚无有效结果"
                    },
                );
                doctor_row(ui, "JSONPath", &source.value_path);
                doctor_row(ui, "Alert", &source.alert_status);
                let cooldown = source.last_alert_ms.map_or_else(
                    || "此前没有已接受的提醒".to_owned(),
                    |last| {
                        let allowed_at = last.saturating_add(source.min_alert_interval_ms);
                        let remaining = allowed_at.saturating_sub(now_ms);
                        format!(
                            "上次 {} · 最早下次 {}{}",
                            timestamp_text(last),
                            timestamp_text(allowed_at),
                            if remaining > 0 {
                                format!("（剩余 {}）", display_millis(remaining))
                            } else {
                                String::new()
                            }
                        )
                    },
                );
                doctor_row(ui, "Cooldown", &cooldown);
                if source.last_alert_ms.is_some_and(|last| last > now_ms)
                    || source.last_request_ms.is_some_and(|last| last > now_ms)
                {
                    doctor_row(ui, "Clock", "检测到时钟回拨，按保守策略延后提醒");
                }
                doctor_row(
                    ui,
                    "Refresh",
                    &format!(
                        "{} · 下次检查 {} 后",
                        display_millis(source.refresh_interval_ms),
                        display_millis(source.next_refresh_in_ms)
                    ),
                );
                let worker = if source.in_flight {
                    "请求进行中".to_owned()
                } else if !source.enabled {
                    "数据源已停用".to_owned()
                } else if let Some(error) = &source.last_error {
                    classify_error(error)
                } else {
                    "等待计划执行".to_owned()
                };
                doctor_row(ui, "Worker", &worker);
                doctor_row(
                    ui,
                    "Storage",
                    source.storage_error.as_deref().unwrap_or("正常"),
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
        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        if !file_name.ends_with(".pds.json") {
            self.set_message("请选择 .pds.json 文件。");
            return;
        }
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                self.set_message(format!("数据源文件 {file_name} 不存在或不可读。"));
                return;
            }
        };
        if metadata.len() > 2_000_000 {
            self.set_message(format!("数据源文件 {file_name} 超过 2 MB。"));
            return;
        }
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                self.set_message(format!("无法读取文件 {file_name}：{error}"));
                return;
            }
        };
        match Definition::parse(&source) {
            Ok(definition) => {
                self.allow_private_network = false;
                self.status.clear();
                self.status_source = None;
                self.candidate = Some(definition);
            }
            Err(error) => {
                self.set_message(format!("数据源 Schema 校验失败（{file_name}）：{error}"));
            }
        }
    }

    fn import_dialog(
        &mut self,
        context: &egui::Context,
        manager: &mut DataSourceManager,
        preferences: &mut CompactPreferences,
    ) {
        let Some(candidate) = self.candidate.clone() else {
            return;
        };
        let origin = candidate.origin().unwrap_or_else(|_| "无效请求目标".into());
        let dialog_width = 470.0;
        let card_width = dialog_width - 28.0;
        let mut open = true;
        egui::Window::new("导入数据源")
            .id(egui::Id::new("datasource-import"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .min_width(dialog_width)
            .show(context, |ui| {
                ui.set_min_width(dialog_width);
                ui.spacing_mut().item_spacing.y = 8.0;

                ui.horizontal(|ui| {
                    import_badge(ui);
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&candidate.name).size(19.0).strong())
                                .truncate(),
                        )
                        .on_hover_text(&candidate.name);
                        ui.add_space(1.0);
                        let id = format!("ID  {}", candidate.id);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&id)
                                    .size(10.5)
                                    .color(ui.visuals().weak_text_color()),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&candidate.id);
                    });
                });

                ui.add_space(2.0);
                panel(ui).show(ui, |ui| {
                    ui.set_width(card_width);
                    ui.label(RichText::new("请求目标").size(10.0).weak());
                    ui.add_space(2.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&origin)
                                .monospace()
                                .size(12.0)
                                .color(accent(ui)),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&origin);
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        method_pill(ui, method_text(candidate.request.method));
                        ui.add_space(6.0);
                        ui.label(RichText::new("请求地址").size(10.5).weak());
                    });
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&candidate.request.url).monospace().size(11.0),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&candidate.request.url);
                });

                ui.add_space(2.0);
                panel(ui).show(ui, |ui| {
                    ui.set_width(card_width);
                    ui.label(RichText::new("权限").size(10.0).weak());
                    ui.add_space(3.0);
                    ui.label(RichText::new("导入后只允许向上述 Origin 发起请求。").size(11.5));
                    ui.add_space(6.0);
                    switch_row(
                        ui,
                        &mut self.allow_private_network,
                        "允许 HTTP / localhost / 私有网络目标",
                    );
                    ui.label(
                        RichText::new("仅当接口不是 HTTPS 公网地址时开启")
                            .size(10.0)
                            .weak(),
                    );
                });

                if origin.starts_with("http://") {
                    ui.add_space(2.0);
                    panel(ui).show(ui, |ui| {
                        ui.set_width(card_width);
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            RichText::new("目标使用未加密的 HTTP；开启上面的权限后才会运行请求。")
                                .size(11.0),
                        );
                    });
                }

                ui.add_space(2.0);
                panel(ui).show(ui, |ui| {
                    ui.set_width(card_width);
                    ui.label(RichText::new("参数中的 Secret").size(10.0).weak());
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new(
                            "字符串参数可以填写明文，或在「秘密」页保存条目后填入 $名称$ 引用。",
                        )
                        .size(11.0),
                    );
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("导入后可随时删除")
                            .size(10.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let primary =
                            egui::Button::new(RichText::new("授权并导入").color(Color32::WHITE))
                                .min_size(Vec2::new(112.0, 32.0))
                                .fill(Color32::from_rgb(28, 92, 154));
                        if ui.add(primary).clicked() {
                            match manager.import(candidate.clone(), self.allow_private_network) {
                                Ok(_) => {
                                    let ids = manager.compact_ids();
                                    if preferences.sync_data_source_items(&ids) {
                                        self.save_preferences(preferences);
                                    }
                                    let compact_id = compact_item_id(&candidate.id);
                                    let _ = manager.set_compact(
                                        &candidate.id,
                                        preferences.is_enabled(&compact_id),
                                    );
                                    self.selected = Some(candidate.id.clone());
                                    self.set_message(format!("已导入 {}。", candidate.name));
                                    self.candidate = None;
                                }
                                Err(error) => self.set_message(format!("导入失败：{error}")),
                            }
                        }
                        if ui
                            .add(egui::Button::new("取消").min_size(Vec2::new(76.0, 32.0)))
                            .clicked()
                        {
                            self.candidate = None;
                        }
                    });
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
                switch(ui, &mut self.delete_history, "同时清除此数据源的通知历史");
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.delete_candidate = None;
                    }
                    if ui.button("删除").clicked() {
                        match manager.delete(&id) {
                            Ok(()) => {
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
                                self.set_message(format!("已删除数据源“{name}”。"));
                            }
                            Err(error) => self.set_message(format!("删除失败：{error}")),
                        }
                    }
                });
            });
        if !open {
            self.delete_candidate = None;
        }
    }

    fn set_source_status(&mut self, source: &str, result: Result<(), String>) {
        self.status = result.map_or_else(|error| error, |_| String::new());
        self.status_source = (!self.status.is_empty()).then(|| source.to_owned());
    }

    fn set_message(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_source = None;
    }

    fn set_source_message(&mut self, source: &str, message: impl Into<String>) {
        self.status = message.into();
        self.status_source = (!self.status.is_empty()).then(|| source.to_owned());
    }

    fn save_preferences(&mut self, preferences: &CompactPreferences) {
        if let Err(error) = preferences.save() {
            self.set_message(format!("无法保存 Compact 设置：{error}"));
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

fn required_inputs_ready(source: &SourceSnapshot) -> bool {
    source.input_schema.iter().all(|(key, schema)| {
        if !schema.required {
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
        .map(|value| {
            let value = value.to_offset(time::UtcOffset::UTC);
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02} UTC",
                value.year(),
                u8::from(value.month()),
                value.day(),
                value.hour(),
                value.minute()
            )
        })
        .unwrap_or_else(|_| milliseconds.to_string())
}

fn updated_time_text(milliseconds: u64) -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let Ok(value) =
        time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
    else {
        return timestamp_text(milliseconds);
    };
    let value = value.to_offset(offset);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    if value.date() == now.date() {
        format!("今天 {:02}:{:02}", value.hour(), value.minute())
    } else if value.year() == now.year() {
        format!(
            "{}月{}日 {:02}:{:02}",
            u8::from(value.month()),
            value.day(),
            value.hour(),
            value.minute()
        )
    } else {
        format!(
            "{}年{}月{}日 {:02}:{:02}",
            value.year(),
            u8::from(value.month()),
            value.day(),
            value.hour(),
            value.minute()
        )
    }
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn duration_parts(milliseconds: u64) -> (DurationUnit, u64) {
    for unit in [
        DurationUnit::Days,
        DurationUnit::Hours,
        DurationUnit::Minutes,
        DurationUnit::Seconds,
    ] {
        if milliseconds >= unit.multiplier() && milliseconds.is_multiple_of(unit.multiplier()) {
            return (unit, milliseconds / unit.multiplier());
        }
    }
    (DurationUnit::Seconds, milliseconds.div_ceil(1000))
}

fn display_millis(milliseconds: u64) -> String {
    let (unit, value) = duration_parts(milliseconds);
    format!("{value} {}", unit.label())
}

fn classify_error(error: &str) -> String {
    if error.contains("缺少必填参数") || error.contains("请填写必填参数") {
        format!("尚未配置完成：{error}")
    } else if error.contains("Secret") {
        format!("Secret 不可用：{error}")
    } else if error.contains("授权") || error.contains("单独授权") {
        format!("权限不足：{error}")
    } else if error.contains("429") || error.contains("限流") {
        format!("API 限流：{error}")
    } else if error.contains("JSONPath") || error.contains("数值") {
        format!("路径或类型转换失败：{error}")
    } else if error.contains("JSON") || error.contains("响应") {
        format!("响应解析失败：{error}")
    } else if error.contains("超时") {
        format!("请求超时：{error}")
    } else if error.contains("HTTP 请求") || error.contains("解析请求目标") {
        format!("网络失败：{error}")
    } else {
        format!("运行时错误：{error}")
    }
}
