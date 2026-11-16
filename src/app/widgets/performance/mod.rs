use crate::{
    app::icons::{Icon, Icons},
    metrics::{format_speed, Snapshot},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

use crate::app::battery;
use network::{paint_rate, NetworkRate};

mod network;

const PANEL: Color32 = Color32::from_rgb(27, 29, 34);
const MUTED: Color32 = Color32::from_rgb(248, 250, 253);
const GREEN: Color32 = Color32::from_rgb(35, 255, 172);
const BLUE: Color32 = Color32::from_rgb(73, 183, 255);
const UPLOAD: Color32 = Color32::from_rgb(35, 255, 172);
const NETWORK_SPEED_SIZE: f32 = 12.0;
const NETWORK_UPLOAD_SIZE: f32 = NETWORK_SPEED_SIZE;
const CARD_GAP: f32 = 8.0;
const PANEL_HEIGHT: f32 = 64.0;

pub(in crate::app) fn expanded_width(context: &egui::Context, metrics: &Snapshot) -> f32 {
    let content = card_models(metrics)
        .iter()
        .map(|card| {
            [
                context_text_width(context, card.title, 11.0),
                context_text_width(context, &max_value(card.title), 16.0),
                context_text_width(context, &max_subtitle(card.title), NETWORK_UPLOAD_SIZE),
            ]
            .into_iter()
            .fold(48.0_f32, f32::max)
                + if card.title == "电池" { 20.0 } else { 12.0 }
        })
        .sum::<f32>()
        + CARD_GAP * 3.0;
    (content + 36.0).clamp(244.0, 360.0)
}

fn max_value(title: &str) -> String {
    match title {
        "CPU" | "GPU" => "100%".into(),
        "网络" => "999.9 MB/s".into(),
        _ => "100%".into(),
    }
}

fn max_subtitle(title: &str) -> String {
    match title {
        "网络" => "999.9 MB/s".into(),
        _ => "".into(),
    }
}

pub(in crate::app) fn paint_metrics(
    ui: &egui::Ui,
    capsule: Rect,
    metrics: &Snapshot,
    progress: f32,
    icons: &Icons,
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
        let card_rect =
            Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(width, PANEL_HEIGHT));
        paint_metric_card(ui, &painter, card_rect, card, metrics, progress, icons);
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
            subtitle: String::new(),
            color: GREEN,
            battery: false,
        },
        MetricCard {
            title: "GPU",
            value: metrics
                .gpu_percent
                .map(|v| format!("{v:.0}%"))
                .unwrap_or_else(|| "不可用".into()),
            subtitle: String::new(),
            color: Color32::from_rgb(216, 174, 255),
            battery: false,
        },
        MetricCard {
            title: "网络",
            value: format_speed(metrics.download_bytes_per_second),
            subtitle: format_speed(metrics.upload_bytes_per_second),
            color: BLUE,
            battery: false,
        },
        MetricCard {
            title: "电池",
            value: metrics
                .battery_percent
                .map(|value| format!("{value}%"))
                .unwrap_or_else(|| "--".into()),
            subtitle: String::new(),
            color: battery::battery_text_color(metrics),
            battery: true,
        },
    ]
}

fn card_width(ui: &egui::Ui, card: &MetricCard) -> f32 {
    [
        text_width(ui, card.title, 9.0),
        text_width(ui, &card.value, 16.0),
        text_width(ui, &card.subtitle, NETWORK_UPLOAD_SIZE),
    ]
    .into_iter()
    .fold(48.0_f32, f32::max)
        + if card.battery { 20.0 } else { 12.0 }
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
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    card: &MetricCard,
    metrics: &Snapshot,
    progress: f32,
    icons: &Icons,
) {
    let x = rect.left();
    let y = rect.top();
    let width = rect.width();
    painter.rect_filled(rect, 9.0, PANEL.linear_multiply(progress));
    let paint_text = |point: Pos2, text: &str, size: f32, color: Color32| {
        painter.text(
            point,
            Align2::CENTER_TOP,
            text,
            FontId::proportional(size),
            color.linear_multiply(progress),
        );
    };
    let center_x = x + width * 0.5;
    paint_text(Pos2::new(center_x, y + 4.0), card.title, 11.0, MUTED);
    if card.battery {
        let value_width = text_width(ui, &card.value, 16.0);
        let group_width = 19.0 + 4.0 + value_width;
        let group_left = center_x - group_width * 0.5;
        let battery_rect = Rect::from_min_size(Pos2::new(group_left, y + 23.0), Vec2::splat(19.0));
        let icon_color = if metrics.charging { BLUE } else { card.color };
        icons.paint_battery(
            painter,
            battery_rect,
            icon_color,
            metrics.charging,
            progress,
        );
        paint_text(
            Pos2::new(group_left + 20.0 + value_width * 0.5, y + 21.0),
            &card.value,
            16.0,
            card.color,
        );
    } else if card.title == "网络" {
        paint_rate(
            ui,
            painter,
            icons,
            NetworkRate {
                center: Pos2::new(center_x, y + 25.0),
                value: &card.value,
                icon: Icon::ArrowDown,
                color: BLUE,
                progress,
            },
        );
        paint_rate(
            ui,
            painter,
            icons,
            NetworkRate {
                center: Pos2::new(center_x, y + 45.0),
                value: &card.subtitle,
                icon: Icon::ArrowUp,
                color: UPLOAD,
                progress,
            },
        );
    } else {
        paint_text(Pos2::new(center_x, y + 24.0), &card.value, 16.0, card.color);
    }
}

#[cfg(test)]
mod tests;
