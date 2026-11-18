use super::{
    model::Section,
    quiet_view::{self, QuietDialog},
    theme::{
        ACCENT, ACCENT_SELECTED, BACKGROUND, BORDER, CHECKBOX_ON, PANEL, ROW, SECONDARY_TEXT,
        SWITCH_OFF, SWITCH_ON, TEXT,
    },
    weather_view::{self, WeatherSearch},
    CompactPreferences,
};
use crate::{
    builtins::{kuaidi, registry},
    extension::ExtensionId,
};
use eframe::egui::{self, Align2, Color32, FontId, Galley, Pos2, Rect, Sense, Stroke, Vec2};
use std::sync::Arc;

const BODY_TOP: f32 = 41.0;
const BODY_BOTTOM: f32 = 8.0;
const BODY_SIDE: f32 = 12.0;
const SIDEBAR_WIDTH: f32 = 108.0;
const COLUMN_GAP: f32 = 9.0;
const PANEL_PADDING: f32 = 8.0;
const COMPACT_ROW_STEP: f32 = 32.0;
const COMPACT_ROW_HEIGHT: f32 = 26.0;
const POMODORO_ROW_STEP: f32 = 44.0;
const POMODORO_ROW_HEIGHT: f32 = 36.0;
const POMODORO_ROWS: usize = 2;
const KUAIDI_CHECKBOX_STEP: f32 = 34.0;
const CHECKBOX_ROW_HEIGHT: f32 = 26.0;
const BILIBILI_INTERVAL_TOP: f32 = 36.0;
const BILIBILI_INTERVAL_HEIGHT: f32 = 28.0;
const STATUS_GAP: f32 = 8.0;
const ABOUT_GAP: f32 = 6.0;

#[derive(Debug, Default)]
pub struct SettingsView {
    section: Section,
    quiet_dialog: Option<QuietDialog>,
    weather_search: WeatherSearch,
}

struct SettingsLayout {
    title: Arc<Galley>,
    description: Option<Arc<Galley>>,
    description_top: f32,
    controls_top: f32,
    details: Vec<Arc<Galley>>,
    height: f32,
}

impl SettingsLayout {
    fn measure(
        ctx: &egui::Context,
        section: Section,
        width: f32,
        prefs: &CompactPreferences,
        bilibili_status: &str,
    ) -> Self {
        let wrap_width = (width - 28.0).max(1.0);
        let text = |label: &str, size, color| {
            ctx.fonts(|fonts| {
                fonts.layout(
                    label.to_owned(),
                    FontId::proportional(size),
                    color,
                    wrap_width,
                )
            })
        };
        let title = text(section.title(), 15.0, TEXT);
        let description = (!section.description().is_empty())
            .then(|| text(section.description(), 11.0, SECONDARY_TEXT));
        let description_top = 9.0 + title.size().y + 3.0;
        let controls_top = description
            .as_ref()
            .map_or(9.0 + title.size().y, |description| {
                description_top + description.size().y
            })
            + 9.0;
        let mut details = Vec::new();
        let controls_height = match section {
            Section::Compact => {
                let registry = registry();
                let count = prefs
                    .compact_items
                    .iter()
                    .filter(|item| registry.get(&item.id).is_some())
                    .count();
                stacked_height(count + 1, COMPACT_ROW_STEP, COMPACT_ROW_HEIGHT)
            }
            Section::QuietHours => quiet_view::body_height(prefs.quiet_hours.periods().len()),
            Section::Pomodoro => {
                stacked_height(POMODORO_ROWS, POMODORO_ROW_STEP, POMODORO_ROW_HEIGHT)
            }
            Section::Bilibili => {
                let mut height = BILIBILI_INTERVAL_TOP + BILIBILI_INTERVAL_HEIGHT;
                if !bilibili_status.is_empty() {
                    let status = text(bilibili_status, 9.5, SECONDARY_TEXT);
                    height += STATUS_GAP + status.size().y;
                    details.push(status);
                }
                height
            }
            Section::Kuaidi => 2.0 * KUAIDI_CHECKBOX_STEP + POMODORO_ROW_HEIGHT,
            Section::Weather => weather_view::CONTENT_HEIGHT,
            Section::About => {
                details.push(text("性能监控 · 番茄钟 · 下载提醒", 11.5, TEXT));
                details.push(text(
                    concat!("灵动岛  ", env!("CARGO_PKG_VERSION")),
                    10.5,
                    SECONDARY_TEXT,
                ));
                details.iter().map(|line| line.size().y).sum::<f32>() + ABOUT_GAP
            }
        };
        Self {
            title,
            description,
            description_top,
            controls_top,
            details,
            height: controls_top + controls_height + PANEL_PADDING,
        }
    }
}

