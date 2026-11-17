use crate::app::icons::{Icon, Icons};
use crate::extension::ExtensionId;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

const MUTED: Color32 = Color32::from_rgb(248, 250, 253);

pub(super) struct HeaderContent<'a> {
    pub title: &'a str,
    pub pinned: bool,
    pub do_not_disturb: bool,
    pub settings_open: bool,
    pub icons: &'a Icons,
    pub opacity: f32,
}

pub(super) fn paint_header(
    ui: &mut egui::Ui,
    rect: Rect,
    content: HeaderContent<'_>,
) -> (bool, bool, bool) {
    let HeaderContent {
        title,
        pinned,
        do_not_disturb,
        settings_open,
        icons,
        opacity,
    } = content;
    ui.painter().text(
        Pos2::new(rect.center().x, rect.top() + 27.0),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(14.0),
        MUTED.linear_multiply(opacity),
    );
    let button_size = Vec2::splat(24.0);
    let pin_rect = header_button(rect, 76.0, button_size);
    let pin = ui.put(pin_rect, egui::Button::new("").frame(false));
    icons.paint(
        ui.painter(),
        Icon::Pin,
        pin_rect.shrink(5.0),
        Color32::WHITE,
        opacity,
    );
    let pin_toggled = pin
        .on_hover_text(if pinned {
            "取消固定"
        } else {
            "固定展开"
        })
        .clicked();

    let quiet_rect = header_button(rect, 47.0, button_size);
    let quiet_toggled = ui
        .put(quiet_rect, egui::Button::new("").frame(false))
        .on_hover_text(if do_not_disturb {
            "关闭免打扰"
        } else {
            "一直开启免打扰"
        })
        .clicked();
    icons.paint(
        ui.painter(),
        Icon::Moon,
        quiet_rect.shrink(5.0),
        Color32::WHITE,
        opacity,
    );

    let gear_rect = header_button(rect, 19.0, button_size);
    let settings = ui
        .put(gear_rect, egui::Button::new("").frame(false))
        .clicked();
    icons.paint(
        ui.painter(),
        if settings_open {
            Icon::Close
        } else {
            Icon::Settings
        },
        gear_rect.shrink(5.0),
        Color32::WHITE,
        opacity,
    );
    (pin_toggled, quiet_toggled, settings)
}

fn header_button(rect: Rect, from_right: f32, size: Vec2) -> Rect {
    Rect::from_center_size(
        Pos2::new(rect.right() - from_right, rect.top() + 26.0),
        size,
    )
}

pub(super) fn paint_tabs(
    ui: &egui::Ui,
    rect: Rect,
    active_widget: &ExtensionId,
    widget_ids: &[ExtensionId],
    opacity: f32,
) {
    let center = Pos2::new(rect.center().x, rect.bottom() - 6.0);
    let active_index = widget_ids
        .iter()
        .position(|id| id == active_widget)
        .unwrap_or(0);
    let middle = (widget_ids.len().saturating_sub(1)) as f32 * 0.5;
    for (index, id) in widget_ids.iter().enumerate() {
        let point = Pos2::new(center.x + (index as f32 - middle) * 16.0, center.y);
        let selected = id == active_widget;
        let size = if selected {
            Vec2::new(12.0, 5.0)
        } else {
            Vec2::splat(5.0)
        };
        let dot = Rect::from_center_size(point, size);
        ui.painter().rect_filled(
            dot,
            3.0,
            if index == active_index {
                Color32::WHITE
            } else {
                MUTED
            }
            .linear_multiply(opacity),
        );
    }
}
