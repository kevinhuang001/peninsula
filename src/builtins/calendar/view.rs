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
    let label = holiday.first().cloned().or_else(|| {
        locale.lunar_enabled().then(|| holiday::chinese_lunar_label(day)).flatten()
    });
    let subtitle = text(
        label.unwrap_or_default(),
        9.0,
        if !holiday.is_empty() { ColorSpec::Warning } else { ColorSpec::SecondaryText },
        !holiday.is_empty(),
    );
    let mut cell = column(vec![
        text(day.day().to_string(), 15.0,
            if selected { ColorSpec::OnAccent } else if today { ColorSpec::Accent } else { ColorSpec::PrimaryText }, true),
        subtitle,
    ], 3.0);
    cell.style.padding = [6.0, 5.0, 5.0, 5.0];
    cell.style.normal.background = Some(if selected { ColorSpec::Accent } else if today { ColorSpec::AccentSubtle } else { ColorSpec::Surface });
    cell.style.normal.corner_radius = Some(10.0);
    cell.style.normal.border_color = Some(if selected || today { ColorSpec::Accent } else { ColorSpec::Border });
    cell.style.normal.border_width = Some(if today || selected { 1.3 } else { 0.4 });
    ViewNode::new(Node::ActionArea {
        action: Action::Custom(format!("calendar:select:{}", day.format("%Y-%m-%d"))),
        label: format!("{} {}", day, holiday.join("、")),
        content: Box::new(cell.sized(Size::Fill(1.0), Size::Fixed(57.0))),
        context_menu: None,
    }).sized(Size::Fill(1.0), Size::Fixed(57.0))
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

    let festivals = holiday::festivals(selected, locale);
    let day_info = if festivals.is_empty() {
        holiday::chinese_lunar_label(selected).filter(|_| locale.lunar_enabled())
            .unwrap_or_else(|| "普通日期".into())
    } else {
        festivals.join(" · ")
    };
    let mut detail = column(vec![
        text(format!("{}  ·  {}", selected.format("%m 月 %d 日"), day_info),
            13.0, ColorSpec::PrimaryText, true),
        text("节日当天显示节日名称；没有节日时才显示农历", 10.0, ColorSpec::SecondaryText, false),
    ], 5.0);
    detail.style.normal.background = Some(ColorSpec::SurfaceVariant);
    detail.style.normal.corner_radius = Some(12.0);
    detail.style.padding = [12.0, 12.0, 12.0, 12.0];

    let mut root = column(vec![header, weekday_grid, calendar_grid, detail], 10.0);
    root.style.padding = [16.0, 16.0, 16.0, 16.0];
    let mut widget = WidgetView::new(root, [550.0, 550.0]);
    widget.bounds = WidgetBounds {
        min: [420.0, 460.0],
        max: [Some(800.0), Some(780.0)],
        resizable: true,
        fit_height: true,
        overflow: Overflow::Scroll(ScrollAxes::Vertical),
        ..Default::default()
    };
    widget
}
