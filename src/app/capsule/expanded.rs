use crate::app::icons::{Icon, Icons};
use crate::extension::ExtensionId;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Vec2};

pub(super) struct HeaderContent<'a> {
    pub title: &'a str,
    pub pinned: bool,
    pub do_not_disturb: bool,
    pub settings_open: bool,
    pub icons: &'a Icons,
    pub opacity: f32,
}

pub(super) fn paint_header(
    ui: &mut egui::Ui,
    rect: Rect,
    content: HeaderContent<'_>,
) -> (bool, bool, bool, bool) {
    let HeaderContent {
        title,
        pinned,
        do_not_disturb,
        settings_open,
        icons,
        opacity,
    } = content;
    let text = if settings_open {
        Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    let button_size = Vec2::splat(24.0);
    let dashboard_rect = header_button(rect, 105.0, button_size);
    let title_bounds = Rect::from_min_max(
        Pos2::new(rect.left() + 15.0, rect.top()),
        Pos2::new(dashboard_rect.left() - 8.0, rect.top() + 46.0),
    );
    if title_bounds.is_positive() {
        let painter = ui.painter().with_clip_rect(title_bounds);
        let galley = painter.layout_no_wrap(title.to_owned(), FontId::proportional(15.0), text);
        let center = rect
            .center()
            .x
            .min(title_bounds.right() - galley.size().x * 0.5);
        painter.galley(
            Pos2::new(
                center - galley.size().x * 0.5,
                rect.top() + 27.0 - galley.size().y * 0.5,
            ),
            galley,
            text.linear_multiply(opacity),
        );
    }
    let dashboard_requested = header_control(
        ui,
        dashboard_rect,
        icons,
        Icon::Dashboard,
        false,
        text,
        opacity,
    )
    .on_hover_text("打开仪表盘")
    .clicked();
    let pin_toggled = header_control(
        ui,
        header_button(rect, 76.0, button_size),
        icons,
        Icon::Pin,
        pinned,
        text,
        opacity,
    )
    .on_hover_text(if pinned {
        "取消固定"
    } else {
        "固定展开"
    })
    .clicked();
    let quiet_toggled = header_control(
        ui,
        header_button(rect, 47.0, button_size),
        icons,
        Icon::Moon,
        do_not_disturb,
        text,
        opacity,
    )
    .on_hover_text(if do_not_disturb {
        "关闭免打扰"
    } else {
        "一直开启免打扰"
    })
    .clicked();
    let settings = header_control(
        ui,
        header_button(rect, 19.0, button_size),
        icons,
        if settings_open {
            Icon::Close
        } else {
            Icon::Settings
        },
        settings_open,
        text,
        opacity,
    )
    .on_hover_text(if settings_open {
        "关闭设置"
    } else {
        "设置"
    })
    .clicked();
    (pin_toggled, quiet_toggled, settings, dashboard_requested)
}

fn header_control(
    ui: &mut egui::Ui,
    rect: Rect,
    icons: &Icons,
    icon: Icon,
    selected: bool,
    text: Color32,
    opacity: f32,
) -> egui::Response {
    let response = ui.put(rect, egui::Button::new("").frame(false));
    let interactive = response.hovered() || response.has_focus();
    if selected || interactive {
        let tint = ui.visuals().selection.bg_fill;
        let strength = if selected || response.is_pointer_button_down_on() {
            1.0
        } else {
            0.45
        };
        ui.painter()
            .rect_filled(rect, 7.0, tint.gamma_multiply(strength * opacity));
    }
    let color = if selected {
        ui.visuals().selection.stroke.color
    } else {
        text
    };
    icons.paint(ui.painter(), icon, rect.shrink(5.0), color, opacity);
    response
}

fn header_button(rect: Rect, from_right: f32, size: Vec2) -> Rect {
    Rect::from_center_size(
        Pos2::new(rect.right() - from_right, rect.top() + 26.0),
        size,
    )
}

pub(in crate::app) const NAVIGATION_HEIGHT: f32 = 22.0;
fn navigation_font(screen_width: f32) -> f32 {
    let screen_width = if screen_width.is_finite() {
        screen_width.max(1.0)
    } else {
        1440.0
    };
    10.0 * (screen_width / 1440.0).sqrt().clamp(0.9, 1.2)
}
const TAB_PADDING: f32 = 3.0;

pub(in crate::app) fn navigation_rect(rect: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(rect.left() + 16.0, rect.bottom() - NAVIGATION_HEIGHT),
        Pos2::new(rect.right() - 16.0, rect.bottom() - 2.0),
    )
}