impl SettingsView {
    pub const WIDTH: f32 = 400.0;

    pub fn extra_height(
        &self,
        ctx: &egui::Context,
        prefs: &CompactPreferences,
        bilibili_status: &str,
    ) -> f32 {
        let panel_width = Self::WIDTH - 2.0 * BODY_SIDE - SIDEBAR_WIDTH - COLUMN_GAP;
        let content =
            SettingsLayout::measure(ctx, self.section, panel_width, prefs, bilibili_status);
        let sidebar = section_row(Rect::ZERO, Section::ALL.len() - 1).bottom() + PANEL_PADDING;
        BODY_TOP + content.height.max(sidebar).ceil() + BODY_BOTTOM
            - crate::app::capsule::EXPANDED_BASE_HEIGHT
    }

    pub(in crate::app) fn show(
        &mut self,
        ui: &mut egui::Ui,
        capsule: Rect,
        preferences: &mut CompactPreferences,
        bilibili_status: &str,
    ) -> bool {
        let body = Rect::from_min_max(
            Pos2::new(capsule.left() + BODY_SIDE, capsule.top() + BODY_TOP),
            Pos2::new(capsule.right() - BODY_SIDE, capsule.bottom() - BODY_BOTTOM),
        );
        if body.height() <= 0.0 || body.width() < SIDEBAR_WIDTH + COLUMN_GAP + 100.0 {
            return false;
        }
        let sidebar = Rect::from_min_size(body.min, Vec2::new(SIDEBAR_WIDTH, body.height()));
        let panel = Rect::from_min_max(
            Pos2::new(sidebar.right() + COLUMN_GAP, body.top()),
            body.max,
        );

        // A click changes the section for the next frame, when the capsule has the new target size.
        let section = self.section;
        paint_panel(ui, sidebar, BACKGROUND);
        paint_panel(ui, panel, PANEL);
        self.paint_sections(&panel_ui(ui, sidebar, "settings-sidebar"), sidebar);
        let mut content_ui = panel_ui(ui, panel, "settings-content");
        let layout = SettingsLayout::measure(
            ui.ctx(),
            section,
            panel.width(),
            preferences,
            bilibili_status,
        );
        // Only scroll when the shell is still growing or the content exceeds the window.
        let changed = if layout.height > panel.height() {
            egui::ScrollArea::vertical()
                .id_salt(section)
                .auto_shrink([false; 2])
                .max_height(panel.height())
                .min_scrolled_height(0.0)
                .show(&mut content_ui, |ui| {
                    let width = ui.available_width();
                    let layout = SettingsLayout::measure(
                        ui.ctx(),
                        section,
                        width,
                        preferences,
                        bilibili_status,
                    );
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(width, layout.height), Sense::hover());
                    paint_content(
                        ui,
                        rect,
                        section,
                        &layout,
                        preferences,
                        &mut self.quiet_dialog,
                        &mut self.weather_search,
                    )
                })
                .inner
        } else {
            paint_content(
                &mut content_ui,
                panel,
                section,
                &layout,
                preferences,
                &mut self.quiet_dialog,
                &mut self.weather_search,
            )
        };
        if section == Section::QuietHours && panel.height() >= quiet_view::DIALOG_MIN_HEIGHT {
            return quiet_view::show_editor(
                &mut content_ui,
                panel,
                &mut preferences.quiet_hours,
                &mut self.quiet_dialog,
            ) | changed;
        }
        changed
    }

    fn paint_sections(&mut self, ui: &egui::Ui, sidebar: Rect) {
        for (index, section) in Section::ALL.into_iter().enumerate() {
            let row = section_row(sidebar, index);
            let response = ui.interact(
                row,
                egui::Id::new(("settings-section", section)),
                Sense::click(),
            );
            let selected = self.section == section;
            if response.clicked() {
                self.section = section;
            }
            ui.painter().rect_filled(
                row,
                8.0,
                if selected {
                    ACCENT_SELECTED
                } else {
                    BACKGROUND
                },
            );
            if selected {
                ui.painter().rect_filled(
                    Rect::from_min_size(row.left_top() + Vec2::new(1.0, 7.0), Vec2::new(3.0, 16.0)),
                    2.0,
                    ACCENT,
                );
            }
            ui.painter().text(
                Pos2::new(row.left() + 12.0, row.center().y),
                Align2::LEFT_CENTER,
                section.label(),
                FontId::proportional(12.0),
                if selected { TEXT } else { SECONDARY_TEXT },
            );
        }
    }
}

