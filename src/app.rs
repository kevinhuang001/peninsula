use crate::{
    app::state::IslandState,
    builtins::BuiltinExtensions,
    core::{
        actions::ActionDispatcher,
        notifications::{self, NotificationCenterEntry, NotificationManager},
        Scheduler,
    },
    extension::{
        Action, ActionInvocation, AlertInteraction, Event, ExtensionId, ExtensionOutput,
        HostScheduler, Notification, NotificationAlertView, NotificationKey, WidgetView,
        NOTIFICATION_CENTER_ID,
    },
};
use eframe::egui::{self, Vec2};
use std::time::{Duration, Instant};

mod animation;
mod capsule;
mod compact_layout;
mod fonts;
mod icons;
mod mascot;
mod notification;
mod platform;
mod settings;
mod state;
mod trackpad;
mod widgets;

use icons::Icons;
use mascot::Mascot;
use settings::{CompactPreferences, SettingsView};

const WINDOW_SIZE: [f32; 2] = [420.0, 340.0];
const CLEAR_ALL_ACTION: &str = "core.notifications.clear_all";
const ACKNOWLEDGE_ACTION_PREFIX: &str = "core.notifications.acknowledge:";
const CENTER_PAGE_ACTION_PREFIX: &str = "core.notifications.page:";

struct IslandApp {
    notifications: NotificationManager,
    scheduler: Scheduler,
    action_dispatcher: ActionDispatcher,
    extensions: BuiltinExtensions,
    state: IslandState,
    icons: Icons,
    mascot: Mascot,
    preferences: CompactPreferences,
    settings_view: SettingsView,
    settings_open: bool,
    notification_center_page: usize,
    window_drag: platform::WindowDrag,
    active_widget: ExtensionId,
    widget_height: animation::Spring,
    trackpad_swipe: trackpad::TrackpadSwipe,
    quiet_bypassed: bool,
    last_platform_check: Option<Instant>,
}

impl IslandApp {
    fn new(context: &egui::Context) -> Self {
        let preferences = CompactPreferences::load();
        let mut extensions = BuiltinExtensions::new(preferences.weather.clone());
        extensions.set_pomodoro_durations(
            preferences.pomodoro_focus_minutes,
            preferences.pomodoro_break_minutes,
        );
        if let Err(error) = extensions.configure_bilibili(
            &preferences.opencli_path,
            preferences.bilibili_dm_enabled,
            preferences.bilibili_likes_enabled,
            preferences.bilibili_show_dm_summary,
            preferences.bilibili_poll_seconds,
        ) {
            eprintln!("Unable to configure Bilibili extension: {error}");
        }
        if preferences.bilibili_enabled {
            if let Err(error) = extensions.set_bilibili_enabled(true) {
                eprintln!("Unable to start Bilibili extension: {error}");
            }
        }
        let active_widget = extensions.widget_ids()[0].clone();
        let initial_widget_height = extensions.widget_height(&active_widget);
        Self {
            notifications: NotificationManager::default(),
            scheduler: Scheduler::default(),
            action_dispatcher: ActionDispatcher::default(),
            extensions,
            state: IslandState::default(),
            icons: Icons::load(context),
            mascot: Mascot::default(),
            preferences,
            settings_view: SettingsView::default(),
            settings_open: false,
            notification_center_page: 0,
            window_drag: platform::WindowDrag::default(),
            active_widget: active_widget.clone(),
            widget_height: animation::Spring::new(initial_widget_height),
            trackpad_swipe: trackpad::TrackpadSwipe::default(),
            quiet_bypassed: false,
            last_platform_check: None,
        }
    }