/// Title tabs use the same runtime-visible widget list as swipe/keyboard paging.
/// Every title has a short line; only the active widget's line is bright.
pub(super) fn paint_tabs(
    ui: &mut egui::Ui,
    rect: Rect,
    active_widget: &mut ExtensionId,
    widget_ids: &[ExtensionId],
    titles: &[String],
    opacity: f32,
) {
    let font_size = navigation_font(ui.input(|input| {
        input
            .viewport()
            .monitor_size
            .unwrap_or(input.screen_rect.size())
            .x
    }));
    let bounds = navigation_rect(rect);
    if !bounds.is_positive() {
        return;
    }
    let primary = ui.visuals().text_color().linear_multiply(opacity);
    let muted = crate::ui::palette::TextPalette::for_mode(ui.visuals().dark_mode)
        .muted
        .linear_multiply(opacity);
    let active_key = ui.id().with("navigation-active-widget");
    let changed = ui.data_mut(|data| {
        let previous = data.get_temp::<ExtensionId>(active_key);
        data.insert_temp(active_key, active_widget.clone());
        previous.as_ref() != Some(active_widget)
    });
    let mut surface = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("widget-title-navigation")
            .max_rect(bounds),
    );
    surface.set_clip_rect(ui.clip_rect().intersect(bounds));
    egui::ScrollArea::horizontal()
        .id_salt("title-tabs")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, false])
        .show(&mut surface, |ui| {
            let entries: Vec<_> = widget_ids
                .iter()
                .enumerate()
                .map(|(index, id)| (id, titles.get(index).map(String::as_str).unwrap_or("扩展")))
                .collect();
            let galleys: Vec<_> = entries
                .iter()
                .map(|(_, title)| {
                    ui.painter().layout_no_wrap(
                        (*title).to_owned(),
                        FontId::proportional(font_size),
                        Color32::PLACEHOLDER,
                    )
                })
                .collect();
            let required: f32 = galleys
                .iter()
                .map(|galley| galley.size().x + 2.0 * TAB_PADDING)
                .sum();
            let total = bounds.width().max(required);
            let (row, _) =
                ui.allocate_exact_size(Vec2::new(total, bounds.height()), egui::Sense::hover());
            let mut x = row.left() + (total - required) * 0.5;
            for ((id, title), galley) in entries.into_iter().zip(galleys) {
                let width = galley.size().x + 2.0 * TAB_PADDING;
                let tab = Rect::from_min_size(
                    Pos2::new(x, row.top() + 1.0),
                    Vec2::new(width, row.height() - 1.0),
                );
                let selected = id == active_widget;
                let response = ui
                    .interact(tab, ui.id().with(("title-tab", id)), egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        selected,
                        title,
                    )
                });
                let color = if response.hovered() || response.has_focus() {
                    muted
                } else {
                    muted.gamma_multiply(0.72)
                };
                let at = Pos2::new(
                    tab.center().x - galley.size().x * 0.5,
                    tab.bottom() - galley.size().y - 1.0,
                );
                ui.painter().rect_filled(
                    Rect::from_center_size(
                        Pos2::new(tab.center().x, at.y - 1.0),
                        Vec2::new(14.0, 1.5),
                    ),
                    1.0,
                    if selected {
                        primary
                    } else {
                        muted.gamma_multiply(0.32)
                    },
                );
                ui.painter().galley(at, galley, color);
                if selected && changed {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    *active_widget = id.clone();
                    ui.ctx().request_repaint();
                }
                x += width;
            }
        });
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    use egui::{vec2, Event, Modifiers, PointerButton, RawInput};

    fn draw(ctx: &egui::Context, active: &mut ExtensionId, events: Vec<Event>) -> egui::FullOutput {
        let ids = [
            ExtensionId::builtin("test/notifications"),
            ExtensionId::builtin("test/performance"),
        ];
        let titles = ["通知中心".to_owned(), "性能监控".to_owned()];
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(440.0, 240.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    paint_tabs(
                        ui,
                        Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0)),
                        active,
                        &ids,
                        &titles,
                        1.0,
                    );
                });
            },
        )
    }

    fn text_center(output: &egui::FullOutput, value: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == value => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("navigation title must be rendered")
    }

    fn indicator(output: &egui::FullOutput) -> Rect {
        let marks: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.width() == 14.0 && rect.rect.height() == 1.5 =>
                {
                    Some((rect.rect, rect.fill))
                }
                _ => None,
            })
            .collect();
        assert_eq!(marks.len(), 2, "every widget title has its own short line");
        let brightness =
            |color: Color32| u16::from(color.r()) + u16::from(color.g()) + u16::from(color.b());
        let selected = marks
            .iter()
            .max_by_key(|(_, color)| brightness(*color))
            .unwrap();
        assert_eq!(
            marks
                .iter()
                .filter(|(_, color)| *color == selected.1)
                .count(),
            1,
            "only the current widget's line is bright"
        );
        selected.0
    }

    #[test]
    fn title_click_changes_widget_and_brightens_its_short_line() {
        let ctx = egui::Context::default();
        crate::app::fonts::install(&ctx);
        let mut active = ExtensionId::builtin("test/performance");
        let initial = draw(&ctx, &mut active, vec![]);
        let mark = indicator(&initial);
        let selected = text_center(&initial, "性能监控");
        assert!((mark.center().x - selected.x).abs() < 1.0);
        assert!(mark.bottom() < selected.y);
        let next = text_center(&initial, "通知中心");
        let button = |pressed| Event::PointerButton {
            pos: next,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        draw(
            &ctx,
            &mut active,
            vec![Event::PointerMoved(next), button(true)],
        );
        draw(&ctx, &mut active, vec![button(false)]);
        assert_eq!(active, ExtensionId::builtin("test/notifications"));
        let updated = draw(&ctx, &mut active, vec![]);
        assert!((indicator(&updated).center().x - text_center(&updated, "通知中心").x).abs() < 1.0);
        assert!(indicator(&updated).center().x < mark.center().x);
    }

    #[test]
    fn navigation_contains_only_widget_titles_without_an_all_entry() {
        let ctx = egui::Context::default();
        crate::app::fonts::install(&ctx);
        let mut active = ExtensionId::builtin("test/performance");
        let output = draw(&ctx, &mut active, vec![]);
        let titles: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(titles, ["通知中心", "性能监控"]);
        let mark = indicator(&output);
        assert!(mark.bottom() < text_center(&output, "性能监控").y);
    }
}

#[cfg(test)]
mod sizing_tests {
    use super::navigation_font;
    #[test]
    fn navigation_text_scales_gently_with_monitor_resolution() {
        assert_eq!(navigation_font(1440.0), 10.0);
        assert!(navigation_font(1920.0) > navigation_font(1440.0));
        assert_eq!(navigation_font(3840.0), 12.0);
        assert_eq!(navigation_font(800.0), 9.0);
    }
}
