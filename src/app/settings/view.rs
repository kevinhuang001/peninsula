use super::{
    model::Section,
    quiet_view::{self, QuietDialog},
    theme::{
        ACCENT, ACCENT_SELECTED, BACKGROUND, BORDER, PANEL, ROW, SECONDARY_TEXT, SETTINGS_BUTTON,
        SWITCH_OFF, SWITCH_ON, TEXT,
    },
    CompactPreferences,
};
use crate::{builtins::registry, extension::ExtensionId};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

const COMPACT_ROW_TOP: f32 = 43.0;
const COMPACT_ROW_STEP: f32 = 32.0;
const COMPACT_ROW_HEIGHT: f32 = 26.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettingsView {
    section: Section,
    quiet_dialog: Option<QuietDialog>,
}

impl SettingsView {
    pub const WIDTH: f32 = 400.0;

    pub fn extra_height(&self) -> f32 {
        self.section.extra_height()
    }

    pub(in crate::app) fn show(
        &mut self,
        ui: &mut egui::Ui,
        capsule: Rect,
        preferences: &mut CompactPreferences,
        bilibili_status: &str,
    ) -> bool {
        let body = Rect::from_min_max(
            Pos2::new(capsule.left() + 12.0, capsule.top() + 41.0),
            Pos2::new(capsule.right() - 12.0, capsule.bottom() - 8.0),
        );
        let sidebar = Rect::from_min_size(body.min, Vec2::new(108.0, body.height()));
        let content = Rect::from_min_max(Pos2::new(sidebar.right() + 9.0, body.top()), body.max);
        paint_panel(ui, sidebar, BACKGROUND);
        paint_panel(ui, content, PANEL);
        self.paint_sections(ui, sidebar);
        self.paint_content(ui, content, preferences, bilibili_status)
    }

    fn paint_sections(&mut self, ui: &egui::Ui, sidebar: Rect) {
        for (index, section) in Section::ALL.into_iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(
                    sidebar.left() + 5.0,
                    sidebar.top() + 8.0 + index as f32 * 36.0,
                ),
                Vec2::new(sidebar.width() - 10.0, 30.0),
            );
            let response = ui.interact(
                row,
                egui::Id::new(("settings-section", section)),
                Sense::click(),
            );
            let selected = self.section == section;
            if response.clicked() {
                self.section = section;
            }
            ui.painter().rect_filled(
                row,
                8.0,
                if selected {
                    ACCENT_SELECTED
                } else {
                    BACKGROUND
                },
            );
            if selected {
                ui.painter().rect_filled(
                    Rect::from_min_size(row.left_top() + Vec2::new(1.0, 7.0), Vec2::new(3.0, 16.0)),
                    2.0,
                    ACCENT,
                );
            }
            ui.painter().text(
                Pos2::new(row.left() + 12.0, row.center().y),
                Align2::LEFT_CENTER,
                section.label(),
                FontId::proportional(12.0),
                if selected { TEXT } else { SECONDARY_TEXT },
            );
        }
    }

    fn paint_content(
        &mut self,
        ui: &mut egui::Ui,
        panel: Rect,
        preferences: &mut CompactPreferences,
        bilibili_status: &str,
    ) -> bool {
        let painter = ui.painter().clone();
        painter.text(
            Pos2::new(panel.left() + 14.0, panel.top() + 9.0),
            Align2::LEFT_TOP,
            self.section.title(),
            FontId::proportional(15.0),
            TEXT,
        );
        let description = self.section.description();
        if !description.is_empty() {
            painter.text(
                Pos2::new(panel.left() + 14.0, panel.top() + 30.0),
                Align2::LEFT_TOP,
                description,
                FontId::proportional(11.0),
                SECONDARY_TEXT,
            );
        }
        match self.section {
            Section::Compact => paint_compact_options(ui, panel, preferences),
            Section::QuietHours => quiet_view::show(
                ui,
                panel,
                &mut preferences.quiet_hours,
                &mut self.quiet_dialog,
            ),
            Section::Pomodoro => paint_pomodoro_options(ui, panel, preferences),
            Section::Bilibili => {
                let changed = paint_bilibili_options(ui, panel, preferences);
                painter.text(
                    Pos2::new(panel.left() + 14.0, panel.top() + 282.0),
                    Align2::LEFT_TOP,
                    bilibili_status,
                    FontId::proportional(9.5),
                    SECONDARY_TEXT,
                );
                changed
            }
            Section::About => {
                painter.text(
                    Pos2::new(panel.left() + 14.0, panel.top() + 76.0),
                    Align2::LEFT_TOP,
                    "性能监控 · 番茄钟 · 下载提醒",
                    FontId::proportional(11.5),
                    TEXT,
                );
                painter.text(
                    Pos2::new(panel.left() + 14.0, panel.top() + 100.0),
                    Align2::LEFT_TOP,
                    "灵动岛  0.1.0",
                    FontId::proportional(10.5),
                    SECONDARY_TEXT,
                );
                false
            }
        }
    }
}

