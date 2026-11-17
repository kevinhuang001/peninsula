use crate::app::icons::{Icon, Icons};
use crate::extension::ExtensionId;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

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
    let text = if settings_open {
        Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    ui.painter().text(
        Pos2::new(rect.center().x, rect.top() + 27.0),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(15.0),
        text.linear_multiply(opacity),
    );
    let button_size = Vec2::splat(24.0);
    let pin_toggled = header_control(
        ui,
        header_button(rect, 76.0, button_size),
        icons,
        Icon::Pin,
        pinned,
        text,
        opacity,
    )
    .on_hover_text(if pinned {
        "取消固定"
    } else {
        "固定展开"
    })
    .clicked();
    let quiet_toggled = header_control(
        ui,
        header_button(rect, 47.0, button_size),
        icons,
        Icon::Moon,
        do_not_disturb,
        text,
        opacity,
    )
    .on_hover_text(if do_not_disturb {
        "关闭免打扰"
    } else {
        "一直开启免打扰"
    })
    .clicked();
    let settings = header_control(
        ui,
        header_button(rect, 19.0, button_size),
        icons,
        if settings_open {
            Icon::Close
        } else {
            Icon::Settings
        },
        settings_open,
        text,
        opacity,
    )
    .on_hover_text(if settings_open {
        "关闭设置"
    } else {
        "设置"
    })
    .clicked();
    (pin_toggled, quiet_toggled, settings)
}

fn header_control(
    ui: &mut egui::Ui,
    rect: Rect,
    icons: &Icons,
    icon: Icon,
    selected: bool,
    text: Color32,
    opacity: f32,
) -> egui::Response {
    let response = ui.put(rect, egui::Button::new("").frame(false));
    let interactive = response.hovered() || response.has_focus();
    if selected || interactive {
        let tint = ui.visuals().selection.bg_fill;
        let strength = if selected || response.is_pointer_button_down_on() {
            1.0
        } else {
            0.45
        };
        ui.painter()
            .rect_filled(rect, 7.0, tint.gamma_multiply(strength * opacity));
    }
    let color = if selected {
        ui.visuals().selection.stroke.color
    } else {
        text
    };
    icons.paint(ui.painter(), icon, rect.shrink(5.0), color, opacity);
    response
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
                ui.visuals().hyperlink_color
            } else {
                ui.visuals().weak_text_color().gamma_multiply(0.58)
            }
            .linear_multiply(opacity),
        );
    }
}