    fn apply_extension_output(&mut self, output: ExtensionOutput, context: &egui::Context) {
        for extension_id in output.invalidated {
            self.scheduler.invalidate(&extension_id);
        }
        for task_id in output.cancel_tasks {
            self.scheduler.cancel(&task_id);
        }
        for task in output.schedule {
            self.scheduler.schedule_at(&task.task_id, task.deadline);
        }
        let published = !output.notifications.is_empty();
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
        context.request_repaint();
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
        if self
            .last_platform_check
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1))
        {
            platform::configure_floating_window(frame);
            self.last_platform_check = Some(now);
        }

        for output in self.extensions.update(now) {
            self.apply_extension_output(output, context);
        }
        self.apply_extension_output(self.extensions.drain_context_tasks(), context);
        self.scheduler.due_tasks(now);
        let events: Vec<_> = self.scheduler.drain_events().collect();
        for event in events {
            for output in self.extensions.on_event(&event, now) {
                self.apply_extension_output(output, context);
            }
        }

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
        let alert_view: Option<NotificationAlertView> = notice.as_ref().map(|notification| {
            self.extensions
                .notification_presentation(notification)
                .alert
        });

        let widget_ids = self.extensions.widget_ids();
        if !widget_ids.contains(&self.active_widget) {
            self.active_widget = widget_ids[0].clone();
        }
        let compact_views = self.extensions.compact_views();
        let width = if self.settings_open {
            SettingsView::WIDTH
        } else {
            self.extensions.widget_width(&self.active_widget)
        };
        self.state.set_expanded_width(width);
        self.state
            .set_compact_width(compact_layout::width(&self.preferences, &compact_views));

        let dt = context.input(|input| input.stable_dt).clamp(0.0, 0.05);
        let (pill_width, progress) = self.state.advance(dt);
        self.widget_height.target(if notice.is_some() {
            76.0
        } else if self.state.hides_widget() {
            0.0
        } else if self.settings_open {
            self.settings_view.extra_height()
        } else {
            self.extensions.widget_height(&self.active_widget)
        });
        self.widget_height.step(dt);
        if progress < 0.99 {
            self.mascot.advance(dt, false);
        }

        let center_entries: Vec<_> = self
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
        let page_size = self.notifications.policy().max_center_items.max(1);
        let page_count = self
            .notifications
            .pending_count()
            .div_ceil(page_size)
            .max(1);
        self.notification_center_page = self
            .notification_center_page
            .min(page_count.saturating_sub(1));
        let widget_view = if self.active_widget.as_str() == NOTIFICATION_CENTER_ID {
            notifications::widget_view(
                &center_entries,
                self.notification_center_page,
                self.notifications.policy().max_center_items,
            )
        } else {
            self.extensions
                .widget_view(&self.active_widget)
                .unwrap_or_else(empty_widget_view)
        };
        let widget_title = self.extensions.widget_title(&self.active_widget);
        let bilibili_status = self.extensions.bilibili_status();

        let mut interactive_rect = None;
        let mut drag_started = false;
        let mut action_result = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(context, |ui| {
                ui.set_min_size(Vec2::from(WINDOW_SIZE));
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    let (
                        rect,
                        toggle_settings,
                        start_drag,
                        alert_interaction,
                        pin_toggled,
                        quiet_toggled,
                        preferences_changed,
                        widget_action,
                    ) = capsule::show(
                        ui,
                        context,
                        pill_width,
                        progress,
                        self.widget_height.value,
                        capsule::CapsuleWidgets {
                            mascot: &mut self.mascot,
                            preferences: &mut self.preferences,
                            compact_views: &compact_views,
                            settings_open: self.settings_open,
                            settings_view: &mut self.settings_view,
                            active_widget: &mut self.active_widget,
                            widget_ids: &widget_ids,
                            widget_title: &widget_title,
                            widget_view: &widget_view,
                            alert_view: alert_view.as_ref(),
                            max_alert_actions: self.notifications.policy().max_alert_actions,
                            pinned: self.state.is_pinned(),
                            icons: &self.icons,
                            notification: notice.as_ref(),
                            do_not_disturb,
                            bilibili_status: &bilibili_status,
                        },
                    );
                    action_result = Some((
                        alert_interaction,
                        widget_action,
                        pin_toggled,
                        quiet_toggled,
                        preferences_changed,
                        toggle_settings,
                        start_drag,
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
            _,
        )) = action_result
        {
            if let (Some(interaction), Some(notification)) = (alert_interaction, notice.as_ref()) {
                self.handle_alert_interaction(interaction, notification);
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
                    SettingsView::WIDTH
                } else {
                    self.extensions.widget_width(&self.active_widget)
                };
                self.state.retarget_expanded_width(width);
                self.widget_height.target(if self.settings_open {
                    self.settings_view.extra_height()
                } else {
                    self.extensions.widget_height(&self.active_widget)
                });
            }
            if preferences_changed {
                for output in self.extensions.set_pomodoro_durations(
                    self.preferences.pomodoro_focus_minutes,
                    self.preferences.pomodoro_break_minutes,
                ) {
                    self.apply_extension_output(output, context);
                }
                if let Err(error) = self.extensions.configure_bilibili(
                    &self.preferences.opencli_path,
                    self.preferences.bilibili_dm_enabled,
                    self.preferences.bilibili_likes_enabled,
                    self.preferences.bilibili_show_dm_summary,
                    self.preferences.bilibili_poll_seconds,
                ) {
                    eprintln!("Unable to configure Bilibili extension: {error}");
                }
                if let Err(error) = self
                    .extensions
                    .set_bilibili_enabled(self.preferences.bilibili_enabled)
                {
                    eprintln!("Unable to update Bilibili extension: {error}");
                }
                if self.preferences.bilibili_check_requested {
                    self.preferences.bilibili_check_requested = false;
                    if let Err(error) = self.extensions.request_bilibili_check() {
                        eprintln!("Unable to check Bilibili connection: {error}");
                    }
                }
                if let Err(error) = self.preferences.save() {
                    eprintln!("Unable to save settings: {error}");
                }
                self.extensions
                    .apply_weather_preferences(&self.preferences.weather);
            }
            if let Some(action) = widget_action {
                self.dispatch_widget_action(action, now, context);
            }
        }

        let dragging = self.window_drag.update(frame);
        platform::update_hit_region(frame, interactive_rect, dragging);

        let pointer = context.input(|input| input.pointer.hover_pos());
        let hovering = platform::pointer_inside(frame, interactive_rect, pointer);
        if hovering && notice.is_some() {
            self.notifications.keep_alert_visible(now);
        }
        let (scroll, time, modifiers) =
            context.input(|input| (input.raw_scroll_delta, input.time, input.modifiers));
        if self.trackpad_swipe.update(
            scroll,
            time,
            hovering
                && self.state.expanded
                && notice.is_none()
                && !self.settings_open
                && modifiers == egui::Modifiers::NONE,
        ) {
            self.active_widget = widgets::next_widget(&self.active_widget, &widget_ids, 1);
            context.request_repaint();
        }
        if drag_started {
            self.window_drag.begin(frame, context);
        }
        if self.state.expanded
            && notice.is_none()
            && !self.settings_open
            && !context.wants_keyboard_input()
        {
            let switch = context.input(|input| {
                input.key_pressed(egui::Key::ArrowLeft) || input.key_pressed(egui::Key::ArrowRight)
            });
            if switch {
                self.active_widget = widgets::next_widget(&self.active_widget, &widget_ids, 1);
                context.request_repaint();
            }
        }
        if self
            .state
            .observe_hover(hovering || dragging || notice.is_some(), dt)
        {
            context.request_repaint();
        }
        if !self.scheduler.take_invalidated().is_empty() {
            context.request_repaint();
        }
        context.request_repaint_after(Duration::from_millis(16));
    }
}

fn empty_widget_view() -> WidgetView {
    WidgetView {
        root: crate::extension::ViewNode::Text {
            text: "扩展当前没有内容".into(),
            style: crate::extension::TextStyle::Muted,
        },
        preferred_size: [300.0, 76.0],
        refresh_hint: crate::extension::RefreshHint::Static,
    }
}

pub fn run() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_title("灵动岛")
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(WINDOW_SIZE)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_position([505.0, 64.0]);
    eframe::run_native(
        "灵动岛",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            fonts::install(&cc.egui_ctx);
            Ok(Box::new(IslandApp::new(&cc.egui_ctx)))
        }),
    )
}
