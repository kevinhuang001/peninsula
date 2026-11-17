mod dialog;

use super::quiet_hours::{QuietHours, QuietPeriod, MAX_QUIET_PERIODS};
use super::theme::{
    ACCENT, ACCENT_BORDER, ACCENT_SELECTED, ACCENT_SOFT, DANGER, ROW, ROW_DISABLED, SECONDARY_TEXT,
    TEXT,
};
pub(super) use dialog::QuietDialog;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};
const DAYS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

pub(super) fn show(
    ui: &mut egui::Ui,
    panel: Rect,
    quiet: &mut QuietHours,
    dialog: &mut Option<QuietDialog>,
) -> bool {
    let list = Rect::from_min_max(
        Pos2::new(panel.left() + 8.0, panel.top() + 39.0),
        Pos2::new(panel.right() - 8.0, panel.bottom() - 35.0),
    );
    let mut changed = draw_period_list(ui, list, quiet, dialog);
    changed |= draw_add_button(ui, panel, quiet, dialog);
    changed |= dialog::show(ui, panel, quiet, dialog);
    changed
}

fn draw_period_list(
    ui: &mut egui::Ui,
    list: Rect,
    quiet: &mut QuietHours,
    dialog: &mut Option<QuietDialog>,
) -> bool {
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let count = quiet.periods().len();
                if count == 0 {
                    ui.painter().text(
                        list.center(),
                        Align2::CENTER_CENTER,
                        "还没有免打扰时段",
                        FontId::proportional(10.5),
                        SECONDARY_TEXT,
                    );
                }
                for index in 0..count {
                    let period = quiet.period(index).unwrap();
                    let row = Rect::from_min_size(
                        Pos2::new(list.left(), list.top() + index as f32 * 28.0),
                        Vec2::new(list.width(), 24.0),
                    );
                    changed |= period_row(ui, row, index, period, quiet, dialog);
                }
            });
    });
    changed
}

fn period_row(
    ui: &mut egui::Ui,
    row: Rect,
    index: usize,
    period: QuietPeriod,
    quiet: &mut QuietHours,
    dialog: &mut Option<QuietDialog>,
) -> bool {
    let response = ui.interact(row, egui::Id::new(("quiet-period", index)), Sense::click());
    ui.painter()
        .rect_filled(row, 6.0, if period.enabled { ROW } else { ROW_DISABLED });
    ui.painter().circle_filled(
        Pos2::new(row.left() + 10.0, row.center().y),
        3.0,
        if period.enabled {
            ACCENT
        } else {
            SECONDARY_TEXT
        },
    );
    ui.painter().text(
        Pos2::new(row.left() + 18.0, row.center().y),
        Align2::LEFT_CENTER,
        period_label(period),
        FontId::proportional(9.2),
        if period.enabled { TEXT } else { SECONDARY_TEXT },
    );
    let edit = action_button(
        ui,
        Rect::from_center_size(
            Pos2::new(row.right() - 47.0, row.center().y),
            Vec2::new(35.0, 20.0),
        ),
        ("quiet-edit", index),
        "编辑",
        TEXT,
    );
    let remove = action_button(
        ui,
        Rect::from_center_size(
            Pos2::new(row.right() - 15.0, row.center().y),
            Vec2::splat(20.0),
        ),
        ("quiet-remove", index),
        "×",
        DANGER,
    );
    if edit {
        *dialog = Some(QuietDialog {
            index: Some(index),
            period,
        });
    } else if remove {
        quiet.remove_period(index);
        return true;
    } else if response.clicked() {
        quiet.update_period(
            index,
            QuietPeriod {
                enabled: !period.enabled,
                ..period
            },
        );
        return true;
    }
    edit || remove
}

fn draw_add_button(
    ui: &mut egui::Ui,
    panel: Rect,
    quiet: &QuietHours,
    dialog: &mut Option<QuietDialog>,
) -> bool {
    let rect = Rect::from_min_size(
        Pos2::new(panel.left() + 8.0, panel.bottom() - 29.0),
        Vec2::new(panel.width() - 16.0, 22.0),
    );
    let can_add = quiet.periods().len() < MAX_QUIET_PERIODS;
    ui.painter().rect_filled(rect, 8.0, ACCENT_SOFT);
    ui.painter().rect_stroke(
        rect,
        8.0,
        egui::Stroke::new(1.0_f32, ACCENT_BORDER),
        egui::StrokeKind::Inside,
    );
    let response = ui.interact(rect, egui::Id::new("quiet-add-period"), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 8.0, Color32::from_white_alpha(16));
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        if can_add {
            "+ 添加时间段"
        } else {
            "已达到时段上限"
        },
        FontId::proportional(11.0),
        if can_add { TEXT } else { SECONDARY_TEXT },
    );
    if response.clicked() && can_add {
        *dialog = Some(QuietDialog {
            index: None,
            period: QuietPeriod::default(),
        });
        return true;
    }
    false
}

fn action_button(
    ui: &egui::Ui,
    rect: Rect,
    id: impl std::hash::Hash,
    label: &str,
    color: Color32,
) -> bool {
    let response = ui.interact(rect, egui::Id::new(id), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 5.0, ACCENT_SELECTED);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(if label == "×" { 14.0 } else { 10.5 }),
        if response.hovered() { TEXT } else { color },
    );
    response.clicked()
}

fn period_label(period: QuietPeriod) -> String {
    let days = if period.days == 0b0001_1111 {
        "工作日".to_owned()
    } else if period.days == 0b0110_0000 {
        "周末".to_owned()
    } else {
        let selected: String = DAYS
            .iter()
            .enumerate()
            .filter(|(day, _)| period.includes(*day as u8))
            .map(|(_, day)| *day)
            .collect();
        format!("周{selected}")
    };
    format!(
        "{days}  {:02}:{:02}–{:02}:{:02}",
        period.start_minute / 60,
        period.start_minute % 60,
        period.end_minute / 60,
        period.end_minute % 60
    )
}

#[cfg(test)]
mod tests {
    use super::period_label;
    use crate::app::settings::quiet_hours::QuietPeriod;

    #[test]
    fn period_summary_uses_compact_weekday_and_time_labels() {
        assert_eq!(period_label(QuietPeriod::default()), "工作日  22:00–07:00");
        assert_eq!(
            period_label(QuietPeriod {
                days: 0b0110_0000,
                ..QuietPeriod::default()
            }),
            "周末  22:00–07:00"
        );
    }
}
