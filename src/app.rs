use crate::{
    app::state::IslandState,
    core::extension_manager::ExtensionManager,
    core::{
        actions::ActionDispatcher,
        notifications::{self, NotificationCenterEntry, NotificationManager},
        Scheduler,
    },
    extension::{
        Action, ActionInvocation, AlertInteraction, Event, ExtensionId, ExtensionOutput,
        HostScheduler, Notification, NotificationKey, WidgetView, NOTIFICATION_CENTER_ID,
    },
};
use eframe::egui::{self, Vec2};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

mod animation;
mod capsule;
mod compact_layout;
mod dashboard;
mod fonts;
pub(crate) mod icons;
mod notification;
mod platform;
mod settings;
mod state;
mod trackpad;
mod widget_layout;
mod widgets;

use icons::Icons;
use settings::{CompactPreferences, SettingsView};

const INITIAL_WINDOW_SIZE: [f32; 2] = [720.0, 372.0];
const WINDOW_TOP_PADDING: f32 = 4.0;
const SCREEN_FRACTION: f32 = 0.75;

fn minimum_widget_size(screen: Vec2) -> Vec2 {
    // Width grows gently with logical screen resolution; height retains its reference ratio.
    egui::vec2(
        widget_layout::width_range(screen.x)[0],
        (screen.y * (160.0 / 900.0)).round(),
    )
}

fn widget_limit(window_size: Vec2) -> [f32; 2] {
    [
        window_size.x,
        (window_size.y
            - WINDOW_TOP_PADDING
            - capsule::EXPANDED_BASE_HEIGHT
            - capsule::NAVIGATION_HEIGHT)
            .max(0.0),
    ]
}
const CLEAR_ALL_ACTION: &str = "core.notifications.clear_all";
const ACKNOWLEDGE_ACTION_PREFIX: &str = "core.notifications.acknowledge:";
const CENTER_PAGE_ACTION_PREFIX: &str = "core.notifications.page:";

struct FittedMeasure {
    view: Arc<WidgetView>,
    key: [f32; 2],
    width: Option<f32>,
    height: Option<(f32, f32)>,
}

struct IslandApp {
    notifications: NotificationManager,
    scheduler: Scheduler,
    action_dispatcher: ActionDispatcher,
    extensions: ExtensionManager,
    mascot: crate::core::mascot::MascotHost,
    state: IslandState,
    icons: Icons,
    preferences: CompactPreferences,
    settings_view: SettingsView,
    settings_open: bool,
    dashboard_open: bool,
    notification_center_page: usize,
    center_view: Option<((u64, usize), Arc<WidgetView>)>,
    fitted_measure: Option<FittedMeasure>,
    alert_surface: Option<(NotificationKey, notification::AlertSurface)>,
    last_expanded_width: Option<f32>,
    page_switch_from: Option<egui::Rect>,
    window_drag: platform::WindowDrag,
    hover_focus: platform::HoverFocus,
    active_widget: ExtensionId,
    widget_height: animation::Spring,
    trackpad_swipe: trackpad::TrackpadSwipe,
    #[cfg(target_os = "macos")]
    scroll_monitor: Option<platform::ScrollMonitor>,
    page_edge_grace: Option<(egui::Rect, Instant)>,
    quiet_bypassed: bool,
    last_platform_check: Option<Instant>,
    window_size: Vec2,
    monitor_size: Option<Vec2>,
}

