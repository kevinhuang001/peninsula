use crate::metrics::{MetricsSampler, Snapshot};
use eframe::egui::{self, Vec2};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

mod animation;
mod battery;
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
use state::IslandState;
use widgets::{expanded_width, pomodoro::Pomodoro, IslandTab};

const WINDOW_SIZE: [f32; 2] = [420.0, 340.0];

struct IslandApp {
    snapshots: Receiver<Snapshot>,
    metrics: Snapshot,
    downloads: Receiver<crate::downloads::Snapshot>,
    download_snapshot: crate::downloads::Snapshot,
    notifications: notification::Notifications,
    state: IslandState,
    icons: Icons,
    mascot: Mascot,
    preferences: CompactPreferences,
    settings_view: SettingsView,
    settings_open: bool,
    window_drag: platform::WindowDrag,
    active_tab: IslandTab,
    widget_height: animation::Spring,
    trackpad_swipe: trackpad::TrackpadSwipe,
    pomodoro: Pomodoro,
    quiet_bypassed: bool,
    last_platform_check: Option<Instant>,
}

impl IslandApp {
    fn new(context: &egui::Context) -> Self {
        Self {
            snapshots: MetricsSampler::start(),
            metrics: Snapshot::default(),
            downloads: crate::downloads::start(),
            download_snapshot: crate::downloads::Snapshot::default(),
            notifications: notification::Notifications::default(),
            state: IslandState::default(),
            icons: Icons::load(context),
            mascot: Mascot::default(),
            preferences: CompactPreferences::load(),
            settings_view: SettingsView::default(),
            settings_open: false,
            window_drag: platform::WindowDrag::default(),
            active_tab: IslandTab::default(),
            widget_height: animation::Spring::new(IslandTab::Performance.extra_height()),
            trackpad_swipe: trackpad::TrackpadSwipe::default(),
            pomodoro: Pomodoro::default(),
            quiet_bypassed: false,
            last_platform_check: None,
        }
    }
}

impl eframe::App for IslandApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let now = Instant::now();
        if self
            .last_platform_check
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1))
        {
            platform::configure_floating_window(frame);
            self.last_platform_check = Some(now);
        }
        while let Ok(snapshot) = self.snapshots.try_recv() {
            self.metrics = snapshot;
        }
        while let Ok(mut snapshot) = self.downloads.try_recv() {
            self.notifications.push(snapshot.completed.drain(..));
            self.download_snapshot = snapshot;
        }
        let scheduled_quiet = self.preferences.quiet_hours.is_scheduled_now();
        if !scheduled_quiet {
            self.quiet_bypassed = false;
        }
        let do_not_disturb = self.preferences.quiet_hours.manual_enabled
            || (scheduled_quiet && !self.quiet_bypassed);
        if do_not_disturb {
            self.notifications.suppress();
        } else if self.notifications.update(now, !self.settings_open) {
            self.state.show_notification();
            ctx.request_repaint();
        }
        let notice = self.notifications.current().cloned();

        let width = if self.settings_open {
            SettingsView::WIDTH
        } else {
            expanded_width(ctx, &self.metrics)
        };
        self.state.set_expanded_width(width);
        self.state.set_compact_width(
            self.preferences
                .compact_width(!self.download_snapshot.active.is_empty()),
        );
        let dt = ctx.input(|input| input.stable_dt).clamp(0.0, 0.05);
        let (pill_width, progress) = self.state.advance(dt);
        self.widget_height.target(if notice.is_some() {
            76.0
        } else if self.settings_open {
            self.settings_view.extra_height()
        } else {
            self.active_tab.extra_height()
        });
        self.widget_height.step(dt);
        if self.pomodoro.advance(dt) {
            ctx.request_repaint();
        }
        if progress < 0.99 {
            self.mascot.advance(dt, false);
        }
        let mut interactive_rect = None;
        let mut drag_started = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.set_min_size(Vec2::from(WINDOW_SIZE));
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    let (
                        rect,
                        toggle_settings,
                        start_drag,
                        dismiss_notification,
                        pin_toggled,
                        quiet_toggled,
                        preferences_changed,
                    ) = capsule::show(
                        ui,
                        ctx,
                        pill_width,
                        progress,
                        self.widget_height.value,
                        capsule::CapsuleWidgets {
                            metrics: &self.metrics,
                            mascot: &mut self.mascot,
                            preferences: &mut self.preferences,
                            settings_open: self.settings_open,
                            settings_view: &mut self.settings_view,
                            active_tab: &mut self.active_tab,
                            timer: &mut self.pomodoro,
                            pinned: self.state.is_pinned(),
                            icons: &self.icons,
                            download: self.download_snapshot.preferred_active(),
                            notification: notice.as_ref(),
                            do_not_disturb,
                        },
                    );
                    if dismiss_notification {
                        self.notifications.dismiss();
                    }
                    if pin_toggled {
                        self.state.toggle_pin();
                        ctx.request_repaint();
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
                            expanded_width(ctx, &self.metrics)
                        };
                        self.state.retarget_expanded_width(width);
                        self.widget_height.target(if self.settings_open {
                            self.settings_view.extra_height()
                        } else {
                            self.active_tab.extra_height()
                        });
                    }
                    if preferences_changed {
                        if let Err(error) = self.preferences.save() {
                            eprintln!("Unable to save settings: {error}");
                        }
                    }
                    drag_started = start_drag;
                    interactive_rect = Some(rect);
                });
            });
        let dragging = self.window_drag.update(frame);
        platform::update_hit_region(frame, interactive_rect, dragging);

        let pointer = ctx.input(|input| input.pointer.hover_pos());
        let hovering = platform::pointer_inside(frame, interactive_rect, pointer);
        if hovering && notice.is_some() {
            self.notifications.keep_visible(now);
        }
        let (scroll, time, modifiers) =
            ctx.input(|input| (input.raw_scroll_delta, input.time, input.modifiers));
        if self.trackpad_swipe.update(
            scroll,
            time,
            hovering
                && self.state.expanded
                && notice.is_none()
                && !self.settings_open
                && modifiers == egui::Modifiers::NONE,
        ) {
            self.active_tab = self.active_tab.other();
            ctx.request_repaint();
        }
        if drag_started {
            self.window_drag.begin(frame, ctx);
        }
        if self.state.expanded
            && notice.is_none()
            && !self.settings_open
            && !ctx.wants_keyboard_input()
        {
            let switch = ctx.input(|input| {
                input.key_pressed(egui::Key::ArrowLeft) || input.key_pressed(egui::Key::ArrowRight)
            });
            if switch {
                self.active_tab = self.active_tab.other();
                ctx.request_repaint();
            }
        }
        if self
            .state
            .observe_hover(hovering || dragging || notice.is_some(), dt)
        {
            ctx.request_repaint();
        }
        ctx.request_repaint_after(Duration::from_millis(16));
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
