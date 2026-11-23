use super::*;

impl DataSourceView {
    pub(super) fn inputs_form(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
    ) {
        let secret_hint = if source
            .input_schema
            .values()
            .any(|schema| schema.kind == InputType::String)
        {
            "字符串参数可使用 $secret$ 引用「秘密」中的 Secret"
        } else {
            ""
        };
        section_title(ui, "参数", secret_hint);
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
                                ui.add_space((ui.available_width() - 224.0).max(0.0));
                                let response = ui.add_sized(
                                    [(ui.available_width() - 4.0).min(220.0).max(20.0), 28.0],
                                    editor.horizontal_align(Align::RIGHT),
                                );
                                let response = if schema.kind == InputType::String {
                                    response.on_hover_text("可填明文，或用 $名称$ 引用「秘密」条目")
                                } else {
                                    response
                                };
                                let edited = response.changed();
                                let commit = response.lost_focus()
                                    || (response.has_focus()
                                        && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                                if (edited || commit) && draft != &initial {
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
                                    if schema.required && draft.trim().is_empty() {
                                        invalid_number = commit;
                                    } else if let Some(value) = parsed {
                                        input_update = Some(value);
                                    } else {
                                        // Intermediate edits such as "-" or "10m" must not
                                        // display a warning until the edit is committed.
                                        invalid_number = commit;
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
                        self.set_source_error(
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

    pub(super) fn runtime_settings(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        manager: &mut DataSourceManager,
        preferences: &mut super::super::settings::AppPreferences,
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
                );
                ui.end_row();
                if let Ok(id) = crate::extension::ExtensionId::new(
                    format!("datasource/{}/notification", source.id),
                ) {
                    if let Some(crate::extension::SettingValue::Duration(ms)) =
                        preferences.contribution_values.get_mut(&id)
                            .and_then(|values| values.get_mut("cooldown"))
                    {
                        form_surface(ui).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("最短提醒间隔");
                                let mut minutes = (*ms).div_ceil(60_000).clamp(1, 43_200);
                                if ui.add(
                                    egui::DragValue::new(&mut minutes)
                                        .range(1..=43_200)
                                        .suffix(" 分钟"),
                                ).changed() {
                                    *ms = minutes * 60_000;
                                    self.preferences_dirty = true;
                                }
                            });
                        });
                        ui.end_row();
                    }
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
    ) {
        let draft_key = format!("{}::{key}", source.id);
        let draft = self
            .runtime_drafts
            .entry(draft_key.clone())
            .or_insert_with(|| current_ms.div_ceil(60_000).to_string());
        let mut changed = false;
        form_surface(ui).show(ui, |ui| {
            ui.set_width((ui.available_width() - 24.0).max(1.0).min(CARD_CONTENT_WIDTH));
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
                        let result = manager.set_refresh_interval(&source.id, interval);
                        self.set_source_status(&source.id, result);
                    }
                }
            }
        }
    }

    pub(super) fn status_panel(&mut self, ui: &mut egui::Ui, source: &SourceSnapshot) {
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


}
