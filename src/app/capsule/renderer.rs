use crate::{
    app::{
        icons::Icons,
        notification,
        widgets::{self, pomodoro::Pomodoro},
    },
    extension::{ExtensionId, Notification, NOTIFICATION_CENTER_ID},
    metrics::Snapshot,
};
use eframe::egui::{self, Color32, Rect};

pub(super) struct WidgetRenderContext<'a> {
    pub extension_id: &'a ExtensionId,
    pub metrics: &'a Snapshot,
    pub timer: &'a mut Pomodoro,
    pub notifications: &'a [Notification],
    pub icons: &'a Icons,
    pub opacity: f32,
}

pub(super) fn show_widget(
    ui: &mut egui::Ui,
    rect: Rect,
    content: WidgetRenderContext<'_>,
) -> Option<(ExtensionId, String)> {
    let WidgetRenderContext {
        extension_id,
        metrics,
        timer,
        notifications,
        icons,
        opacity,
    } = content;
    match extension_id.as_str() {
        widgets::PERFORMANCE_ID => {
            let _ = widgets::paint_metrics(ui, rect, metrics, opacity, icons);
            None
        }
        widgets::POMODORO_ID => {
            widgets::pomodoro::show(ui, rect, timer);
            None
        }
        NOTIFICATION_CENTER_ID => notification::show_center(ui, rect, notifications),
        _ => {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "扩展当前没有内容",
                egui::FontId::proportional(11.0),
                Color32::from_rgb(177, 183, 194),
            );
            None
        }
    }
}
