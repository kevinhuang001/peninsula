use chrono::{Datelike, NaiveDate};
use crate::{
    extension::{Action, WidgetView},
    ui::declarative::*,
};
use super::{
    holiday,
    model::{self, CalendarSettings},
    CalendarExtension,
};

fn text(value: impl Into<String>, size: f32, color: ColorSpec, bold: bool) -> ViewNode {
    ViewNode::text(value, TextStyle {
        font_size: FontSize::Custom(size),
        font_weight: if bold { FontWeight::Bold } else { FontWeight::Normal },
        align: Align::Center,
        wrap: false,
        ..Default::default()
    })
    .color(color)
}

fn festival_lines(names: &[String]) -> String {
    // Keep English words intact where a date cell can fit them; the renderer
    // still wraps an individual word if the window is especially narrow.
    names.iter()
        .map(|name| {
            if name.chars().any(|ch| ch.is_ascii_alphabetic()) {
                name.split_whitespace().collect::<Vec<_>>().join("\n")
            } else {
                name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Date columns size independently: the longest word in a Thursday
/// holiday must not make Monday, Tuesday, etc. wider as well.
/// The seven tracks and their preferred overall width use the SAME measurements.
const MIN_DAY_COLUMN_WIDTH: f32 = 54.0;
const LATIN_CHAR_WIDTH: f32 = 7.5;
const WIDE_CHAR_WIDTH: f32 = 10.5;
const LABEL_HORIZONTAL_PADDING: f32 = 12.0;
const CALENDAR_GAP: f32 = 6.0;
const CALENDAR_SIDE_PADDING: f32 = 20.0;
const CAPSULE_SIDES: f32 = 44.0;

fn estimated_label_width(word: &str) -> f32 {
    word.chars()
        .map(|ch| if ch.is_ascii() { LATIN_CHAR_WIDTH } else { WIDE_CHAR_WIDTH })
        .sum()
}

/// One width for each weekday column, based on the longest label that
/// actually appears in THAT column during the displayed month.
fn calendar_column_widths(month: NaiveDate, settings: &CalendarSettings) -> [f32; 7] {
    let mut columns = [MIN_DAY_COLUMN_WIDTH; 7];
    for (slot, (day, in_month)) in model::visible_grid_dates(month).into_iter().enumerate() {
        if !in_month {
            continue;
        }
        let festivals = holiday::festivals(day, settings);
        let required = if festivals.is_empty() {
            if settings.lunar_enabled() {
                holiday::chinese_lunar_label(day)
                    .map(|label| estimated_label_width(&label))
                    .unwrap_or(0.0)
            } else {
                0.0
            }
        } else {
            festivals.iter()
                .flat_map(|name| name.split_whitespace())
                .map(estimated_label_width)
                .fold(0.0_f32, f32::max)
        };
        columns[slot % 7] = columns[slot % 7]
            .max(required + LABEL_HORIZONTAL_PADDING);
    }
    columns.map(f32::ceil)
}

fn calendar_content_width(columns: &[f32; 7]) -> f32 {
    columns.iter().sum::<f32>()
        + CALENDAR_GAP * 6.0
        + CALENDAR_SIDE_PADDING
        + CAPSULE_SIDES
}

fn preferred_calendar_width(month: NaiveDate, settings: &CalendarSettings) -> f32 {
    calendar_content_width(&calendar_column_widths(month, settings)).ceil()
}

fn wrapped_festival(name: String, color: ColorSpec) -> ViewNode {
    ViewNode::text(name, TextStyle {
        font_size: FontSize::Custom(9.0),
        font_weight: FontWeight::Bold,
        align: Align::Center,
        line_height: Some(12.5),
        wrap: true,
        max_lines: None,
        ..Default::default()
    })
    .color(color)
    .sized(Size::Fill(1.0), Size::Auto)
}

fn column(children: Vec<ViewNode>, gap: f32) -> ViewNode {
    ViewNode::column(children, Flow {
        gap: Some(gap),
        align: Align::Stretch,
        ..Default::default()
    })
    .sized(Size::Fill(1.0), Size::Auto)
}

fn button(label: &str, action: &str) -> ViewNode {
    let mut node = ViewNode::subtle_button(label, Action::Custom(action.into()))
        .sized(Size::Fixed(if label == "今天" { 48.0 } else { 30.0 }), Size::Fixed(30.0));
    node.style.normal.background = Some(ColorSpec::SurfaceVariant);
    node.style.normal.border_color = Some(ColorSpec::Border);
    node.style.normal.border_width = Some(1.0);
    node.style.normal.corner_radius = Some(9.0);
    node
}

fn weekday_cell(label: &str, weekend: bool) -> ViewNode {
    let mut node = text(
        label,
        10.5,
        if weekend { ColorSpec::Accent } else { ColorSpec::SecondaryText },
        true,
    )
    .sized(Size::Fill(1.0), Size::Fixed(24.0));
    node.style.normal.background = Some(ColorSpec::SurfaceVariant);
    node.style.normal.corner_radius = Some(8.0);
    node.style.padding = [2.0, 5.0, 2.0, 5.0];
    node
}

fn day_cell(
    day: NaiveDate,
    today: NaiveDate,
    chosen: NaiveDate,
    locale: &CalendarSettings,
    in_month: bool,
) -> ViewNode {
    let festivals = if in_month { holiday::festivals(day, locale) } else { vec![] };
    let selected = day == chosen && in_month;
    let is_today = day == today;
    // A holiday fully replaces the lunar subtitle. Outside-month dates are
    // intentionally quiet so the current month remains the focal point.
    let subtitle = if !festivals.is_empty() {
        festival_lines(&festivals)
    } else if in_month && locale.lunar_enabled() {
        holiday::chinese_lunar_label(day).unwrap_or_default()
    } else {
        String::new()
    };
    // Omit empty subtitle text: even an empty Text node consumes one line
    // box, making every ordinary day unnecessarily tall.
    let mut lines = vec![text(
        day.day().to_string(),
        14.0,
        if selected { ColorSpec::OnAccent }
        else if !in_month { ColorSpec::MutedText }
        else if is_today { ColorSpec::Accent }
        else { ColorSpec::PrimaryText },
        true,
    )];
    if !subtitle.is_empty() {
        lines.push(wrapped_festival(
            subtitle,
            if selected { ColorSpec::OnAccent }
            else if !festivals.is_empty() { ColorSpec::Warning }
            else { ColorSpec::SecondaryText },
        ));
    }
    // The ActionArea itself owns the background and fills the tallest row.
    let content = column(lines, 3.0);

    let mut cell = ViewNode::new(Node::ActionArea {
        action: Action::Custom(format!("calendar:select:{}", day.format("%Y-%m-%d"))),
        label: format!("{} {}", day, festivals.join("、")),
        content: Box::new(content),
        context_menu: None,
    })
    .sized(Size::Fill(1.0), Size::Auto);
    cell.style.padding = [5.0, 6.0, 5.0, 5.0];
    cell.size.min_height = Some(34.0);
    cell.style.normal.background = Some(
        if selected { ColorSpec::Accent }
        else if is_today { ColorSpec::AccentSubtle }
        else if in_month { ColorSpec::Surface }
        else { ColorSpec::SurfaceVariant },
    );
    cell.style.normal.corner_radius = Some(10.0);
    cell.style.normal.border_color = Some(
        if selected || is_today { ColorSpec::Accent } else { ColorSpec::Border },
    );
    cell.style.normal.border_width = Some(if selected || is_today { 1.35 } else { 0.6 });
    cell.style.hovered.border_color = Some(ColorSpec::Accent);
    cell.style.hovered.border_width = Some(1.0);
    cell
}

pub(super) fn build(calendar: &CalendarExtension) -> WidgetView {
    let locale = &calendar.settings;
    let today = locale.today();
    let shown = calendar.month.unwrap_or_else(|| model::start_month(today));
    let selected = calendar.selected.unwrap_or(today);
    let month_label = if locale.country == "CN" {
        format!("{} 年 {} 月", shown.year(), shown.month())
    } else {
        format!("{} / {:02}", shown.year(), shown.month())
    };

    // Keep navigation as one compact action group on the right. The flexible
    // spacer takes the available width, rather than spreading all 4 elements.
    let controls = ViewNode::row(vec![
        button("‹", "calendar:previous"),
        button("今天", "calendar:today"),
        button("›", "calendar:next"),
    ], Flow { gap: Some(5.0), align: Align::Center, ..Default::default() });
    let spacer = ViewNode::new(Node::Spacer)
        .sized(Size::Fill(1.0), Size::Fixed(1.0));
    let header = ViewNode::row(vec![
        text(month_label, 21.0, ColorSpec::PrimaryText, true),
        spacer,
        controls,
    ], Flow {
        gap: Some(8.0),
        align: Align::Center,
        justify: Justify::Start,
        ..Default::default()
    })
    .sized(Size::Fill(1.0), Size::Fixed(40.0));

    // Both the weekday header and day grid share the very same measured
    // column tracks. A long Thursday label widens ONLY Thursday's column.
    let column_widths = calendar_column_widths(shown, locale);
    let mut grid = Grid::new(Columns::Tracks(
        column_widths.into_iter().map(Track::Fixed).collect()
    ));
    grid.column_gap = Some(CALENDAR_GAP);
    grid.row_gap = Some(4.0);
    let weekdays = ["一", "二", "三", "四", "五", "六", "日"]
        .into_iter().enumerate()
        .map(|(index, label)| weekday_cell(label, index >= 5))
        .collect::<Vec<_>>();
    let weekday_grid = ViewNode::new(Node::Grid(grid.clone(), weekdays))
        .sized(Size::Fill(1.0), Size::Fixed(24.0));

    let cells = model::visible_grid_dates(shown)
        .into_iter()
        .map(|(day, in_month)| day_cell(day, today, selected, locale, in_month))
        .collect::<Vec<_>>();
    // Each column has its own content-based Fixed width; every day in that
    // column shares it. Each week uses a separate Auto height determined by
    // its tallest day. No other week is enlarged by a multi-line festival.
    grid.rows = vec![Track::Auto; cells.len() / 7];
    let calendar_grid = ViewNode::new(Node::Grid(grid, cells))
        .sized(Size::Fill(1.0), Size::Auto);

    let mut root = column(vec![header, weekday_grid, calendar_grid], 7.0);
    root.style.padding = [10.0, 10.0, 10.0, 10.0];
    let requested_width = calendar_content_width(&column_widths).ceil();
    let mut widget = WidgetView::new(root, [requested_width, 320.0]);
    widget.bounds = WidgetBounds {
        min: [420.0, 280.0],
        max: [Some(1050.0), None],
        resizable: true,
        fit_height: true,
        overflow: Overflow::Scroll(ScrollAxes::Vertical),
        ..Default::default()
    };
    widget
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_holiday_labels_break_between_english_words() {
        assert_eq!(
            festival_lines(&["Thanksgiving Day".into()]),
            "Thanksgiving\nDay"
        );
        assert_eq!(
            festival_lines(&["Veterans Day".into(), "Observance".into()]),
            "Veterans\nDay\nObservance"
        );
        assert_eq!(festival_lines(&["春节".into()]), "春节");
    }

    #[test]
    fn holiday_only_widens_its_own_weekday_column() {
        let locale = CalendarSettings {
            country: "US".into(),
            region: "US-NY".into(),
            show_lunar: false,
        };
        let november = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let columns = calendar_column_widths(november, &locale);
        // Thanksgiving is a Thursday; Veterans Day is a Wednesday.
        assert!(columns[3] > columns[0], "Thursday must fit Thanksgiving");
        assert!(columns[2] > columns[0], "Wednesday must fit Veterans Day");
        assert!(columns[3] >= estimated_label_width("Thanksgiving") + LABEL_HORIZONTAL_PADDING);
        assert!(columns.iter().any(|&column| column == MIN_DAY_COLUMN_WIDTH));
        let width = preferred_calendar_width(november, &locale);
        assert!(width > 420.0);
        assert!(width < 650.0, "only the columns that need space should grow");

        let mut ext = CalendarExtension::default();
        ext.settings = locale;
        ext.month = Some(november);
        let view = build(&ext);
        assert_eq!(view.preferred_size[0], width);
        let Node::Column(_, content) = &view.root.kind else {
            panic!("calendar widget must be a column");
        };
        for index in [1, 2] {
            let Node::Grid(grid, _) = &content[index].kind else {
                panic!("weekday labels and dates must both use the same grid");
            };
            assert_eq!(grid.columns, Columns::Tracks(
                columns.into_iter().map(Track::Fixed).collect()
            ));
        }
        assert!(view.bounds.resizable);
    }

    #[test]
    fn long_holiday_wraps_and_expands_its_date_cell() {
        let subtitle = wrapped_festival(
            "International Very Long Public Holiday".into(),
            ColorSpec::Warning,
        );
        assert_eq!(subtitle.size.width, Size::Fill(1.0));
        assert_eq!(subtitle.size.height, Size::Auto);
        let Node::Text { text_style, .. } = &subtitle.kind else {
            panic!("festival subtitle must remain a text node");
        };
        assert!(text_style.wrap);
        assert_eq!(text_style.max_lines, None);
        let date = NaiveDate::from_ymd_opt(2026, 2, 17).unwrap();
        let cell = day_cell(date, date, date, &CalendarSettings::default(), true);
        assert_eq!(cell.size.height, Size::Auto);
        let Node::ActionArea { content, .. } = &cell.kind else {
            panic!("calendar days must remain individually selectable");
        };
        assert!(matches!(content.size.height, Size::Auto));
        assert_eq!(cell.size.width, Size::Fill(1.0));
        assert_eq!(cell.size.height, Size::Auto);
        assert_eq!(cell.size.min_height, Some(34.0));
        assert!(cell.style.normal.background.is_some());
    }

    #[test]
    fn regular_dates_skip_empty_subtitle_and_holidays_keep_dynamic_height() {
        let locale = CalendarSettings {
            country: "US".into(),
            region: "US-NY".into(),
            show_lunar: false,
        };
        let regular = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let ordinary = day_cell(regular, regular, regular, &locale, true);
        let Node::ActionArea { content, .. } = &ordinary.kind else {
            panic!("normal day should be an ActionArea");
        };
        let Node::Column(_, lines) = &content.kind else {
            panic!("normal day should hold a column");
        };
        assert_eq!(lines.len(), 1, "blank subtitles must not reserve height");
        assert_eq!(ordinary.size.min_height, Some(34.0));

        let holiday = NaiveDate::from_ymd_opt(2026, 11, 26).unwrap();
        let festival = day_cell(holiday, regular, regular, &locale, true);
        let Node::ActionArea { content, .. } = &festival.kind else {
            panic!("holiday should be an ActionArea");
        };
        let Node::Column(_, lines) = &content.kind else {
            panic!("holiday should hold a column");
        };
        assert_eq!(lines.len(), 2, "holiday subtitle remains visible");
        assert_eq!(festival.size.height, Size::Auto);
    }

    #[test]
    fn widget_has_compact_navigation_and_complete_month_grid_only() {
        let extension = CalendarExtension::default();
        let widget = build(&extension);
        let Node::Column(_, children) = &widget.root.kind else {
            panic!("month root must be a column");
        };
        assert_eq!(children.len(), 3); // header, weekdays, calendar only
        let Node::Row(_, header) = &children[0].kind else {
            panic!("calendar header is a row");
        };
        assert_eq!(header.len(), 3); // month, spacer, grouped navigation
        let Node::Row(_, actions) = &header[2].kind else {
            panic!("calendar navigation stays together");
        };
        assert_eq!(actions.len(), 3);
        let Node::Grid(grid, cells) = &children[2].kind else {
            panic!("the month is a grid");
        };
        let Columns::Tracks(columns) = &grid.columns else {
            panic!("calendar needs seven shared columns");
        };
        let month = extension.month.unwrap_or_else(|| {
            model::start_month(extension.settings.today())
        });
        let desired = calendar_column_widths(month, &extension.settings);
        assert_eq!(columns, &desired.into_iter().map(Track::Fixed).collect::<Vec<_>>());
        assert_eq!(columns.len(), 7);
        assert_eq!(grid.rows, vec![Track::Auto; cells.len() / 7]);
        for week in cells.chunks(7) {
            assert_eq!(week.len(), 7);
            for cell in week {
                assert_eq!(cell.size.width, Size::Fill(1.0));
                assert_eq!(cell.size.height, Size::Auto);
                assert!(cell.style.normal.background.is_some());
            }
        }
        assert!(widget.bounds.fit_height);
        assert_eq!(widget.bounds.min[1], 280.0);
        assert_eq!(widget.bounds.max[1], None);
    }

    #[test]
    fn adjacent_month_days_are_dimmed_but_selectable() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        let cell = day_cell(date, date, date, &CalendarSettings::default(), false);
        let Node::ActionArea { action, content, .. } = &cell.kind else {
            panic!("adjacent month day needs an action");
        };
        assert_eq!(action, &Action::Custom("calendar:select:2026-10-26".into()));
        let Node::Column(_, children) = &content.kind else {
            panic!("date cell content is a column");
        };
        // Adjacent-month days have no subtitle, so there is no empty line box.
        assert_eq!(children.len(), 1);
        assert_eq!(cell.size.min_height, Some(34.0));
    }
}
