use super::{Account, EmailMessage, OpenedMessage};
use imap::{types::Flag, Session};
use mailparse::{MailHeaderMap, ParsedMail};
use native_tls::{TlsConnector, TlsStream};
use std::{
    collections::{BTreeSet, HashMap},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

const MESSAGE_WINDOW: usize = 80;
const NEW_MESSAGE_BATCH: usize = 100;
// Keep UID sets short for servers that truncate long FETCH commands.
const UID_FETCH_BATCH: usize = 8;
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_OTP_SCAN_BYTES: usize = 20 * 1024 * 1024;

pub(super) struct MailboxSnapshot {
    pub messages: Vec<EmailMessage>,
    pub new_messages: Vec<EmailMessage>,
    pub unread_count: usize,
    pub next_uid: Option<u32>,
}

fn connect(account: &Account, password: &str) -> Result<Session<TlsStream<TcpStream>>, String> {
    let tls = TlsConnector::builder()
        .build()
        .map_err(|error| format!("无法初始化 TLS：{error}"))?;
    let address = (account.server.as_str(), account.port)
        .to_socket_addrs()
        .map_err(|error| format!("解析 IMAP 服务器失败：{error}"))?
        .next()
        .ok_or_else(|| "无法解析 IMAP 服务器地址".to_owned())?;
    let socket = TcpStream::connect_timeout(&address, Duration::from_secs(15))
        .map_err(|error| format!("连接 IMAP 服务器失败：{error}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(20)))
        .map_err(|error| error.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(20)))
        .map_err(|error| error.to_string())?;
    let stream = tls
        .connect(&account.server, socket)
        .map_err(|error| format!("建立 IMAP 加密连接失败：{error}"))?;
    let mut client = imap::Client::new(stream);
    client
        .read_greeting()
        .map_err(|error| format!("IMAP 服务器未返回欢迎信息：{error}"))?;
    client
        .login(&account.email, password)
        .map_err(|(error, _client)| format!("IMAP 登录失败：{error}"))
}

pub(super) fn fetch_mailbox(
    account: &Account,
    password: &str,
    last_uid: Option<u32>,
) -> Result<MailboxSnapshot, String> {
    let mut session = connect(account, password)?;
    let result = (|| {
        session
            .select("INBOX")
            .map_err(|error| format!("打开收件箱失败：{error}"))?;
        let all_uids = session
            .uid_search("ALL")
            .map_err(|error| format!("读取邮件列表失败：{error}"))?;
        let unread_count = session
            .uid_search("UNSEEN")
            .map_err(|error| format!("读取未读数量失败：{error}"))?
            .len();

        let mut sorted_uids: Vec<u32> = all_uids.into_iter().collect();
        sorted_uids.sort_unstable();
        let highest_uid = sorted_uids.last().copied();
        let mut selected = BTreeSet::new();
        selected.extend(sorted_uids.iter().rev().take(MESSAGE_WINDOW).copied());
        let new_uids: Vec<u32> = last_uid.map_or_else(Vec::new, |last| {
            sorted_uids
                .iter()
                .copied()
                .filter(|uid| *uid > last)
                .take(NEW_MESSAGE_BATCH)
                .collect()
        });
        selected.extend(new_uids.iter().copied());

        let mut messages = Vec::new();
        let mut message_sizes = HashMap::new();
        if !selected.is_empty() {
            let selected_uids: Vec<_> = selected.into_iter().collect();
            for chunk in selected_uids.chunks(UID_FETCH_BATCH) {
                let uid_set = chunk
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let fetched = session
                    .uid_fetch(
                        uid_set,
                        "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])",
                    )
                    .map_err(|error| format!("读取邮件标题失败：{error}"))?;
                for item in fetched.iter() {
                    let (Some(uid), Some(header)) = (item.uid, item.header()) else {
                        continue;
                    };
                    if let Some(message) = parse_headers(uid, header, item.flags()) {
                        message_sizes.insert(uid, item.size.map(|size| size as usize));
                        messages.push(message);
                    }
                }
            }
        }

        // Inspect only new messages. BODY.PEEK leaves the server's unread flag
        // untouched, just like the header-only inbox refresh above.
        let mut scan_bytes = 0usize;
        let mut otp_uids = Vec::new();
        for uid in new_uids.iter().rev() {
            let Some(size) = message_sizes.get(uid).copied().flatten() else {
                continue;
            };
            if size > MAX_BODY_BYTES || scan_bytes.saturating_add(size) > MAX_OTP_SCAN_BYTES {
                continue;
            }
            scan_bytes += size;
            otp_uids.push(*uid);
        }
        if !otp_uids.is_empty() {
            for chunk in otp_uids.chunks(UID_FETCH_BATCH) {
                let uid_set = chunk
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let Ok(fetched) = session.uid_fetch(uid_set, "(UID BODY.PEEK[])") else {
                    continue;
                };
                for item in fetched.iter() {
                    let (Some(uid), Some(raw)) = (item.uid, item.body()) else {
                        continue;
                    };
                    if raw.len() > MAX_BODY_BYTES {
                        continue;
                    }
                    let Some(code) = detect_verification_code(raw) else {
                        continue;
                    };
                    if let Some(message) = messages.iter_mut().find(|message| message.uid == uid) {
                        message.verification_code = Some(code);
                    }
                }
            }
        }
        messages.sort_by_key(|message| std::cmp::Reverse(message.uid));
        let new_messages = messages
            .iter()
            .filter(|message| last_uid.is_some_and(|last| message.uid > last))
            .cloned()
            .collect();
        let next_uid = if last_uid.is_none() {
            highest_uid
        } else {
            new_uids.last().copied().or(highest_uid)
        };
        Ok(MailboxSnapshot {
            messages,
            new_messages,
            unread_count,
            next_uid,
        })
    })();
    let _ = session.logout();
    result
}

pub(super) fn open_message(
    account: &Account,
    password: &str,
    uid: u32,
) -> Result<OpenedMessage, String> {
    let mut session = connect(account, password)?;
    let result = (|| {
        session
            .select("INBOX")
            .map_err(|error| format!("打开收件箱失败：{error}"))?;
        let fetched = session
            .uid_fetch(uid.to_string(), "(UID FLAGS INTERNALDATE BODY.PEEK[])")
            .map_err(|error| format!("读取邮件正文失败：{error}"))?;
        let item = fetched
            .iter()
            .find(|item| item.uid == Some(uid))
            .ok_or_else(|| "服务器上已找不到这封邮件".to_owned())?;
        let raw = item
            .body()
            .ok_or_else(|| "服务器没有返回邮件正文".to_owned())?;
        if raw.len() > MAX_BODY_BYTES {
            return Err("邮件正文超过 4 MB，暂不打开".into());
        }
        let parsed =
            mailparse::parse_mail(raw).map_err(|error| format!("邮件格式无法读取：{error}"))?;
        let subject = parsed
            .headers
            .get_first_value("Subject")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "无主题".into());
        let from = parsed
            .headers
            .get_first_value("From")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "未知发件人".into());
        let date = parsed
            .headers
            .get_first_value("Date")
            .unwrap_or_else(|| "".into());
        let body = extract_body(&parsed);

        session
            .uid_store(uid.to_string(), "+FLAGS (\\Seen)")
            .map_err(|error| format!("在服务器上标记已读失败：{error}"))?;
        let unread_count = session
            .uid_search("UNSEEN")
            .map_err(|error| format!("已读状态已同步，但无法读取未读数量：{error}"))?
            .len();
        Ok(OpenedMessage {
            from,
            subject,
            date,
            body,
            unread_count,
        })
    })();
    let _ = session.logout();
    result
}

