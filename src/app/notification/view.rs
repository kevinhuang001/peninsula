use crate::{
    app::icons::{Icon, Icons},
    extension::{ExtensionId, Notification},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Vec2};

pub fn show(ui: &mut egui::Ui, rect: Rect, item: &Notification, icons: &Icons) -> bool {
    let painter = ui.painter();
    painter.text(
        Pos2::new(rect.left() + 22.0, rect.top() + 27.0),
        Align2::LEFT_CENTER,
        &item.title,
        FontId::proportional(13.0),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(rect.left() + 22.0, rect.top() + 52.0),
        Align2::LEFT_CENTER,
        truncate(&item.body, 28),
        FontId::proportional(11.0),
        Color32::from_rgb(247, 249, 252),
    );
    let y = rect.bottom() - 40.0;
    let button_left = rect.center().x - 110.0;
    let open_action = item.actions.iter().find(|action| action.id == "open");
    let reveal_action = item.actions.iter().find(|action| action.id == "reveal");
    let open = Rect::from_min_size(Pos2::new(button_left, y), Vec2::new(100.0, 26.0));
    let reveal = Rect::from_min_size(Pos2::new(button_left + 108.0, y), Vec2::new(112.0, 26.0));
    let close = Rect::from_min_size(
        Pos2::new(rect.right() - 48.0, rect.top() + 14.0),
        Vec2::splat(26.0),
    );
    if open_action.is_some_and(|action| {
        ui.put(
            open,
            egui::Button::new(RichText::new(&action.label).color(Color32::WHITE))
                .fill(Color32::from_rgb(56, 62, 74))
                .corner_radius(8),
        )
        .clicked()
    }) {
        if let Some(path) = open_action.and_then(|action| action.data.as_deref()) {
            open_file(std::path::Path::new(path), false);
        }
        return true;
    }
    if reveal_action.is_some_and(|action| {
        ui.put(
            reveal,
            egui::Button::new(RichText::new(&action.label).color(Color32::WHITE))
                .fill(Color32::from_rgb(56, 62, 74))
                .corner_radius(8),
        )
        .clicked()
    }) {
        if let Some(path) = reveal_action.and_then(|action| action.data.as_deref()) {
            open_file(std::path::Path::new(path), true);
        }
        return true;
    }
    let dismissed = ui.put(close, egui::Button::new("").frame(false)).clicked();
    icons.paint(
        ui.painter(),
        Icon::Close,
        close.shrink(8.0),
        Color32::from_rgb(247, 249, 252),
        1.0,
    );
    dismissed
}

pub fn show_center(
    ui: &mut egui::Ui,
    rect: Rect,
    notifications: &[Notification],
) -> Option<(ExtensionId, String)> {
    if notifications.is_empty() {
        ui.painter().text(
            Pos2::new(rect.center().x, rect.center().y),
            Align2::CENTER_CENTER,
            "没有未读通知",
            FontId::proportional(11.0),
            Color32::from_rgb(177, 183, 194),
        );
        return None;
    }
    let visible = notifications.len().min(4);
    for (index, notification) in notifications.iter().take(visible).enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.top() + index as f32 * 38.0),
            Vec2::new(rect.width() - 12.0, 34.0),
        );
        let response = ui.interact(
            row,
            egui::Id::new((
                "notification-center-row",
                &notification.source_id,
                &notification.id,
            )),
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
            truncate(&notification.title, 30),
            FontId::proportional(10.5),
            Color32::WHITE,
        );
        ui.painter().text(
            Pos2::new(row.left() + 9.0, row.top() + 24.0),
            Align2::LEFT_CENTER,
            truncate(&notification.body, 42),
            FontId::proportional(9.5),
            Color32::from_rgb(177, 183, 194),
        );
        if response.clicked() {
            return Some((notification.source_id.clone(), notification.id.clone()));
        }
    }
    if notifications.len() > visible {
        ui.painter().text(
            Pos2::new(rect.center().x, rect.bottom() - 6.0),
            Align2::CENTER_BOTTOM,
            format!("还有 {} 条未读", notifications.len() - visible),
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

fn open_file(path: &std::path::Path, reveal: bool) {
    #[cfg(target_os = "macos")]
    let result = {
        let mut command = std::process::Command::new("open");
        if reveal {
            command.arg("-R");
        }
        command.arg("--").arg(path).spawn()
    };
    #[cfg(target_os = "linux")]
    let result = {
        let target = if reveal {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        std::process::Command::new("xdg-open").arg(target).spawn()
    };
    #[cfg(target_os = "windows")]
    let result = {
        let mut command = std::process::Command::new("explorer.exe");
        if reveal {
            let mut argument = std::ffi::OsString::from("/select,");
            argument.push(path);
            command.arg(argument);
        } else {
            command.arg(path);
        }
        command.spawn()
    };
    if let Err(error) = result {
        eprintln!("Could not open download: {error}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn long_unicode_names_are_truncated_safely() {
        assert_eq!(super::truncate("一个很长的文件名称.pdf", 6), "一个很长的…");
        assert_eq!(super::truncate("a.pdf", 10), "a.pdf");
    }
}
