use crate::metrics::Snapshot;
use eframe::egui::Color32;

pub(super) fn battery_text_color(metrics: &Snapshot) -> Color32 {
    if metrics.battery_percent.is_some_and(|value| value < 20) {
        Color32::from_rgb(255, 91, 91)
    } else if metrics.charging {
        Color32::from_rgb(35, 255, 172)
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