fn paint_bilibili_options(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
) -> bool {
    let mut changed = false;
    let rows = [
        ("启用 Bilibili 通知", &mut preferences.bilibili_enabled),
        ("新私信通知", &mut preferences.bilibili_dm_enabled),
        ("收到的赞通知", &mut preferences.bilibili_likes_enabled),
        (
            "显示私信内容摘要",
            &mut preferences.bilibili_show_dm_summary,
        ),
    ];
    for (index, (label, enabled)) in rows.into_iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(
                panel.left() + 12.0,
                panel.top() + 60.0 + index as f32 * 30.0,
            ),
            Vec2::new(panel.width() - 24.0, 26.0),
        );
        changed |= paint_checkbox_row(ui, row, index, label, enabled);
    }
    let interval = Rect::from_min_size(
        Pos2::new(panel.left() + 12.0, panel.top() + 182.0),
        Vec2::new(panel.width() - 24.0, 28.0),
    );
    ui.painter().text(
        interval.left_center() + Vec2::new(3.0, 0.0),
        Align2::LEFT_CENTER,
        "检查间隔",
        FontId::proportional(11.0),
        TEXT,
    );
    changed |= ui
        .put(
            Rect::from_min_size(
                Pos2::new(interval.right() - 92.0, interval.top()),
                Vec2::new(86.0, 27.0),
            ),
            egui::DragValue::new(&mut preferences.bilibili_poll_seconds)
                .range(15..=3600)
                .suffix(" 秒"),
        )
        .changed();
    let path = Rect::from_min_size(
        Pos2::new(panel.left() + 12.0, panel.top() + 216.0),
        Vec2::new(panel.width() - 24.0, 28.0),
    );
    changed |= ui
        .put(
            path,
            egui::TextEdit::singleline(&mut preferences.opencli_path)
                .hint_text("OpenCLI 路径（自动检测）"),
        )
        .changed();
    let buttons = Rect::from_min_size(
        Pos2::new(panel.left() + 12.0, panel.top() + 250.0),
        Vec2::new(panel.width() - 24.0, 26.0),
    );
    let each_width = (buttons.width() - 8.0) * 0.5;
    let check_clicked = ui
        .put(
            Rect::from_min_size(buttons.min, Vec2::new(each_width, buttons.height())),
            egui::Button::new(egui::RichText::new("检查连接").color(Color32::WHITE))
                .fill(SETTINGS_BUTTON),
        )
        .clicked();
    let immediate_clicked = ui
        .put(
            Rect::from_min_size(
                Pos2::new(buttons.left() + each_width + 8.0, buttons.top()),
                Vec2::new(each_width, buttons.height()),
            ),
            egui::Button::new(egui::RichText::new("立即检查").color(Color32::WHITE))
                .fill(SETTINGS_BUTTON),
        )
        .clicked();
    if check_clicked || immediate_clicked {
        preferences.bilibili_check_requested = true;
        changed = true;
    }
    changed
}

fn paint_checkbox_row(
    ui: &mut egui::Ui,
    row: Rect,
    index: usize,
    label: &str,
    enabled: &mut bool,
) -> bool {
    let response = ui.interact(
        row,
        egui::Id::new(("bilibili-option", index)),
        Sense::click(),
    );
    ui.painter().rect_filled(row, 6.0, ROW);
    let checkbox = Rect::from_center_size(
        Pos2::new(row.left() + 19.0, row.center().y),
        Vec2::splat(16.0),
    );
    ui.painter()
        .rect_filled(checkbox, 4.0, if *enabled { SWITCH_ON } else { BACKGROUND });
    ui.painter().rect_stroke(
        checkbox,
        4.0,
        Stroke::new(1.0, if *enabled { SWITCH_ON } else { BORDER }),
        egui::StrokeKind::Inside,
    );
    if *enabled {
        let points = [
            Pos2::new(checkbox.left() + 4.0, checkbox.center().y),
            Pos2::new(checkbox.left() + 7.0, checkbox.bottom() - 4.0),
            Pos2::new(checkbox.right() - 3.0, checkbox.top() + 4.0),
        ];
        ui.painter().add(egui::Shape::line(
            points.to_vec(),
            Stroke::new(1.7, Color32::WHITE),
        ));
    }
    ui.painter().text(
        Pos2::new(row.left() + 34.0, row.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        TEXT,
    );
    if response.clicked() {
        *enabled = !*enabled;
        true
    } else {
        false
    }
}

fn paint_pomodoro_options(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
) -> bool {
    let mut changed = false;
    for (index, (label, value)) in [
        ("专注时长", &mut preferences.pomodoro_focus_minutes),
        ("休息时长", &mut preferences.pomodoro_break_minutes),
    ]
    .into_iter()
    .enumerate()
    {
        let row = Rect::from_min_size(
            Pos2::new(
                panel.left() + 10.0,
                panel.top() + 62.0 + index as f32 * 44.0,
            ),
            Vec2::new(panel.width() - 20.0, 36.0),
        );
        ui.painter().rect_filled(row, 7.0, ROW);
        ui.painter().text(
            Pos2::new(row.left() + 10.0, row.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.0),
            TEXT,
        );
        let response = ui.put(
            Rect::from_min_size(
                Pos2::new(row.right() - 95.0, row.top() + 4.0),
                Vec2::new(86.0, 28.0),
            ),
            egui::DragValue::new(value).range(1..=180).suffix(" 分钟"),
        );
        changed |= response.changed();
    }
    changed
}

fn paint_compact_options(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
) -> bool {
    let mut changed = false;
    let registry = registry();
    let items: Vec<_> = preferences
        .ordered_compact_items()
        .into_iter()
        .filter_map(|item| {
            registry.get(&item.id).map(|descriptor| {
                (
                    item.id.clone(),
                    descriptor.title.clone(),
                    preferences.is_enabled(&item.id),
                )
            })
        })
        .collect();
    for (index, (id, title, enabled)) in items.into_iter().enumerate() {
        let row = compact_row(panel, index);
        changed |= paint_option(ui, row, preferences, id, &title, enabled);
    }
    changed
}

fn compact_row(panel: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(
            panel.left() + 8.0,
            panel.top() + COMPACT_ROW_TOP + index as f32 * COMPACT_ROW_STEP,
        ),
        Vec2::new(panel.width() - 16.0, COMPACT_ROW_HEIGHT),
    )
}

