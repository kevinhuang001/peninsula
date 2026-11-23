use super::{Asset, MarketExtension, MarketQuote, WIDGET_ID};
use crate::{
    extension::{Action, CapsuleView, ExtensionId},
    ui::declarative::{
        Align, ColorSpec, Flow, FontSize, FontWeight, ImageFit, Justify, Node, Paint, Size, Style,
        Table, TableColumn, TableRowMenu, TextStyle, ViewNode,
    },
};

const ADD_ACTION: &str = "market:add";
const ADD_OPEN_ACTION: &str = "market:add-open";
const ADD_CANCEL_ACTION: &str = "market:add-cancel";
const REMOVE_ACTION: &str = "market:remove:";

pub(super) fn build(market: &MarketExtension) -> crate::extension::WidgetView {
    let updated = ViewNode::text(
        market
            .last_updated
            .as_ref()
            .map(|time| format!("更新于 {time}"))
            .unwrap_or_default(),
        TextStyle {
            font_size: FontSize::Small,
            align: Align::End,
            ..Default::default()
        },
    )
    .color(ColorSpec::SecondaryText);
    let refresh = ViewNode::subtle_button(
        if market.refreshing {
            "更新中…"
        } else {
            "刷新"
        },
        Action::Advance,
    )
    .sized(Size::Auto, Size::Fixed(26.0));
    let add = ViewNode::subtle_button("添加", Action::Custom(ADD_OPEN_ACTION.into()))
        .sized(Size::Auto, Size::Fixed(26.0));
    let heading = ViewNode::row(
        vec![updated, add, refresh],
        Flow {
            gap: Some(8.0),
            align: Align::Center,
            justify: Justify::End,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fixed(30.0));
    let mut children = vec![heading];
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

    let mut root = ViewNode::column(
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
    if market.add_open {
        root = add_modal(root, market);
    }
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
    let rows = market
        .watchlist
        .assets
        .iter()
        .map(|asset| quote_row(asset, market.quotes.get(&asset.cache_key())))
        .collect();
    let row_menus = market
        .watchlist
        .assets
        .iter()
        .map(|asset| {
            Some(TableRowMenu {
                title: asset.symbol.clone(),
                label: "删除此自选".into(),
                action: Action::Custom(format!(
                    "{REMOVE_ACTION}{}:{}",
                    asset.kind.key(),
                    asset.symbol
                )),
            })
        })
        .collect();
    ViewNode::new(Node::Table(Table {
        columns: vec![
            TableColumn::new("资产", 2.3).with_header_align(Align::Start),
            TableColumn::new("最新价", 1.3).with_header_align(Align::Center),
            TableColumn::new("24 小时", 0.9).with_header_align(Align::Center),
        ],
        rows,
        row_menus,
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

fn add_modal(base: ViewNode, market: &MarketExtension) -> ViewNode {
    let mut input = ViewNode::new(Node::TextInput {
        key: "market-symbol".into(),
        value: market.draft.clone(),
        hint: "代码，如 AAPL、BTC-USD 或 0700.HK".into(),
    })
    .sized(Size::Fill(1.0), Size::Fixed(34.0));
    input.style.normal.background = Some(ColorSpec::Surface);
    input.style.normal.border_color = Some(ColorSpec::Border);
    input.style.normal.border_width = Some(1.0);
    input.style.normal.corner_radius = Some(8.0);
    input.style.padding = [7.0; 4];
    let mut children = vec![
        ViewNode::row(
            vec![
                ViewNode::text(
                    "添加自选",
                    TextStyle {
                        font_size: FontSize::Relative(1.25),
                        font_weight: FontWeight::Bold,
                        align: Align::Start,
                        ..Default::default()
                    },
                )
                .sized(Size::Fill(1.0), Size::Auto),
                ViewNode::subtle_button("×", Action::Custom(ADD_CANCEL_ACTION.into())),
            ],
            Flow {
                align: Align::Center,
                ..Default::default()
            },
        ),
        input,
    ];
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
    children.push(ViewNode::row(
        vec![
            ViewNode::subtle_button("取消", Action::Custom(ADD_CANCEL_ACTION.into())),
            ViewNode::subtle_button("添加", Action::Custom(ADD_ACTION.into())),
        ],
        Flow {
            gap: Some(7.0),
            justify: Justify::End,
            ..Default::default()
        },
    ));
    let mut dialog = ViewNode::column(
        children,
        Flow {
            gap: Some(9.0),
            align: Align::Stretch,
            ..Default::default()
        },
    )
    .sized(Size::Fixed(390.0), Size::Auto);
    dialog.style.padding = [18.0; 4];
    dialog.style.normal.background = Some(ColorSpec::SurfaceVariant);
    dialog.style.normal.border_color = Some(ColorSpec::Border);
    dialog.style.normal.border_width = Some(1.0);
    dialog.style.normal.corner_radius = Some(12.0);
    let mut backdrop = ViewNode::new(Node::ActionArea {
        action: Action::Custom(ADD_CANCEL_ACTION.into()),
        label: "关闭添加自选弹窗".into(),
        content: Box::new(ViewNode::new(Node::Spacer)),
        context_menu: None,
    })
    .sized(Size::Fill(1.0), Size::Fill(1.0));
    backdrop.style.normal.background = Some(ColorSpec::Rgba {
        r: 10,
        g: 13,
        b: 20,
        a: 190,
    });
    ViewNode::new(Node::Stack(
        [Align::Center; 2],
        vec![base, backdrop, dialog],
    ))
    .sized(Size::Fill(1.0), Size::Fill(1.0))
}

pub(super) fn capsule(market: &MarketExtension) -> Vec<CapsuleView> {
    market
        .watchlist
        .assets
        .iter()
        .map(|asset| {
            let quote = market.quotes.get(&asset.cache_key());
            let logo = quote.and_then(|quote| quote.logo.as_ref()).map_or_else(
                || {
                    ViewNode::new(Node::Badge(
                        asset.symbol.chars().take(2).collect::<String>(),
                    ))
                    .sized(Size::Fixed(18.0), Size::Fixed(18.0))
                    .styled(Style {
                        normal: Paint {
                            foreground: Some(ColorSpec::OnAccentSubtle),
                            background: Some(ColorSpec::AccentSubtle),
                            corner_radius: Some(9.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                },
                |image| {
                    let mut logo = ViewNode::new(Node::RasterImage(image.clone()))
                        .sized(Size::Fixed(18.0), Size::Fixed(18.0));
                    logo.image_fit = Some(ImageFit::Contain);
                    logo.style.normal.corner_radius = Some(5.0);
                    logo
                },
            );
            let price = quote
                .and_then(|quote| {
                    quote
                        .price
                        .map(|price| format_price(price, &quote.currency))
                })
                .unwrap_or_else(|| "—".into());
            let price = ViewNode::text(
                price,
                TextStyle {
                    font_size: FontSize::Relative(0.8333),
                    font_weight: FontWeight::Bold,
                    monospace: true,
                    wrap: false,
                    ..Default::default()
                },
            );
            CapsuleView {
                id: super::capsule_item_id(asset),
                source_extension_id: ExtensionId::builtin(WIDGET_ID),
                content: ViewNode::row(
                    vec![logo, price],
                    Flow {
                        gap: Some(3.0),
                        align: Align::Center,
                        ..Default::default()
                    },
                )
                .sized(Size::Auto, Size::Fixed(24.0)),
                visibility: Default::default(),
                min_width: 58.0,
                preferred_width: 82.0,
                fit_width: true,
                priority: 0,
                tooltip: Some(format!("{} 最新价格", asset.symbol)),
            }
        })
        .collect()
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
                    foreground: Some(ColorSpec::OnAccentSubtle),
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
                    font_size: FontSize::Relative(0.9167),
                    font_weight: FontWeight::Bold,
                    align: Align::Start,
                    wrap: false,
                    ..Default::default()
                },
            ),
            ViewNode::text(
                detail,
                TextStyle {
                    font_size: FontSize::Relative(0.75),
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
    vec![
        identity,
        cell(&price, ColorSpec::PrimaryText, Align::Center, true),
        cell(&change, change_color, Align::Center, true),
    ]
}

fn cell(value: &str, color: ColorSpec, align: Align, monospace: bool) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Relative(0.8333),
            align,
            monospace,
            wrap: false,
            ..Default::default()
        },
    )
    .color(color)
}

pub(super) fn format_price(price: f64, currency: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::icons::Icons, extension::ExtensionId, ui::renderer};
    use eframe::egui::{self, pos2, vec2, Rect};

    #[test]
    fn market_header_is_capsule_and_prices_are_centered_under_their_headers() {
        let mut market = MarketExtension::new();
        market.watchlist.assets.truncate(1);
        market.last_updated = Some("09:14:48".into());
        let asset = market.watchlist.assets[0].clone();
        let mut quote = MarketQuote::loading(asset.clone());
        quote.name = "Apple Inc.".into();
        quote.price = Some(349.21);
        quote.change_percent = Some(-0.92);
        market.quotes.insert(asset.cache_key(), quote);
        for width in [430.0, 780.0] {
            let ctx = egui::Context::default();
            let icons = Icons::load(&ctx);
            let view = build(&market);
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    renderer::show_widget(
                        ui,
                        Rect::from_min_size(pos2(20.0, 20.0), vec2(width, 400.0)),
                        &ExtensionId::builtin("builtin.market/overview"),
                        &view,
                        &icons,
                        1.0,
                    );
                });
            });
            let text_bounds = |value: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == value => {
                            Some(text.galley.rect.translate(text.pos.to_vec2()))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing {value}"))
            };
            assert!(text_bounds("更新于 09:14:48").right() < text_bounds("刷新").left());
            assert!((text_bounds("AAPL").left() - text_bounds("Apple Inc.").left()).abs() < 1.0);
            assert!(text_bounds("AAPL").left() < text_bounds("资产").left() + 40.0);

            assert!(
                (text_bounds("最新价").center().x - text_bounds("$349.21").center().x).abs() < 1.0
            );
            assert!(
                (text_bounds("24 小时").center().x - text_bounds("-0.92%").center().x).abs() < 1.0
            );
            assert!(!output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Text(text) =>
                    text.galley.job.text == "×"
                        || text.galley.job.text.contains("项自选")
                        || text.galley.job.text.contains("最新报价"),
                _ => false,
            }));
        }
    }
}
