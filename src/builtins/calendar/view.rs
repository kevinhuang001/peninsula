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

/// Each word is kept intact whenever the display can accommodate the
/// requested calendar width. On a genuinely narrow monitor, the renderer
/// may still wrap instead of clipping the user's holiday text.
const MIN_DAY_COLUMN_WIDTH: f32 = 96.0;
const HOLIDAY_CHAR_WIDTH: f32 = 7.0;
const DAY_PADDING_WIDTH: f32 = 26.0;
const CALENDAR_GAP: f32 = 6.0;
const CALENDAR_SIDE_PADDING: f32 = 24.0;
const CAPSULE_SIDES: f32 = 44.0;

fn preferred_calendar_width(month: NaiveDate, settings: &CalendarSettings) -> f32 {
    let longest_word = model::visible_grid_dates(month)
        .into_iter()
        .filter(|(_, active)| *active)
        .flat_map(|(date, _)| holiday::festivals(date, settings))
        .flat_map(|festival| {
            festival.split_whitespace()
                .map(|word| word.chars().count())
                .collect::<Vec<_>>()
        })
        .max()
        .unwrap_or(0) as f32;
    let col_width = MIN_DAY_COLUMN_WIDTH
        .max(longest_word * HOLIDAY_CHAR_WIDTH + DAY_PADDING_WIDTH);
    (col_width * 7.0 + CALENDAR_GAP * 6.0
        + CALENDAR_SIDE_PADDING + CAPSULE_SIDES).ceil()
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
    .sized(Size::Fill(1.0), Size::Fixed(28.0));
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
    // The whole ActionArea, not just the intrinsic text Column, owns the
    // visual surface. Grid Auto rows size to their tallest festival; the
    // ActionArea is then stretched to the complete row height. Every day in
    // the same week therefore has an equally tall background and hit target.
    let content = column(vec![
        text(
            day.day().to_string(),
            15.0,
            if selected { ColorSpec::OnAccent }
            else if !in_month { ColorSpec::MutedText }
            else if is_today { ColorSpec::Accent }
            else { ColorSpec::PrimaryText },
            true,
        ),
        wrapped_festival(
            subtitle,
            if selected { ColorSpec::OnAccent }
            else if !festivals.is_empty() { ColorSpec::Warning }
            else { ColorSpec::SecondaryText },
        ),
    ], 4.0);

    let mut cell = ViewNode::new(Node::ActionArea {
        action: Action::Custom(format!("calendar:select:{}", day.format("%Y-%m-%d"))),
        label: format!("{} {}", day, festivals.join("、")),
        content: Box::new(content),
        context_menu: None,
    })
    .sized(Size::Fill(1.0), Size::Auto);
    cell.style.padding = [5.0, 9.0, 5.0, 8.0];
    cell.size.min_height = Some(54.0);
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
    .sized(Size::Fill(1.0), Size::Fixed(46.0));

    let mut grid = Grid::new(Columns::Tracks(vec![Track::Fraction(1.0); 7]));
    grid.column_gap = Some(6.0);
    grid.row_gap = Some(6.0);
    let weekdays = ["一", "二", "三", "四", "五", "六", "日"]
        .into_iter().enumerate()
        .map(|(index, label)| weekday_cell(label, index >= 5))
        .collect::<Vec<_>>();
    let weekday_grid = ViewNode::new(Node::Grid(grid.clone(), weekdays))
        .sized(Size::Fill(1.0), Size::Fixed(28.0));

    let cells = model::visible_grid_dates(shown)
        .into_iter()
        .map(|(day, in_month)| day_cell(day, today, selected, locale, in_month))
        .collect::<Vec<_>>();
    // All columns share the same 1fr track width. Each calendar week has
    // one shared Auto track: a multi-line holiday expands the ENTIRE row.
    grid.rows = vec![Track::Auto; cells.len() / 7];
    let calendar_grid = ViewNode::new(Node::Grid(grid, cells))
        .sized(Size::Fill(1.0), Size::Auto);

    let mut root = column(vec![header, weekday_grid, calendar_grid], 9.0);
    root.style.padding = [12.0, 12.0, 12.0, 12.0];
    let requested_width = preferred_calendar_width(shown, locale);
    let mut widget = WidgetView::new(root, [requested_width, 450.0]);
    widget.bounds = WidgetBounds {
        min: [420.0, 380.0],
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
    fn a_long_english_festival_increases_month_width() {
        let mut settings = CalendarSettings::default();
        settings.country = "US".into();
        settings.region = "US-NY".into();
        let nov = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let width = preferred_calendar_width(nov, &settings);
        assert!(width >= 830.0, "Thanksgiving must fit without splitting its word");
        let mut ext = CalendarExtension::default();
        ext.settings = settings;
        ext.month = Some(nov);
        let view = build(&ext);
        assert_eq!(view.preferred_size[0], width);
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
        assert!(cell.size.min_height.unwrap_or_default() >= 54.0);
        assert!(cell.style.normal.background.is_some());
    }

    #[test]
    fn widget_has_compact_navigation_and_complete_month_grid_only() {
        let widget = build(&CalendarExtension::default());
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
        assert_eq!(columns, &vec![Track::Fraction(1.0); 7]);
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
        let Node::Text { text_style, .. } = &children[1].kind else {
            panic!("subtitle is a text node");
        };
        assert_eq!(text_style.max_lines, None);
    }
}
