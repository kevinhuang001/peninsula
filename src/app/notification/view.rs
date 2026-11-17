use crate::{
    app::icons::{Icon, Icons},
    builtins,
    extension::{ExtensionId, Notification, NotificationAction},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Vec2};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AlertInteraction {
    Confirm,
    Action(NotificationAction),
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CenterInteraction {
    ClearAll,
    Open(ExtensionId, String),
}

const BUTTON_HEIGHT: f32 = 26.0;
const BUTTON_GAP: f32 = 6.0;
const MAX_EXTENSION_ACTIONS: usize = 4;

/// Core always appends Confirm after the extension's optional actions.
pub fn alert_extra_height(notification: &Notification) -> f32 {
    let rows = (notification.actions.len().min(MAX_EXTENSION_ACTIONS) + 2) / 2;
    86.0 + 32.0 * rows.saturating_sub(1) as f32
}

pub fn show(
    ui: &mut egui::Ui,
    rect: Rect,
    item: &Notification,
    icons: &Icons,
) -> Option<AlertInteraction> {
    let model = builtins::notification_alert_view(item);
    let painter = ui.painter();
    painter.text(
        Pos2::new(rect.left() + 22.0, rect.top() + 27.0),
        Align2::LEFT_CENTER,
        &model.title,
        FontId::proportional(13.0),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(rect.left() + 22.0, rect.top() + 52.0),
        Align2::LEFT_CENTER,
        truncate(&model.body, 38),
        FontId::proportional(11.0),
        Color32::from_rgb(247, 249, 252),
    );

    let close = Rect::from_min_size(
        Pos2::new(rect.right() - 42.0, rect.top() + 10.0),
        Vec2::splat(25.0),
    );
    if ui.put(close, egui::Button::new("").frame(false)).clicked() {
        return Some(AlertInteraction::Close);
    }
    icons.paint(
        ui.painter(),
        Icon::Close,
        close.shrink(7.0),
        Color32::from_rgb(247, 249, 252),
        1.0,
    );

    let actions: Vec<_> = model.actions.iter().take(MAX_EXTENSION_ACTIONS).collect();
    let count = actions.len() + 1; // Confirm is always present.
    let left = rect.left() + 18.0;
    let available = rect.width() - 36.0;
    let width = (available - BUTTON_GAP) * 0.5;
    for index in 0..count {
        let row = index / 2;
        let column = index % 2;
        let last = index + 1 == count;
        let button_rect = Rect::from_min_size(
            Pos2::new(left + column as f32 * (width + BUTTON_GAP),
                rect.top() + 78.0 + row as f32 * 32.0),
            Vec2::new(width, BUTTON_HEIGHT),
        );
        let label = if last { "确定" } else { &actions[index].label };
        let fill = if last {
            Color32::WHITE
        } else {
            Color32::from_rgb(56, 62, 74)
        };
        let color = if last { Color32::BLACK } else { Color32::WHITE };
        if ui.put(
            button_rect,
            egui::Button::new(RichText::new(label).color(color))
                .fill(fill)
                .corner_radius(8),
        ).clicked() {
            return Some(if last {
                AlertInteraction::Confirm
            } else {
                AlertInteraction::Action(actions[index].clone())
            });
        }
    }
    None
}

pub fn show_center(
    ui: &mut egui::Ui,
    rect: Rect,
    notifications: &[Notification],
) -> Option<CenterInteraction> {
    let top = rect.top() + 2.0;
    if !notifications.is_empty() {
        let clear_rect = Rect::from_min_size(
            Pos2::new(rect.right() - 88.0, top),
            Vec2::new(80.0, 24.0),
        );
        if ui.put(
            clear_rect,
            egui::Button::new(RichText::new("全部清理").size(10.0))
                .corner_radius(7),
        ).clicked() {
            return Some(CenterInteraction::ClearAll);
        }
        ui.painter().text(
            Pos2::new(rect.left() + 8.0, top + 12.0),
            Align2::LEFT_CENTER,
            format!("{} 条待处理", notifications.len()),
            FontId::proportional(10.0),
            Color32::from_rgb(177, 183, 194),
        );
    } else {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "暂无通知",
            FontId::proportional(11.0),
            Color32::from_rgb(177, 183, 194),
        );
        return None;
    }

    let visible = notifications.len().min(4);
    for (index, notification) in notifications.iter().take(visible).enumerate() {
        let model = builtins::notification_center_item_view(notification);
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.top() + 31.0 + index as f32 * 38.0),
            Vec2::new(rect.width() - 12.0, 34.0),
        );
        let response = ui.interact(
            row,
            egui::Id::new(("notification-center-row", &notification.source_id, &notification.id)),
            egui::Sense::click(),
        );
        ui.painter().rect_filled(
            row,
            7.0,
            if response.hovered() {
                Color32::from_rgb(47, 53, 64)
            } else {
                Color32::from_rgb(35, 39, 47)
            },
        );
        ui.painter().text(
            Pos2::new(row.left() + 9.0, row.top() + 10.0),
            Align2::LEFT_CENTER,
            truncate(&model.title, 30),
            FontId::proportional(10.5),
            Color32::WHITE,
        );
        ui.painter().text(
            Pos2::new(row.left() + 9.0, row.top() + 24.0),
            Align2::LEFT_CENTER,
            truncate(&model.body, 42),
            FontId::proportional(9.5),
            Color32::from_rgb(177, 183, 194),
        );
        if response.clicked() {
            return Some(CenterInteraction::Open(
                notification.source_id.clone(), notification.id.clone(),
            ));
        }
    }
    if notifications.len() > visible {
        ui.painter().text(
            Pos2::new(rect.center().x, rect.bottom() - 6.0),
            Align2::CENTER_BOTTOM,
            format!("还有 {} 条待处理", notifications.len() - visible),
            FontId::proportional(9.0),
            Color32::from_rgb(177, 183, 194),
        );
    }
    None
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    format!("{}…", text.chars().take(max - 1).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::alert_extra_height;
    use crate::extension::{ExtensionId, Notification, NotificationAction, NotificationPriority};
    use std::time::SystemTime;

    #[test]
    fn popup_height_accommodates_multiple_extension_buttons_and_confirm() {
        let mut notice = Notification {
            source_id: ExtensionId::builtin("builtin.downloads/notifications"),
            id: "a".into(),
            title: "download".into(),
            body: "ready".into(),
            icon: None,
            priority: NotificationPriority::Normal,
            actions: vec![],
            created_at: SystemTime::now(),
        };
        assert_eq!(alert_extra_height(&notice), 86.0);
        notice.actions = (0..4).map(|i| NotificationAction {
            id: i.to_string(), label: i.to_string(), data: None,
        }).collect();
        assert_eq!(alert_extra_height(&notice), 150.0);
    }

    #[test]
    fn long_unicode_names_are_truncated_safely() {
        assert_eq!(super::truncate("一个很长的文件名称.pdf", 6), "一个很长的…");
        assert_eq!(super::truncate("a.pdf", 10), "a.pdf");
    }
}