impl IslandApp {
    fn new(context: &egui::Context) -> Self {
        let mut preferences = CompactPreferences::load();
        preferences
            .widget_theme
            .install(context, preferences.widget_accent);
        let extensions = ExtensionManager::new(crate::builtins::runtimes());
        preferences.read_extension_settings(&extensions);
        let active_widget = extensions.widget_ids()[0].clone();
        let window_size = Vec2::from(INITIAL_WINDOW_SIZE);
        let initial_widget_height =
            extensions.widget_size(&active_widget, None, widget_limit(window_size), [None; 2])[1];
        let notifications =
            NotificationManager::open(extensions.data_dir().join("notifications.json"));
        let secret_index_path = extensions.data_dir().join("secret-keys.json");
        let secret_keys = std::fs::read(&secret_index_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
            .unwrap_or_default();
        let settings_view = SettingsView::default().with_secret_store(
            crate::sdk::secrets::SecretStore::new(
                "Peninsula::core.host",
                extensions.data_dir().join("secrets.json"),
            ),
            secret_index_path,
            &secret_keys,
        );
        Self {
            notifications,
            scheduler: Scheduler::default(),
            action_dispatcher: ActionDispatcher::default(),
            extensions,
            mascot: Default::default(),
            state: IslandState::default(),
            icons: Icons::load(context),
            preferences,
            settings_view,
            settings_open: false,
            dashboard_open: false,
            notification_center_page: 0,
            center_view: None,
            fitted_measure: None,
            alert_surface: None,
            last_expanded_width: None,
            page_switch_from: None,
            window_drag: platform::WindowDrag::default(),
            hover_focus: platform::HoverFocus::default(),
            active_widget: active_widget.clone(),
            widget_height: animation::Spring::new(initial_widget_height),
            trackpad_swipe: trackpad::TrackpadSwipe::default(),
            #[cfg(target_os = "macos")]
            scroll_monitor: None,
            page_edge_grace: None,
            quiet_bypassed: false,
            last_platform_check: None,
            window_size,
            monitor_size: None,
        }
    }

    fn update_viewport_size(&mut self, context: &egui::Context, compact_width: f32) {
        let (monitor, outer) = context.input(|input| {
            let viewport = input.viewport();
            (viewport.monitor_size, viewport.outer_rect)
        });
        let Some(monitor) = monitor.filter(|size| size.is_finite() && size.x > 0.0 && size.y > 0.0)
        else {
            return;
        };
        self.monitor_size = Some(monitor);
        let mut size = (monitor * SCREEN_FRACTION).floor().max(Vec2::splat(1.0));
        // A lyric may exceed the normal widget canvas. Keep room for the
        // spring overshoot as well, so the native window cannot crop it.
        size.x = size.x.max((compact_width * 1.08).ceil());
        if self.window_size == size {
            return;
        }
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        if let Some(rect) = outer {
            // Preserve the center of the pill and its top edge when changing displays.
            let position = egui::pos2(rect.center().x - size.x * 0.5, rect.top());
            context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        }
        self.window_size = size;
    }

    fn screen_size(&self) -> Vec2 {
        self.monitor_size
            .unwrap_or(self.window_size / SCREEN_FRACTION)
    }

    fn notification_center_view(&mut self) -> Arc<WidgetView> {
        let key = (self.notifications.revision(), self.notification_center_page);
        if self
            .center_view
            .as_ref()
            .is_none_or(|(previous, _)| *previous != key)
        {
            let entries: Vec<_> = self
                .notifications
                .pending()
                .map(|notification| NotificationCenterEntry {
                    key: notification.key(),
                    view: self
                        .extensions
                        .notification_presentation(notification)
                        .center_item,
                })
                .collect();
            let view = notifications::widget_view(&entries, self.notification_center_page);
            self.center_view = Some((key, Arc::new(view)));
        }
        self.center_view.as_ref().unwrap().1.clone()
    }

    fn show_dashboard(
        &mut self,
        context: &egui::Context,
        widget_ids: &[ExtensionId],
        now: Instant,
    ) {
        let cards = widget_ids
            .iter()
            .map(|id| {
                let view = if id.as_str() == NOTIFICATION_CENTER_ID {
                    self.notification_center_view()
                } else {
                    self.extensions
                        .widget_view(id, now)
                        .unwrap_or_else(|| Arc::new(empty_widget_view()))
                };
                dashboard::Card {
                    id: id.clone(),
                    title: self.extensions.widget_title(id),
                    view,
                }
            })
            .collect::<Vec<_>>();
        let outcome = dashboard::show(context, &self.icons, &cards, self.screen_size());
        if outcome.close {
            self.dashboard_open = false;
        }
        for action in outcome.actions {
            self.dispatch_widget_action(action, now, context);
        }
    }

    fn apply_extension_output(&mut self, output: ExtensionOutput, context: &egui::Context) {
        // Context task drains and idle extensions return empty output each pass.
        // Requesting a repaint for those creates a permanent busy render loop.
        if output == ExtensionOutput::default() {
            return;
        }
        let repaint = !output.invalidated.is_empty()
            || !output.notifications.is_empty()
            || !output.resolved_notifications.is_empty()
            || !output.events.is_empty()
            || !output.mascot_events.is_empty();
        for extension_id in output.invalidated {
            self.scheduler.invalidate(&extension_id);
        }
        for task_id in output.cancel_tasks {
            self.scheduler.cancel(&task_id);
        }
        for task in output.schedule {
            self.scheduler.schedule_at(&task.task_id, task.deadline);
        }
        for event in output.events {
            self.scheduler.on_event(event);
        }
        for event in &output.mascot_events {
            if self.preferences.mascot_enabled {
                self.mascot
                    .event(event, &self.extensions.mascot_sources(), Instant::now());
            }
        }
        let published = !output.notifications.is_empty();
        for key in &output.resolved_notifications {
            self.notifications.acknowledge(key);
        }
        for notification in output.notifications.iter().cloned() {
            self.notifications.publish(notification);
        }
        if !output.notifications.is_empty() {
            self.extensions
                .on_notifications_committed(&output.notifications);
        }
        if published {
            self.scheduler.on_event(Event::NotificationPublished);
        }
        if repaint {
            context.request_repaint();
        }
    }

    fn handle_alert_interaction(
        &mut self,
        interaction: AlertInteraction,
        notification: &Notification,
    ) {
        let key = notification.key();
        match interaction {
            AlertInteraction::Close => self.notifications.dismiss_alert(),
            AlertInteraction::Acknowledge => {
                self.notifications.acknowledge(&key);
            }
            AlertInteraction::RunAction(action) => {
                self.notifications
                    .record_action(key.clone(), action.clone());
                if let Err(error) = self
                    .extensions
                    .on_notification_action(notification, &action)
                {
                    eprintln!("Notification action {} failed: {error}", action.id);
                }
                // Clicking any extension action acknowledges the notification,
                // even when the extension reports an error.
                self.notifications.acknowledge(&key);
            }
        }
    }

    fn dispatch_widget_action(
        &mut self,
        invocation: ActionInvocation,
        now: Instant,
        context: &egui::Context,
    ) {
        if invocation.source_extension_id.as_str() == crate::core::mascot::MASCOT_SLOT_ID {
            self.mascot.cycle_idle();
            context.request_repaint();
            return;
        }
        if invocation.source_extension_id.as_str() == NOTIFICATION_CENTER_ID {
            let mut opened = false;
            let (dispatcher, notifications) =
                (&mut self.action_dispatcher, &mut self.notifications);
            let result = dispatcher.dispatch(invocation, |action| {
                match &action.action {
                    Action::Custom(id) if id == CLEAR_ALL_ACTION => {
                        notifications.acknowledge_all();
                    }
                    Action::Custom(id) if id.starts_with(CENTER_PAGE_ACTION_PREFIX) => {
                        if let Ok(page) = id[CENTER_PAGE_ACTION_PREFIX.len()..].parse::<usize>() {
                            self.notification_center_page = page;
                        }
                    }
                    Action::Custom(id) if id.starts_with(ACKNOWLEDGE_ACTION_PREFIX) => {
                        if let Some(key) =
                            NotificationKey::from_stable_id(&id[ACKNOWLEDGE_ACTION_PREFIX.len()..])
                        {
                            notifications.acknowledge(&key);
                        }
                    }
                    Action::Select(id) => {
                        if let Some(key) = NotificationKey::from_stable_id(id) {
                            opened = notifications.show_again(&key, now);
                        }
                    }
                    _ => {}
                }
                Ok(())
            });
            if let Err(error) = result {
                eprintln!("Notification center action failed: {error}");
            }
            if opened {
                self.state.show_notification();
            }
            context.request_repaint();
            return;
        }

        let (dispatcher, extensions) = (&mut self.action_dispatcher, &mut self.extensions);
        let mut output = None;
        let result = dispatcher.dispatch(invocation, |action| {
            match extensions.on_action(&action.source_extension_id, action.action.clone(), now) {
                Ok(extension_output) => {
                    output = Some(extension_output);
                    Ok(())
                }
                Err(error) => Err(error),
            }
        });
        match result {
            Ok(()) => {
                if let Some(output) = output {
                    self.apply_extension_output(output, context);
                }
            }
            Err(error) => eprintln!("Widget action failed: {error}"),
        }
    }
}

impl eframe::App for IslandApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, context: &egui::Context, frame: &mut eframe::Frame) {
        let now = Instant::now();
        let surface_limit = widget_limit((self.screen_size() * SCREEN_FRACTION).floor());
        let mut widget_limit = surface_limit;
        if self
            .last_platform_check
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1))
        {
            platform::configure_floating_window(frame);
            self.last_platform_check = Some(now);
        }

        self.extensions
            .set_visible(self.state.expanded && !self.settings_open);
        for output in self.extensions.update(now) {
            self.apply_extension_output(output, context);
        }
        for result in self.extensions.take_setting_search_results() {
            self.settings_view.apply_search_result(
                &result.extension_id,
                &result.setting_key,
                &result.query,
                result.result,
            );
            context.request_repaint();
        }
        let context_tasks = self.extensions.drain_context_tasks();
        self.apply_extension_output(context_tasks, context);
        self.scheduler.due_tasks(now);
        let events: Vec<_> = self.scheduler.drain_events().collect();
        for event in events {
            for output in self.extensions.on_event(&event, now) {
                self.apply_extension_output(output, context);
            }
        }

        self.preferences.refresh_status(&self.extensions);
        let scheduled_quiet = self.preferences.quiet_hours.is_scheduled_now();
        if !scheduled_quiet {
            self.quiet_bypassed = false;
        }
        let do_not_disturb = self.preferences.quiet_hours.manual_enabled
            || (scheduled_quiet && !self.quiet_bypassed);
        if self
            .notifications
            .update(now, !self.settings_open, do_not_disturb)
        {
            self.state.show_notification();
            context.request_repaint();
        }
        let notice = self.notifications.current().cloned();
        if notice.is_none() {
            self.state.close_notification();
        }
        if let Some(notification) = &notice {
            let key = notification.key();
            if self
                .alert_surface
                .as_ref()
                .is_none_or(|(previous, _)| *previous != key)
            {
                let data = self
                    .extensions
                    .notification_presentation(notification)
                    .alert;
                self.alert_surface = Some((
                    key.clone(),
                    notification::AlertSurface::new(
                        &key,
                        data,
                        self.notifications.policy().max_alert_actions,
                    ),
                ));
            }
        } else {
            self.alert_surface = None;
        }

        let mut widget_ids = self.extensions.widget_ids();
        widget_ids[1..].sort_by_key(|id| {
            self.preferences
                .widget_order
                .iter()
                .position(|saved| saved == id)
                .unwrap_or(usize::MAX)
        });
        let widget_titles: Vec<_> = widget_ids
            .iter()
            .map(|id| self.extensions.widget_title(id))
            .collect();
        if !widget_ids.contains(&self.active_widget) {
            self.active_widget = widget_ids[0].clone();
        }
        let page_size = notifications::CENTER_PAGE_SIZE;
        let page_count = self
            .notifications
            .pending_count()
            .div_ceil(page_size)
            .max(1);
        self.notification_center_page = self
            .notification_center_page
            .min(page_count.saturating_sub(1));
        for id in self.scheduler.take_invalidated() {
            self.extensions.invalidate_views(&id);
        }
        let widget_view = if self.active_widget.as_str() == NOTIFICATION_CENTER_ID {
            Some(self.notification_center_view())
        } else {
            self.extensions.widget_view(&self.active_widget, now)
        };
        let widths = widget_layout::width_range(self.screen_size().x);
        widget_limit[0] = widget_limit[0].min(widths[1]);
        let mut minimum = minimum_widget_size(self.screen_size());
        minimum.x = widths[0];
        // Retain the source Arc so pointer identity cannot be recycled. Measure
        // only on data/layout changes, never on every frame of the capsule spring.
        if let Some(view) = widget_view.as_ref() {
            let key = [widget_limit[0], context.pixels_per_point()];
            if self
                .fitted_measure
                .as_ref()
                .is_none_or(|cached| !Arc::ptr_eq(&cached.view, view) || cached.key != key)
            {
                self.fitted_measure = Some(FittedMeasure {
                    view: view.clone(),
                    key,
                    width: view.bounds.fit_width.then(|| {
                        let limit = capsule::body_size(widget_limit[0], 0.0).x;
                        (crate::ui::renderer::natural_content_width(
                            context,
                            &view.root,
                            limit,
                            &self.icons,
                        ) + 2.0 * capsule::BODY_SIDE
                            + 1.0)
                            .ceil()
                    }),
                    height: None,
                });
            }
        } else {
            self.fitted_measure = None;
        }
        let fitted_width = self.fitted_measure.as_ref().and_then(|cached| cached.width);
        let [widget_width, preferred_height] = self.extensions.widget_size(
            &self.active_widget,
            widget_view.as_deref(),
            widget_limit,
            [fitted_width, None],
        );
        let widget_width = widget_width
            .max(
                capsule::navigation_width(context, &widget_ids, &widget_titles)
                    .min(widget_limit[0]),
            )
            .max(minimum.x.min(widget_limit[0]))
            .min(widget_limit[0]);
        let content_height =
            if let Some(view) = widget_view.as_deref().filter(|view| view.bounds.fit_height) {
                let body_width = capsule::body_size(widget_width, 0.0).x;
                let cache = self.fitted_measure.as_mut().unwrap();
                let measured = match cache.height {
                    Some((width, height)) if width == body_width => height,
                    _ => {
                        let height = crate::ui::renderer::content_height(
                            context,
                            &view.root,
                            body_width,
                            &self.icons,
                        );
                        cache.height = Some((body_width, height));
                        height
                    }
                };
                self.extensions.widget_size(
                    &self.active_widget,
                    Some(view),
                    widget_limit,
                    [fitted_width, Some(measured + capsule::BODY_BOTTOM)],
                )[1]
            } else {
                preferred_height
            };
        let minimum_body_height = (minimum.y - capsule::EXPANDED_BASE_HEIGHT).max(0.0);
        let widget_height = content_height.max(minimum_body_height.min(widget_limit[1]));
        let pointer = context.input(|input| input.pointer.hover_pos());
        if let Some(previous) = self.page_switch_from.take() {
            let next = capsule::resized_bounds(previous, widget_width, widget_height);
            if self.state.expanded
                && !self.settings_open
                && notice.is_none()
                && (next.height() + 1.0 < previous.height()
                    || next.width() + 1.0 < previous.width())
                && platform::pointer_inside(
                    frame,
                    Some(previous),
                    pointer,
                    context.pixels_per_point(),
                )
                && !platform::pointer_inside(frame, Some(next), pointer, context.pixels_per_point())
            {
                self.state.defer_collapse_after_shrink();
                self.page_edge_grace = Some((previous, now + Duration::from_millis(1000)));
            }
        }
        let width = if self.settings_open {
            self.settings_view
                .width(context, &self.preferences)
                .min(surface_limit[0])
        } else {
            widget_width
        };
        if notice.is_none() {
            if self.state.expanded
                && self
                    .last_expanded_width
                    .is_some_and(|previous| previous != width)
            {
                self.state.retarget_expanded_width(width);
            } else {
                self.state.set_expanded_width(width);
            }
            self.last_expanded_width = Some(width);
        }
        let widget_title = format!(
            "[widget:{}] {}",
            self.active_widget.as_str().split('/').next().unwrap_or(""),
            self.extensions.widget_title(&self.active_widget)
        );
        crate::ui::renderer::set_reduced_motion(
            context,
            self.preferences.reduced_motion || self.preferences.motion_off,
        );
        self.extensions
            .set_motion_policy(if self.preferences.motion_off {
                crate::core::mascot::MotionPolicy::Off
            } else if self.preferences.reduced_motion {
                crate::core::mascot::MotionPolicy::Reduced
            } else {
                crate::core::mascot::MotionPolicy::Full
            });
        let active_prompt = self.extensions.active_prompt(
            now,
            !self.state.expanded && !self.settings_open && notice.is_none(),
        );
        let prompt_active = active_prompt.is_some();
        let mut available_views = active_prompt.as_ref().map_or_else(
            || self.extensions.compact_views(now),
            |prompt| vec![Arc::new(prompt.compact_view())],
        );
        let sources = self.extensions.mascot_sources();
        let mascot_visible =
            self.preferences.mascot_enabled && !self.state.expanded && !prompt_active;
        self.mascot.update(&sources, mascot_visible, now);
        if self.preferences.mascot_enabled && !prompt_active {
            let policy = if self.preferences.motion_off || self.preferences.mascot_motion == "Off" {
                crate::core::mascot::MotionPolicy::Off
            } else if self.preferences.reduced_motion || self.preferences.mascot_motion == "Reduced"
            {
                crate::core::mascot::MotionPolicy::Reduced
            } else {
                crate::core::mascot::MotionPolicy::Full
            };
            available_views.insert(
                0,
                Arc::new(
                    self.mascot
                        .compact_slot(f32::from(self.preferences.mascot_size), policy),
                ),
            );
        }
        if mascot_visible {
            if let Some(wake) = self.mascot.next_wake() {
                context.request_repaint_after(wake.saturating_duration_since(now));
            }
        }
        let mut compact_views = compact_layout::measure_views(
            context,
            available_views.iter().map(Arc::as_ref).filter(|view| {
                prompt_active
                    || self.preferences.is_enabled(&view.id)
                    || (self.preferences.mascot_enabled
                        && view.id.as_str() == crate::core::mascot::MASCOT_SLOT_ID)
            }),
            &self.icons,
        );
        if prompt_active {
            for view in &mut compact_views {
                view.exclusive = true;
                view.prompt = active_prompt.clone();
            }
        }
        let compact_width = compact_layout::width(&self.preferences, &compact_views);
        self.update_viewport_size(
            context,
            compact_width.max(widget_width).max(self.state.width.value),
        );
        self.state.set_compact_width(compact_width);
        let dt = context.input(|input| input.unstable_dt).clamp(0.0, 0.1);
        if self.preferences.motion_off {
            self.state.finish_motion();
        }
        let (pill_width, progress) = self.state.advance(dt);
        self.widget_height
            .target(if let Some((_, surface)) = &self.alert_surface {
                surface
                    .extra_height(context, &self.icons)
                    .min(widget_limit[1])
            } else if self.state.hides_widget() {
                0.0
            } else if self.settings_open {
                self.settings_view
                    .extra_height(context, &self.preferences)
                    .min(widget_limit[1])
            } else {
                widget_height
            });
        if self.preferences.motion_off {
            self.widget_height.finish();
        } else {
            self.widget_height.step(dt);
        }
        let fallback = widget_view.is_none().then(empty_widget_view);
        let widget_view = widget_view.as_deref().or(fallback.as_ref()).unwrap();

        let page_before_input = self.active_widget.clone();
        let mut interactive_rect = None;
        let mut drag_started = false;
        let mut action_result = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(context, |ui| {
                ui.set_min_size(self.window_size);
                ui.vertical_centered(|ui| {
                    ui.add_space(WINDOW_TOP_PADDING);
                    let (
                        rect,
                        toggle_settings,
                        start_drag,
                        alert_interaction,
                        pin_toggled,
                        quiet_toggled,
                        preferences_changed,
                        widget_action,
                        dashboard_requested,
                    ) = capsule::show(
                        ui,
                        context,
                        pill_width.min(self.window_size.x),
                        progress,
                        self.widget_height.value.clamp(0.0, widget_limit[1]),
                        capsule::CapsuleWidgets {
                            preferences: &mut self.preferences,
                            compact_views: &compact_views,
                            settings_open: self.settings_open,
                            settings_view: &mut self.settings_view,
                            active_widget: &mut self.active_widget,
                            widget_ids: &widget_ids,
                            widget_titles: &widget_titles,
                            widget_title: &widget_title,
                            widget_view,
                            widget_body_limit: capsule::body_size(widget_width, content_height),
                            alert_view: self.alert_surface.as_ref().map(|(_, surface)| surface),
                            pinned: self.state.is_pinned(),
                            icons: &self.icons,
                            notification: notice.as_ref(),
                            do_not_disturb,
                        },
                    );
                    action_result = Some((
                        alert_interaction,
                        widget_action,
                        pin_toggled,
                        quiet_toggled,
                        preferences_changed,
                        toggle_settings,
                        dashboard_requested,
                    ));
                    drag_started = start_drag;
                    interactive_rect = Some(rect);
                });
            });

        if let Some((
            alert_interaction,
            widget_action,
            pin_toggled,
            quiet_toggled,
            preferences_changed,
            toggle_settings,
            dashboard_requested,
        )) = action_result
        {
            if let (Some(interaction), Some(notification)) = (alert_interaction, notice.as_ref()) {
                self.handle_alert_interaction(interaction, notification);
            }
            if dashboard_requested {
                self.dashboard_open = true;
                context
                    .send_viewport_cmd_to(dashboard::viewport_id(), egui::ViewportCommand::Focus);
            }
            if pin_toggled {
                self.state.toggle_pin();
                context.request_repaint();
            }
            if quiet_toggled {
                self.preferences
                    .quiet_hours
                    .toggle_manual(scheduled_quiet, &mut self.quiet_bypassed);
                if let Err(error) = self.preferences.save() {
                    eprintln!("Unable to save settings: {error}");
                }
            }
            if toggle_settings {
                self.settings_open = !self.settings_open;
                let width = if self.settings_open {
                    self.settings_view
                        .width(context, &self.preferences)
                        .min(surface_limit[0])
                } else {
                    widget_width
                };
                self.state.retarget_expanded_width(width);
                self.widget_height.target(if self.settings_open {
                    self.settings_view
                        .extra_height(context, &self.preferences)
                        .min(widget_limit[1])
                } else {
                    widget_height
                });
            }
            if preferences_changed {
                context.request_repaint();
                for page in self.preferences.pages.clone() {
                    match self
                        .extensions
                        .update_settings(&page.descriptor.id, page.values)
                    {
                        Ok(output) => self.apply_extension_output(output, context),
                        Err(error) => eprintln!("Unable to save extension settings: {error}"),
                    }
                    if let Err(error) = self
                        .extensions
                        .set_enabled(&page.descriptor.id, page.enabled)
                    {
                        eprintln!("Unable to change extension lifecycle: {error}");
                    }
                }
                self.preferences.read_extension_settings(&self.extensions);
                self.preferences
                    .widget_theme
                    .install(context, self.preferences.widget_accent);
                if let Err(error) = self.preferences.save() {
                    eprintln!("Unable to save settings: {error}");
                }
            }
            if let Some(action) = widget_action {
                self.dispatch_widget_action(action, now, context);
            }
        }

        if self.settings_open {
            for request in self.settings_view.take_due_searches(&self.preferences, now) {
                if let Err(error) = self.extensions.search_setting(
                    &request.extension_id,
                    &request.setting_key,
                    &request.query,
                ) {
                    self.settings_view.apply_search_result(
                        &request.extension_id,
                        &request.setting_key,
                        &request.query,
                        Err(error),
                    );
                }
            }
        }
        if self.settings_open
            && (self.extensions.has_setting_searches()
                || self.settings_view.has_searches_in_flight())
        {
            context.request_repaint_after(Duration::from_millis(100));
        }

        let dragging = self.window_drag.update(frame);
        let can_turn_page = self.state.expanded && notice.is_none() && !self.settings_open;
        let grace_rect = self.page_edge_grace.take().filter(|(rect, deadline)| {
            can_turn_page
                && now < *deadline
                && platform::pointer_inside(frame, Some(*rect), pointer, context.pixels_per_point())
        });
        self.page_edge_grace = grace_rect;
        let swipe_region = match grace_rect {
            Some((previous, _)) => {
                Some(interactive_rect.map_or(previous, |rect| rect.union(previous)))
            }
            None => interactive_rect,
        };
        let menu_rect = crate::ui::renderer::context_menu_rect(context);
        let hit_region = match (swipe_region, menu_rect) {
            (Some(rect), Some(menu)) => Some(rect.union(menu)),
            (rect, menu) => rect.or(menu),
        };
        platform::update_hit_region(frame, hit_region, dragging, context.pixels_per_point());

        let hovering =
            platform::pointer_inside(frame, interactive_rect, pointer, context.pixels_per_point());
        if self.hover_focus.entered(hovering) {
            context.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if hovering && notice.is_some() {
            self.notifications.keep_alert_visible(now);
        }
        let (scroll, time, modifiers) =
            context.input(|input| (input.raw_scroll_delta, input.time, input.modifiers));
        let swipe_hovering =
            platform::pointer_inside(frame, swipe_region, pointer, context.pixels_per_point());
        let eligible = swipe_hovering
            && !interactive_rect
                .zip(pointer)
                .is_some_and(|(rect, point)| capsule::navigation_rect(rect).contains(point))
            && can_turn_page
            && !crate::ui::renderer::scrolls_horizontally(widget_view)
            && modifiers == egui::Modifiers::NONE;
        #[cfg(target_os = "macos")]
        let native_samples = {
            if self.scroll_monitor.is_none() {
                self.scroll_monitor = platform::ScrollMonitor::new(frame);
            }
            self.scroll_monitor
                .as_ref()
                .map_or_else(Vec::new, platform::ScrollMonitor::drain)
        };
        let mut directions = Vec::new();
        #[cfg(target_os = "macos")]
        for &(phase, delta) in &native_samples {
            directions.extend(self.trackpad_swipe.phased(phase, delta, eligible));
        }
        #[cfg(target_os = "macos")]
        let use_raw_scroll = native_samples.is_empty();
        #[cfg(not(target_os = "macos"))]
        let use_raw_scroll = true;
        if use_raw_scroll {
            directions.extend(self.trackpad_swipe.update(scroll, time, eligible));
        }
        if can_turn_page && self.active_widget == page_before_input {
            if let Some(direction) = context
                .input(widgets::key_direction)
                .filter(|_| !context.wants_keyboard_input())
            {
                directions.clear();
                directions.push(direction);
            }
            for direction in directions {
                self.active_widget =
                    widgets::next_widget(&self.active_widget, &widget_ids, direction);
                context.request_repaint();
            }
        }
        if self.active_widget != page_before_input && swipe_hovering {
            self.page_switch_from = swipe_region;
        }
        if drag_started {
            self.window_drag.begin(frame, context);
        }
        if self.state.observe_interaction(
            hovering || dragging || notice.is_some() || menu_rect.is_some(),
            self.settings_open,
            dt,
        ) {
            context.request_repaint();
        }
        if self.dashboard_open {
            self.show_dashboard(context, &widget_ids, now);
        }
        // Active services, visible views and Core deadlines determine the next wake.
        // With no running source or pending transition, egui can sleep until input.
        let animated = self.state.animating()
            || self.state.hover_pending()
            || !self.widget_height.settled()
            || dragging;
        let extension_wake = self.extensions.next_wake(
            now,
            !self.state.expanded,
            (self.state.expanded && !self.settings_open).then_some(&self.active_widget),
        );
        let quiet_wake = (!self.preferences.quiet_hours.periods().is_empty())
            .then_some(now + Duration::from_secs(30));
        let wake = self
            .scheduler
            .next_deadline()
            .into_iter()
            .chain(extension_wake)
            .chain(quiet_wake)
            .chain(self.notifications.next_deadline())
            .chain(
                self.settings_open
                    .then(|| self.settings_view.next_search_wake())
                    .flatten(),
            )
            .chain(animated.then_some(now + Duration::from_millis(16)))
            .min();
        if let Some(wake) = wake {
            context.request_repaint_after(wake.saturating_duration_since(Instant::now()));
        }
    }
}

fn empty_widget_view() -> WidgetView {
    use crate::ui::declarative::*;
    let mut view = WidgetView::new(
        ViewNode::text("扩展当前没有内容", TextStyle::default()).color(ColorSpec::MutedText),
        [300.0, 76.0],
    );
    view.refresh_hint = RefreshHint::Static;
    view
}

pub fn run() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_title("灵动岛")
        .with_inner_size(INITIAL_WINDOW_SIZE)
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_position([355.0, 64.0]);
    eframe::run_native(
        "灵动岛",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| {
            fonts::install(&cc.egui_ctx);
            Ok(Box::new(IslandApp::new(&cc.egui_ctx)))
        }),
    )
}

#[cfg(test)]
mod sizing_tests {
    use super::*;

    #[test]
    fn minimum_widget_dimensions_follow_the_screen() {
        assert_eq!(
            minimum_widget_size(egui::vec2(1440.0, 900.0)),
            egui::vec2(280.0, 160.0)
        );
        assert_eq!(
            minimum_widget_size(egui::vec2(2160.0, 1350.0)),
            egui::vec2(343.0, 240.0)
        );
    }
}
