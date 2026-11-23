//! Data source manager displayed in its own native viewport.
use crate::core::datasource::{self, DataSourceManager, Definition, InputType, SourceSnapshot};
use crate::ui::typography::Typography;
use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use serde_json::Value;
use std::{collections::HashMap, fs, sync::mpsc, time::Duration};

mod components;
mod dialogs;
mod forms;
mod support;
mod view;
use components::*;
use support::*;

const LEFT_WIDTH: f32 = 212.0;
const COLUMN_GAP: f32 = 12.0;
const VIEW_WIDTH: f32 = 980.0;
const VIEW_HEIGHT: f32 = 680.0;
const COMPACT_WIDTH: f32 = 620.0;

/// Resolution-based layout, never based on physical pixels or OS DPI directly.
fn compact_layout(width: f32) -> bool {
    width < COMPACT_WIDTH
}

fn sidebar_width(width: f32) -> f32 {
    LEFT_WIDTH.min((width * 0.32).max(164.0))
}

pub(super) fn preferred_size(limit: [f32; 2]) -> Vec2 {
    // The available native viewport is already in logical points after OS
    // DPI scaling. Never request a surface bigger than its actual bounds.
    Vec2::new(
        VIEW_WIDTH.min(limit[0].max(1.0)),
        VIEW_HEIGHT.min(limit[1].max(1.0)),
    )
}

pub(super) fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("peninsula-data-sources")
}

/// Data sources live in a resizable OS window, not in the dynamic capsule.
/// The embedded fallback is used only when the eframe integration does not
/// support additional native viewports.
pub(super) fn show_window(
    context: &egui::Context,
    view: &mut DataSourceView,
    manager: &mut DataSourceManager,
    preferences: &mut super::settings::AppPreferences,
    monitor: Vec2,
) -> (bool, bool) {
    let size = preferred_size([monitor.x * 0.90, monitor.y * 0.90]);
    let builder = egui::ViewportBuilder::default()
        .with_title("Peninsula 数据源")
        .with_inner_size(size)
        .with_min_inner_size([360.0, 320.0]);
    let mut close = false;
    let mut changed = false;
    context.show_viewport_immediate(viewport_id(), builder, |context, class| {
        if context.input(|input| {
            (class != egui::ViewportClass::Embedded && input.viewport().close_requested())
                || input.key_pressed(egui::Key::Escape)
        }) {
            close = true;
            return;
        }
        if class == egui::ViewportClass::Embedded {
            let mut open = true;
            egui::Window::new("Peninsula 数据源")
                .open(&mut open)
                .default_size(size)
                .resizable(true)
                .show(context, |ui| {
                    changed |= view.show(ui, manager, preferences);
                });
            close = !open;
        } else {
            egui::CentralPanel::default().show(context, |ui| {
                ui.add_space(8.0);
                changed |= view.show(ui, manager, preferences);
            });
        }
    });
    (close, changed)
}

#[cfg(test)]
mod responsive_tests {
    use super::*;

    #[test]
    fn data_source_surface_respects_small_and_high_density_logical_viewports() {
        for [w, h] in [[360.0, 320.0], [560.0, 440.0], [790.0, 570.0], [1600.0, 1200.0]] {
            let size = preferred_size([w, h]);
            assert!(size.x > 0.0 && size.x <= w);
            assert!(size.y > 0.0 && size.y <= h);
        }
    }

    #[test]
    fn detail_rows_fill_right_pane_and_keep_controls_inside() {
        // Logical point widths; the same geometry is used at all DPI scales.
        for width in [360.0, 540.0, 790.0, 1200.0] {
            let context = egui::Context::default();
            let mut geometry = None;
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO, Vec2::new(width, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ctx, |ui| {
                            let available = ui.available_width();
                            let info = info_card(ui, "开放 Issue", "0 个", 13.0);
                            let mut control = egui::Rect::NOTHING;
                            let notification = runtime_row(ui, "通知提醒", |ui| {
                                control = ui
                                    .add_sized([50.0, 22.0], egui::Button::new("切换"))
                                    .rect;
                            });
                            geometry = Some((available, info, notification, control));
                        });
                },
            );
            let (available, info, notification, control) = geometry.unwrap();
            assert!(
                (info.width() - available).abs() <= 5.0,
                "at {width} pt info width {} does not fill {available}",
                info.width(),
            );
            assert!(
                (notification.width() - available).abs() <= 5.0,
                "at {width} pt notification width {} does not fill {available}",
                notification.width(),
            );
            assert!(
                notification.contains(control.center()),
                "at {width} pt notification control is outside its row",
            );
            assert!(
                control.right() <= notification.right() + 1.0,
                "at {width} pt notification control overflows right edge",
            );
        }
    }

    #[test]
    fn source_picker_stacks_below_the_compact_breakpoint() {
        for width in [320.0, 480.0, 619.0] {
            assert!(compact_layout(width));
        }
        for width in [620.0, 790.0, 1440.0] {
            assert!(!compact_layout(width));
            assert!(sidebar_width(width) <= LEFT_WIDTH);
            assert!(sidebar_width(width) + COLUMN_GAP < width);
        }
    }
}

#[derive(Default)]
pub(super) struct DataSourceView {
    selected: Option<String>,
    candidate: Option<Definition>,
    candidate_icon: Option<egui::TextureHandle>,
    allow_private_network: bool,
    status: String,
    status_source: Option<String>,
    preferences_dirty: bool,
    delete_candidate: Option<String>,
    text_drafts: HashMap<String, String>,
    runtime_drafts: HashMap<String, String>,
    picker: Option<mpsc::Receiver<Result<Option<Definition>, String>>>,
}
