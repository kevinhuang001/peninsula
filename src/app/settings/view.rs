use eframe::egui::{self, Align, Color32, Layout, RichText};

use super::CompactPreferences;

const BLUE: Color32 = Color32::from_rgb(0, 112, 245);
const SIDEBAR: Color32 = Color32::from_rgb(239, 240, 243);
const GROUP: Color32 = Color32::from_rgb(255, 255, 255);

pub struct SettingsPanel {
    selected: usize,
    search: String,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self {
            selected: 1,
            search: String::new(),
        }
    }
}

impl SettingsPanel {
    pub fn show(&mut self, ui: &mut egui::Ui, preferences: &mut CompactPreferences) -> bool {
        let mut close = false;
        ui.allocate_ui_with_layout(
            ui.available_size(),
            Layout::left_to_right(Align::Min),
            |ui| {
                self.sidebar(ui);
                ui.separator();
                close = self.content(ui, preferences);
            },
        );
        close
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SIDEBAR)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_min_width(180.0);
                ui.label(RichText::new("灵动岛设置").strong().size(20.0));
                ui.add_space(18.0);
                ui.add_sized(
                    [156.0, 32.0],
                    egui::TextEdit::singleline(&mut self.search).hint_text("搜索"),
                );
                ui.add_space(12.0);
                for (index, name) in ["通用", "收起状态", "番茄钟"].into_iter().enumerate()
                {
                    if ui.selectable_label(self.selected == index, name).clicked() {
                        self.selected = index;
                    }
                }
            });
    }

    fn content(&mut self, ui: &mut egui::Ui, prefs: &mut CompactPreferences) -> bool {
        ui.add_space(20.0);
        let mut close = false;
        ui.horizontal(|ui| {
            ui.heading(match self.selected {
                1 => "收起状态",
                2 => "番茄钟",
                _ => "通用",
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("完成").clicked() {
                    close = true;
                }
            });
        });
        ui.add_space(12.0);
        match self.selected {
            1 => self.compact_options(ui, prefs),
            2 => self.pomodoro_info(ui),
            _ => self.general_info(ui),
        }
        close
    }

    fn compact_options(&self, ui: &mut egui::Ui, prefs: &mut CompactPreferences) {
        ui.label("选择胶囊收起时显示的内容");
        ui.add_space(12.0);
        egui::Frame::group(ui.style()).fill(GROUP).show(ui, |ui| {
            for (label, value) in [
                ("动态表情", &mut prefs.mascot),
                ("电池与充电状态", &mut prefs.battery),
                ("CPU 使用率", &mut prefs.cpu),
                ("GPU 使用率", &mut prefs.gpu),
                ("网络速度", &mut prefs.network),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.checkbox(value, "");
                    });
                });
                ui.separator();
            }
        });
    }

    fn general_info(&self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style()).fill(GROUP).show(ui, |ui| {
            ui.label(RichText::new("灵动岛").strong());
            ui.label("悬停展开性能监控与番茄钟。拖动胶囊可将它放在屏幕任意位置。");
            ui.hyperlink_to(RichText::new("完成").color(BLUE), "#");
        });
    }

    fn pomodoro_info(&self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style()).fill(GROUP).show(ui, |ui| {
            ui.label(RichText::new("番茄钟").strong());
            ui.label("专注 25 分钟，休息 5 分钟。可在灵动岛的番茄钟标签中开始、暂停和重置。");
        });
    }
}