fn parse_headers(uid: u32, bytes: &[u8], flags: &[Flag<'_>]) -> Option<EmailMessage> {
    let parsed = mailparse::parse_mail(bytes).ok()?;
    let subject = parsed
        .headers
        .get_first_value("Subject")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "无主题".into());
    let from = parsed
        .headers
        .get_first_value("From")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "未知发件人".into());
    let date = parsed.headers.get_first_value("Date").unwrap_or_default();
    let timestamp = mailparse::dateparse(&date).unwrap_or_default();
    Some(EmailMessage {
        uid,
        from,
        subject,
        date,
        timestamp,
        is_read: flags.iter().any(|flag| matches!(flag, Flag::Seen)),
        verification_code: None,
    })
}

fn detect_verification_code(raw: &[u8]) -> Option<String> {
    let parsed = mailparse::parse_mail(raw).ok()?;
    let subject = parsed
        .headers
        .get_first_value("Subject")
        .unwrap_or_default();
    let from = parsed.headers.get_first_value("From").unwrap_or_default();
    let mut text = String::new();
    let mut html = String::new();
    collect_otp_content(&parsed, &mut text, &mut html);
    if text.trim().is_empty() && !html.trim().is_empty() {
        text = super::otp::html_to_text(&html);
    }
    let input = super::otp::Input {
        subject: &subject,
        text: &text,
        html: (!html.is_empty()).then_some(html.as_str()),
        from: &from,
        promotional: false,
    };
    // Detection is best-effort; a malformed email must not terminate the IMAP worker.
    let detection = match std::panic::catch_unwind(|| super::otp::detect(&input)) {
        Ok(Some(detection)) => detection,
        _ => return None,
    };
    (detection.kind == super::otp::Kind::Code
        && detection.confidence >= super::otp::TOAST_THRESHOLD)
        .then_some(detection.value)
}

fn collect_otp_content(part: &ParsedMail<'_>, text: &mut String, html: &mut String) {
    if part
        .headers
        .get_first_value("Content-Disposition")
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("attachment"))
    {
        return;
    }
    if !part.subparts.is_empty() {
        for child in &part.subparts {
            collect_otp_content(child, text, html);
        }
        return;
    }
    if !matches!(part.ctype.mimetype.as_str(), "text/plain" | "text/html") {
        return;
    }
    let Ok(body) = part.get_body() else {
        return;
    };
    if part.ctype.mimetype == "text/html" {
        html.push_str(&body);
        html.push('\n');
    } else {
        text.push_str(&body);
        text.push('\n');
    }
}

fn extract_body(message: &ParsedMail<'_>) -> String {
    let mut html = None;
    let mut plain = None;
    collect_body(message, &mut html, &mut plain);
    html.or(plain.map(|text| format!("<pre>{}</pre>", escape_html(&text))))
        .unwrap_or_else(|| "<p>这封邮件没有可显示的正文。</p>".into())
}

fn collect_body(part: &ParsedMail<'_>, html: &mut Option<String>, plain: &mut Option<String>) {
    if part
        .headers
        .get_first_value("Content-Disposition")
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("attachment"))
    {
        return;
    }
    if !part.subparts.is_empty() {
        for child in &part.subparts {
            collect_body(child, html, plain);
        }
        return;
    }
    let Ok(body) = part.get_body() else {
        return;
    };
    if body.len() > MAX_BODY_BYTES {
        return;
    }
    match part.ctype.mimetype.as_str() {
        "text/html" => *html = Some(body),
        "text/plain" if plain.is_none() => *plain = Some(body),
        _ => {}
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
