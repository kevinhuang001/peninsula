use crate::{
    app::settings::{CompactPreferences, SettingsView},
    downloads::Download,
    metrics::Snapshot,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

mod compact;
mod expanded;

use super::{
    compact_layout::CAPSULE_HEIGHT,
    icons::Icons,
    mascot::Mascot,
    widgets::{paint_metrics, pomodoro, IslandTab},
};

const BLACK: Color32 = Color32::from_rgb(15, 16, 19);
const EXPANDED_BASE_HEIGHT: f32 = 48.0;

pub(super) fn draw_compact_chip(ui: &egui::Ui, rect: Rect, opacity: f32) {
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        rect.height() * 0.5,
        Color32::from_rgb(30, 33, 40).linear_multiply(opacity),
    );
    painter.rect_stroke(
        rect,
        rect.height() * 0.5,
        Stroke::new(1.0, Color32::from_rgb(66, 73, 85).linear_multiply(opacity)),
        egui::StrokeKind::Inside,
    );
}

pub(super) struct CapsuleWidgets<'a> {
    pub metrics: &'a Snapshot,
    pub mascot: &'a mut Mascot,
    pub preferences: &'a mut CompactPreferences,
    pub settings_open: bool,
    pub settings_view: &'a mut SettingsView,
    pub active_tab: &'a mut IslandTab,
    pub timer: &'a mut pomodoro::Pomodoro,
    pub pinned: bool,
    pub icons: &'a Icons,
    pub download: Option<&'a Download>,
    pub notification: Option<&'a Download>,
    pub do_not_disturb: bool,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    context: &egui::Context,
    width: f32,
    progress: f32,
    expanded_height: f32,
    content: CapsuleWidgets<'_>,
) -> (Rect, bool, bool, bool, bool, bool, bool) {
    let height = capsule_height(progress, expanded_height);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let capsule = ui.interact(
        rect,
        egui::Id::new("island-capsule"),
        Sense::click_and_drag(),
    );
    let drag_area = drag_handle_bounds(rect, progress);
    let handle = ui.interact(
        drag_area,
        egui::Id::new("island-drag-handle"),
        Sense::drag(),
    );
    let drag_started = handle.drag_started();
    if progress > 0.9
        && !content.settings_open
        && content.notification.is_none()
        && swipe_finished(context, &capsule)
    {
        *content.active_tab = content.active_tab.other();
        context.request_repaint();
    }
    paint_shell(ui, rect, progress);
    let mut dismiss_notification = false;
    let mut settings_changed = false;
    let mut quiet_toggled = false;
    let (settings, pin_toggled) = if progress < 0.9 {
        (
            compact::show(
                ui,
                context,
                rect,
                progress,
                compact::CompactContent {
                    metrics: content.metrics,
                    mascot: content.mascot,
                    prefs: content.preferences,
                    download: content.download,
                    icons: content.icons,
                },
            ),
            false,
        )
    } else if let Some(notification) = content.notification {
        dismiss_notification = super::notification::show(ui, rect, notification, content.icons);
        (false, false)
    } else {
        let (settings, pin_toggled, quiet, changed) = paint_expanded(
            ui,
            rect,
            progress,
            ExpandedContent {
                metrics: content.metrics,
                active_tab: content.active_tab,
                timer: content.timer,
                pinned: content.pinned,
                do_not_disturb: content.do_not_disturb,
                icons: content.icons,
                settings_open: content.settings_open,
                settings_view: content.settings_view,
                preferences: content.preferences,
            },
        );
        settings_changed = changed;
        quiet_toggled = quiet;
        (settings, pin_toggled)
    };
    capsule.context_menu(|ui| {
        if ui.button("退出灵动岛").clicked() {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
    (
        rect,
        settings,
        drag_started,
        dismiss_notification,
        pin_toggled,
        quiet_toggled,
        settings_changed,
    )
}

fn capsule_height(progress: f32, expanded_height: f32) -> f32 {
    let reveal = progress.clamp(0.0, 1.0);
    CAPSULE_HEIGHT + (EXPANDED_BASE_HEIGHT - CAPSULE_HEIGHT + expanded_height) * reveal
}

fn swipe_finished(context: &egui::Context, response: &egui::Response) -> bool {
    let id = egui::Id::new("island-swipe-origin");
    if response.drag_started() {
        if let Some(origin) = context.input(|input| input.pointer.press_origin()) {
            context.data_mut(|data| data.insert_temp(id, origin));
        }
    }
    if response.drag_stopped() {
        let origin = context.data_mut(|data| data.remove_temp::<Pos2>(id));
        let end = context.input(|input| input.pointer.interact_pos());
        return origin.zip(end).is_some_and(|(start, end)| {
            let delta = end - start;
            delta.x.abs() > 35.0 && delta.x.abs() > delta.y.abs()
        });
    }
    false
}

#[cfg(test)]
mod drag_tests;

fn drag_handle_bounds(rect: Rect, progress: f32) -> Rect {
    if progress >= 0.9 {
        Rect::from_min_max(
            Pos2::new(rect.left() + 48.0, rect.top()),
            Pos2::new(rect.right() - 82.0, rect.top() + 43.0),
        )
    } else {
        rect
    }
}

fn paint_shell(ui: &egui::Ui, rect: Rect, progress: f32) {
    let painter = ui.painter();
    let corner_radius = corner_radius(progress);
    painter.rect_filled(rect, corner_radius, BLACK);
    painter.rect_stroke(
        rect,
        corner_radius,
        Stroke::new(1.0, Color32::from_rgb(44, 47, 54)),
        egui::StrokeKind::Inside,
    );
}

fn corner_radius(progress: f32) -> f32 {
    26.0 + 8.0 * progress.clamp(0.0, 1.0)
}

struct ExpandedContent<'a> {
    metrics: &'a Snapshot,
    active_tab: &'a mut IslandTab,
    timer: &'a mut pomodoro::Pomodoro,
    pinned: bool,
    do_not_disturb: bool,
    icons: &'a Icons,
    settings_open: bool,
    settings_view: &'a mut SettingsView,
    preferences: &'a mut CompactPreferences,
}

fn paint_expanded(
    ui: &mut egui::Ui,
    rect: Rect,
    progress: f32,
    content: ExpandedContent<'_>,
) -> (bool, bool, bool, bool) {
    let ExpandedContent {
        metrics,
        active_tab,
        timer,
        pinned,
        do_not_disturb,
        icons,
        settings_open,
        settings_view,
        preferences,
    } = content;
    let reveal = progress.clamp(0.0, 1.0);
    let title = if settings_open {
        "设置"
    } else if *active_tab == IslandTab::Performance {
        "性能监控"
    } else {
        "番茄钟"
    };
    let (pin_toggled, quiet_toggled, settings) = expanded::paint_header(
        ui,
        rect,
        expanded::HeaderContent {
            title,
            pinned,
            do_not_disturb,
            settings_open,
            icons,
            opacity: reveal,
        },
    );
    let settings_changed = if settings_open {
        settings_view.show(ui, rect, preferences)
    } else {
        let body = Rect::from_min_max(
            Pos2::new(rect.left() + 10.0, rect.top() + 48.0),
            Pos2::new(rect.right() - 10.0, rect.bottom() - 15.0),
        );
        if *active_tab == IslandTab::Performance {
            let _ = paint_metrics(ui, body, metrics, reveal, icons);
        } else {
            pomodoro::show(ui, body, timer);
        }
        false
    };
    if !settings_open {
        expanded::paint_tabs(ui, rect, active_tab, reveal);
    }
    (settings, pin_toggled, quiet_toggled, settings_changed)
}

#[cfg(test)]
mod tests;
