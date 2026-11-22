use super::{Account, EmailMessage, MailExtension, NotificationTarget};
use crate::{
    extension::Action,
    ui::declarative::{
        Align, ColorSpec, ContextMenuAction, Flow, FontSize, FontWeight, Node, ScrollAxes, Size,
        Style, TextStyle, ViewNode, WidgetBounds, WidgetView,
    },
};

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

fn wrapped_text(value: impl Into<String>, size: f32, color: ColorSpec) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Custom(size),
            align: Align::Start,
            wrap: true,
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
}

fn button(label: &str, action: &str) -> ViewNode {
    ViewNode::subtle_button(label, Action::Custom(action.into()))
        .sized(Size::Auto, Size::Fixed(28.0))
}

fn input(key: &str, value: &str, hint: &str) -> ViewNode {
    let mut node = ViewNode::new(Node::TextInput {
        key: key.into(),
        value: value.into(),
        hint: hint.into(),
    })
    .sized(Size::Fill(1.0), Size::Fixed(32.0));
    node.style.normal.background = Some(ColorSpec::Surface);
    node.style.normal.border_color = Some(ColorSpec::Border);
    node.style.normal.border_width = Some(1.0);
    node.style.normal.corner_radius = Some(8.0);
    node.style.padding = [6.0; 4];
    node
}

fn password_input(value: &str) -> ViewNode {
    let mut node = ViewNode::new(Node::PasswordInput {
        key: "mail-password".into(),
        value: value.into(),
        hint: "邮箱密码或应用专用密码".into(),
    })
    .sized(Size::Fill(1.0), Size::Fixed(32.0));
    node.style.normal.background = Some(ColorSpec::Surface);
    node.style.normal.border_color = Some(ColorSpec::Border);
    node.style.normal.border_width = Some(1.0);
    node.style.normal.corner_radius = Some(8.0);
    node.style.padding = [6.0; 4];
    node
}

fn card(mut node: ViewNode) -> ViewNode {
    node.style.padding = [10.0; 4];
    node.style.normal.background = Some(ColorSpec::SurfaceVariant);
    node.style.normal.corner_radius = Some(12.0);
    node
}

fn badge(count: usize) -> ViewNode {
    let mut node = ViewNode::new(Node::Badge(count.to_string()))
        .sized(Size::Fixed(34.0), Size::Fixed(20.0))
        .color(ColorSpec::OnAccentSubtle);
    node.style.normal.background = Some(ColorSpec::AccentSubtle);
    node
}

