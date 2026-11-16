use crate::{app::settings::CompactPreferences, metrics::Snapshot};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};

use super::super::{battery, mascot::Mascot};

const FACE_SIZE: Vec2 = Vec2::new(42.0, 34.0);

pub(super) fn show(
    ui: &egui::Ui,
    context: &egui::Context,
    rect: Rect,
    metrics: &Snapshot,
    mascot: &mut Mascot,
    progress: f32,
    prefs: &CompactPreferences,
) -> bool {
    let opacity = 1.0 - progress.clamp(0.0, 1.0);
    let widths = compact_item_widths(ui, metrics, prefs);
    let total = widths.iter().sum::<f32>() + (widths.len().saturating_sub(1) as f32) * 10.0;
    let mut x = rect.center().x - total * 0.5;
    let center_y = rect.center().y;
    if prefs.mascot {
        let face = Rect::from_center_size(Pos2::new(x + FACE_SIZE.x * 0.5, center_y), FACE_SIZE);
        let response = ui.interact(face, egui::Id::new("island-mascot"), Sense::click());
        mascot.paint(context, ui.painter(), face, opacity);
        if response.clicked() {
            mascot.advance(0.0, true);
            context.request_repaint();
        }
        x += widths[0] + 10.0;
    }
    if prefs.battery {
        battery::paint_chip(ui.painter(), Pos2::new(x, center_y - 7.0), metrics, opacity);
        x += widths[usize::from(prefs.mascot)] + 10.0;
    }
    let mut metric_index = usize::from(prefs.mascot) + usize::from(prefs.battery);
    for (enabled, value) in [
        (prefs.cpu, format!("CPU {:.0}%", metrics.cpu_percent)),
        (
            prefs.gpu,
            format!(
                "GPU {}",
                metrics
                    .gpu_percent
                    .map_or("--".into(), |n| format!("{n:.0}%"))
            ),
        ),
        (
            prefs.network,
            format!(
                "↓ {}",
                crate::metrics::format_speed(metrics.download_bytes_per_second)
            ),
        ),
    ] {
        if enabled {
            ui.painter().text(
                Pos2::new(x, center_y),
                Align2::LEFT_CENTER,
                value,
                FontId::proportional(10.0),
                Color32::from_rgb(112, 175, 255).linear_multiply(opacity),
            );
            x += widths[metric_index] + 10.0;
            metric_index += 1;
        }
    }
    false
}

fn compact_item_widths(ui: &egui::Ui, metrics: &Snapshot, prefs: &CompactPreferences) -> Vec<f32> {
    let mut widths = Vec::new();
    if prefs.mascot {
        widths.push(FACE_SIZE.x);
    }
    if prefs.battery {
        let value = metrics
            .battery_percent
            .map_or("--".into(), |n| format!("{n}%"));
        widths.push(20.0 + text_width(ui, &value, 12.0));
    }
    if prefs.cpu {
        widths.push(text_width(ui, "CPU 100%", 10.0));
    }
    if prefs.gpu {
        widths.push(text_width(ui, "GPU 100%", 10.0));
    }
    if prefs.network {
        widths.push(text_width(ui, "↓ 999.9 MB/s", 10.0));
    }
    widths
}

fn text_width(ui: &egui::Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(text.into(), FontId::proportional(size), Color32::WHITE)
        .size()
        .x
}
