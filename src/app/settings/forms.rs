use super::theme::{BORDER, ROW, SECONDARY_TEXT, TEXT};
use crate::extension::{
    DurationDisplay, EditorHint, ExtensionId, SearchOption, SettingSchema, SettingValue,
};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Vec2};
use std::{collections::HashMap, time::Instant};

pub(super) const SWITCH_ON: Color32 = Color32::from_rgb(24, 62, 105);
pub(super) const SWITCH_OFF: Color32 = Color32::from_rgb(75, 81, 92);
const CONTROL_WIDTH: f32 = 132.0;
pub(super) const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(350);

#[derive(Clone, Debug, Default)]
pub(super) struct SearchFieldState {
    pub query: String,
    pub results: Vec<SearchOption>,
    pub error: Option<String>,
    pub searching: bool,
    pub(super) pending_since: Option<Instant>,
    pub(super) submitted_query: Option<String>,
}

#[derive(Clone, Debug)]
pub(in crate::app) struct SearchRequest {
    pub extension_id: crate::extension::ExtensionId,
    pub setting_key: String,
    pub query: String,
}

pub(super) fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(title)
            .size(11.5)
            .strong()
            .color(SECONDARY_TEXT),
    );
    ui.add_space(3.0);
}

/// A measured label on the left and a bounded control column on the right.
/// Long schema labels grow the row instead of pushing controls underneath.
pub(super) fn row(
    ui: &mut egui::Ui,
    label: &str,
    control: impl FnOnce(&mut egui::Ui) -> bool,
) -> bool {
    row_impl(ui, label, true, control)
}

/// Editable inputs have a single visible outline: the control's own.
/// Do not draw a second framed settings row around TextEdit, ComboBox,
/// DragValue or other editable fields.
pub(super) fn input_row(
    ui: &mut egui::Ui,
    label: &str,
    control: impl FnOnce(&mut egui::Ui) -> bool,
) -> bool {
    row_impl(ui, label, false, control)
}

fn row_impl(
    ui: &mut egui::Ui,
    label: &str,
    framed: bool,
    control: impl FnOnce(&mut egui::Ui) -> bool,
) -> bool {
    let width = if ui.is_sizing_pass() {
        let label_width = ui
            .painter()
            .layout_no_wrap(label.into(), FontId::proportional(11.5), TEXT)
            .size()
            .x;
        (label_width + CONTROL_WIDTH + 30.0)
            .ceil()
            .min(ui.available_width())
    } else {
        ui.available_width()
    }
    .max(1.0);
    let control_width = CONTROL_WIDTH.min(width * 0.47);
    let label_width = (width - control_width - 30.0).max(1.0);
    let galley = ui
        .painter()
        .layout(label.into(), FontId::proportional(11.5), TEXT, label_width);
    let height = (galley.size().y + 16.0).max(36.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if framed {
        ui.painter().rect_filled(rect, 7.0, ROW);
        ui.painter().rect_stroke(
            rect,
            7.0,
            Stroke::new(1.0_f32, BORDER),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5),
        galley,
        TEXT,
    );
    let control_rect = Rect::from_min_max(
        egui::pos2(rect.right() - control_width - 8.0, rect.top() + 6.0),
        rect.max - egui::vec2(8.0, 6.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(response.id)
            .max_rect(control_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    // The child can use the full control width for hit testing, but its
    // centered focus stroke needs a little room outside that rectangle.
    child.set_clip_rect(ui.clip_rect().intersect(control_rect.expand(2.0)));
    let changed = control(&mut child);
    ui.add_space(5.0);
    changed
}

pub(super) fn toggle(ui: &mut egui::Ui, value: &mut bool, label: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 18.0), Sense::click());
    let changed = response.clicked();
    if changed {
        *value = !*value;
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    ui.painter()
        .rect_filled(rect, 9.0, if *value { SWITCH_ON } else { SWITCH_OFF });
    ui.painter().circle_filled(
        egui::pos2(
            if *value {
                rect.right() - 9.0
            } else {
                rect.left() + 9.0
            },
            rect.center().y,
        ),
        7.0,
        TEXT,
    );
    response.on_hover_text(format!(
        "{label}：{}",
        if *value { "已开启" } else { "已关闭" }
    ));
    changed
}

pub(super) fn boolean(ui: &mut egui::Ui, label: &str, value: &mut bool) -> bool {
    row(ui, label, |ui| toggle(ui, value, label))
}

pub(super) fn value(ui: &mut egui::Ui, label: &str, text: &str) {
    row(ui, label, |ui| {
        ui.add(
            egui::Label::new(egui::RichText::new(text).size(11.0).color(SECONDARY_TEXT)).truncate(),
        )
        .on_hover_text(text);
        false
    });
}

pub(super) fn doctor(ui: &mut egui::Ui, checks: &[crate::extension::DiagnosticCheck]) {
    if checks.is_empty() {
        egui::Frame::new()
            .fill(ROW)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(9.0)
            .inner_margin(egui::Margin::symmetric(12, 11))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("暂无检查项").size(11.5).color(TEXT));
                ui.label(
                    egui::RichText::new("此扩展尚未提供诊断结果")
                        .size(10.5)
                        .color(SECONDARY_TEXT),
                );
            });
        return;
    }
    for check in checks {
        let (symbol, status, color) = match check.status {
            crate::extension::DiagnosticStatus::Passed => {
                ("✓", "正常", Color32::from_rgb(105, 207, 161))
            }
            crate::extension::DiagnosticStatus::Warning => {
                ("!", "注意", Color32::from_rgb(245, 188, 95))
            }
            crate::extension::DiagnosticStatus::Failed => {
                ("×", "异常", Color32::from_rgb(245, 130, 132))
            }
            crate::extension::DiagnosticStatus::Unknown => ("·", "待检查", SECONDARY_TEXT),
        };
        egui::Frame::new()
            .fill(ROW.lerp_to_gamma(color, 0.035))
            .stroke(Stroke::new(1.0_f32, BORDER.lerp_to_gamma(color, 0.22)))
            .corner_radius(9.0)
            .inner_margin(egui::Margin::symmetric(11, 9))
            .show(ui, |ui| {
                let width = if ui.is_sizing_pass() {
                    (ui.painter()
                        .layout_no_wrap(check.title.clone(), FontId::proportional(11.5), TEXT)
                        .size()
                        .x
                        + 90.0)
                        .min(ui.available_width())
                } else {
                    ui.available_width()
                };
                ui.set_width(width);
                ui.horizontal(|ui| {
                    egui::Frame::new()
                        .fill(color.gamma_multiply(0.16))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(7, 2))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(symbol).size(12.0).strong().color(color));
                        });
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&check.title)
                                .size(11.5)
                                .strong()
                                .color(TEXT),
                        )
                        .truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(status).size(10.0).strong().color(color));
                    });
                });
                if let Some(remediation) = &check.remediation {
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new(format!("建议 · {remediation}"))
                            .size(10.5)
                            .color(color),
                    );
                }
            });
        ui.add_space(5.0);
    }
}

