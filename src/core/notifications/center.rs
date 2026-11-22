use crate::{
    extension::{
        Action, ContributionDescriptor, ContributionKind, NotificationCenterItemView,
        NotificationKey, NOTIFICATION_CENTER_ID,
    },
    ui::declarative::*,
};

pub const CENTER_PAGE_SIZE: usize = 8;
const WIDTH: f32 = 320.0;
const MIN_HEIGHT: f32 = 128.0;
const ROW_HEIGHT: f32 = 31.0;
const ROW_GAP: f32 = 3.0;
const FOOTER_HEIGHT: f32 = 23.0;
const FOOTER_GAP: f32 = 6.0;
// The capsule reserves the last 15 points for the Widget page indicator.
const CAPSULE_BOTTOM: f32 = 15.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationCenterEntry {
    pub key: NotificationKey,
    pub view: NotificationCenterItemView,
}

pub fn descriptor() -> ContributionDescriptor {
    ContributionDescriptor::builtin(
        NOTIFICATION_CENTER_ID,
        ContributionKind::Widget,
        "通知中心",
        "bell",
        true,
        0,
    )
    .with_size_hint(WIDTH, MIN_HEIGHT)
    .with_widget_bounds(WidgetBounds {
        resizable: true,
        ..Default::default()
    })
}

