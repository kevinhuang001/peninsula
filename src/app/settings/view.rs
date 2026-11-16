use super::{
    model::Section,
    quiet_view::{self, QuietDialog},
    theme::{
        ACCENT, ACCENT_SELECTED, BACKGROUND, BORDER, PANEL, ROW, SECONDARY_TEXT, SWITCH_OFF, TEXT,
    },
    CompactPreferences,
};
use crate::extension::{builtins, ExtensionId};
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
        self.paint_content(ui, content, preferences)
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

fn paint_compact_options(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
) -> bool {
    let mut changed = false;
    let registry = builtins();
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
        .rect_filled(switch, 9.0, if enabled { ACCENT } else { SWITCH_OFF });
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
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 5.0, Color32::from_white_alpha(18));
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        if response.hovered() {
            TEXT
        } else {
            SECONDARY_TEXT
        },
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