fn stacked_height(count: usize, step: f32, row_height: f32) -> f32 {
    if count == 0 {
        0.0
    } else {
        (count - 1) as f32 * step + row_height
    }
}

fn section_row(sidebar: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        sidebar.left_top() + Vec2::new(5.0, PANEL_PADDING + index as f32 * 36.0),
        Vec2::new(SIDEBAR_WIDTH - 10.0, 30.0),
    )
}

fn panel_ui(ui: &mut egui::Ui, rect: Rect, id: &str) -> egui::Ui {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child
}

fn paint_content(
    ui: &mut egui::Ui,
    panel: Rect,
    section: Section,
    layout: &SettingsLayout,
    preferences: &mut CompactPreferences,
    dialog: &mut Option<QuietDialog>,
    search: &mut WeatherSearch,
) -> bool {
    let painter = ui.painter().clone();
    let left = panel.left() + 14.0;
    painter.galley(
        Pos2::new(left, panel.top() + 9.0),
        layout.title.clone(),
        TEXT,
    );
    if let Some(description) = &layout.description {
        painter.galley(
            Pos2::new(left, panel.top() + layout.description_top),
            description.clone(),
            SECONDARY_TEXT,
        );
    }
    let controls = Rect::from_min_max(
        Pos2::new(panel.left(), panel.top() + layout.controls_top),
        Pos2::new(panel.right(), panel.top() + layout.height - PANEL_PADDING),
    );
    match section {
        Section::Compact => paint_compact_options(ui, controls, preferences),
        Section::QuietHours => quiet_view::show(ui, controls, &mut preferences.quiet_hours, dialog),
        Section::Pomodoro => paint_pomodoro_options(ui, controls, preferences),
        Section::Kuaidi => paint_kuaidi_options(ui, controls, preferences),
        Section::Weather => weather_view::show(ui, controls, preferences, search),
        Section::Bilibili => {
            let changed = paint_bilibili_options(ui, controls, preferences);
            if let Some(status) = layout.details.first() {
                let top =
                    controls.top() + BILIBILI_INTERVAL_TOP + BILIBILI_INTERVAL_HEIGHT + STATUS_GAP;
                painter.galley(Pos2::new(left, top), status.clone(), SECONDARY_TEXT);
            }
            changed
        }
        Section::About => {
            let mut y = controls.top();
            for line in &layout.details {
                painter.galley(Pos2::new(left, y), line.clone(), TEXT);
                y += line.size().y + ABOUT_GAP;
            }
            false
        }
    }
}

fn paint_kuaidi_options(ui: &mut egui::Ui, panel: Rect, prefs: &mut CompactPreferences) -> bool {
    let mut changed = false;
    for (index, (label, enabled)) in [
        ("快递变更通知", &mut prefs.kuaidi_change_notifications),
        ("取件通知", &mut prefs.kuaidi_pickup_notifications),
    ]
    .into_iter()
    .enumerate()
    {
        let row = Rect::from_min_size(
            panel.left_top() + Vec2::new(10.0, index as f32 * KUAIDI_CHECKBOX_STEP),
            Vec2::new(panel.width() - 20.0, CHECKBOX_ROW_HEIGHT),
        );
        changed |= paint_checkbox_row(ui, row, label, enabled);
    }
    let interval = Rect::from_min_max(
        panel.min + Vec2::new(0.0, 2.0 * KUAIDI_CHECKBOX_STEP),
        panel.max,
    );
    changed
        | paint_minutes_row(
            ui,
            interval,
            0,
            "刷新间隔",
            &mut prefs.kuaidi_refresh_minutes,
            kuaidi::MAX_REFRESH_MINUTES,
        )
}

