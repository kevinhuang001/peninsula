use crate::{
    app::icons::{Icon, Icons},
    extension::{AlertInteraction, NotificationAlertView},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Vec2};

const MAX_EXTENSION_ACTIONS: usize = 4;

pub fn show_alert(
    ui: &mut egui::Ui,
    rect: Rect,
    view: &NotificationAlertView,
    icons: &Icons,
    max_actions: usize,
) -> Option<AlertInteraction> {
    let painter = ui.painter().clone();
    if let Some(icon) = view.icon.as_deref().and_then(icon_from_name) {
        icons.paint(
            &painter,
            icon,
            Rect::from_min_size(rect.left_top() + Vec2::new(14.0, 16.0), Vec2::splat(18.0)),
            Color32::from_rgb(91, 190, 255),
            1.0,
        );
    }
    let text_left = if view.icon.is_some() {
        rect.left() + 40.0
    } else {
        rect.left() + 22.0
    };
    painter.text(
        Pos2::new(text_left, rect.top() + 27.0),
        Align2::LEFT_CENTER,
        &view.title,
        FontId::proportional(13.0),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(text_left, rect.top() + 52.0),
        Align2::LEFT_CENTER,
        truncate(&view.body, 48),
        FontId::proportional(11.0),
        Color32::from_rgb(247, 249, 252),
    );

    let close = Rect::from_min_size(
        Pos2::new(rect.right() - 48.0, rect.top() + 14.0),
        Vec2::splat(26.0),
    );
    if ui.put(close, egui::Button::new("").frame(false)).clicked() {
        return Some(AlertInteraction::Close);
    }
    icons.paint(
        &painter,
        Icon::Close,
        close.shrink(8.0),
        Color32::from_rgb(247, 249, 252),
        1.0,
    );

    let actions = view
        .actions
        .iter()
        .take(max_actions.min(MAX_EXTENSION_ACTIONS));
    let actions: Vec<_> = actions.collect();
    let count = actions.len() + 1;
    let gap = 6.0;
    let width = ((rect.width() - 36.0 - gap * (count - 1) as f32) / count as f32).max(48.0);
    let row_width = width * count as f32 + gap * (count - 1) as f32;
    let left = rect.center().x - row_width * 0.5;
    let top = rect.bottom() - 38.0;
    for (index, action) in actions.into_iter().enumerate() {
        let button = Rect::from_min_size(
            Pos2::new(left + index as f32 * (width + gap), top),
            Vec2::new(width, 26.0),
        );
        if ui
            .put(
                button,
                egui::Button::new(
                    RichText::new(&action.label)
                        .color(Color32::WHITE)
                        .size(10.0),
                )
                .fill(Color32::from_rgb(56, 62, 74))
                .corner_radius(8.0),
            )
            .clicked()
        {
            return Some(AlertInteraction::RunAction(action.clone()));
        }
    }
    let confirm = Rect::from_min_size(
        Pos2::new(left + (count - 1) as f32 * (width + gap), top),
        Vec2::new(width, 26.0),
    );
    if ui
        .put(
            confirm,
            egui::Button::new(RichText::new("确定").color(Color32::WHITE).size(10.0))
                .fill(Color32::from_rgb(56, 62, 74))
                .corner_radius(8.0),
        )
        .clicked()
    {
        return Some(AlertInteraction::Acknowledge);
    }
    None
}

fn icon_from_name(name: &str) -> Option<Icon> {
    Some(match name {
        "battery" => Icon::Battery,
        "battery-charging" => Icon::BatteryCharging,
        "arrow-down" => Icon::ArrowDown,
        "arrow-up" => Icon::ArrowUp,
        "clock" => Icon::Clock,
        _ => return None,
    })
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    format!(
        "{}…",
        text.chars().take(max.saturating_sub(1)).collect::<String>()
    )
}
