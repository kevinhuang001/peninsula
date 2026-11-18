use crate::{
    app::settings::{CompactPreferences, SettingsView},
    extension::{ActionInvocation, AlertInteraction, ExtensionId, Notification, WidgetView},
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

mod compact;
mod expanded;
use crate::ui::renderer;
pub(super) use expanded::{navigation_rect, navigation_width, NAVIGATION_HEIGHT};

use super::{
    compact_layout::{CompactItem, CAPSULE_HEIGHT},
    icons::Icons,
    widgets,
};

pub(super) const EXPANDED_BASE_HEIGHT: f32 = 48.0;
pub(super) const ALERT_WIDTH: f32 = 320.0;
pub(super) const BODY_SIDE: f32 = 10.0;
pub(super) const BODY_BOTTOM: f32 = 15.0;

pub(super) fn body_size(width: f32, widget_height: f32) -> Vec2 {
    Vec2::new(
        (width - BODY_SIDE * 2.0).max(0.0),
        (widget_height - BODY_BOTTOM).max(0.0),
    )
}

pub(super) fn draw_compact_chip(ui: &egui::Ui, rect: Rect, opacity: f32) {
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        rect.height() * 0.5,
        ui.visuals()
            .widgets
            .noninteractive
            .bg_fill
            .linear_multiply(opacity),
    );
    painter.rect_stroke(
        rect,
        rect.height() * 0.5,
        Stroke::new(
            1.0_f32,
            ui.visuals()
                .widgets
                .noninteractive
                .bg_stroke
                .color
                .linear_multiply(opacity),
        ),
        egui::StrokeKind::Inside,
    );
}

pub(super) struct CapsuleWidgets<'a> {
    pub preferences: &'a mut CompactPreferences,
    pub compact_views: &'a [CompactItem<'a>],
    pub settings_open: bool,
    pub settings_view: &'a mut SettingsView,
    pub active_widget: &'a mut ExtensionId,
    pub widget_ids: &'a [ExtensionId],
    pub widget_titles: &'a [String],
    pub widget_title: &'a str,
    pub widget_view: &'a WidgetView,
    pub widget_body_limit: Vec2,
    pub alert_view: Option<&'a super::notification::AlertSurface>,
    pub pinned: bool,
    pub icons: &'a Icons,
    pub notification: Option<&'a Notification>,
    pub do_not_disturb: bool,
}

pub(super) type CapsuleResult = (
    Rect,
    bool,
    bool,
    Option<AlertInteraction>,
    bool,
    bool,
    bool,
    Option<ActionInvocation>,
    bool,
);