fn paint_bilibili_options(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
) -> bool {
    let row = Rect::from_min_size(
        panel.left_top() + Vec2::new(12.0, 0.0),
        Vec2::new(panel.width() - 24.0, 26.0),
    );
    let enabled_changed = paint_checkbox_row(
        ui,
        row,
        "启用 Bilibili 通知",
        &mut preferences.bilibili_enabled,
    );
    let interval = Rect::from_min_size(
        Pos2::new(panel.left() + 12.0, panel.top() + BILIBILI_INTERVAL_TOP),
        Vec2::new(panel.width() - 24.0, BILIBILI_INTERVAL_HEIGHT),
    );
    ui.painter().text(
        interval.left_center() + Vec2::new(3.0, 0.0),
        Align2::LEFT_CENTER,
        "检查间隔",
        FontId::proportional(11.0),
        TEXT,
    );
    let interval_changed = ui
        .put(
            Rect::from_min_size(
                Pos2::new(interval.right() - 92.0, interval.top()),
                Vec2::new(86.0, 27.0),
            ),
            egui::DragValue::new(&mut preferences.bilibili_poll_seconds)
                .range(15..=3600)
                .suffix(" 秒"),
        )
        .changed();
    enabled_changed | interval_changed
}

fn paint_checkbox_row(ui: &mut egui::Ui, row: Rect, label: &str, enabled: &mut bool) -> bool {
    let response = ui.interact(row, ui.make_persistent_id(label), Sense::click());
    ui.painter().rect_filled(row, 6.0, ROW);
    let checkbox = Rect::from_center_size(
        Pos2::new(row.left() + 19.0, row.center().y),
        Vec2::splat(16.0),
    );
    ui.painter().rect_filled(
        checkbox,
        4.0,
        if *enabled { CHECKBOX_ON } else { BACKGROUND },
    );
    ui.painter().rect_stroke(
        checkbox,
        4.0,
        Stroke::new(1.0, if *enabled { CHECKBOX_ON } else { BORDER }),
        egui::StrokeKind::Inside,
    );
    if *enabled {
        let points = [
            Pos2::new(checkbox.left() + 4.0, checkbox.center().y),
            Pos2::new(checkbox.left() + 7.0, checkbox.bottom() - 4.0),
            Pos2::new(checkbox.right() - 3.0, checkbox.top() + 4.0),
        ];
        ui.painter().add(egui::Shape::line(
            points.to_vec(),
            Stroke::new(1.7, Color32::WHITE),
        ));
    }
    ui.painter().text(
        Pos2::new(row.left() + 34.0, row.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        TEXT,
    );
    if response.clicked() {
        *enabled = !*enabled;
        true
    } else {
        false
    }
}

fn paint_pomodoro_options(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
) -> bool {
    let mut changed = false;
    for (index, (label, value)) in [
        ("专注时长", &mut preferences.pomodoro_focus_minutes),
        ("休息时长", &mut preferences.pomodoro_break_minutes),
    ]
    .into_iter()
    .enumerate()
    {
        changed |= paint_minutes_row(ui, panel, index, label, value, 180);
    }
    changed
}

fn paint_minutes_row(
    ui: &mut egui::Ui,
    panel: Rect,
    index: usize,
    label: &str,
    value: &mut u16,
    max: u16,
) -> bool {
    let row = Rect::from_min_size(
        Pos2::new(
            panel.left() + 10.0,
            panel.top() + index as f32 * POMODORO_ROW_STEP,
        ),
        Vec2::new(panel.width() - 20.0, POMODORO_ROW_HEIGHT),
    );
    ui.painter().rect_filled(row, 7.0, ROW);
    ui.painter().text(
        Pos2::new(row.left() + 10.0, row.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        TEXT,
    );
    ui.put(
        Rect::from_min_size(
            Pos2::new(row.right() - 95.0, row.top() + 4.0),
            Vec2::new(86.0, 28.0),
        ),
        egui::DragValue::new(value).range(1..=max).suffix(" 分钟"),
    )
    .changed()
}

fn paint_compact_options(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
) -> bool {
    let mut changed = false;
    let registry = registry();
    let items: Vec<_> = preferences
        .ordered_compact_items()
        .into_iter()
        .filter_map(|item| {
            registry.get(&item.id).map(|descriptor| {
                (
                    item.id.clone(),
                    descriptor.title.clone(),
                    preferences.is_enabled(&item.id),
                )
            })
        })
        .collect();
    let motion_row = compact_row(panel, items.len());
    for (index, (id, title, enabled)) in items.into_iter().enumerate() {
        let row = compact_row(panel, index);
        changed |= paint_option(ui, row, preferences, id, &title, enabled);
    }
    changed |= paint_checkbox_row(
        ui,
        motion_row,
        "减少动态效果",
        &mut preferences.reduced_motion,
    );
    changed
}

fn compact_row(panel: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(
            panel.left() + 8.0,
            panel.top() + index as f32 * COMPACT_ROW_STEP,
        ),
        Vec2::new(panel.width() - 16.0, COMPACT_ROW_HEIGHT),
    )
}

fn paint_panel(ui: &egui::Ui, rect: Rect, fill: Color32) {
    ui.painter().rect_filled(rect, 10.0, fill);
    ui.painter().rect_stroke(
        rect,
        10.0,
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );
}

fn paint_option(
    ui: &mut egui::Ui,
    rect: Rect,
    preferences: &mut CompactPreferences,
    option: ExtensionId,
    title: &str,
    enabled: bool,
) -> bool {
    ui.painter().rect_filled(rect, 7.0, ROW);
    ui.painter().text(
        Pos2::new(rect.left() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        TEXT,
    );
    let up = text_button(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.right() - 67.0, rect.center().y),
            Vec2::new(18.0, 22.0),
        ),
        ("compact-order-up", option.clone()),
        "↑",
    );
    let down = text_button(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.right() - 47.0, rect.center().y),
            Vec2::new(18.0, 22.0),
        ),
        ("compact-order-down", option.clone()),
        "↓",
    );
    let switch = Rect::from_center_size(
        Pos2::new(rect.right() - 19.0, rect.center().y),
        Vec2::new(30.0, 18.0),
    );
    let toggle = ui.interact(
        switch,
        egui::Id::new(("settings-option", option.clone())),
        Sense::click(),
    );
    if toggle.clicked() {
        preferences.set_enabled(&option, !enabled);
    }
    ui.painter()
        .rect_filled(switch, 9.0, if enabled { SWITCH_ON } else { SWITCH_OFF });
    ui.painter().circle_filled(
        Pos2::new(
            if enabled {
                switch.right() - 9.0
            } else {
                switch.left() + 9.0
            },
            switch.center().y,
        ),
        7.0,
        TEXT,
    );
    let mut changed = toggle.clicked();
    if up {
        changed |= preferences.move_compact_item(&option, -1);
    }
    if down {
        changed |= preferences.move_compact_item(&option, 1);
    }
    changed
}

