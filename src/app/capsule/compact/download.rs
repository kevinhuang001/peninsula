use eframe::egui::{Pos2, Vec2};

fn progress_points(center: Pos2, radius: f32, fraction: f32) -> Vec<Pos2> {
    let fraction = fraction.clamp(0.0, 1.0);
    let segments = (fraction * 48.0).ceil().max(1.0) as usize;
    (0..=segments)
        .map(|index| {
            let angle = -std::f32::consts::FRAC_PI_2
                + std::f32::consts::TAU * fraction * index as f32 / segments as f32;
            center + Vec2::new(angle.cos(), angle.sin()) * radius
        })
        .collect()
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
