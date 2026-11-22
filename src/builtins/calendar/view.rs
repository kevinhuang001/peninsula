use chrono::{Datelike, NaiveDate};
use crate::{extension::{Action, WidgetView}, ui::declarative::*};
use super::{holiday, model::{self, CalendarSettings}, CalendarExtension};

fn text(value: impl Into<String>, size: f32, color: ColorSpec, bold: bool) -> ViewNode {
    ViewNode::text(value, TextStyle {
        font_size: FontSize::Custom(size),
        font_weight: if bold { FontWeight::Bold } else { FontWeight::Normal },
        wrap: false,
        ..Default::default()
    }).color(color)
}

fn wrapped_festival(name: String, color: ColorSpec) -> ViewNode {
    // The renderer measures this text at the actual grid column width.
    // No max_lines: the row grows with the whole holiday name.
    ViewNode::text(name, TextStyle {
        font_size: FontSize::Custom(9.0),
        font_weight: FontWeight::Bold,
        wrap: true,
        max_lines: None,
        ..Default::default()
    })
    .color(color)
    .sized(Size::Fill(1.0), Size::Auto)
}

fn column(children: Vec<ViewNode>, gap: f32) -> ViewNode {
    ViewNode::column(children, Flow { gap: Some(gap), align: Align::Stretch, ..Default::default() })
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

fn day_cell(date: Option<NaiveDate>, today: NaiveDate, chosen: NaiveDate, locale: &CalendarSettings) -> ViewNode {
    let Some(day) = date else {
        return ViewNode::new(Node::Spacer).sized(Size::Fill(1.0), Size::Fixed(57.0));
    };
    let holiday = holiday::festivals(day, locale);
    let selected = day == chosen;
    let today = day == today;
    // A holiday entirely replaces the lunar subtitle. Multiple holidays
    // remain readable in the cell instead of being hidden in a details card.
    let label = if holiday.is_empty() {
        locale.lunar_enabled().then(|| holiday::chinese_lunar_label(day)).flatten()
    } else {
        Some(holiday.join(" · "))
    };
    let subtitle = wrapped_festival(
        label.unwrap_or_default(),
        if selected { ColorSpec::OnAccent }
        else if !holiday.is_empty() { ColorSpec::Warning }
        else { ColorSpec::SecondaryText },
    );
    let mut cell = column(vec![
        text(day.day().to_string(), 15.0,
            if selected { ColorSpec::OnAccent } else if today { ColorSpec::Accent } else { ColorSpec::PrimaryText }, true),
        subtitle,
    ], 3.0);
    cell.style.padding = [6.0, 5.0, 5.0, 5.0];
    cell.size.min_height = Some(57.0);
    cell.style.normal.background = Some(if selected { ColorSpec::Accent } else if today { ColorSpec::AccentSubtle } else { ColorSpec::Surface });
    cell.style.normal.corner_radius = Some(10.0);
    cell.style.normal.border_color = Some(if selected || today { ColorSpec::Accent } else { ColorSpec::Border });
    cell.style.normal.border_width = Some(if today || selected { 1.3 } else { 0.4 });
    ViewNode::new(Node::ActionArea {
        action: Action::Custom(format!("calendar:select:{}", day.format("%Y-%m-%d"))),
        label: format!("{} {}", day, holiday.join("、")),
        content: Box::new(cell),
        context_menu: None,
    }).sized(Size::Fill(1.0), Size::Auto)
}

pub(super) fn build(calendar: &CalendarExtension) -> WidgetView {
    let locale = &calendar.settings;
    let today = locale.today();
    let shown = calendar.month.unwrap_or_else(|| model::start_month(today));
    let selected = calendar.selected.unwrap_or(today);
    let month_label = if locale.country == "CN" {
        format!("{} 年  {} 月", shown.year(), shown.month())
    } else {
        format!("{} / {:02}", shown.year(), shown.month())
    };
    let header = ViewNode::row(vec![
        column(vec![
            text(month_label, 21.0, ColorSpec::PrimaryText, true),
            text(format!("{} · {}", locale.country_label(), locale.timezone().name()),
                10.0, ColorSpec::SecondaryText, false),
        ], 5.0),
        button("‹", "calendar:previous"),
        button("今天", "calendar:today"),
        button("›", "calendar:next"),
    ], Flow { gap: Some(7.0), align: Align::Center, justify: Justify::SpaceBetween, ..Default::default() })
    .sized(Size::Fill(1.0), Size::Fixed(52.0));

    let weekdays = ["一", "二", "三", "四", "五", "六", "日"].into_iter()
        .map(|label| text(label, 11.0, ColorSpec::SecondaryText, true))
        .collect::<Vec<_>>();
    let mut grid = Grid::new(Columns::Tracks(vec![Track::Fraction(1.0); 7]));
    grid.column_gap = Some(5.0);
    grid.row_gap = Some(5.0);
    let weekday_grid = ViewNode::new(Node::Grid(grid.clone(), weekdays))
        .sized(Size::Fill(1.0), Size::Fixed(22.0));
    let cells = model::month_dates(shown).into_iter()
        .map(|day| day_cell(day, today, selected, locale))
        .collect();
    let calendar_grid = ViewNode::new(Node::Grid(grid, cells))
        .sized(Size::Fill(1.0), Size::Auto);

    // Only the seven-column month grid remains: no date-details or
    // explanatory card below it. Auto-height rows preserve wrapped holidays.
    let mut root = column(vec![header, weekday_grid, calendar_grid], 10.0);
    root.style.padding = [16.0, 16.0, 16.0, 16.0];
    let mut widget = WidgetView::new(root, [550.0, 465.0]);
    widget.bounds = WidgetBounds {
        min: [420.0, 390.0],
        max: [Some(900.0), None],
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
    fn a_long_holiday_wraps_in_an_intrinsically_sized_calendar_cell() {
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
        let cell = day_cell(Some(date), date, date, &CalendarSettings::default());
        assert_eq!(cell.size.height, Size::Auto);
        let Node::ActionArea { content, .. } = &cell.kind else {
            panic!("calendar days must be individually selectable");
        };
        assert!(matches!(content.size.height, Size::Auto));
        assert!(content.size.min_height.unwrap_or_default() >= 57.0);
    }

    #[test]
    fn month_widget_contains_no_date_details_or_explanation_card() {
        let widget = build(&CalendarExtension::default());
        let Node::Column(_, children) = &widget.root.kind else {
            panic!("month root must be a column");
        };
        assert_eq!(children.len(), 3); // header, weekdays, calendar only
        assert!(matches!(children[2].kind, Node::Grid(_, _)));
        assert!(widget.bounds.fit_height);
        assert_eq!(widget.bounds.max[1], None);
    }
}