fn item(entry: &NotificationCenterEntry) -> ViewNode {
    let id = entry.key.stable_id();
    let icon = entry.view.icon.as_deref().unwrap_or("bell");
    let label = |text: &str, primary: bool| {
        ViewNode::text(
            text,
            TextStyle {
                font_size: if primary {
                    FontSize::Body
                } else {
                    FontSize::Small
                },
                align: Align::Start,
                ..Default::default()
            },
        )
        .color(if primary {
            ColorSpec::PrimaryText
        } else {
            ColorSpec::SecondaryText
        })
        .sized(
            Size::Fill(1.0),
            Size::Fixed(if primary { 15.0 } else { 12.0 }),
        )
    };
    let content = ViewNode::row(
        vec![
            ViewNode::icon(icon, 14.0)
                .color(ColorSpec::Info)
                .sized(Size::Fixed(27.0), Size::Fixed(22.0)),
            ViewNode::column(
                vec![
                    label(&entry.view.title, true),
                    label(&entry.view.body, false),
                ],
                Flow {
                    gap: Some(0.0),
                    justify: Justify::Center,
                    ..Default::default()
                },
            )
            .sized(Size::Fill(1.0), Size::Fill(1.0)),
        ],
        Flow {
            gap: Some(2.0),
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fill(1.0));
    let open = ViewNode::new(Node::ActionArea {
        action: Action::Select(id.clone()),
        label: entry.view.title.clone(),
        content: Box::new(content),
    })
    .sized(Size::Fill(1.0), Size::Fill(1.0))
    .styled(Style {
        hovered: Paint {
            background: Some(ColorSpec::AccentSubtle),
            ..Default::default()
        },
        ..Default::default()
    });
    let acknowledge = ViewNode::subtle_button(
        "×",
        Action::Custom(format!("core.notifications.acknowledge:{id}")),
    )
    .sized(Size::Fixed(20.0), Size::Fixed(20.0));
    let mut row = ViewNode::row(
        vec![open, acknowledge],
        Flow {
            gap: Some(5.0),
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fixed(ROW_HEIGHT))
    .styled(Style {
        normal: Paint {
            background: Some(ColorSpec::SurfaceVariant),
            border_color: Some(ColorSpec::Border),
            border_width: Some(1.0),
            corner_radius: Some(8.0),
            ..Default::default()
        },
        padding: [2.0, 0.0, 6.0, 0.0],
        ..Default::default()
    });
    row.key = Some(id);
    row.clip = true;
    row
}

fn empty_state() -> ViewNode {
    ViewNode::column(
        vec![
            ViewNode::icon("bell", 19.0)
                .sized(Size::Fixed(34.0), Size::Fixed(32.0))
                .styled(Style {
                    normal: Paint {
                        foreground: Some(ColorSpec::Info),
                        background: Some(ColorSpec::AccentSubtle),
                        corner_radius: Some(10.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ViewNode::text("没有通知", TextStyle::default()).color(ColorSpec::SecondaryText),
        ],
        Flow {
            gap: Some(6.0),
            justify: Justify::Center,
            ..Default::default()
        },
    )
}

fn page_button(label: &str, target: usize, enabled: bool) -> ViewNode {
    let mut button = ViewNode::subtle_button(
        label,
        Action::Custom(format!("core.notifications.page:{target}")),
    )
    .sized(Size::Fixed(44.0), Size::Fixed(21.0));
    if let Node::Control {
        enabled: active, ..
    } = &mut button.kind
    {
        *active = enabled;
    }
    button
}

fn footer(page: usize, page_count: usize, has_entries: bool) -> ViewNode {
    let mut children = Vec::new();
    if page_count > 1 {
        children.extend([
            page_button("上一页", page.saturating_sub(1), page > 0),
            ViewNode::text(
                format!("{} / {}", page + 1, page_count),
                TextStyle {
                    font_size: FontSize::Small,
                    ..Default::default()
                },
            )
            .color(ColorSpec::SecondaryText),
            page_button("下一页", page + 1, page + 1 < page_count),
        ]);
    }
    children.push(ViewNode::new(Node::Spacer).sized(Size::Fill(1.0), Size::Auto));
    let mut clear = ViewNode::button(
        "全部清理",
        Action::Custom("core.notifications.clear_all".into()),
    )
    .sized(Size::Fixed(76.0), Size::Fixed(FOOTER_HEIGHT));
    if let Node::Control { enabled, .. } = &mut clear.kind {
        *enabled = has_entries;
    }
    children.push(clear);
    ViewNode::row(
        children,
        Flow {
            gap: Some(6.0),
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fixed(FOOTER_HEIGHT))
}

pub fn widget_view(entries: &[NotificationCenterEntry], page: usize) -> WidgetView {
    let page_count = entries.len().div_ceil(CENTER_PAGE_SIZE).max(1);
    let page = page.min(page_count - 1);
    let body = if entries.is_empty() {
        empty_state()
    } else {
        let rows = ViewNode::column(
            entries
                .iter()
                .skip(page * CENTER_PAGE_SIZE)
                .take(CENTER_PAGE_SIZE)
                .map(item)
                .collect(),
            Flow {
                gap: Some(ROW_GAP),
                ..Default::default()
            },
        );
        let mut scroll = ViewNode::new(Node::Scroll(ScrollAxes::Vertical, Box::new(rows)));
        scroll.key = Some(format!("notification-page-{page}"));
        scroll
    };
    let row_count = entries
        .len()
        .saturating_sub(page * CENTER_PAGE_SIZE)
        .min(CENTER_PAGE_SIZE);
    let list_height = ROW_HEIGHT * row_count as f32 + ROW_GAP * row_count.saturating_sub(1) as f32;
    let height = (list_height + FOOTER_GAP + FOOTER_HEIGHT + CAPSULE_BOTTOM).max(MIN_HEIGHT);
    let mut root = ViewNode::column(
        vec![
            body.sized(Size::Fill(1.0), Size::Fill(1.0)),
            footer(page, page_count, !entries.is_empty()),
        ],
        Flow {
            gap: Some(FOOTER_GAP),
            ..Default::default()
        },
    );
    root.style.padding = [4.0, 0.0, 4.0, 0.0];
    WidgetView::new(root, [WIDTH, height])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::ExtensionId;

    fn entry(number: usize) -> NotificationCenterEntry {
        NotificationCenterEntry {
            key: NotificationKey {
                source_extension_id: ExtensionId::builtin("builtin.test/notifications"),
                notification_id: number.to_string(),
            },
            view: NotificationCenterItemView {
                title: format!("通知 {number}"),
                body: format!("内容 {number}"),
                icon: None,
            },
        }
    }

    #[test]
    fn empty_center_has_bottom_cleanup_without_an_active_action() {
        let Node::Column(_, children) = widget_view(&[], 0).root.kind else {
            panic!("expected column");
        };
        let Node::Column(_, empty) = &children[0].kind else {
            panic!("expected empty state");
        };
        assert!(matches!(empty[1].kind, Node::Text { ref text, .. } if text == "没有通知"));
        let Node::Row(_, footer) = &children[1].kind else {
            panic!("expected footer");
        };
        assert!(matches!(&footer.last().unwrap().kind, Node::Control {
            enabled: false, action: Action::Custom(id), ..
        } if id == "core.notifications.clear_all"));
    }

    #[test]
    fn pagination_keeps_actions_reachable_in_groups_of_eight() {
        let entries: Vec<_> = (1..=9).map(entry).collect();
        for (page, expected) in [&entries[..8], &entries[8..]].into_iter().enumerate() {
            let view = widget_view(&entries, page);
            if page > 0 {
                assert!(view.preferred_size[1] < widget_view(&entries, 0).preferred_size[1]);
            }
            let Node::Column(_, children) = view.root.kind else {
                panic!("expected column");
            };
            let Node::Scroll(ScrollAxes::Vertical, list) = &children[0].kind else {
                panic!("expected scrollable list");
            };
            let Node::Column(_, rows) = &list.kind else {
                panic!("expected rows");
            };
            assert_eq!(rows.len(), expected.len());
            for (node, entry) in rows.iter().zip(expected) {
                let id = entry.key.stable_id();
                let Node::Row(_, parts) = &node.kind else {
                    panic!("expected row");
                };
                assert!(
                    matches!(&parts[0].kind, Node::ActionArea { action: Action::Select(key), .. } if *key == id)
                );
                assert!(
                    matches!(&parts[1].kind, Node::Control { action: Action::Custom(key), .. } if *key == format!("core.notifications.acknowledge:{id}"))
                );
            }
            let Node::Row(_, footer) = &children[1].kind else {
                panic!("expected footer");
            };
            assert!(
                matches!(&footer[0].kind, Node::Control { enabled, .. } if *enabled == (page == 1))
            );
            assert!(
                matches!(&footer[2].kind, Node::Control { enabled, .. } if *enabled == (page == 0))
            );
            assert_eq!(footer[0].size.width, Size::Fixed(44.0));
        }
    }

    #[test]
    fn a_full_page_renders_eight_rows_above_the_footer() {
        use eframe::egui::{self, pos2, vec2, Rect};
        let entries: Vec<_> = (1..=9).map(entry).collect();
        let view = widget_view(&entries, 0);
        let ctx = egui::Context::default();
        let icons = crate::app::icons::Icons::load(&ctx);
        let body = vec2(
            view.preferred_size[0],
            view.preferred_size[1] - CAPSULE_BOTTOM,
        );
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::ui::renderer::show_widget(
                    ui,
                    Rect::from_min_size(pos2(16.0, 16.0), body),
                    &ExtensionId::builtin(NOTIFICATION_CENTER_ID),
                    &view,
                    &icons,
                    1.0,
                );
            });
        });
        let y = |label: &str| {
            output.shapes.iter().find_map(|shape| {
                let egui::Shape::Text(text) = &shape.shape else {
                    return None;
                };
                (text.galley.job.text == label).then_some(text.pos.y)
            })
        };
        let last_row = y("通知 8").expect("eighth notification should render");
        assert!(y("全部清理").expect("footer should render") > last_row);
        assert!(y("通知 9").is_none());
    }
}
