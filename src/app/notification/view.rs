use crate::{
    app::icons::{Icon, Icons},
    downloads::Download,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Vec2};

pub fn show(ui: &mut egui::Ui, rect: Rect, item: &Download, icons: &Icons) -> bool {
    let painter = ui.painter();
    painter.text(
        Pos2::new(rect.left() + 22.0, rect.top() + 27.0),
        Align2::LEFT_CENTER,
        "下载完成",
        FontId::proportional(13.0),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(rect.left() + 22.0, rect.top() + 52.0),
        Align2::LEFT_CENTER,
        truncate(&item.filename(), 28),
        FontId::proportional(11.0),
        Color32::from_rgb(247, 249, 252),
    );
    let y = rect.bottom() - 40.0;
    let button_left = rect.center().x - 110.0;
    let open = Rect::from_min_size(Pos2::new(button_left, y), Vec2::new(100.0, 26.0));
    let reveal = Rect::from_min_size(Pos2::new(button_left + 108.0, y), Vec2::new(112.0, 26.0));
    let close = Rect::from_min_size(
        Pos2::new(rect.right() - 48.0, rect.top() + 14.0),
        Vec2::splat(26.0),
    );
    if ui
        .put(
            open,
            egui::Button::new(RichText::new("打开文件").color(Color32::WHITE))
                .fill(Color32::from_rgb(56, 62, 74))
                .corner_radius(8),
        )
        .clicked()
    {
        open_file(&item.path, false);
        return true;
    }
    if ui
        .put(
            reveal,
            egui::Button::new(RichText::new("显示所在位置").color(Color32::WHITE))
                .fill(Color32::from_rgb(56, 62, 74))
                .corner_radius(8),
        )
        .clicked()
    {
        open_file(&item.path, true);
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
