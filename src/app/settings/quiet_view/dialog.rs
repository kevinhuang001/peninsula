use super::super::quiet_hours::{QuietHours, QuietPeriod};
use super::super::theme::{ACCENT, ACCENT_SELECTED, BORDER, PANEL, ROW, SECONDARY_TEXT, TEXT};
use super::DAYS;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuietDialog {
    pub(super) index: Option<usize>,
    pub(super) period: QuietPeriod,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    panel: Rect,
    quiet: &mut QuietHours,
    dialog: &mut Option<QuietDialog>,
) -> bool {
    let Some(editor) = dialog.as_mut() else {
        return false;
    };
    let backdrop = ui.interact(
        panel,
        egui::Id::new("quiet-period-backdrop"),
        Sense::click(),
    );
    ui.painter()
        .rect_filled(panel, 10.0, Color32::from_black_alpha(190));
    let popup = panel.shrink(10.0);
    ui.painter().rect_filled(popup, 10.0, PANEL);
    ui.painter().rect_stroke(
        popup,
        10.0,
        Stroke::new(1.0, BORDER),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        Pos2::new(popup.left() + 12.0, popup.top() + 13.0),
        Align2::LEFT_CENTER,
        if editor.index.is_some() {
            "编辑免打扰时段"
        } else {
            "添加免打扰时段"
        },
        FontId::proportional(12.0),
        TEXT,
    );
    paint_day_picker(ui, popup, &mut editor.period);
    draw_time_fields(ui, popup, &mut editor.period, true);
    draw_time_fields(ui, popup, &mut editor.period, false);
    let button_top = popup.bottom() - 28.0;
    let cancel = ui.put(
        Rect::from_min_size(
            Pos2::new(popup.left() + 12.0, button_top),
            Vec2::new(80.0, 21.0),
        ),
        egui::Button::new(egui::RichText::new("取消").color(TEXT)).frame(false),
    );
    let save = ui.put(
        Rect::from_min_size(
            Pos2::new(popup.right() - 92.0, button_top),
            Vec2::new(80.0, 21.0),
        ),
        egui::Button::new("完成")
            .fill(ACCENT)
            .corner_radius(7.0)
            .min_size(Vec2::new(70.0, 21.0)),
    );
    if backdrop.clicked() || cancel.clicked() {
        *dialog = None;
        return false;
    }
    if save.clicked() && editor.period.days != 0 {
        match editor.index {
            Some(index) => quiet.update_period(index, editor.period),
            None => quiet.add_period(editor.period),
        };
        *dialog = None;
        return true;
    }
    false
}

fn paint_day_picker(ui: &egui::Ui, popup: Rect, period: &mut QuietPeriod) {
    let left = popup.left() + 12.0;
    let top = popup.top() + 31.0;
    ui.painter().text(
        Pos2::new(left, top + 10.0),
        Align2::LEFT_CENTER,
        "星期",
        FontId::proportional(9.5),
        SECONDARY_TEXT,
    );
    let day_left = left + 34.0;
    let gap = 2.0;
    let width = ((popup.width() - 60.0) / 7.0).min(23.0);
    for (day, label) in DAYS.iter().enumerate() {
        let rect = Rect::from_min_size(
            Pos2::new(day_left + day as f32 * (width + gap), top),
            Vec2::new(width, 20.0),
        );
        let response = ui.interact(
            rect,
            egui::Id::new(("quiet-dialog-day", day)),
            Sense::click(),
        );
        if response.clicked() {
            period.toggle_day(day as u8);
        }
        let selected = period.includes(day as u8);
        ui.painter()
            .rect_filled(rect, 5.0, if selected { ACCENT_SELECTED } else { ROW });
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.5),
            if selected { TEXT } else { SECONDARY_TEXT },
        );
    }
}

fn draw_time_fields(ui: &mut egui::Ui, popup: Rect, period: &mut QuietPeriod, start: bool) {
    let top = popup.top() + if start { 61.0 } else { 84.0 };
    let minutes = if start {
        &mut period.start_minute
    } else {
        &mut period.end_minute
    };
    let mut hour = i32::from(*minutes / 60);
    let mut minute = i32::from(*minutes % 60);
    ui.painter().text(
        Pos2::new(popup.left() + 12.0, top + 10.0),
        Align2::LEFT_CENTER,
        if start { "开始" } else { "结束" },
        FontId::proportional(9.5),
        SECONDARY_TEXT,
    );
    let field_width = 42.0;
    let minute_rect = Rect::from_min_size(
        Pos2::new(popup.right() - 12.0 - field_width, top),
        Vec2::new(field_width, 20.0),
    );
    let hour_rect = minute_rect.translate(Vec2::new(-field_width - 8.0, 0.0));
    let hour_changed = ui
        .put(
            hour_rect,
            egui::DragValue::new(&mut hour)
                .range(0..=23)
                .fixed_decimals(0),
        )
        .changed();
    let minute_changed = ui
        .put(
            minute_rect,
            egui::DragValue::new(&mut minute)
                .range(0..=59)
                .fixed_decimals(0),
        )
        .changed();
    ui.painter().text(
        Pos2::new(hour_rect.right() + 4.0, hour_rect.center().y),
        Align2::CENTER_CENTER,
        ":",
        FontId::proportional(10.0),
        TEXT,
    );
    if hour_changed || minute_changed {
        *minutes = (hour.clamp(0, 23) * 60 + minute.clamp(0, 59)) as u16;
    }
}