pub(super) fn show(
    ui: &mut egui::Ui,
    context: &egui::Context,
    width: f32,
    progress: f32,
    expanded_height: f32,
    content: CapsuleWidgets<'_>,
) -> CapsuleResult {
    let navigation_height = if !content.settings_open && content.notification.is_none() {
        NAVIGATION_HEIGHT
    } else {
        0.0
    };
    let height = capsule_height(progress, expanded_height + navigation_height);
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
    let page_swipe = if progress > 0.9
        && !content.settings_open
        && content.notification.is_none()
        && !context
            .input(|input| input.pointer.press_origin())
            .is_some_and(|point| navigation_rect(rect).contains(point))
        && !renderer::scrolls_horizontally(content.widget_view)
    {
        swipe_finished(context, &capsule)
    } else {
        None
    };
    paint_shell(ui, rect, progress, content.settings_open);
    let mut alert_interaction = None;
    let mut widget_action = None;
    let mut settings_changed = false;
    let mut quiet_toggled = false;
    let mut dashboard_requested = false;
    let (settings, pin_toggled) = if progress < 0.9 {
        widget_action = compact::show(
            ui,
            rect,
            progress,
            compact::CompactContent {
                prefs: content.preferences,
                views: content.compact_views,
                icons: content.icons,
            },
        );
        (false, false)
    } else if content.notification.is_some() {
        alert_interaction = content
            .alert_view
            .and_then(|view| view.show(ui, rect, content.icons));
        (false, false)
    } else {
        let (settings, pin_toggled, quiet, changed, action, dashboard) = paint_expanded(
            ui,
            rect,
            progress,
            ExpandedContent {
                active_widget: content.active_widget,
                widget_ids: content.widget_ids,
                widget_titles: content.widget_titles,
                widget_title: content.widget_title,
                widget_view: content.widget_view,
                widget_body_limit: content.widget_body_limit,
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
        widget_action = action;
        dashboard_requested = dashboard;
        (settings, pin_toggled)
    };
    capsule.context_menu(|ui| {
        if ui
            .add(
                egui::Button::new(egui::RichText::new("退出灵动岛").color(egui::Color32::WHITE))
                    .fill(ui.visuals().selection.bg_fill),
            )
            .clicked()
        {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
    if let Some(direction) = page_swipe {
        *content.active_widget =
            widgets::next_widget(content.active_widget, content.widget_ids, direction);
        context.request_repaint();
    }
    (
        rect,
        settings,
        drag_started,
        alert_interaction,
        pin_toggled,
        quiet_toggled,
        settings_changed,
        widget_action,
        dashboard_requested,
    )
}

fn capsule_height(progress: f32, expanded_height: f32) -> f32 {
    let reveal = progress.clamp(0.0, 1.0);
    CAPSULE_HEIGHT + (EXPANDED_BASE_HEIGHT - CAPSULE_HEIGHT + expanded_height) * reveal
}

pub(super) fn resized_bounds(previous: Rect, width: f32, extra_height: f32) -> Rect {
    Rect::from_min_size(
        Pos2::new(previous.center().x - width * 0.5, previous.top()),
        Vec2::new(width, capsule_height(1.0, extra_height + NAVIGATION_HEIGHT)),
    )
}

fn swipe_finished(
    context: &egui::Context,
    response: &egui::Response,
) -> Option<widgets::Direction> {
    let id = egui::Id::new("island-swipe-origin");
    if response.drag_started() {
        if let Some(origin) = context.input(|input| input.pointer.press_origin()) {
            context.data_mut(|data| data.insert_temp(id, origin));
        }
    }
    if response.drag_stopped() {
        let origin = context.data_mut(|data| data.remove_temp::<Pos2>(id));
        let end = context.input(|input| input.pointer.interact_pos());
        let (start, end) = origin.zip(end)?;
        let delta = end - start;
        if delta.x.abs() > 35.0 && delta.x.abs() > delta.y.abs() {
            return Some(widgets::Direction::from_swipe(delta.x));
        }
    }
    None
}

#[cfg(test)]
mod drag_tests;

fn drag_handle_bounds(rect: Rect, progress: f32) -> Rect {
    if progress >= 0.9 {
        Rect::from_min_max(
            Pos2::new(rect.left() + 48.0, rect.top()),
            Pos2::new(rect.right() - 118.0, rect.top() + 43.0),
        )
    } else {
        rect
    }
}

fn paint_shell(ui: &egui::Ui, rect: Rect, progress: f32, settings_open: bool) {
    let painter = ui.painter();
    let corner_radius = corner_radius(progress);
    let dark = ui.visuals().dark_mode || settings_open;
    let fill = if settings_open {
        super::settings::theme::WIDGET_SURFACE_DARK
    } else {
        ui.visuals().panel_fill
    };
    let border = if dark {
        Color32::from_rgb(43, 56, 76)
    } else {
        Color32::from_rgb(202, 213, 230)
    };
    painter.rect_filled(rect, corner_radius, fill);
    painter.rect_stroke(
        rect,
        corner_radius,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
}

fn corner_radius(progress: f32) -> f32 {
    26.0 + 8.0 * progress.clamp(0.0, 1.0)
}

struct ExpandedContent<'a> {
    active_widget: &'a mut ExtensionId,
    widget_ids: &'a [ExtensionId],
    widget_titles: &'a [String],
    widget_title: &'a str,
    widget_view: &'a WidgetView,
    widget_body_limit: Vec2,
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
) -> (bool, bool, bool, bool, Option<ActionInvocation>, bool) {
    let ExpandedContent {
        active_widget,
        widget_ids,
        widget_titles,
        widget_title,
        widget_view,
        widget_body_limit,
        pinned,
        do_not_disturb,
        icons,
        settings_open,
        settings_view,
        preferences,
    } = content;
    let mut widget_action = None;
    let reveal = progress.clamp(0.0, 1.0);
    let title = if settings_open {
        "设置".to_owned()
    } else {
        widget_title.to_owned()
    };
    let (pin_toggled, quiet_toggled, settings, dashboard_requested) = expanded::paint_header(
        ui,
        rect,
        expanded::HeaderContent {
            title: &title,
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
        let available = body_size(
            rect.width(),
            rect.height() - EXPANDED_BASE_HEIGHT - NAVIGATION_HEIGHT,
        );
        // Lay out at the destination width throughout the spring animation.
        // Rewrapping a large parcel table at every intermediate width stalls frames
        // and makes its intrinsic height jump. Reveal it through a clipped viewport.
        let size = Vec2::new(widget_body_limit.x, available.y.min(widget_body_limit.y));
        if size.x > 0.0 && size.y > 0.0 {
            let top_left = Pos2::new(
                rect.center().x - size.x * 0.5,
                rect.top() + EXPANDED_BASE_HEIGHT + (available.y - size.y) * 0.5,
            );
            let body = Rect::from_min_size(top_left, size);
            let visible =
                Rect::from_min_size(Pos2::new(rect.left() + BODY_SIDE, top_left.y), available);
            let mut surface = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("stable-widget-body")
                    .max_rect(body),
            );
            surface.set_clip_rect(ui.clip_rect().intersect(visible));
            widget_action = renderer::show_widget(
                &mut surface,
                body,
                active_widget,
                widget_view,
                icons,
                reveal,
            );
        }
        false
    };
    if !settings_open {
        expanded::paint_tabs(ui, rect, active_widget, widget_ids, widget_titles, reveal);
    }
    (
        settings,
        pin_toggled,
        quiet_toggled,
        settings_changed,
        widget_action,
        dashboard_requested,
    )
}

#[cfg(test)]
mod tests;
