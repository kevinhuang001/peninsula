use super::*;

impl DataSourceView {
    pub(in crate::app) fn show(&mut self, ui: &mut egui::Ui, manager: &mut DataSourceManager, preferences: &mut super::super::settings::AppPreferences) -> bool {
        ui.spacing_mut().item_spacing.y = 8.0;
        self.content(ui, manager, preferences);
        std::mem::take(&mut self.preferences_dirty)
    }
    fn content(
        &mut self,
        ui: &mut egui::Ui,
        manager: &mut DataSourceManager,
        preferences: &mut super::super::settings::AppPreferences,
    ) {
        let sources = manager.snapshots();
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !sources.iter().any(|source| &source.id == id))
        {
            self.selected = sources.first().map(|source| source.id.clone());
        }
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
            surface(ui)
                .inner_margin(egui::Margin::symmetric(10, 12))
                .show(ui, |ui| {
                    let inner_width = LEFT_WIDTH - 20.0;
                    let inner_height = (height - 24.0).max(1.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(inner_width, inner_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            let import_height = 32.0;
                            let gap = 8.0;
                            let list_height = (inner_height - import_height - gap).max(1.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(inner_width, list_height),
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
                                        .fill(accent(ui)),
                                )
                                .on_hover_text("选择 .pds.json 文件")
                                .clicked()
                            {
                                self.pick_definition(ui.ctx(), manager);
                            }
                        },
                    );
                });
            ui.add_space(COLUMN_GAP);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width().max(1.0), height),
                Layout::top_down(Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("datasource-detail")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if self.candidate.is_some() {
                                self.import_review(ui, manager);
                            } else if self.delete_candidate.is_some() {
                                self.delete_review(ui, manager);
                            } else if let Some(selected) = self.selected.as_ref()
                                .and_then(|id| sources.iter().find(|source| &source.id == id)).cloned()
                            {
                                self.source_detail(ui, &selected, manager, preferences);
                            } else {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(60.0);
                                    ui.label(RichText::new("◈").size(36.0).color(accent(ui)));
                                    ui.add_space(8.0);
                                    ui.heading("还没有数据源");
                                    ui.label("导入 .pds.json 文件，开始获取和展示数据。");
                                });
                            }
                        });
                },
            );
        });
    }

    fn source_list(
        &mut self,
        ui: &mut egui::Ui,
        sources: &[SourceSnapshot],
        manager: &DataSourceManager,
    ) {
        ui.label(RichText::new(format!("我的数据源 · {}", sources.len())).strong());
        ui.add_sized([ui.available_width(), 28.0], egui::TextEdit::singleline(&mut self.filter).hint_text("搜索数据源…"));
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("datasource-list")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let query = self.filter.trim().to_lowercase();
                for source in sources.iter().filter(|source| {
                    query.is_empty() || source.name.to_lowercase().contains(&query) || source.id.to_lowercase().contains(&query)
                }) {
                    let selected = self.selected.as_deref() == Some(&source.id);
                    let response = ui.allocate_response(
                        Vec2::new(ui.available_width(), 54.0),
                        egui::Sense::click(),
                    );
                    if response.clicked() {
                        self.selected = Some(source.id.clone());
                        self.candidate = None;
                        self.delete_candidate = None;
                    }
                    let rect = response.rect;
                    let fill = if selected {
                        ui.visuals().selection.bg_fill.gamma_multiply(0.16)
                    } else if response.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, 8.0, fill);
                    if selected { ui.painter().rect_filled(egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom())), 2.0, accent(ui)); }
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
                        rect.left_center() + Vec2::new(38.0, -8.0),
                        egui::Align2::LEFT_CENTER,
                        &source.name,
                        Typography::font_id(ui.ctx(), 13.0),
                        ui.visuals().text_color(),
                    );
                    painter.text(
                        rect.left_center() + Vec2::new(38.0, 11.0),
                        egui::Align2::LEFT_CENTER,
                        source.formatted.as_deref().unwrap_or(if source.enabled { "等待数据" } else { "已停用" }),
                        Typography::font_id(ui.ctx(), Typography::SMALL),
                        ui.visuals().weak_text_color(),
                    );
                    response.context_menu(|ui| {
                        ui.label(&source.name);
                        ui.separator();
                        if ui.button("删除数据源…").clicked() {
                            self.delete_candidate = Some(source.id.clone());
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
        preferences: &mut super::super::settings::AppPreferences,
    ) {
        ui.horizontal(|ui| {
            if let Some(icon) = manager.icon_texture(&source.id) {
                ui.image((icon.id(), Vec2::splat(24.0)));
            }
            ui.label(RichText::new(&source.name).text_style(egui::TextStyle::Heading).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("删除…").on_hover_text("删除当前数据源").clicked() {
                    self.delete_candidate = Some(source.id.clone());
                }
                if ui
                    .add_enabled(
                        source.enabled,
                        egui::Button::new(if source.in_flight {
                            "刷新中…"
                        } else {
                            "↻  刷新"
                        })
                        .min_size(Vec2::new(96.0, 32.0)),
                    )
                    .on_hover_text("立即刷新当前数据源")
                    .clicked()
                {
                    self.set_source_status(&source.id, manager.refresh_now(&source.id));
                }
            });
        });
        if !source.description.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&source.description).weak());
        }
        ui.add_space(8.0);
        ui.label(RichText::new(if !source.enabled { "已停用" } else if source.in_flight { "正在刷新" } else if source.last_error.is_some() { "需要检查" } else { "运行中" }).small().color(if source.last_error.is_some() { ui.visuals().warn_fg_color } else { accent(ui) }));
        ui.add_space(8.0);
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
                let result = manager.set_enabled(&source.id, enabled);
                self.set_source_status(&source.id, result);
            }
        });
        ui.add_space(8.0);
        ui.add_space(12.0);
        self.inputs_form(ui, source, manager);
        ui.add_space(12.0);
        self.runtime_settings(ui, source, manager, preferences);
        ui.add_space(12.0);
        if let Some(error) = &source.last_error {
            ui.colored_label(egui::Color32::from_rgb(239, 132, 112), error);
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


}
