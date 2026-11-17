use crate::extension::{
    Action, ExtensionDescriptor, ExtensionKind, NotificationCenterItemView, NotificationKey,
    TextStyle, ViewNode, WidgetView, NOTIFICATION_CENTER_ID,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationCenterEntry {
    pub key: NotificationKey,
    pub view: NotificationCenterItemView,
}

pub fn descriptor() -> ExtensionDescriptor {
    ExtensionDescriptor::builtin(
        NOTIFICATION_CENTER_ID,
        ExtensionKind::Widget,
        "通知中心",
        "bell",
        true,
        0,
    )
    .with_size_hint(300.0, 200.0)
}

pub fn widget_view(
    entries: &[NotificationCenterEntry],
    page: usize,
    max_items: usize,
) -> WidgetView {
    let mut children = Vec::new();
    if entries.is_empty() {
        children.push(ViewNode::Text {
            text: "没有通知".into(),
            style: TextStyle::Muted,
        });
    } else {
        let page_size = max_items.max(1);
        let page_count = entries.len().div_ceil(page_size);
        let page = page.min(page_count.saturating_sub(1));
        let start = page * page_size;
        let end = (start + page_size).min(entries.len());
        children.extend(entries.iter().skip(start).take(end - start).map(|entry| {
            ViewNode::NotificationItem {
                stable_id: entry.key.stable_id(),
                title: entry.view.title.clone(),
                body: entry.view.body.clone(),
                icon: entry.view.icon.clone(),
            }
        }));
        if page_count > 1 {
            let previous = if page > 0 {
                ViewNode::Button {
                    label: "上一页".into(),
                    action: Action::Custom(format!("core.notifications.page:{}", page - 1)),
                }
            } else {
                ViewNode::Text {
                    text: String::new(),
                    style: TextStyle::Small,
                }
            };
            let next = if page + 1 < page_count {
                ViewNode::Button {
                    label: "下一页".into(),
                    action: Action::Custom(format!("core.notifications.page:{}", page + 1)),
                }
            } else {
                ViewNode::Text {
                    text: String::new(),
                    style: TextStyle::Small,
                }
            };
            children.push(ViewNode::Row(vec![
                previous,
                ViewNode::Text {
                    text: format!("{}/{}", page + 1, page_count),
                    style: TextStyle::Small,
                },
                next,
            ]));
        }
    }
    children.push(ViewNode::Button {
        label: "全部清理".into(),
        action: Action::Custom("core.notifications.clear_all".into()),
    });
    WidgetView {
        root: ViewNode::Column(children),
        preferred_size: [300.0, 200.0],
        refresh_hint: crate::extension::RefreshHint::OnInvalidation,
    }
}

#[cfg(test)]
mod tests {
    use super::{widget_view, NotificationCenterEntry};
    use crate::extension::{
        ExtensionId, NotificationCenterItemView, NotificationKey, ViewNode, NOTIFICATION_CENTER_ID,
    };

    fn entry(id: &str) -> NotificationCenterEntry {
        NotificationCenterEntry {
            key: NotificationKey {
                source_extension_id: ExtensionId::builtin("builtin.test/notifications"),
                notification_id: id.into(),
            },
            view: NotificationCenterItemView {
                title: format!("title {id}"),
                body: format!("body {id}"),
                icon: None,
            },
        }
    }

    #[test]
    fn empty_center_keeps_clear_action_and_empty_state() {
        let view = widget_view(&[], 0, 3);
        let ViewNode::Column(children) = view.root else {
            panic!("notification center should render as a column");
        };
        assert!(matches!(children[0], ViewNode::Text { ref text, .. } if text == "没有通知"));
        assert!(matches!(children[1], ViewNode::Button { ref label, .. } if label == "全部清理"));
        assert_eq!(view.preferred_size[1], 200.0);
        assert_eq!(NOTIFICATION_CENTER_ID, "builtin.notifications/center");
    }

    #[test]
    fn center_pages_keep_each_notification_reachable() {
        let entries = [entry("one"), entry("two"), entry("three"), entry("four")];
        let ViewNode::Column(first) = widget_view(&entries, 0, 3).root else {
            panic!("notification center should render as a column");
        };
        let ViewNode::Column(second) = widget_view(&entries, 1, 3).root else {
            panic!("notification center should render as a column");
        };
        assert!(
            matches!(first[0], ViewNode::NotificationItem { ref title, .. } if title == "title one")
        );
        assert!(
            matches!(first[2], ViewNode::NotificationItem { ref title, .. } if title == "title three")
        );
        assert!(
            matches!(second[0], ViewNode::NotificationItem { ref title, .. } if title == "title four")
        );
        assert!(matches!(first[3], ViewNode::Row(_)));
        assert!(matches!(first[4], ViewNode::Button { ref label, .. } if label == "全部清理"));
        assert!(matches!(
            second[1],
            ViewNode::Row(ref controls)
                if matches!(controls[0], ViewNode::Button { ref label, .. } if label == "上一页")
                    && matches!(controls[2], ViewNode::Text { ref text, .. } if text.is_empty())
        ));
        assert!(matches!(second[2], ViewNode::Button { ref label, .. } if label == "全部清理"));
    }
}
