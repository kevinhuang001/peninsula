use crate::metrics::Snapshot;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Vec2};

use super::{
    compact_layout::{COMPACT_ICON_GAP, COMPACT_ICON_SIZE, COMPACT_TEXT_SIZE},
    icons::Icons,
};

const GREEN: Color32 = Color32::from_rgb(35, 255, 172);
const BLUE: Color32 = Color32::from_rgb(80, 190, 255);

pub(super) fn paint_chip(
    ui: &egui::Ui,
    rect: Rect,
    metrics: &Snapshot,
    opacity: f32,
    icons: &Icons,
) -> f32 {
    let percent = metrics
        .battery_percent
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "--".into());
    let label_color = battery_text_color(metrics);
    let icon_color = if metrics.battery_percent.is_some_and(|value| value < 20) && !metrics.charging
    {
        label_color
    } else {
        BLUE
    };
    let label = ui.painter().layout_no_wrap(
        percent,
        FontId::proportional(COMPACT_TEXT_SIZE),
        label_color,
    );
    let group_width = COMPACT_ICON_SIZE + COMPACT_ICON_GAP + label.size().x;
    let group_left = rect.center().x - group_width * 0.5;
    icons.paint_battery(
        ui.painter(),
        Rect::from_min_size(
            Pos2::new(group_left, rect.center().y - COMPACT_ICON_SIZE * 0.5),
            Vec2::splat(COMPACT_ICON_SIZE),
        ),
        icon_color,
        metrics.charging,
        opacity,
    );
    ui.painter().galley(
        Pos2::new(
            group_left + COMPACT_ICON_SIZE + COMPACT_ICON_GAP,
            rect.center().y - label.size().y * 0.5,
        ),
        label.clone(),
        label_color.linear_multiply(opacity),
    );
    COMPACT_ICON_SIZE + COMPACT_ICON_GAP + label.size().x
}

pub(super) fn battery_text_color(metrics: &Snapshot) -> Color32 {
    if metrics.battery_percent.is_some_and(|value| value < 20) {
        Color32::from_rgb(255, 91, 91)
    } else if metrics.charging {
        GREEN
    } else {
        Color32::from_rgb(245, 247, 250)
    }
}

#[cfg(test)]
mod tests {
    use super::battery_text_color;
    use crate::metrics::Snapshot;
    use eframe::egui::Color32;

    #[test]
    fn battery_color_is_high_contrast_for_low_charge_and_charging() {
        assert_eq!(
            battery_text_color(&Snapshot {
                battery_percent: Some(10),
                ..Snapshot::default()
            }),
            Color32::from_rgb(255, 91, 91)
        );
        assert_eq!(
            battery_text_color(&Snapshot {
                battery_percent: Some(80),
                charging: true,
                ..Snapshot::default()
            }),
            Color32::from_rgb(35, 255, 172)
        );
    }
}
