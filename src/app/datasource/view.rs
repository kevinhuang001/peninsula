use super::*;

impl DataSourceView {
    pub(in crate::app) fn show(&mut self, ui: &mut egui::Ui, manager: &mut DataSourceManager, preferences: &mut super::super::settings::AppPreferences) -> bool {
        self.poll_definition(ui.ctx(), manager);
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
        let width = ui.available_width().max(1.0);
        if compact_layout(width) {
            // On low-resolution windows, the detail panel stays full-width
            // below a short, scrollable source picker. Both remain accessible.
            let list_height = (height * 0.28).clamp(116.0, 186.0).min(height * 0.55);
            ui.vertical(|ui| {
                self.source_sidebar(ui, &sources, manager, width, list_height);
                ui.add_space(6.0);
                self.source_detail_area(
                    ui,
                    &sources,
                    manager,
                    preferences,
                    (height - list_height - 10.0).max(1.0),
                );
            });
        } else {
            ui.horizontal_top(|ui| {
                self.source_sidebar(ui, &sources, manager, sidebar_width(width), height);
                ui.add_space(COLUMN_GAP);
                self.source_detail_area(ui, &sources, manager, preferences, height);
            });
        }
    }

    fn source_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        sources: &[SourceSnapshot],
        manager: &mut DataSourceManager,
        width: f32,
        height: f32,
    ) {
        surface(ui)
            .inner_margin(egui::Margin::symmetric(10, 12))
            .show(ui, |ui| {
                let inner_width = (width - 20.0).max(1.0);
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
                            |ui| self.source_list(ui, sources, manager),
                        );
                        ui.add_space(gap);
                        let choosing = self.picker.is_some();
                        let button_width = ui.available_width();
                        if ui
                            .add_enabled(
                                !choosing,
                                egui::Button::new(if choosing { "选择文件中…" } else { "＋  导入数据源" })
                                    .fill(accent(ui))
                                    .min_size(Vec2::new(button_width, import_height)),
                            )
                            .on_hover_text("选择 .pds.json 文件")
                            .clicked()
                        {
                            self.pick_definition(ui.ctx());
                        }
                    },
                );
            });
    }

    fn source_detail_area(
        &mut self,
        ui: &mut egui::Ui,
        sources: &[SourceSnapshot],
        manager: &mut DataSourceManager,
        preferences: &mut super::super::settings::AppPreferences,
        height: f32,
    ) {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width().max(1.0), height.max(1.0)),
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
                                ui.add_space(24.0);
                                ui.label(RichText::new("◈").size(36.0).color(accent(ui)));
                                ui.add_space(8.0);
                                ui.heading("还没有数据源");
                                ui.label("导入 .pds.json 文件，开始获取和展示数据。");
                            });
                        }
                    });
            },
        );
    }

    fn source_list(
        &mut self,
        ui: &mut egui::Ui,
        sources: &[SourceSnapshot],
        manager: &DataSourceManager,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("数据源").size(Typography::font_id(ui.ctx(), Typography::TITLE).size).strong());
            ui.label(RichText::new(sources.len().to_string()).size(Typography::font_id(ui.ctx(), Typography::BODY).size).weak());
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("datasource-list")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for source in sources {
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
                        if source.in_flight {
                            "正在刷新".to_owned()
                        } else if !source.metrics.is_empty() {
                            format!("{} 个指标  {}", source.metrics.len(),
                                if source.enabled { "自动监控" } else { "仅手动" })
                        } else {
                            source.formatted.clone().unwrap_or_else(|| {
                                if source.enabled { "等待数据".into() } else { "仅手动刷新".into() }
                            })
                        },
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
        let width = ui.available_width().max(1.0);
        // Title, status and refresh share a single compact toolbar. Manual
        // refresh works even when no background Capsule/Notification is active.
        ui.horizontal(|ui| {
            if let Some(icon) = manager.icon_texture(&source.id) {
                ui.image((icon.id(), Vec2::splat(24.0)));
            }
            let title_width = (width - 158.0).max(72.0);
            ui.add_sized(
                [title_width, 31.0],
                egui::Label::new(RichText::new(&source.name).size(Typography::font_id(ui.ctx(), Typography::HEADING).size).strong()).truncate(),
            ).on_hover_text(&source.name);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let clicked = ui.add_enabled(
                    !source.in_flight,
                    egui::Button::new(if source.in_flight { "刷新中…" } else { "↻ 刷新" })
                        .min_size(Vec2::new(96.0, 32.0)),
                ).on_hover_text("立即获取全部指标，不会开启后台监控").clicked();
                if clicked {
                    self.set_source_status(&source.id, manager.refresh_now(&source.id));
                    ui.ctx().request_repaint();
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(if source.in_flight {
                "正在获取数据"
            } else if source.enabled {
                "自动监控中"
            } else {
                "仅手动刷新"
            }).size(Typography::font_id(ui.ctx(), Typography::FORM_HINT).size).color(if source.enabled {
                accent(ui)
            } else {
                ui.visuals().weak_text_color()
            }));
            if let Some(updated) = source.updated_ms {
                ui.label(RichText::new(format!("上次更新 {}", updated_time_text(updated)))
                    .size(Typography::font_id(ui.ctx(), Typography::FORM_HINT).size).weak());
            }
        });
        if !source.description.is_empty() {
            ui.add(egui::Label::new(RichText::new(&source.description).size(Typography::font_id(ui.ctx(), Typography::BODY).size).weak())
                .truncate()).on_hover_text(&source.description);
        }
        ui.add_space(10.0);

        // Always use a single column: one metric occupies the complete detail
        // width. Responsive layout is handled inside the row, not with a grid.
        if source.metrics.is_empty() {
            metric_row(
                ui, "当前值", source.formatted.as_deref().unwrap_or("—"),
                source.last_error.as_deref(),
            );
        } else {
            for metric in &source.metrics {
                metric_row(
                    ui, &metric.name, metric.formatted.as_deref().unwrap_or("—"),
                    metric.last_error.as_deref(),
                );
                ui.add_space(5.0);
            }
        }
        if let Some(error) = &source.last_error {
            ui.add_space(6.0);
            ui.colored_label(ui.visuals().warn_fg_color, error);
        }
        if self.status_source.as_deref() == Some(source.id.as_str()) && !self.status.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, &self.status);
        }

        ui.add_space(14.0);
        self.presentation_settings(ui, source, preferences);
        ui.add_space(10.0);

        egui::CollapsingHeader::new("名称与参数")
            .id_salt(("datasource-inputs-open", &source.id))
            .default_open(false)
            .show(ui, |ui| {
                ui.add_space(7.0);
                let configured_template = source.name_override.as_deref()
                    .or(source.name_template.as_deref()).unwrap_or("");
                let draft_key = format!("{}::display-name", source.id);
                let draft = self.text_drafts.entry(draft_key)
                    .or_insert_with(|| configured_template.to_owned());
                let mut name_update = None;
                form_surface(ui).show(ui, |ui| {
                    ui.label(RichText::new("显示名称").size(Typography::font_id(ui.ctx(), Typography::BODY).size).strong());
                    let response = ui.add(
                        egui::TextEdit::singleline(draft)
                            .desired_width(ui.available_width().min(500.0)),
                    );
                    let commit = response.lost_focus() || (response.has_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    if commit && draft.trim() != configured_template {
                        name_update = Some(draft.clone());
                    }
                });
                if let Some(name) = name_update {
                    self.set_source_status(&source.id, manager.set_display_name(&source.id, name));
                }
                self.inputs_form(ui, source, manager);
            });
        ui.add_space(8.0);
        egui::CollapsingHeader::new("刷新设置")
            .id_salt(("datasource-refresh-open", &source.id))
            .default_open(false)
            .show(ui, |ui| {
                self.runtime_settings(ui, source, manager);
            });
        ui.add_space(8.0);
        egui::CollapsingHeader::new("诊断")
            .id_salt(("datasource-doctor-open", &source.id))
            .default_open(false)
            .show(ui, |ui| {
                self.status_panel(ui, source);
            });
    }
}
