use crate::metrics::{format_speed, Snapshot};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

use crate::app::battery;

const PANEL: Color32 = Color32::from_rgb(27, 29, 34);
const MUTED: Color32 = Color32::from_rgb(139, 145, 157);
const GREEN: Color32 = Color32::from_rgb(110, 231, 183);
const BLUE: Color32 = Color32::from_rgb(112, 175, 255);
const CARD_GAP: f32 = 8.0;
const PANEL_HEIGHT: f32 = 64.0;

pub(in crate::app) fn expanded_width(context: &egui::Context, metrics: &Snapshot) -> f32 {
    let content = card_models(metrics)
        .iter()
        .map(|card| {
            [
                context_text_width(context, card.title, 9.0),
                context_text_width(context, &max_value(card.title), 14.0),
                context_text_width(context, &max_subtitle(card.title), 8.0),
            ]
            .into_iter()
            .fold(48.0_f32, f32::max)
                + 12.0
        })
        .sum::<f32>()
        + CARD_GAP * 3.0;
    (content + 36.0).clamp(244.0, 360.0)
}

fn max_value(title: &str) -> String {
    match title {
        "CPU" | "GPU" => "100%".into(),
        "网络" => "↓ 999.9 MB/s".into(),
        _ => "100%".into(),
    }
}

fn max_subtitle(title: &str) -> String {
    match title {
        "网络" => "↑ 999.9 MB/s".into(),
        "电池" => "正在充电".into(),
        "CPU" => "系统总占用".into(),
        _ => "图形处理器".into(),
    }
}

pub(in crate::app) fn paint_metrics(
    ui: &egui::Ui,
    capsule: Rect,
    metrics: &Snapshot,
    progress: f32,
) -> Rect {
    let cards = card_models(metrics);
    let widths: Vec<f32> = cards.iter().map(|card| card_width(ui, card)).collect();
    let content_width = widths.iter().sum::<f32>() + CARD_GAP * (cards.len() - 1) as f32;
    let left = capsule.center().x - content_width * 0.5;
    let top = capsule.top();
    let rect = Rect::from_min_size(Pos2::new(left, top), Vec2::new(content_width, PANEL_HEIGHT));
    let reveal = Rect::from_min_max(
        Pos2::new(capsule.left(), top),
        Pos2::new(capsule.right(), top + PANEL_HEIGHT * progress),
    );
    let painter = ui.painter().with_clip_rect(reveal);
    let mut x = left;
    for (card, width) in cards.iter().zip(widths) {
        paint_metric_card(&painter, x, rect.top(), width, card, metrics, progress);
        x += width + CARD_GAP;
    }
    rect
}

struct MetricCard {
    title: &'static str,
    value: String,
    subtitle: String,
    color: Color32,
    battery: bool,
}

fn card_models(metrics: &Snapshot) -> [MetricCard; 4] {
    [
        MetricCard {
            title: "CPU",
            value: format!("{:.0}%", metrics.cpu_percent),
            subtitle: "系统总占用".into(),
            color: GREEN,
            battery: false,
        },
        MetricCard {
            title: "GPU",
            value: metrics
                .gpu_percent
                .map(|v| format!("{v:.0}%"))
                .unwrap_or_else(|| "不可用".into()),
            subtitle: "图形处理器".into(),
            color: Color32::from_rgb(202, 164, 255),
            battery: false,
        },
        MetricCard {
            title: "网络",
            value: format!("↓ {}", format_speed(metrics.download_bytes_per_second)),
            subtitle: format!("↑ {}", format_speed(metrics.upload_bytes_per_second)),
            color: BLUE,
            battery: false,
        },
        MetricCard {
            title: "电池",
            value: metrics
                .battery_percent
                .map(|value| format!("{value}%"))
                .unwrap_or_else(|| "--".into()),
            subtitle: if metrics.charging {
                "正在充电"
            } else {
                "电量"
            }
            .into(),
            color: battery::battery_text_color(metrics),
            battery: true,
        },
    ]
}

