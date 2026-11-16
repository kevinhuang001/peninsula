use crate::app::compact_layout::{COMPACT_ICON_GAP, COMPACT_ICON_SIZE, COMPACT_TEXT_SIZE};
use crate::{downloads::Download, metrics::format_speed};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};

const PRIMARY: Color32 = Color32::from_rgb(247, 249, 252);
const DOWN: Color32 = Color32::from_rgb(80, 190, 255);

pub(super) fn paint_download(ui: &egui::Ui, chip: Rect, download: &Download, opacity: f32) {
    let painter = ui.painter();
    let icon_left = 6.0;
    let center = Pos2::new(
        chip.left() + icon_left + COMPACT_ICON_SIZE * 0.5,
        chip.center().y,
    );
    let radius = COMPACT_ICON_SIZE * 0.5;
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(2.0, Color32::from_rgb(61, 70, 84).linear_multiply(opacity)),
    );
    if let Some(fraction) = download.progress() {
        if fraction > 0.0 {
            painter.add(egui::Shape::line(
                progress_points(center, radius, fraction),
                Stroke::new(2.0, DOWN.linear_multiply(opacity)),
            ));
        }
        painter.text(
            center,
            Align2::CENTER_CENTER,
            format!("{:.0}%", fraction * 100.0),
            FontId::proportional(5.8),
            PRIMARY.linear_multiply(opacity),
        );
    } else {
        painter.text(
            center,
            Align2::CENTER_CENTER,
            "↓",
            FontId::proportional(COMPACT_ICON_SIZE * 0.75),
            DOWN.linear_multiply(opacity),
        );
    }
    let label = download.total.filter(|total| *total > 0).map_or_else(
        || compact_size(download.received),
        |total| compact_pair(download.received, total),
    );
    painter.text(
        Pos2::new(
            chip.left() + icon_left + COMPACT_ICON_SIZE + COMPACT_ICON_GAP,
            chip.center().y - 6.0,
        ),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(COMPACT_TEXT_SIZE),
        PRIMARY.linear_multiply(opacity),
    );
    painter.text(
        Pos2::new(
            chip.left() + icon_left + COMPACT_ICON_SIZE + COMPACT_ICON_GAP,
            chip.center().y + 6.0,
        ),
        Align2::LEFT_CENTER,
        format_speed(download.bytes_per_second),
        FontId::proportional(COMPACT_TEXT_SIZE),
        DOWN.linear_multiply(opacity),
    );
}

fn progress_points(center: Pos2, radius: f32, fraction: f32) -> Vec<Pos2> {
    let fraction = fraction.clamp(0.0, 1.0);
    let segments = (fraction * 48.0).ceil().max(1.0) as usize;
    let mut points = Vec::with_capacity(segments + 1);
    for index in 0..=segments {
        let angle = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::TAU * fraction * index as f32 / segments as f32;
        points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
    }
    points
}

fn compact_pair(received: u64, total: u64) -> String {
    let unit = unit_index(received.max(total));
    let divisor = 1024_f64.powi(unit as i32);
    format!(
        "{}/{}{}",
        compact_number(received as f64 / divisor),
        compact_number(total as f64 / divisor),
        ["B", "K", "M", "G"][unit]
    )
}

fn compact_size(bytes: u64) -> String {
    let unit = unit_index(bytes);
    let value = bytes as f64 / 1024_f64.powi(unit as i32);
    format!("{}{}", compact_number(value), ["B", "K", "M", "G"][unit])
}

fn unit_index(bytes: u64) -> usize {
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024 && unit < 3 {
        value /= 1024;
        unit += 1;
    }
    unit
}

fn compact_number(value: f64) -> String {
    if value >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::{compact_pair, compact_size, progress_points};
    use eframe::egui::Pos2;

    #[test]
    fn download_ring_reflects_real_fraction_without_animation() {
        let points = progress_points(Pos2::ZERO, 8.0, 0.25);
        assert_eq!(points.len(), 13);
        assert!((points.first().unwrap().y + 8.0).abs() < 0.01);
        assert!((points.last().unwrap().x - 8.0).abs() < 0.01);
    }

    #[test]
    fn compact_download_labels_include_received_and_total_sizes() {
        assert_eq!(compact_size(256), "256B");
        assert_eq!(compact_size(1536), "1.5K");
        assert_eq!(compact_pair(139_200_000, 250_000_000), "133/238M");
    }
}