fn text_button(ui: &egui::Ui, rect: Rect, id: impl std::hash::Hash, label: &str) -> bool {
    let response = ui.interact(rect, egui::Id::new(id), Sense::click());
    ui.painter()
        .rect_filled(rect, 5.0, ui.visuals().selection.bg_fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::WHITE,
    );
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_fit_all_capsule_items_and_sidebar_tabs() {
        let ctx = egui::Context::default();
        let prefs = CompactPreferences::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let view = SettingsView::default();
            let layout = SettingsLayout::measure(ctx, Section::Compact, 259.0, &prefs, "");
            let controls = Rect::from_min_size(
                Pos2::new(0.0, layout.controls_top),
                Vec2::new(259.0, layout.height - layout.controls_top),
            );
            let count = prefs.compact_items.len();
            assert!(count >= 8);
            assert_eq!(
                compact_row(controls, count).bottom() + PANEL_PADDING,
                layout.height
            );
            let height = view.extra_height(ctx, &prefs, "");
            assert!(
                height + crate::app::capsule::EXPANDED_BASE_HEIGHT + 4.0
                    <= 900.0 * crate::app::SCREEN_FRACTION
            );

            let about = SettingsView {
                section: Section::About,
                ..view
            };
            let about_height = about.extra_height(ctx, &prefs, "");
            let panel_height =
                about_height + crate::app::capsule::EXPANDED_BASE_HEIGHT - BODY_TOP - BODY_BOTTOM;
            assert!(
                section_row(Rect::ZERO, Section::ALL.len() - 1).bottom() + PANEL_PADDING
                    <= panel_height
            );
            assert!(about_height < height);
        });
    }

    #[test]
    fn settings_grow_with_quiet_hours_and_wrapped_status() {
        let ctx = egui::Context::default();
        let mut prefs = CompactPreferences::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let quiet = SettingsView {
                section: Section::QuietHours,
                ..Default::default()
            };
            let empty_height = quiet.extra_height(ctx, &prefs, "");
            for _ in 0..8 {
                prefs
                    .quiet_hours
                    .add_period(super::super::quiet_hours::QuietPeriod::default());
            }
            assert!(quiet.extra_height(ctx, &prefs, "") > empty_height);
            let bilibili = SettingsView {
                section: Section::Bilibili,
                ..Default::default()
            };
            assert!(
                bilibili.extra_height(ctx, &prefs, &"connection unavailable ".repeat(24))
                    > bilibili.extra_height(ctx, &prefs, "ready")
            );
        });
    }
}