fn account_row(mail: &MailExtension, account: &Account, unread: usize) -> ViewNode {
    let selected = mail.selected.as_ref() == Some(&account.id);
    let mut content = vec![text(
        &account.email,
        11.5,
        if selected {
            ColorSpec::Accent
        } else {
            ColorSpec::PrimaryText
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)];
    if unread > 0 {
        content.push(badge(unread));
    }
    let select = ViewNode::new(Node::ActionArea {
        action: Action::Select(account.id.clone()),
        label: account.email.clone(),
        context_menu: Some(ContextMenuAction {
            title: account.email.clone(),
            label: "删除此邮箱账号".into(),
            action: Action::Custom(format!("remove-account:{}", account.id)),
        }),
        content: Box::new(ViewNode::row(
            content,
            Flow {
                gap: Some(4.0),
                align: Align::Center,
                ..Default::default()
            },
        )),
    })
    .sized(Size::Fill(1.0), Size::Fixed(34.0));
    let mut select = select;
    select.style.padding = [7.0, 8.0, 7.0, 8.0];
    select.style.normal.background = Some(if selected {
        ColorSpec::AccentSubtle
    } else {
        ColorSpec::Transparent
    });
    select.style.hovered.background = Some(ColorSpec::AccentSubtle);
    select.style.normal.corner_radius = Some(8.0);
    select
}

fn sidebar(mail: &MailExtension) -> ViewNode {
    let rows = if mail.accounts.is_empty() {
        vec![text("添加一个 IMAP 账号", 11.0, ColorSpec::SecondaryText)]
    } else {
        mail.accounts
            .iter()
            .map(|account| {
                account_row(
                    mail,
                    account,
                    mail.mailboxes
                        .get(&account.id)
                        .map_or(0, |mailbox| mailbox.unread_count),
                )
            })
            .collect()
    };
    let scroller = ViewNode::new(Node::Scroll(
        ScrollAxes::Vertical,
        Box::new(column(rows, 5.0)),
    ))
    .sized(Size::Fill(1.0), Size::Fill(1.0));
    card(column(
        vec![
            text("账号", 11.0, ColorSpec::SecondaryText),
            scroller,
            ViewNode::subtle_button("＋ 添加账号", Action::Custom("add".into()))
                .sized(Size::Fill(1.0), Size::Fixed(30.0)),
        ],
        8.0,
    ))
    .sized(Size::Fixed(194.0), Size::Fill(1.0))
}

fn provider_button(label: &str, id: &str) -> ViewNode {
    ViewNode::subtle_button(label, Action::Select(format!("provider:{id}")))
        .sized(Size::Auto, Size::Fixed(26.0))
}

fn account_form(mail: &MailExtension) -> ViewNode {
    let provider_row = ViewNode::row(
        vec![
            provider_button("Gmail", "gmail"),
            provider_button("Outlook", "outlook"),
            provider_button("Yahoo", "yahoo"),
            provider_button("iCloud", "icloud"),
            provider_button("QQ", "qq"),
            provider_button("163", "163"),
            provider_button("126", "126"),
            provider_button("自定义", "custom"),
        ],
        Flow {
            gap: Some(5.0),
            wrap: true,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    let mut dialog = column(
        vec![
            ViewNode::row(
                vec![
                    text("新增邮箱", 16.0, ColorSpec::PrimaryText).sized(Size::Fill(1.0), Size::Auto),
                    button("×", "cancel"),
                ],
                Flow { align: Align::Center, ..Default::default() },
            ).sized(Size::Fill(1.0), Size::Auto),
            text("选择服务商并填写 IMAP 登录信息", 10.5, ColorSpec::SecondaryText),
            text("邮箱服务商", 10.5, ColorSpec::SecondaryText),
            provider_row,
            text("邮箱地址", 10.5, ColorSpec::SecondaryText),
            input("mail-email", &mail.email_draft, "name@example.com"),
            text("IMAP 服务器与端口", 10.5, ColorSpec::SecondaryText),
            ViewNode::row(
                vec![
                    input("mail-server", &mail.server_draft, "IMAP 服务器"),
                    input("mail-port", &mail.port_draft, "993"),
                ],
                Flow {
                    gap: Some(7.0),
                    ..Default::default()
                },
            )
            .sized(Size::Fill(1.0), Size::Auto),
            text("应用专用密码", 10.5, ColorSpec::SecondaryText),
            password_input(&mail.password_draft),
            text(
                "使用 IMAP SSL。多数服务商需要先开启 IMAP，并填写应用专用密码；密码保存在 Peninsula 本地 Secret 文件。",
                10.0,
                ColorSpec::SecondaryText,
            ),
            ViewNode::row(
                vec![button("连接账号", "save-account"), button("取消", "cancel")],
                Flow {
                    gap: Some(7.0),
                    justify: crate::ui::declarative::Justify::End,
                    ..Default::default()
                },
            )
            .sized(Size::Fill(1.0), Size::Auto),
        ],
        8.0,
    ).sized(Size::Fixed(430.0), Size::Auto);
    dialog.style.padding = [18.0; 4];
    dialog.style.normal.background = Some(ColorSpec::SurfaceVariant);
    dialog.style.normal.border_color = Some(ColorSpec::Border);
    dialog.style.normal.border_width = Some(1.0);
    dialog.style.normal.corner_radius = Some(12.0);
    dialog
}

fn account_modal(base: ViewNode, mail: &MailExtension) -> ViewNode {
    let mut backdrop = ViewNode::new(Node::ActionArea {
        action: Action::Custom("cancel".into()),
        label: "关闭新增邮箱弹窗".into(),
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
        vec![base, backdrop, account_form(mail)],
    ))
    .sized(Size::Fill(1.0), Size::Fill(1.0))
}

fn message_row(message: &EmailMessage, account: &Account, pending: bool) -> ViewNode {
    let target = serde_json::to_string(&NotificationTarget {
        account_id: account.id.clone(),
        uid: message.uid,
    })
    .unwrap_or_default();
    let primary_color = if message.is_read {
        ColorSpec::SecondaryText
    } else {
        ColorSpec::PrimaryText
    };
    let sender = text(&message.from, 11.5, primary_color);
    let subject = ViewNode::text(
        &message.subject,
        TextStyle {
            font_size: FontSize::Custom(12.0),
            font_weight: if message.is_read {
                FontWeight::Normal
            } else {
                FontWeight::Bold
            },
            align: Align::Start,
            max_lines: Some(2),
            wrap: true,
            ..Default::default()
        },
    )
    .color(primary_color)
    .sized(Size::Fill(1.0), Size::Auto);
    let dot = if pending {
        text("打开中…", 9.0, ColorSpec::Accent)
    } else if message.is_read {
        ViewNode::new(Node::Spacer)
    } else {
        let mut dot = ViewNode::new(Node::Spacer).sized(Size::Fixed(8.0), Size::Fixed(8.0));
        dot.style.normal.background = Some(ColorSpec::Accent);
        dot.style.normal.corner_radius = Some(4.0);
        dot
    };
    let content = ViewNode::row(
        vec![
            column(
                vec![
                    ViewNode::row(
                        vec![
                            sender.sized(Size::Fill(1.0), Size::Auto),
                            text(display_date(&message.date), 9.0, ColorSpec::SecondaryText),
                        ],
                        Flow {
                            gap: Some(8.0),
                            ..Default::default()
                        },
                    )
                    .sized(Size::Fill(1.0), Size::Auto),
                    subject,
                ],
                3.0,
            )
            .sized(Size::Fill(1.0), Size::Auto),
            dot.sized(Size::Fixed(48.0), Size::Fixed(22.0)),
        ],
        Flow {
            gap: Some(7.0),
            align: Align::Center,
            ..Default::default()
        },
    );
    let mut row = ViewNode::new(Node::ActionArea {
        action: Action::Custom(format!("open:{target}")),
        label: format!("{}：{}", message.from, message.subject),
        content: Box::new(content),
        context_menu: None,
    })
    .sized(Size::Fill(1.0), Size::Fixed(68.0));
    row.style.padding = [8.0, 9.0, 8.0, 9.0];
    row.style.normal.background = Some(ColorSpec::Transparent);

    row.style.normal.corner_radius = Some(9.0);
    row.style.hovered.background = Some(ColorSpec::AccentSubtle);
    row.clip = true;
    row
}

fn display_date(date: &str) -> String {
    let value = date.split_once(',').map_or(date, |(_, rest)| rest.trim());
    value.chars().take(17).collect()
}

fn messages(mail: &MailExtension) -> Vec<ViewNode> {
    let mut entries: Vec<(&EmailMessage, &Account)> = mail
        .accounts
        .iter()
        .filter(|account| mail.selected.as_ref().is_none_or(|id| id == &account.id))
        .flat_map(|account| {
            mail.mailboxes
                .get(&account.id)
                .into_iter()
                .flat_map(move |mailbox| mailbox.messages.iter().map(move |email| (email, account)))
        })
        .collect();
    entries.sort_by_key(|(message, _)| {
        (
            std::cmp::Reverse(message.timestamp),
            std::cmp::Reverse(message.uid),
        )
    });
    if entries.is_empty() {
        let has_accounts = !mail.accounts.is_empty();
        return vec![text(
            if has_accounts {
                "这个邮箱的收件箱暂时为空，点刷新同步服务器邮件。"
            } else {
                "添加邮箱账号后，这里会显示该邮箱的邮件。"
            },
            12.0,
            ColorSpec::SecondaryText,
        )];
    }
    entries
        .into_iter()
        .map(|(message, account)| {
            message_row(
                message,
                account,
                mail.opening
                    .as_ref()
                    .is_some_and(|(id, uid)| id == &account.id && *uid == message.uid),
            )
        })
        .collect()
}

fn status(mail: &MailExtension) -> String {
    if mail.opening.is_some() {
        return "正在从服务器读取邮件并同步已读状态…".into();
    }
    let mailbox = mail.selected.as_ref().and_then(|id| mail.mailboxes.get(id));
    if mailbox.is_some_and(|mailbox| mailbox.pending) {
        return "正在与邮箱服务器同步…".into();
    }
    if mail.accounts.is_empty() {
        return "尚未登录邮箱".into();
    }
    let unread = mailbox.map_or(0, |mailbox| mailbox.unread_count);
    format!("{unread} 封未读 · IMAP 服务器状态")
}

pub(super) fn build(mail: &MailExtension) -> WidgetView {
    let title = if mail.selected.is_some() {
        mail.accounts
            .iter()
            .find(|account| Some(&account.id) == mail.selected.as_ref())
            .map_or_else(|| "邮件".to_owned(), |account| account.email.clone())
    } else {
        "选择邮箱".into()
    };
    let actions = vec![ViewNode::subtle_button(
        if mail.mailboxes.values().any(|mailbox| mailbox.pending) {
            "同步中…"
        } else {
            "刷新"
        },
        Action::Advance,
    )
    .sized(Size::Auto, Size::Fixed(28.0))];
    let top = ViewNode::row(
        vec![
            ViewNode::icon("mail", 18.0)
                .color(ColorSpec::Accent)
                .sized(Size::Fixed(24.0), Size::Fixed(24.0)),
            text(title, 14.0, ColorSpec::PrimaryText).sized(Size::Fill(1.0), Size::Auto),
        ]
        .into_iter()
        .chain(actions)
        .collect(),
        Flow {
            gap: Some(7.0),
            align: Align::Center,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fixed(30.0));

    let mut content = vec![text(status(mail), 10.0, ColorSpec::SecondaryText)];
    if let Some(error) = &mail.error {
        content.push(text(error, 10.0, ColorSpec::Warning));
    }
    for account in mail
        .accounts
        .iter()
        .filter(|account| mail.selected.as_ref().is_none_or(|id| id == &account.id))
    {
        if let Some(warning) = mail
            .mailboxes
            .get(&account.id)
            .and_then(|box_| box_.credential_warning.as_ref())
        {
            content.push(text(
                format!("{}：{warning}", account.email),
                10.0,
                ColorSpec::Warning,
            ));
        }
        if let Some(error) = mail
            .mailboxes
            .get(&account.id)
            .and_then(|box_| box_.error.as_ref())
        {
            content.push(wrapped_text(
                format!("{}：{}", account.email, error),
                10.0,
                ColorSpec::Warning,
            ));
        }
    }
    if mail.opening.is_none() || !mail.adding {
        let list = ViewNode::new(Node::Scroll(
            ScrollAxes::Vertical,
            Box::new(column(messages(mail), 5.0)),
        ))
        .sized(Size::Fill(1.0), Size::Fill(1.0));
        content.push(list);
    }
    let mut main = column(
        vec![
            top,
            column(content, 7.0).sized(Size::Fill(1.0), Size::Fill(1.0)),
        ],
        8.0,
    )
    .sized(Size::Fill(1.0), Size::Fill(1.0))
    .styled(Style {
        padding: [8.0, 10.0, 8.0, 2.0],
        ..Default::default()
    });
    main.clip = true;
    let base = ViewNode::row(
        vec![sidebar(mail), main],
        Flow {
            gap: Some(10.0),
            align: Align::Stretch,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fill(1.0));
    let root = if mail.adding {
        account_modal(base, mail)
    } else {
        base
    };
    let mut view = WidgetView::new(root, [720.0, 450.0]);
    view.bounds = WidgetBounds {
        min: [500.0, 300.0],
        max: [Some(900.0), Some(720.0)],
        resizable: true,

        ..Default::default()
    };
    view
}