fn paint_panel(ui: &egui::Ui, rect: Rect, fill: Color32) {
    ui.painter().rect_filled(rect, 10.0, fill);
    ui.painter().rect_stroke(
        rect,
        10.0,
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );
}

fn paint_option(
    ui: &mut egui::Ui,
    rect: Rect,
    preferences: &mut CompactPreferences,
    option: ExtensionId,
    title: &str,
    enabled: bool,
) -> bool {
    ui.painter().rect_filled(rect, 7.0, ROW);
    ui.painter().text(
        Pos2::new(rect.left() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        TEXT,
    );
    let up = text_button(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.right() - 67.0, rect.center().y),
            Vec2::new(18.0, 22.0),
        ),
        ("compact-order-up", option.clone()),
        "↑",
    );
    let down = text_button(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.right() - 47.0, rect.center().y),
            Vec2::new(18.0, 22.0),
        ),
        ("compact-order-down", option.clone()),
        "↓",
    );
    let switch = Rect::from_center_size(
        Pos2::new(rect.right() - 19.0, rect.center().y),
        Vec2::new(30.0, 18.0),
    );
    let toggle = ui.interact(
        switch,
        egui::Id::new(("settings-option", option.clone())),
        Sense::click(),
    );
    if toggle.clicked() {
        preferences.set_enabled(&option, !enabled);
    }
    ui.painter()
        .rect_filled(switch, 9.0, if enabled { SWITCH_ON } else { SWITCH_OFF });
    ui.painter().circle_filled(
        Pos2::new(
            if enabled {
                switch.right() - 9.0
            } else {
                switch.left() + 9.0
            },
            switch.center().y,
        ),
        7.0,
        TEXT,
    );
    let mut changed = toggle.clicked();
    if up {
        changed |= preferences.move_compact_item(&option, -1);
    }
    if down {
        changed |= preferences.move_compact_item(&option, 1);
    }
    changed
}

fn text_button(ui: &egui::Ui, rect: Rect, id: impl std::hash::Hash, label: &str) -> bool {
    let response = ui.interact(rect, egui::Id::new(id), Sense::click());
    ui.painter().rect_filled(rect, 5.0, SETTINGS_BUTTON);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::WHITE,
    );
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::{compact_row, Section, SettingsView, COMPACT_ROW_STEP};
    use eframe::egui::{pos2, vec2, Rect};

    #[test]
    fn compact_options_use_one_column_and_fit_inside_the_taller_panel() {
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(260.0, 269.0));
        let rows: Vec<_> = (0..7).map(|index| compact_row(panel, index)).collect();
        assert!(rows.iter().all(|row| row.left() == rows[0].left()));
        assert!(rows
            .windows(2)
            .all(|pair| (pair[1].top() - pair[0].top() - COMPACT_ROW_STEP).abs() < 0.01));
        assert!(rows.last().unwrap().bottom() < panel.bottom());
    }

    #[test]
    fn settings_height_tracks_the_selected_section() {
        let mut view = SettingsView::default();
        assert_eq!(view.extra_height(), Section::Compact.extra_height());
        view.section = Section::QuietHours;
        assert_eq!(view.extra_height(), Section::QuietHours.extra_height());
    }
}
