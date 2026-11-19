use super::{Account, EmailMessage, MailExtension, NotificationTarget};
use crate::{
    extension::Action,
    ui::declarative::{
        Align, ColorSpec, Flow, FontSize, FontWeight, Node, Overflow, ScrollAxes, Size, Style,
        TextStyle, ViewNode, WidgetBounds, WidgetView,
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
        hint: "é‚®ç®±å¯†ç æˆ–åº”ç”¨ä¸“ç”¨å¯†ç ".into(),
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
        .color(ColorSpec::Info);
    node.style.normal.background = Some(ColorSpec::AccentSubtle);
    node
}

fn account_row(mail: &MailExtension, account: &Account, unread: usize) -> ViewNode {
    let selected = mail.selected.as_ref() == Some(&account.id);
    let mut content = vec![text(
        &account.email,
        11.5,
        if selected {
            ColorSpec::Info
        } else {
            ColorSpec::PrimaryText
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)];
    if unread > 0 {
        content.push(badge(unread));
    }
    let mut row = ViewNode::new(Node::ActionArea {
        action: Action::Select(account.id.clone()),
        label: account.email.clone(),
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
    row.style.padding = [7.0, 8.0, 7.0, 8.0];
    row.style.normal.background = Some(if selected {
        ColorSpec::AccentSubtle
    } else {
        ColorSpec::Transparent
    });
    row.style.hovered.background = Some(ColorSpec::AccentSubtle);
    row.style.normal.corner_radius = Some(8.0);
    row
}

fn all_accounts_row(mail: &MailExtension, unread: usize) -> ViewNode {
    let selected = mail.selected.is_none();
    let mut content = vec![
        ViewNode::icon("mail", 15.0)
            .color(ColorSpec::Info)
            .sized(Size::Fixed(20.0), Size::Fixed(20.0)),
        text(
            "å…¨éƒ¨é‚®ä»¶",
            12.0,
            if selected {
                ColorSpec::Info
            } else {
                ColorSpec::PrimaryText
            },
        )
        .sized(Size::Fill(1.0), Size::Auto),
    ];
    if unread > 0 {
        content.push(badge(unread));
    }
    let mut row = ViewNode::new(Node::ActionArea {
        action: Action::Select("all-mail".into()),
        label: "å…¨éƒ¨é‚®ä»¶".into(),
        content: Box::new(ViewNode::row(
            content,
            Flow {
                gap: Some(4.0),
                align: Align::Center,
                ..Default::default()
            },
        )),
    })
    .sized(Size::Fill(1.0), Size::Fixed(36.0));
    row.style.padding = [7.0, 8.0, 7.0, 8.0];
    row.style.normal.background = Some(if selected {
        ColorSpec::AccentSubtle
    } else {
        ColorSpec::Transparent
    });
    row.style.hovered.background = Some(ColorSpec::AccentSubtle);
    row.style.normal.corner_radius = Some(8.0);
    row
}

fn sidebar(mail: &MailExtension) -> ViewNode {
    let unread_total = mail
        .mailboxes
        .values()
        .map(|mailbox| mailbox.unread_count)
        .sum();
    let mut rows = vec![all_accounts_row(mail, unread_total)];
    rows.push(
        ViewNode::new(Node::Divider { vertical: false }).sized(Size::Fill(1.0), Size::Fixed(1.0)),
    );
    if mail.accounts.is_empty() {
        rows.push(text("æ·»åŠ ä¸€ä¸ª IMAP è´¦å·", 11.0, ColorSpec::SecondaryText));
    } else {
        rows.extend(mail.accounts.iter().map(|account| {
            account_row(
                mail,
                account,
                mail.mailboxes
                    .get(&account.id)
                    .map_or(0, |mailbox| mailbox.unread_count),
            )
        }));
    }
    let scroller = ViewNode::new(Node::Scroll(
        ScrollAxes::Vertical,
        Box::new(column(rows, 5.0)),
    ))
    .sized(Size::Fill(1.0), Size::Fill(1.0));
    card(column(
        vec![
            text("è´¦å·", 11.0, ColorSpec::SecondaryText),
            scroller,
            ViewNode::subtle_button("ï¼‹ æ·»åŠ è´¦å·", Action::Custom("add".into()))
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
            provider_button("è‡ªå®šä¹‰", "custom"),
        ],
        Flow {
            gap: Some(5.0),
            wrap: true,
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    card(column(
        vec![
            text("æ·»åŠ é‚®ç®±è´¦å·", 14.0, ColorSpec::PrimaryText),
            provider_row,
            input("mail-email", &mail.email_draft, "name@example.com"),
            ViewNode::row(
                vec![
                    input("mail-server", &mail.server_draft, "IMAP æœåŠ¡å™¨"),
                    input("mail-port", &mail.port_draft, "993"),
                ],
                Flow {
                    gap: Some(7.0),
                    ..Default::default()
                },
            )
            .sized(Size::Fill(1.0), Size::Auto),
            password_input(&mail.password_draft),
            text(
                "ä½¿ç”¨ IMAP SSLã€‚å¤šæ•°æœåŠ¡å•†éœ€è¦å…ˆå¼€å¯ IMAPï¼Œå¹¶å¡«å†™åº”ç”¨ä¸“ç”¨å¯†ç ï¼›å¯†ç ä¿å­˜åœ¨ç³»ç»Ÿé’¥åŒ™ä¸²ã€‚",
                10.0,
                ColorSpec::SecondaryText,
            ),
            ViewNode::row(
                vec![button("è¿æ¥è´¦å·", "save-account"), button("å–æ¶ˆ", "cancel")],
                Flow {
                    gap: Some(7.0),
                    justify: crate::ui::declarative::Justify::End,
                    ..Default::default()
                },
            )
            .sized(Size::Fill(1.0), Size::Auto),
        ],
        8.0,
    ))
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
            max_lines: Some(1),
            wrap: false,
            ..Default::default()
        },
    )
    .color(primary_color)
    .sized(Size::Fill(1.0), Size::Auto);
    let dot = if pending {
        text("æ‰“å¼€ä¸­â€¦", 9.0, ColorSpec::Info)
    } else if message.is_read {
        ViewNode::new(Node::Spacer)
    } else {
        let mut dot = ViewNode::new(Node::Spacer).sized(Size::Fixed(8.0), Size::Fixed(8.0));
        dot.style.normal.background = Some(ColorSpec::Info);
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
                            text(&display_date(&message.date), 9.0, ColorSpec::SecondaryText),
                        ],
                        Flow {
                            gap: Some(8.0),
                            ..Default::default()
                        },
                    )
                    .sized(Size::Fill(1.0), Size::Auto),
                    subject,
                    text(&account.email, 9.0, ColorSpec::MutedText),
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
        label: format!("{}ï¼š{}", message.from, message.subject),
        content: Box::new(content),
    })
    .sized(Size::Fill(1.0), Size::Fixed(62.0));
    row.style.padding = [8.0, 9.0, 8.0, 9.0];
    row.style.normal.background = Some(ColorSpec::Surface);
    row.style.normal.border_color = Some(ColorSpec::Border);
    row.style.normal.border_width = Some(1.0);
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
                "æ”¶ä»¶ç®±æš‚æ—¶ä¸ºç©ºï¼Œç‚¹åˆ·æ–°åŒæ­¥æœåŠ¡å™¨é‚®ä»¶ã€‚"
            } else {
                "æ·»åŠ é‚®ç®±è´¦å·åï¼Œè¿™é‡Œä¼šæ±‡æ€»æ‰€æœ‰æ”¶ä»¶ç®±ã€‚"
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
        return "æ­£åœ¨ä»æœåŠ¡å™¨è¯»å–é‚®ä»¶å¹¶åŒæ­¥å·²è¯»çŠ¶æ€â€¦".into();
    }
    if mail.mailboxes.values().any(|mailbox| mailbox.pending) {
        return "æ­£åœ¨ä¸é‚®ç®±æœåŠ¡å™¨åŒæ­¥â€¦".into();
    }
    let unread = mail
        .mailboxes
        .values()
        .map(|mailbox| mailbox.unread_count)
        .sum::<usize>();
    format!(
        "{} ä¸ªè´¦å· Â· {} å°æœªè¯» Â· IMAP æœåŠ¡å™¨çŠ¶æ€",
        mail.accounts.len(),
        unread
    )
}

pub(super) fn build(mail: &MailExtension) -> WidgetView {
    let title = if mail.selected.is_some() {
        mail.accounts
            .iter()
            .find(|account| Some(&account.id) == mail.selected.as_ref())
            .map_or_else(|| "é‚®ä»¶".to_owned(), |account| account.email.clone())
    } else {
        "å…¨éƒ¨é‚®ä»¶".into()
    };
    let mut actions = vec![ViewNode::subtle_button(
        if mail.mailboxes.values().any(|mailbox| mailbox.pending) {
            "åŒæ­¥ä¸­â€¦"
        } else {
            "åˆ·æ–°"
        },
        Action::Advance,
    )
    .sized(Size::Auto, Size::Fixed(28.0))];
    if mail.selected.is_some() {
        actions.push(button("åˆ é™¤è´¦å·", "remove-account"));
    }
    actions.push(button("ï¼‹ æ·»åŠ ", "add"));
    let top = ViewNode::row(
        vec![
            ViewNode::icon("mail", 18.0)
                .color(ColorSpec::Info)
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

    let mut content = vec![text(&status(mail), 10.0, ColorSpec::SecondaryText)];
    if let Some(error) = &mail.error {
        content.push(text(error, 10.0, ColorSpec::Warning));
    }
    for account in mail
        .accounts
        .iter()
        .filter(|account| mail.selected.as_ref().is_none_or(|id| id == &account.id))m«ëŒ+Š×®º+º$zzb¥à¢°¢–bÆWB6öÖR†W'&÷"’ÒÖ–À¢æÖ–Æ&÷†W0¢ævWB‚f66÷VçBæ–B¢ææE÷F†Vâ‡Æ&÷…÷Â&÷…òæW'&÷"æ5÷&Vb‚’¢°¢6öçFVçBçW6‚‡FW‡B€¢f÷&ÖB‚'·ŞûÉ§·Ò"Â66÷VçBæVÖ–ÂÂW'&÷"’À¢ãÀ¢6öÆ÷%7V3£¥v&æ–ærÀ¢’“°¢Ğ¢Ğ¢–bÖ–ÂæFF–ær°¢6öçFVçBçW6‚†66÷VçEöf÷&Ò†Ö–Â’“°¢Ğ¢–bÖ–Âæ÷Væ–æræ—5öæöæR‚’ÇÂÖ–ÂæFF–ær°¢ÆWBÆ—7BÒf–WtæöFS£¦æWr„æöFS£¥67&öÆÂ€¢67&öÆÄ†W3£¥fW'F–6ÂÀ¢&÷ƒ£¦æWr†6öÇVÖâ†ÖW76vW2†Ö–Â’ÂRã’’À¢’¢ç6—¦VB…6—¦S£¤f–ÆÂƒã’Â6—¦S£¤f–ÆÂƒã’“°¢6öçFVçBçW6‚†Æ—7B“°¢Ğ¢ÆWBÖ–âÒ6öÇVÖâ‡fV2·F÷Â6öÇVÖâ†6öçFVçBÂrã•ÒÂ‚ã¢ç6—¦VB…6—¦S£¤f–ÆÂƒã’Â6—¦S£¤f–ÆÂƒã’¢ç7G–ÆVB…7G–ÆR°¢FF–æs¢³‚ãÂãÂ‚ãÂ"ãÒÀ¢âäFVfVÇC£¦FVfVÇB‚¢Ò“°¢ÆWB&ö÷BÒf–WtæöFS£§&÷r€¢fV2·6–FV&"†Ö–Â’ÂÖ–åÒÀ¢fÆ÷r°¢v¢6öÖRƒã’À¢Æ–vã¢Æ–vã£¥7G&WF6‚À¢âäFVfVÇC£¦FVfVÇB‚¢ÒÀ¢¢ç6—¦VB…6—¦S£¤f–ÆÂƒã’Â6—¦S£¤f–ÆÂƒã’“°¢ÆWB×WBf–WrÒv–FvWEf–Ws£¦æWr‡&ö÷BÂ³s#ãÂCSãÒ“°¢f–Wræ&÷VæG2Òv–FvWD&÷VæG2°¢Ö–ã¢³SãÂ3ãÒÀ¢Öƒ¢µ6öÖRƒ“ã’Â6öÖRƒs#ã•ÒÀ¢&W6—¦&ÆS¢G'VRÀ¢÷fW&fÆ÷s¢÷fW&fÆ÷s£¥67&öÆÂ…67&öÆÄ†W3£¥fW'F–6Â’À¢âäFVfVÇC£¦FVfVÇB‚¢Ó°¢f–Wp§Ğ