/// Every extension shares this renderer; business code supplies only a schema.
pub(super) fn setting_control(
    ui: &mut egui::Ui,
    field: &SettingSchema,
    value: &mut SettingValue,
) -> bool {
    let old = value.clone();
    let is_boolean = matches!(value, SettingValue::Boolean(_));
    let render = |ui: &mut egui::Ui| {
        match value {
            SettingValue::Boolean(v) => {
                toggle(ui, v, &field.description);
            }
            SettingValue::Integer(v) => {
                ui.add(egui::DragValue::new(v)
                    .range(field.constraints.min.unwrap_or(i32::MIN as f64) as i64
                        ..=field.constraints.max.unwrap_or(i32::MAX as f64) as i64)
                    .suffix(field.presentation.unit.as_deref().unwrap_or("")));
            }
            SettingValue::Float(v) => {
                ui.add(egui::DragValue::new(v).speed(0.1)
                    .range(field.constraints.min.unwrap_or(-1e9)
                        ..=field.constraints.max.unwrap_or(1e9))
                    .suffix(field.presentation.unit.as_deref().unwrap_or("")));
            }
            SettingValue::Duration(v) => {
                let (scale, suffix) = match field.presentation.duration_display {
                    DurationDisplay::Milliseconds => (1_u64, " ms"),
                    DurationDisplay::Minutes => (60_000_u64, " 分钟"),
                };
                let mut display = v.div_ceil(scale);
                let lower = (field.constraints.min.unwrap_or(0.0) as u64).div_ceil(scale);
                let upper = (field.constraints.max.unwrap_or(u32::MAX as f64) as u64) / scale;
                if ui.add(egui::DragValue::new(&mut display)
                    .suffix(suffix).range(lower..=upper)).changed() {
                    *v = display.saturating_mul(scale);
                }
            }
            SettingValue::String(v) => {
                ui.add_sized(
                    [ui.available_width(), 24.0],
                    egui::TextEdit::singleline(v)
                        .char_limit(field.constraints.max_length.unwrap_or(4096)),
                );
            }
            SettingValue::Enum(v) => {
                egui::ComboBox::from_id_salt(&field.key)
                    .width(ui.available_width())
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
            SettingValue::Search(_) => {}
        }
        false
    };
    if is_boolean {
        row(ui, &field.description, render);
    } else {
        input_row(ui, &field.description, render);
    }
    if field.validate(value).is_err() {
        *value = old;
        return false;
    }
    *value != old
}

/// Keep incomplete text edits in the UI, separate from the validated stored value.
/// Every numeric text editor uses this path; no business-specific parser is needed.
pub(super) fn parse_number_text(field: &SettingSchema, draft: &str) -> Option<SettingValue> {
    let input = draft.trim();
    let value = match field.value_type {
        crate::extension::ValueType::Float => {
            SettingValue::Float(if input.is_empty() { 0.0 } else { input.parse().ok()? })
        }
        crate::extension::ValueType::Integer => {
            SettingValue::Integer(if input.is_empty() { 0 } else { input.parse().ok()? })
        }
        _ => return None,
    };
    field.validate(&value).ok().map(|_| value)
}

pub(super) fn setting_control_with_draft(
    ui: &mut egui::Ui,
    field: &SettingSchema,
    value: &mut SettingValue,
    owner: &ExtensionId,
    drafts: &mut HashMap<(ExtensionId, String), String>,
) -> bool {
    if field.presentation.editor != EditorHint::NumberText {
        return setting_control(ui, field, value);
    }
    let draft = drafts.entry((owner.clone(), field.key.clone())).or_insert_with(|| {
        match value {
            SettingValue::Float(n) if *n == 0.0 => String::new(),
            SettingValue::Integer(0) => String::new(),
            SettingValue::Float(n) => n.to_string(),
            SettingValue::Integer(n) => n.to_string(),
            _ => String::new(),
        }
    });
    let mut changed = false;
    input_row(ui, &field.description, |ui| {
        let response = ui.add_sized(
            [ui.available_width().max(24.0), 25.0],
            egui::TextEdit::singleline(draft)
                .hint_text(field.presentation.placeholder.as_deref().unwrap_or("")),
        );
        if response.changed() {
            if let Some(parsed) = parse_number_text(field, draft) {
                if parsed != *value {
                    *value = parsed;
                    changed = true;
                }
            }
        }
        false
    });
    if parse_number_text(field, draft).is_none() {
        ui.label(egui::RichText::new("请输入有效范围内的数字")
            .size(10.5).color(SECONDARY_TEXT));
    }
    changed
}

pub(super) fn search_setting(
    ui: &mut egui::Ui,
    field: &SettingSchema,
    value: &mut SettingValue,
    state: &mut SearchFieldState,
) -> bool {
    let Some(schema) = field.search.as_ref() else {
        return false;
    };
    let mut changed = false;
    ui.add_space(5.0);
    ui.label(
        egui::RichText::new(&field.description)
            .size(11.5)
            .strong()
            .color(TEXT),
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // Keep the left half of TextEdit's focused border inside the
        // scroll area's clip rectangle.
        ui.add_space(2.0);
        let width = (ui.available_width() - 55.0).max(80.0);
        let response = ui.add_sized(
            [width, 25.0],
            egui::TextEdit::singleline(&mut state.query)
                .hint_text(&schema.placeholder)
                .char_limit(128),
        );
        if response.changed() {
            state.pending_since = Some(Instant::now());
            state.submitted_query = None;
            state.results.clear();
            state.error = None;
        }
        let pressed = ui.button("搜索").clicked();
        if pressed {
            state.pending_since = Some(Instant::now() - SEARCH_DEBOUNCE);
            state.submitted_query = None;
        }
    });
    if let SettingValue::Search(selected) = value {
        if let Some(option) = selected.clone() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("已选择：{}", option.label))
                        .size(10.5)
                        .color(SECONDARY_TEXT),
                )
                .on_hover_text(if option.detail.is_empty() {
                    option.label.clone()
                } else {
                    format!("{} · {}", option.label, option.detail)
                });
                if ui.small_button("清除").clicked() {
                    *selected = None;
                    changed = true;
                }
            });
        }
    }
    if state.searching {
        ui.label(
            egui::RichText::new("正在搜索…")
                .size(10.5)
                .color(SECONDARY_TEXT),
        );
    } else if let Some(error) = &state.error {
        ui.label(
            egui::RichText::new(error)
                .size(10.5)
                .color(Color32::LIGHT_RED),
        );
    } else if matches!(value, SettingValue::Search(None))
        && state.submitted_query.as_deref() == Some(state.query.as_str())
        && !state.searching
        && state.results.is_empty()
    {
        ui.label(
            egui::RichText::new("没有找到匹配结果")
                .size(10.5)
                .color(SECONDARY_TEXT),
        );
    } else if state.pending_since.is_none()
        && !state.query.trim().is_empty()
        && state.query.trim().chars().count() < schema.min_query_chars
    {
        ui.label(
            egui::RichText::new(format!("至少输入 {} 个字符", schema.min_query_chars))
                .size(10.5)
                .color(SECONDARY_TEXT),
        );
    }
    for option in state.results.clone() {
        let detail = if option.detail.is_empty() {
            option.label.clone()
        } else {
            format!("{} · {}", option.label, option.detail)
        };
        let selected =
            matches!(value, SettingValue::Search(Some(current)) if current.id == option.id);
        if ui
            .selectable_label(selected, egui::RichText::new(detail).size(10.5))
            .clicked()
        {
            *value = SettingValue::Search(Some(option));
            state.results.clear();
            state.submitted_query = None;
            changed = true;
        }
    }
    ui.add_space(5.0);
    changed
}

pub(super) fn reorder(ui: &mut egui::Ui) -> i8 {
    let mut direction = 0;
    if ui.small_button("↓").on_hover_text("下移").clicked() {
        direction = 1;
    }
    if ui.small_button("↑").on_hover_text("上移").clicked() {
        direction = -1;
    }
    direction
}

pub(super) fn empty(ui: &mut egui::Ui, text: &str) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.0),
        SECONDARY_TEXT,
    );
}
