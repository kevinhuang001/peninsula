use super::{Asset, MarketExtension, MarketQuote};
use crate::{
    extension::Action,
    ui::declarative::{
        Align, ColorSpec, Flow, FontSize, FontWeight, ImageFit, Node, Paint, Size, Style, Table,
        TableColumn, TextStyle, ViewNode,
    },
};

const ADD_KEY: &str = "market-symbol";
const ADD_ACTION: &str = "market:add";
const REMOVE_ACTION: &str = "market:remove:";

pub(super) fn build(market: &MarketExtension) -> crate::extension::WidgetView {
    let title = ViewNode::text(
        "市场行情",
        TextStyle {
            font_size: FontSize::Custom(15.0),
            font_weight: FontWeight::Bold,
            align: Align::Start,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    let count = ViewNode::new(Node::Badge(format!(
        "{} 项自选",
        market.watchlist.assets.len()
    )))
    .sized(Size::Auto, Size::Fixed(22.0))
    .color(ColorSpec::Info);
    let refresh = ViewNode::subtle_button(
        if market.refreshing {
            "更新中…"
        } else {
            "刷新"
        },
        Action::Advance,
    )
    .sized(Size::Auto, Size::Fixed(26.0));
    let heading = ViewNode::row(
        vec![
            ViewNode::icon("chart", 17.0).color(ColorSpec::Info),
            title,
            count,
            refresh,
        ],
        Flow {
            gap: Some(8.0),
            align: Align::Center,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fixed(30.0));

    let status_text = if market.refreshing {
        "正在同步最新报价…".to_owned()
    } else if let Some(time) = &market.last_updated {
        format!("最新报价 · 更新于 {time} · 每 30 秒刷新")
    } else {
        "股票与加密资产 · 获取最新报价中".into()
    };
    let status = ViewNode::text(
        status_text,
        TextStyle {
            font_size: FontSize::Small,
            align: Align::Start,
            wrap: false,
            ..Default::default()
        },
    )
    .color(ColorSpec::SecondaryText)
    .sized(Size::Fill(1.0), Size::Fixed(18.0));

    let mut input = ViewNode::new(Node::TextInput {
        key: ADD_KEY.into(),
        value: market.draft.clone(),
        hint: "代码，如 AAPL、BTC-USD 或 0700.HK".into(),
    })
    .sized(Size::Fill(1.0), Size::Fixed(32.0));
    input.key = Some("market-add-input".into());
    input.style.normal.background = Some(ColorSpec::SurfaceVariant);
    input.style.normal.border_color = Some(ColorSpec::Border);
    input.style.normal.border_width = Some(1.0);
    input.style.normal.corner_radius = Some(9.0);
    input.style.padding = [4.0; 4];
    let add = ViewNode::subtle_button("添加", Action::Custom(ADD_ACTION.into()))
        .sized(Size::Fixed(62.0), Size::Fixed(32.0));
    let input_row = ViewNode::row(
        vec![input, add],
        Flow {
            gap: Some(8.0),
            align: Align::Center,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fixed(32.0));

    let mut children = vec![heading, status, input_row];
    if let Some(error) = &market.error {
        children.push(
            ViewNode::text(
                error,
                TextStyle {
                    font_size: FontSize::Small,
                    align: Align::Start,
                    wrap: true,
                    ..Default::default()
                },
            )
            .color(ColorSpec::Warning),
        );
    }
    children.push(table(market));

    let root = ViewNode::column(
        children,
        Flow {
            gap: Some(8.0),
            align: Align::Stretch,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)
    .styled(Style {
        padding: [8.0, 10.0, 8.0, 10.0],
        ..Default::default()
    });
    let mut view = crate::extension::WidgetView::new(root, [560.0, 430.0]);
    view.bounds = crate::ui::declarative::WidgetBounds {
        min: [430.0, 290.0],
        max: [Some(780.0), Some(690.0)],
        resizable: true,
        fit_height: true,
        overflow: crate::ui::declarative::Overflow::Scroll(
            crate::ui::declarative::ScrollAxes::Vertical,
        ),
        ..Default::default()
    };
    view
}

fn table(market: &MarketExtension) -> ViewNode {
    let rows = if market.watchlist.assets.is_empty() {
        vec![vec![
            cell(
                "＋ 添加股票或加密代码",
                ColorSpec::SecondaryText,
                Align::Start,
                false,
            ),
            cell("—", ColorSpec::MutedText, Align::End, true),
            cell("—", ColorSpec::MutedText, Align::End, true),
            cell("", ColorSpec::MutedText, Align::End, false),
        ]]
    } else {
        market
            .watchlist
            .assets
            .iter()
            .map(|asset| quote_row(asset, market.quotes.get(&asset.cache_key())))
            .collect()
    };
    ViewNode::new(Node::Table(Table {
        columns: vec![
            TableColumn::new("资产", 2.3),
            TableColumn::new("最新价", 1.3),
            TableColumn::new("24 小时", 0.9),
            TableColumn::new("", 0.35),
        ],
        rows,
        row_menus: Vec::new(),
    }))
    .sized(Size::Fill(1.0), Size::Auto)
    .styled(Style {
        normal: Paint {
            background: Some(ColorSpec::Surface),
            border_color: Some(ColorSpec::Border),
            border_width: Some(1.0),
            corner_radius: Some(12.0),
            ..Default::default()
        },
        padding: [2.0; 4],
        ..Default::default()
    })
}

fn quote_row(asset: &Asset, quote: Option<&MarketQuote>) -> Vec<ViewNode> {
    let loading = MarketQuote::loading(asset.clone());
    let quote = quote.unwrap_or(&loading);
    let mut logo = quote.logo.as_ref().map_or_else(
        || {
            ViewNode::new(Node::Badge(
                asset.symbol.chars().take(2).collect::<String>(),
            ))
            .sized(Size::Fixed(28.0), Size::Fixed(28.0))
            .styled(Style {
                normal: Paint {
                    foreground: Some(ColorSpec::OnAccent),
                    background: Some(ColorSpec::AccentSubtle),
                    corner_radius: Some(14.0),
                    ..Default::default()
                },
                ..Default::default()
            })
        },
        |image| {
            let mut node = ViewNode::new(Node::RasterImage(image.clone()))
                .sized(Size::Fixed(28.0), Size::Fixed(28.0));
            node.image_fit = Some(ImageFit::Contain);
            node.style.normal.corner_radius = Some(8.0);
            node
        },
    );
    logo.style.normal.background = Some(ColorSpec::SurfaceVariant);
    let detail = quote.error.as_deref().unwrap_or(quote.name.as_str());
    let identity = ViewNode::column(
        vec![
            ViewNode::text(
                &asset.symbol,
                TextStyle {
                    font_size: FontSize::Custom(11.0),
                    font_weight: FontWeight::Bold,
                    align: Align::Start,
                    wrap: false,
                    ..Default::default()
                },
            ),
            ViewNode::text(
                detail,
                TextStyle {
                    font_size: FontSize::Custom(9.0),
                    align: Align::Start,
                    wrap: false,
                    ..Default::default()
                },
            )
            .color(if quote.error.is_some() {
                ColorSpec::Warning
            } else {
                ColorSpec::SecondaryText
            }),
        ],
        Flow {
            gap: Some(2.0),
            align: Align::Start,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    let identity = ViewNode::row(
        vec![logo, identity],
        Flow {
            gap: Some(8.0),
            align: Align::Center,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    let price = quote.price.map_or_else(
        || "—".to_owned(),
        |value| format_price(value, &quote.currency),
    );
    let change = quote
        .change_percent
        .map_or_else(|| "—".to_owned(), |value| format!("{value:+.2}%"));
    let change_color = quote.change_percent.map_or(ColorSpec::MutedText, |value| {
        if value >= 0.0 {
            ColorSpec::Success
        } else {
            ColorSpec::Danger
        }
    });
    let remove = ViewNode::button(
        "×",
        Action::Custom(format!(
            "{REMOVE_ACTION}{}:{}",
            asset.kind.key(),
            asset.symbol
        )),
    )
    .sized(Size::Fixed(24.0), Size::Fixed(26.0))
    .color(ColorSpec::SecondaryText);
    vec![
        identity,
        cell(&price, ColorSpec::PrimaryText, Align::End, true),
        cell(&change, change_color, Align::End, true),
        remove,
    ]
}

fn cell(value: &str, color: ColorSpec, align: Align, monospace: bool) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Custom(10.0),
            align,
            monospace,
            wrap: false,
            ..Default::default()
        },
    )
    .color(color)
}

fn format_price(price: f64, currency: &str) -> String {
    if !price.is_finite() {
        return "—".into();
    }
    let precision = if price >= 0.01 { 2 } else { 6 };
    let amount = grouped_number(price, precision);
    let symbol = match currency {
        "USD" => "$",
        "HKD" => "HK$",
        "CNY" | "JPY" => "¥",
        "EUR" => "€",
        "GBP" => "£",
        _ => return format!("{currency} {amount}"),
    };
    format!("{symbol}{amount}")
}

fn grouped_number(value: f64, precision: usize) -> String {
    let formatted = format!("{value:.precision$}");
    let (integer, fraction) = formatted
        .split_once('.')
        .map_or((formatted.as_str(), None), |(integer, fraction)| {
            (integer, Some(fraction))
        });
    let mut grouped = String::with_capacity(formatted.len() + integer.len() / 3);
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if let Some(fraction) = fraction {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
}
