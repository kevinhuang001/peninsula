mod download;

use crate::{
    app::{
        compact_layout::{
            self, CHIP_HEIGHT, COMPACT_ICON_GAP, COMPACT_ICON_SIZE, COMPACT_TEXT_SIZE, ITEM_GAP,
            MASCOT_SIZE,
        },
        icons::{Icon, Icons},
        settings::{CompactPreferences, OptionItem},
    },
    downloads::Download,
    metrics::{format_speed, Snapshot},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};

use super::{
    super::{battery, mascot::Mascot},
    draw_compact_chip,
};
use download::paint_download;

const DOWN: Color32 = Color32::from_rgb(80, 190, 255);
const UP: Color32 = Color32::from_rgb(35, 255, 172);

struct NetworkRow {
    center_y: f32,
    icon: Icon,
    color: Color32,
    value: String,
}

pub(super) struct CompactContent<'a> {
    pub(super) metrics: &'a Snapshot,
    pub(super) mascot: &'a mut Mascot,
    pub(super) prefs: &'a CompactPreferences,
    pub(super) download: Option<&'a Download>,
    pub(super) icons: &'a Icons,
}

pub(super) fn show(
    ui: &egui::Ui,
    context: &egui::Context,
    rect: Rect,
    progress: f32,
    content: CompactContent<'_>,
) -> bool {
    let CompactContent {
        metrics,
        mascot,
        prefs,
        download,
        icons,
    } = content;
    let opacity = 1.0 - progress.clamp(0.0, 1.0);
    let widths = compact_layout::item_widths(prefs, download.is_some());
    let total = widths.iter().sum::<f32>() + ITEM_GAP * widths.len().saturating_sub(1) as f32;
    let mut x = rect.center().x - total * 0.5;
    let center_y = rect.center().y;

    for item in prefs.ordered_compact_items() {
        if !item.enabled(prefs) || (item == OptionItem::Downloads && download.is_none()) {
            continue;
        }
        let width = compact_layout::item_width(item);
        if item == OptionItem::Mascot {
            paint_mascot(ui, context, mascot, x, center_y, opacity);
        } else {
            let chip = chip_rect(x, center_y, width);
            draw_compact_chip(ui, chip, opacity);
            paint_chip(ui, chip, item, metrics, download, icons, opacity);
        }
        x += width + ITEM_GAP;
    }
    false
}

fn paint_mascot(
    ui: &egui::Ui,
    context: &egui::Context,
    mascot: &mut Mascot,
    x: f32,
    center_y: f32,
    opacity: f32,
) {
    let face = Rect::from_center_size(
        Pos2::new(x + MASCOT_SIZE[0] * 0.5, center_y),
        Vec2::new(MASCOT_SIZE[0], MASCOT_SIZE[1]),
    );
    let response = ui.interact(face, egui::Id::new("island-mascot"), Sense::click());
    mascot.paint(context, ui.painter(), face, opacity);
    if response.clicked() {
        mascot.advance(0.0, true);
        context.request_repaint();
    }
}

fn paint_chip(
    ui: &egui::Ui,
    chip: Rect,
    item: OptionItem,
    metrics: &Snapshot,
    download: Option<&Download>,
    icons: &Icons,
    opacity: f32,
) {
    match item {
        OptionItem::Battery => {
            battery::paint_chip(ui, chip, metrics, opacity, icons);
        }
        OptionItem::Cpu => paint_metric(
            ui,
            icons,
            chip,
            Icon::Cpu,
            format!("{:.0}%", metrics.cpu_percent),
            Color32::from_rgb(35, 255, 172),
            opacity,
        ),
        OptionItem::Gpu => paint_metric(
            ui,
            icons,
            chip,
            Icon::Gpu,
            metrics
                .gpu_percent
                .map_or("--".into(), |value| format!("{value:.0}%")),
            Color32::from_rgb(216, 174, 255),
            opacity,
        ),
        OptionItem::Network => paint_network(ui, icons, chip, metrics, opacity),
        OptionItem::Downloads => {
            if let Some(download) = download {
                paint_download(ui, chip, download, opacity);
            }
        }
        OptionItem::Time => paint_time(ui, icons, chip, opacity),
        OptionItem::Mascot => {}
    }
}

