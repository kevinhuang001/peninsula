use crate::metrics::{MetricsSampler, Snapshot};
#[cfg(not(target_os = "macos"))]
use eframe::egui::Pos2;
use eframe::egui::{self, Color32, Vec2};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

mod animation;
mod battery;
mod capsule;
mod fonts;
mod indicator;
mod mascot;
mod platform;
mod settings;
mod state;
mod trackpad;
mod widgets;

use mascot::Mascot;
use settings::{CompactPreferences, SettingsPanel};
#[cfg(not(target_os = "macos"))]
use state::window_left_for_center;
use state::IslandState;
use widgets::{expanded_width, pomodoro::Pomodoro, IslandTab};

const COLLAPSED_SIZE: [f32; 2] = [420.0, 190.0];
const SETTINGS_SIZE: [f32; 2] = [760.0, 520.0];

struct IslandApp {
    snapshots: Receiver<Snapshot>,
    metrics: Snapshot,
    state: IslandState,
    mascot: Mascot,
    preferences: CompactPreferences,
    settings_panel: SettingsPanel,
    settings_open: bool,
    active_tab: IslandTab,
    indicator: indicator::Indicator,
    widget_height: animation::Spring,
    trackpad_swipe: trackpad::TrackpadSwipe,
    pomodoro: Pomodoro,
    last_platform_check: Option<Instant>,
    native_size: [f32; 2],
}

impl Default for IslandApp {
    fn default() -> Self {
        Self {
            snapshots: MetricsSampler::start(),
            metrics: Snapshot::default(),
            state: IslandState::default(),
            mascot: Mascot::default(),
            preferences: CompactPreferences::load(),
            settings_panel: SettingsPanel::default(),
            settings_open: false,
            active_tab: IslandTab::default(),
            indicator: indicator::Indicator::default(),
            widget_height: animation::Spring::new(IslandTab::Performance.extra_height()),
            trackpad_swipe: trackpad::TrackpadSwipe::default(),
            pomodoro: Pomodoro::default(),
            last_platform_check: None,
            native_size: COLLAPSED_SIZE,
        }
    }
}

impl eframe::App for IslandApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.set_visuals(if self.settings_open {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        });
        let now = Instant::now();
        let previous_tab = self.active_tab;
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

        let width = expanded_width(ctx, &self.metrics);
        self.state.set_expanded_width(width);
        self.state
            .set_compact_width(self.preferences.compact_width());
        let dt = ctx.input(|input| input.stable_dt).clamp(0.0, 0.05);
        let (pill_width, progress) = self.state.advance(dt);
        self.widget_height.target(self.active_tab.extra_height());
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
            .frame(egui::Frame::new().fill(if self.settings_open {
                Color32::from_rgb(247, 247, 249)
            } else {
                Color32::TRANSPARENT
            }))
            .show(ctx, |ui| {
                ui.set_min_size(Vec2::from(self.native_size));
                if self.settings_open {
                    let panel = ui.max_rect();
                    let before = self.preferences;
                    egui::Frame::new()
                        .fill(Color32::from_rgb(247, 247, 249))
                        .corner_radius(18.0)
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            if self.settings_panel.show(ui, &mut self.preferences) {
                                self.settings_open = false;
                            }
                        });
                    if before != self.preferences {
                        let _ = self.preferences.save();
                    }
                    interactive_rect = Some(panel);
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        let (rect, open_settings, start_drag) = capsule::show(
                            ui,
                            ctx,
                            pill_width,
                            progress,
                            self.widget_height.value,
                            capsule::CapsuleWidgets {
                                metrics: &self.metrics,
                                mascot: &mut self.mascot,
                                preferences: &self.preferences,
                                active_tab: &mut self.active_tab,
                                timer: &mut self.pomodoro,
                                indicator_opacity: self.indicator.opacity(now),
                            },
                        );
                        if open_settings {
                            self.settings_open = true;
                        }
                        drag_started = start_drag;
                        interactive_rect = Some(rect);
                    });
                }
            });

        self.resize_native(frame, ctx);
        platform::update_hit_region(frame, interactive_rect);

        let pointer = ctx.input(|input| input.pointer.hover_pos());
        let hovering =
            pointer.is_some_and(|point| interactive_rect.is_some_and(|rect| rect.contains(point)));
        let (scroll, time, modifiers) =
            ctx.input(|input| (input.raw_scroll_delta, input.time, input.modifiers));
        if self.trackpad_swipe.update(
            scroll,
            time,
            hovering
                && self.state.expanded
                && !self.settings_open
                && modifiers == egui::Modifiers::NONE,
        ) {
            self.active_tab = self.active_tab.other();
            ctx.request_repaint();
        }
        if drag_started && !self.settings_open {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if !self.settings_open && self.state.expanded && !ctx.wants_keyboard_input() {
            let switch = ctx.input(|input| {
                input.key_pressed(egui::Key::ArrowLeft) || input.key_pressed(egui::Key::ArrowRight)
            });
            if switch {
                self.active_tab = self.active_tab.other();
                ctx.request_repaint();
            }
        }
        if !self.settings_open && self.state.observe_hover(hovering, dt) {
            ctx.request_repaint();
        }
        if self.active_tab != previous_tab {
            self.indicator.show(now);
            ctx.request_repaint();
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn window_target_size(
    settings_open: bool,
    expanded: bool,
    animating: bool,
    compact_width: f32,
) -> [f32; 2] {
    if settings_open {
        SETTINGS_SIZE
    } else {
        let _ = (expanded, animating, compact_width);
        COLLAPSED_SIZE
    }
}

impl IslandApp {
    fn resize_native(&mut self, frame: &eframe::Frame, ctx: &egui::Context) -> bool {
        let target = window_target_size(
            self.settings_open,
            self.state.expanded,
            self.state.animating(),
            self.preferences.compact_width(),
        );
        let changed = target != self.native_size;
        if !changed {
            return false;
        }
        if platform::resize_anchored(frame, target) {
            self.native_size = target;
            // AppKit can resize the transparent window without going through
            // egui's viewport command path; repaint now to clear the old backing
            // surface at the cropped edge.
            ctx.request_repaint();
            return false;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let outer = ctx.input(|input| input.viewport().outer_rect);
            let center_x = outer.map_or(715.0, |rect| rect.center().x);
            let top_y = outer.map_or(64.0, |rect| rect.top());
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target.into()));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(
                window_left_for_center(center_x, target[0]),
                top_y,
            )));
            self.native_size = target;
            ctx.request_repaint();
            false
        }
        #[cfg(target_os = "macos")]
        {
            // Wait for AppKit to provide the NSWindow instead of resizing against
            // an uninitialized frame or guessing a position from stale egui data.
            ctx.request_repaint_after(Duration::from_millis(16));
            true
        }
    }
}

pub fn run() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_title("灵动岛")
        .with_inner_size(COLLAPSED_SIZE)
        .with_min_inner_size(COLLAPSED_SIZE)
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
            Ok(Box::<IslandApp>::default())
        }),
    )
}

#[cfg(test)]
mod tests;