fn card_width(ui: &egui::Ui, card: &MetricCard) -> f32 {
    [
        text_width(ui, card.title, 9.0),
        text_width(ui, &card.value, 14.0),
        text_width(ui, &card.subtitle, 8.0),
    ]
    .into_iter()
    .fold(48.0_f32, f32::max)
        + 12.0
}

fn text_width(ui: &egui::Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(size), Color32::WHITE)
        .size()
        .x
}

fn context_text_width(context: &egui::Context, text: &str, size: f32) -> f32 {
    context.fonts(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), FontId::proportional(size), Color32::WHITE)
            .size()
            .x
    })
}

fn paint_metric_card(
    painter: &egui::Painter,
    x: f32,
    y: f32,
    width: f32,
    card: &MetricCard,
    metrics: &Snapshot,
    progress: f32,
) {
    let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, PANEL_HEIGHT));
    painter.rect_filled(rect, 9.0, PANEL.linear_multiply(progress));
    let paint_text = |point: Pos2, text: &str, size: f32, color: Color32| {
        painter.text(
            point,
            Align2::LEFT_TOP,
            text,
            FontId::proportional(size),
            color.linear_multiply(progress),
        );
    };
    paint_text(Pos2::new(x + 6.0, y + 4.0), card.title, 9.0, MUTED);
    if card.battery {
        let battery_rect = Rect::from_min_size(Pos2::new(x + 6.0, y + 23.0), Vec2::new(16.0, 14.0));
        let icon_color = if metrics.charging { BLUE } else { card.color };
        battery::paint_icon(
            painter,
            battery_rect,
            metrics.battery_percent,
            icon_color,
            metrics.charging,
            progress,
        );
        paint_text(Pos2::new(x + 27.0, y + 21.0), &card.value, 14.0, card.color);
        paint_text(Pos2::new(x + 6.0, y + 45.0), &card.subtitle, 8.0, MUTED);
    } else {
        paint_text(Pos2::new(x + 6.0, y + 18.0), &card.value, 14.0, card.color);
        paint_text(Pos2::new(x + 6.0, y + 41.0), &card.subtitle, 8.0, MUTED);
    }
}

#[cfg(test)]
mod tests {
    use super::{expanded_width, paint_metrics};
    use crate::metrics::Snapshot;
    use eframe::egui::{self, Pos2, Rect, Vec2};

    #[test]
    fn expanded_metrics_fit_inside_the_capsule() {
        let context = egui::Context::default();
        let mut metrics_rect = Rect::NOTHING;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 204.0))),
            ..Default::default()
        };
        let _ = context.run(input, |context| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(context, |ui| {
                    let capsule =
                        Rect::from_min_size(Pos2::new(15.0, 7.0), Vec2::new(330.0, 140.0));
                    metrics_rect = paint_metrics(ui, capsule, &Snapshot::default(), 1.0);
                    assert!(metrics_rect.left() >= capsule.left());
                    assert!(metrics_rect.right() <= capsule.right());
                    assert!(metrics_rect.bottom() < capsule.bottom());
                });
        });
        assert!(metrics_rect.width() > 240.0);
        assert!(metrics_rect.height() > 60.0);
    }

    #[test]
    fn expanded_width_stays_stable_as_live_network_rates_change() {
        let context = egui::Context::default();
        let mut widths = (0.0, 0.0);
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 204.0))),
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |_ui| {
                    let compact = Snapshot::default();
                    let busy = Snapshot {
                        download_bytes_per_second: 8_000_000.0,
                        upload_bytes_per_second: 3_000_000.0,
                        ..Snapshot::default()
                    };
                    widths = (
                        expanded_width(context, &compact),
                        expanded_width(context, &busy),
                    );
                });
            },
        );
        assert_eq!(widths.1, widths.0, "widths: {widths:?}");
        assert!(widths.1 <= 360.0, "capsule should stay compact: {widths:?}");
    }

    #[test]
    fn charging_battery_is_included_in_expanded_metrics() {
        let cards = super::card_models(&Snapshot {
            charging: true,
            battery_percent: Some(42),
            ..Snapshot::default()
        });
        assert!(cards[3].battery);
        assert_eq!(cards[3].subtitle, "正在充电");
    }
}
