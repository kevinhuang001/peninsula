use crate::app::icons::{Icon, Icons};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

use super::{text_width, NETWORK_SPEED_SIZE};

pub(super) struct NetworkRate<'a> {
    pub center: Pos2,
    pub value: &'a str,
    pub icon: Icon,
    pub color: Color32,
    pub progress: f32,
}

pub(super) fn paint_rate(
    ui: &egui::Ui,
    painter: &egui::Painter,
    icons: &Icons,
    rate: NetworkRate<'_>,
) {
    let NetworkRate {
        center,
        value,
        icon,
        color,
        progress,
    } = rate;
    let width = text_width(ui, value, NETWORK_SPEED_SIZE);
    let group_width = 10.0 + 4.0 + width;
    let left = center.x - group_width * 0.5;
    icons.paint(
        painter,
        icon,
        Rect::from_min_size(Pos2::new(left, center.y - 5.0), Vec2::splat(10.0)),
        color,
        progress,
    );
    painter.text(
        Pos2::new(left + 14.0, center.y),
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(NETWORK_SPEED_SIZE),
        color.linear_multiply(progress),
    );
}