fn paint_metric(
    ui: &egui::Ui,
    icons: &Icons,
    chip: Rect,
    icon: Icon,
    value: String,
    color: Color32,
    opacity: f32,
) {
    let value_width = ui
        .painter()
        .layout_no_wrap(
            value.clone(),
            FontId::proportional(COMPACT_TEXT_SIZE),
            color,
        )
        .size()
        .x;
    let icon_size = COMPACT_ICON_SIZE;
    let gap = COMPACT_ICON_GAP;
    let left = chip.center().x - (icon_size + gap + value_width) * 0.5;
    icons.paint(
        ui.painter(),
        icon,
        Rect::from_center_size(
            Pos2::new(left + icon_size * 0.5, chip.center().y),
            Vec2::splat(icon_size),
        ),
        color,
        opacity,
    );
    ui.painter().text(
        Pos2::new(left + icon_size + gap, chip.center().y),
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(COMPACT_TEXT_SIZE),
        color.linear_multiply(opacity),
    );
}

fn paint_network(ui: &egui::Ui, icons: &Icons, chip: Rect, metrics: &Snapshot, opacity: f32) {
    network_row(
        ui,
        icons,
        chip,
        NetworkRow {
            center_y: chip.center().y - 7.0,
            icon: Icon::ArrowDown,
            color: DOWN,
            value: format_speed(metrics.download_bytes_per_second),
        },
        opacity,
    );
    network_row(
        ui,
        icons,
        chip,
        NetworkRow {
            center_y: chip.center().y + 7.0,
            icon: Icon::ArrowUp,
            color: UP,
            value: format_speed(metrics.upload_bytes_per_second),
        },
        opacity,
    );
}

fn chip_rect(x: f32, center_y: f32, width: f32) -> Rect {
    Rect::from_center_size(
        Pos2::new(x + width * 0.5, center_y),
        Vec2::new(width, CHIP_HEIGHT),
    )
}

fn network_row(ui: &egui::Ui, icons: &Icons, chip: Rect, row: NetworkRow, opacity: f32) {
    let text_width = ui
        .painter()
        .layout_no_wrap(
            row.value.clone(),
            FontId::proportional(COMPACT_TEXT_SIZE),
            row.color,
        )
        .size()
        .x;
    let row_width = COMPACT_ICON_SIZE + COMPACT_ICON_GAP + text_width;
    let left = chip.center().x - row_width * 0.5;
    icons.paint(
        ui.painter(),
        row.icon,
        Rect::from_center_size(
            Pos2::new(left + COMPACT_ICON_SIZE * 0.5, row.center_y),
            Vec2::splat(COMPACT_ICON_SIZE),
        ),
        row.color,
        opacity,
    );
    ui.painter().text(
        Pos2::new(left + COMPACT_ICON_SIZE + COMPACT_ICON_GAP, row.center_y),
        Align2::LEFT_CENTER,
        row.value,
        FontId::proportional(COMPACT_TEXT_SIZE),
        row.color.linear_multiply(opacity),
    );
}

fn paint_time(ui: &egui::Ui, icons: &Icons, chip: Rect, opacity: f32) {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let label = format!("{:02}:{:02}", now.hour(), now.minute());
    let label_width = ui
        .painter()
        .layout_no_wrap(
            label.clone(),
            FontId::proportional(COMPACT_TEXT_SIZE),
            Color32::WHITE,
        )
        .size()
        .x;
    let left = chip.center().x - (COMPACT_ICON_SIZE + COMPACT_ICON_GAP + label_width) * 0.5;
    icons.paint(
        ui.painter(),
        Icon::Clock,
        Rect::from_min_size(
            Pos2::new(left, chip.center().y - COMPACT_ICON_SIZE * 0.5),
            Vec2::splat(COMPACT_ICON_SIZE),
        ),
        Color32::WHITE,
        opacity,
    );
    ui.painter().text(
        Pos2::new(left + COMPACT_ICON_SIZE + COMPACT_ICON_GAP, chip.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(COMPACT_TEXT_SIZE),
        Color32::WHITE.linear_multiply(opacity),
    );
}
