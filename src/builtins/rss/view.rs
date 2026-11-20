use super::RssExtension;
use crate::{extension::Action, ui::declarative::*};

fn text(value: impl Into<String>, size: f32, color: ColorSpec) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Custom(size),
            align: Align::Start,
            ..Default::default()
        },
    )
    .color(color)
}

fn column(children: Vec<ViewNode>, gap: f32) -> ViewNode {
    ViewNode::column(
        children,
        Flow {
            gap: Some(gap),
            align: Align::Stretch,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)
}

fn button(label: &str, action: &str) -> ViewNode {
    ViewNode::subtle_button(label, Action::Custom(action.into()))
        .sized(Size::Auto, Size::Fixed(28.0))
}

fn panel(mut node: ViewNode) -> ViewNode {
    node.style.padding = [10.0; 4];
    node.style.normal.background = Some(ColorSpec::SurfaceVariant);
    node.style.normal.corner_radius = Some(12.0);
    node
}

pub(super) fn build(rss: &RssExtension) -> WidgetView {
    let selected = rss
        .feeds
        .iter()
        .find(|feed| Some(&feed.url) == rss.selected.as_ref());
    let mut sources = Vec::new();
    let mut source_menus = Vec::new();
    for feed in &rss.feeds {
        let active = Some(&feed.url) == rss.selected.as_ref();
        let unread = feed.unread_count();
        let mut heading = vec![text(
            &feed.title,
            11.5,
            if active {
                ColorSpec::Info
            } else {
                ColorSpec::PrimaryText
            },
        )
        .sized(Size::Fill(1.0), Size::Auto)];
        if unread > 0 {
            let mut badge = ViewNode::new(Node::Badge(unread.to_string()))
                .sized(Size::Fixed(32.0), Size::Fixed(18.0))
                .color(ColorSpec::Info);
            badge.style.normal.background = Some(ColorSpec::AccentSubtle);
            heading.push(badge);
        }
        let mut select = ViewNode::new(Node::ActionArea {
            action: Action::Select(feed.url.clone()),
            label: feed.title.clone(),
            content: Box::new(column(
                vec![
                    ViewNode::row(
                        heading,
                        Flow {
                            gap: Some(5.0),
                            ..Default::default()
                        },
                    )
                    .sized(Size::Fill(1.0), Size::Auto),
                    text(&feed.url, 9.0, ColorSpec::SecondaryText),
                ],
                4.0,
            )),
        })
        .sized(Size::Fill(1.0), Size::Fixed(48.0));
        select.key = Some(feed.url.clone());
        select.style.padding = [7.0, 8.0, 7.0, 8.0];
        select.style.normal.background = Some(if active {
            ColorSpec::AccentSubtle
        } else {
            ColorSpec::Transparent
        });
        select.style.hovered.background = Some(ColorSpec::AccentSubtle);
        select.style.normal.corner_radius = Some(8.0);
        sources.push(vec![select]);
        source_menus.push(Some(TableRowMenu {
            title: feed.title.clone(),
            label: "删除此订阅".into(),
            action: Action::Custom(format!("remove-feed:{}", feed.url)),
        }));
    }
    if sources.is_empty() {
        sources.push(vec![text("还没有订阅", 11.0, ColorSpec::SecondaryText)]);
        source_menus.push(None);
    }
    let source_table = Table {
        columns: vec![TableColumn::new("", 1.0)],
        rows: sources,
        row_menus: source_menus,
    };
    let sidebar = panel(column(
        vec![
            text("订阅", 13.0, ColorSpec::PrimaryText),
            ViewNode::new(Node::Scroll(
                ScrollAxes::Vertical,
                Box::new(ViewNode::new(Node::Table(source_table))),
            ))
            .sized(Size::Fill(1.0), Size::Fixed(246.0)),
            ViewNode::subtle_button("＋ 添加订阅", Action::Custom("add".into()))
                .sized(Size::Fill(1.0), Size::Fixed(30.0)),
        ],
        8.0,
    ))
    .sized(Size::Fixed(194.0), Size::Fill(1.0));

    let mut articles = Vec::new();
    if let Some(feed) = selected {
        if let Some(error) = rss.errors.get(&feed.url) {
            articles.push(text(error, 11.0, ColorSpec::Warning));
        }
        for article in &feed.articles {
            let read = feed.read_ids.contains(&article.id);
            let title = ViewNode::text(
                &article.title,
                TextStyle {
                    font_size: FontSize::Custom(12.0),
                    align: Align::Start,
                    wrap: true,
                    max_lines: Some(2),
                    font_weight: if read {
                        FontWeight::Normal
                    } else {
                        FontWeight::Bold
                    },
                    ..Default::default()
                },
            )
            .sized(Size::Fill(1.0), Size::Auto)
            .color(if read {
                ColorSpec::SecondaryText
            } else {
                ColorSpec::PrimaryText
            });
            let mut content = vec![title];
            let state = if read {
                ViewNode::new(Node::Spacer).sized(Size::Fixed(18.0), Size::Fixed(18.0))
            } else {
                let mut dot = ViewNode::new(Node::Spacer).sized(Size::Fixed(8.0), Size::Fixed(8.0));
                dot.style.normal.background = Some(ColorSpec::Info);
                dot.style.normal.corner_radius = Some(4.0);
                let mut state = ViewNode::new(Node::ActionArea {
                    action: Action::Custom(format!("read:{}", article.id)),
                    label: "标为已读".into(),
                    content: Box::new(dot),
                })
                .sized(Size::Fixed(18.0), Size::Fixed(18.0));
                state.style.padding = [5.0; 4];
                state.style.normal.background = Some(ColorSpec::Transparent);
                state
            };
            content.push(
                ViewNode::row(
                    vec![
                        text(&article.date, 9.0, ColorSpec::SecondaryText)
                            .sized(Size::Fill(1.0), Size::Auto),
                        state,
                    ],
                    Flow {
                        gap: Some(5.0),
                        ..Default::default()
                    },
                )
                .sized(Size::Fill(1.0), Size::Auto),
            );
            let mut row = ViewNode::new(Node::ActionArea {
                action: Action::Custom(format!("open:{}", article.id)),
                label: article.title.clone(),
                content: Box::new(column(content, 5.0)),
            })
            .sized(Size::Fill(1.0), Size::Auto);
            row.key = Some(article.id.clone());
            row.style.padding = [10.0; 4];
            row.style.normal.corner_radius = Some(8.0);
            row.style.hovered.background = Some(ColorSpec::AccentSubtle);
            articles.push(row);
            articles.push(
                ViewNode::new(Node::Divider { vertical: false })
                    .sized(Size::Fill(1.0), Size::Fixed(1.0)),
            );
        }
        if feed.articles.is_empty() {
            articles.push(text(
                if rss.pending.contains(&feed.url) {
                    "正在读取文章…"
                } else {
                    "这个订阅暂时没有文章"
                },
                12.0,
                ColorSpec::SecondaryText,
            ));
        }
    } else {
        articles.push(text(
            "添加 RSS 或 Atom 链接后，在这里阅读文章",
            12.0,
            ColorSpec::SecondaryText,
        ));
    }
    let mut reader = vec![ViewNode::row(
        vec![
            text(
                selected.map_or("文章", |feed| feed.title.as_str()),
                13.0,
                ColorSpec::PrimaryText,
            )
            .sized(Size::Fill(1.0), Size::Auto),
            ViewNode::subtle_button(
                if rss
                    .selected
                    .as_ref()
                    .is_some_and(|url| rss.pending.contains(url))
                {
                    "刷新中…"
                } else {
                    "刷新"
                },
                Action::Advance,
            )
            .sized(Size::Auto, Size::Fixed(28.0)),
        ],
        Flow {
            gap: Some(8.0),
            align: Align::Center,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)];
    if rss.adding {
        reader.push(panel(column(
            vec![
                ViewNode::new(Node::TextInput {
                    key: "rss-url".into(),
                    value: rss.draft.clone(),
                    hint: "https://example.com/feed.xml".into(),
                })
                .sized(Size::Fill(1.0), Size::Fixed(30.0)),
                ViewNode::row(
                    vec![button("添加订阅", "subscribe"), button("取消", "cancel")],
                    Flow {
                        gap: Some(7.0),
                        justify: Justify::End,
                        ..Default::default()
                    },
                )
                .sized(Size::Fill(1.0), Size::Auto),
            ],
            8.0,
        )));
    }
    reader.push(
        ViewNode::new(Node::Scroll(
            ScrollAxes::Vertical,
            Box::new(column(articles, 1.0)),
        ))
        .sized(Size::Fill(1.0), Size::Fixed(256.0)),
    );
    let reading_list = column(reader, 10.0).sized(Size::Fill(1.0), Size::Auto);
    let mut grid = Grid::new(Columns::Tracks(vec![
        Track::Fraction(1.0),
        Track::Fraction(2.2),
    ]));
    grid.column_gap = Some(10.0);
    let mut children = Vec::new();
    if let Some(error) = &rss.error {
        children.push(text(error, 11.0, ColorSpec::Danger));
    }
    children.push(
        ViewNode::new(Node::Grid(grid, vec![sidebar, reading_list]))
            .sized(Size::Fill(1.0), Size::Auto),
    );
    let mut view = WidgetView::new(column(children, 10.0), [580.0, 340.0]);
    view.bounds.fit_height = true;
    view
}
