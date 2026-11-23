use super::*;

impl DataSourceView {
    /// Opening the native chooser in a synchronous UI callback blocks the
    /// eframe render loop (and may deadlock the winit event loop on macOS).
    /// Create the async dialog on the UI thread, then await it and parse the
    /// selected file on a worker. The resulting definition is applied on the
    /// UI thread during a later frame.
    pub(super) fn pick_definition(&mut self, context: &egui::Context) {
        if self.picker.is_some() {
            return;
        }
        let future = rfd::AsyncFileDialog::new()
            .add_filter("Peninsula 数据源", &["json"])
            .set_title("导入 Peninsula 数据源")
            .pick_file();
        let (sender, receiver) = mpsc::channel();
        self.picker = Some(receiver);
        let repaint = context.clone();
        std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("文件选择器启动失败：{error}"))
                .and_then(|runtime| {
                    let selected = runtime.block_on(future);
                    selected.map(|file| read_definition_file(file.path())).transpose()
                });
            let _ = sender.send(result);
            repaint.request_repaint();
        });
    }

    pub(super) fn poll_definition(&mut self, context: &egui::Context, manager: &DataSourceManager) {
        let result = self.picker.as_ref().and_then(|receiver| {
            match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) =>
                    Some(Err("文件选择器意外退出，请重试。".to_owned())),
            }
        });
        let Some(result) = result else { return };
        self.picker = None;
        match result {
            Ok(Some(definition)) => {
                self.allow_private_network = false;
                self.status.clear();
                self.status_source = None;
                self.candidate_icon = manager.preview_icon(context, &definition);
                self.candidate = Some(definition);
            }
            Ok(None) => {} // Cancelled.
            Err(error) => self.set_error(error),
        }
    }

    /// Import permissions are reviewed in the independent data-source window.
    pub(super) fn import_review(&mut self, ui: &mut egui::Ui, manager: &mut DataSourceManager) {
        let Some(candidate) = self.candidate.clone() else { return; };
        let origin = candidate.origin().unwrap_or_else(|_| "无效请求目标".into());
        section_title(ui, "确认导入", "检查请求来源并明确授权后添加数据源。");
        panel(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                import_badge(ui, self.candidate_icon.as_ref());
                ui.vertical(|ui| {
                    ui.label(RichText::new(&candidate.name).strong());
                    ui.label(RichText::new(&candidate.id).small().weak());
                });
            });
            ui.add_space(10.0);
            ui.label(RichText::new("请求目标").small().weak());
            ui.add(egui::Label::new(RichText::new(&origin).monospace().color(accent(ui))).truncate())
                .on_hover_text(&origin);
            ui.add_space(8.0);
            ui.separator();
            for (request_id, request) in candidate.request.iter().map(|request| ("默认", request))
                .chain(candidate.requests.iter().map(|(id, request)| (id.as_str(), request)))
            {
                ui.horizontal(|ui| {
                    method_pill(ui, method_text(request.method));
                    ui.label(RichText::new(format!("请求地址  {request_id}")).small().weak());
                });
                ui.add(egui::Label::new(RichText::new(&request.url).monospace().small()).truncate())
                    .on_hover_text(&request.url);
            }
        });
        ui.add_space(12.0);
        form_surface(ui).show(ui, |ui| {
            switch_row(ui, &mut self.allow_private_network, "允许 HTTP / localhost / 私有网络目标");
        });
        if candidate.origins().is_ok_and(|origins| origins.iter().any(|origin| origin.starts_with("http://"))) {
            ui.colored_label(ui.visuals().warn_fg_color,
                "该目标使用未加密的 HTTP；开启上述权限后才会执行请求。");
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let tokens = crate::ui::palette::accent_tokens(ui.ctx());
            if ui.add(egui::Button::new(RichText::new("授权并导入").color(tokens.on_accent))
                .fill(tokens.accent)).clicked()
            {
                match manager.import(candidate.clone(), self.allow_private_network) {
                    Ok(_) => {
                        self.selected = Some(candidate.id.clone());
                        self.candidate = None;
                        self.candidate_icon = None;
                        self.status.clear();
                        self.status_source = None;
                    }
                    Err(error) => self.set_error(format!("导入失败：{error}")),
                }
            }
            if ui.button("取消").clicked() {
                self.candidate = None;
                self.candidate_icon = None;
            }
        });
    }

    /// Destructive actions use the same capsule page, keeping its hit region intact.
    pub(super) fn delete_review(&mut self, ui: &mut egui::Ui, manager: &mut DataSourceManager) {
        let Some(id) = self.delete_candidate.clone() else { return; };
        let name = manager.snapshots().into_iter()
            .find(|source| source.id == id).map(|source| source.name)
            .unwrap_or_else(|| id.clone());
        section_title(ui, "删除数据源", "删除后会移除其本地定义、配置和运行状态。");
        panel(ui).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("确定删除");
                ui.label(RichText::new(&name).strong());
                ui.label("吗？");
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("确认删除")
                    .fill(Color32::from_rgb(176, 56, 62))).clicked() {
                    match manager.delete(&id) {
                        Ok(()) => {
                            self.selected = None;
                            self.delete_candidate = None;
                            self.status.clear();
                            self.status_source = None;
                            self.text_drafts.retain(|key, _| !key.starts_with(&format!("{id}::")));
                            self.runtime_drafts.retain(|key, _| !key.starts_with(&format!("{id}::")));
                        }
                        Err(error) => self.set_error(format!("删除失败：{error}")),
                    }
                }
                if ui.button("取消").clicked() {
                    self.delete_candidate = None;
                }
            });
        });
    }

    pub(super) fn set_source_status(&mut self, source: &str, result: Result<(), String>) {
        self.status = result.map_or_else(|error| error, |_| String::new());
        self.status_source = (!self.status.is_empty()).then(|| source.to_owned());
    }

    pub(super) fn set_error(&mut self, error: impl Into<String>) {
        self.status = error.into();
        self.status_source = None;
    }

    pub(super) fn set_source_error(&mut self, source: &str, error: impl Into<String>) {
        self.status = error.into();
        self.status_source = (!self.status.is_empty()).then(|| source.to_owned());
    }
}

fn read_definition_file(path: &std::path::Path) -> Result<Definition, String> {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    if !name.ends_with(".pds.json") {
        return Err("请选择 .pds.json 文件。".into());
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("无法读取文件 {name}：{error}"))?;
    if metadata.len() > 2_000_000 {
        return Err(format!("数据源文件 {name} 超过 2 MB。"));
    }
    let source = fs::read_to_string(path)
        .map_err(|error| format!("无法读取文件 {name}：{error}"))?;
    Definition::parse(&source)
        .map_err(|error| format!("数据源 Schema 校验失败（{name}）：{error}"))
}

#[cfg(test)]
mod import_tests {
    use super::*;

    #[test]
    fn picker_rejects_bad_extensions_and_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let wrong = dir.path().join("anything.txt");
        fs::write(&wrong, "{}").unwrap();
        assert!(read_definition_file(&wrong).unwrap_err().contains(".pds.json"));
        let invalid = dir.path().join("broken.pds.json");
        fs::write(&invalid, "{").unwrap();
        assert!(read_definition_file(&invalid).unwrap_err().contains("Schema"));
    }
}
