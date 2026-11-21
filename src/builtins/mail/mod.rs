mod client;
mod otp;
mod view;

use crate::{
    builtins::rss::reader,
    extension::{
        Action, ContributionDescriptor, ContributionKind, ExtensionContext, ExtensionDescriptor,
        ExtensionId, ExtensionOutput, Notification, NotificationAction,
        NotificationPresentationProvider, NotificationPriority, ReloadPolicy, RuntimeExtension,
        SettingSchema, SettingValue, Settings, WidgetView,
    },
    sdk::{
        secrets::{SecretBackend, SecretStore},
        storage::ExtensionStorage,
        tasks::{CancellationToken, TaskScope},
    },
    ui::declarative::WidgetBounds,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant, SystemTime},
};

const ID: &str = "builtin.mail";
const WIDGET_ID: &str = "builtin.mail/inbox";
const SERVICE_ID: &str = "builtin.mail/service.sync";
const NOTIFICATION_ID: &str = "builtin.mail/updates";
const ACCOUNT_FILE: &str = "accounts.json";
const REFRESH_MINUTES_KEY: &str = "refresh_minutes";
const NOTIFY_NEW_KEY: &str = "notify_new";
const DEFAULT_REFRESH_MINUTES: i64 = 3;
const MAX_MESSAGES: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Account {
    id: String,
    email: String,
    server: String,
    port: u16,
    #[serde(default)]
    last_uid: Option<u32>,
}

#[derive(Clone, Debug)]
struct EmailMessage {
    uid: u32,
    from: String,
    subject: String,
    date: String,
    timestamp: i64,
    is_read: bool,
    verification_code: Option<String>,
}

#[derive(Clone, Debug)]
struct OpenedMessage {
    from: String,
    subject: String,
    date: String,
    body: String,
    unread_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NotificationTarget {
    account_id: String,
    uid: u32,
}

#[derive(Default)]
struct Mailbox {
    messages: Vec<EmailMessage>,
    unread_count: usize,
    pending: bool,
    error: Option<String>,
    credential_warning: Option<String>,
}

enum WorkerRequest {
    Refresh {
        account: Account,
        password: String,
    },
    Open {
        account: Account,
        password: String,
        uid: u32,
    },
    Stop,
}

enum WorkerResult {
    Refreshed {
        account_id: String,
        result: Result<client::MailboxSnapshot, String>,
    },
    Opened {
        account_id: String,
        uid: u32,
        result: Result<OpenedMessage, String>,
    },
}

struct Worker {
    requests: Sender<WorkerRequest>,
    results: Receiver<WorkerResult>,
}

impl Worker {
    fn start(tasks: TaskScope, notifications: Arc<AtomicBool>) -> Result<Self, String> {
        let (requests, incoming) = mpsc::channel();
        let (outgoing, results) = mpsc::channel();
        tasks
            .spawn("mail-imap", move |cancel: CancellationToken| {
                while !cancel.is_cancelled() {
                    let Ok(request) = incoming.recv() else {
                        break;
                    };
                    if cancel.is_cancelled() {
                        break;
                    }
                    let result = match request {
                        WorkerRequest::Refresh { account, password } => {
                            let account_id = account.id.clone();
                            WorkerResult::Refreshed {
                                account_id,
                                result: client::fetch_mailbox(
                                    &account,
                                    &password,
                                    account.last_uid,
                                    &cancel,
                                    &notifications,
                                ),
                            }
                        }
                        WorkerRequest::Open {
                            account,
                            password,
                            uid,
                        } => {
                            let account_id = account.id.clone();
                            WorkerResult::Opened {
                                account_id,
                                uid,
                                result: client::open_message(&account, &password, uid, &cancel),
                            }
                        }
                        WorkerRequest::Stop => break,
                    };
                    if outgoing.send(result).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| format!("无法启动邮件同步：{error}"))?;
        Ok(Self { requests, results })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.requests.send(WorkerRequest::Stop);
    }
}

pub struct MailExtension {
    accounts: Vec<Account>,
    mailboxes: HashMap<String, Mailbox>,
    passwords: HashMap<String, String>,
    selected: Option<String>,
    storage: Option<ExtensionStorage>,
    secrets: Option<SecretStore>,
    worker: Option<Worker>,
    widget_worker: Option<Worker>,
    opening: Option<(String, u32)>,
    reader: Option<reader::Reader>,
    adding: bool,
    email_draft: String,
    server_draft: String,
    port_draft: String,
    password_draft: String,
    error: Option<String>,
    pending_notifications: Vec<Notification>,
    refresh_interval: Duration,
    next_refresh: Instant,
    notify_new: Arc<AtomicBool>,
}

impl Default for MailExtension {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            mailboxes: HashMap::new(),
            passwords: HashMap::new(),
            selected: None,
            storage: None,
            secrets: None,
            worker: None,
            widget_worker: None,
            opening: None,
            reader: None,
            adding: false,
            email_draft: String::new(),
            server_draft: "imap.gmail.com".into(),
            port_draft: "993".into(),
            password_draft: String::new(),
            error: None,
            pending_notifications: Vec::new(),
            refresh_interval: Duration::from_secs(DEFAULT_REFRESH_MINUTES as u64 * 60),
            next_refresh: Instant::now(),
            notify_new: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl NotificationPresentationProvider for MailExtension {
    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == NOTIFICATION_ID
    }

    fn on_notification_action(
        &mut self,
        notification: &Notification,
        action: &NotificationAction,
    ) -> Result<(), String> {
        if !self.handles_notification(&notification.source_id) {
            return Err("未知的邮件通知操作".into());
        }
        if action.id == "copy-code" {
            let code = action
                .data
                .as_deref()
                .filter(|code| !code.is_empty())
                .ok_or("通知中没有验证码")?;
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|error| format!("无法访问系统剪贴板：{error}"))?;
            return clipboard
                .set_text(code.to_owned())
                .map_err(|error| format!("复制验证码失败：{error}"));
        }
        if action.id != "view" {
            return Err("未知的邮件通知操作".into());
        }
        let target: NotificationTarget =
            serde_json::from_str(action.data.as_deref().ok_or("通知中没有邮件信息")?)
                .map_err(|error| error.to_string())?;
        self.queue_open(&target.account_id, target.uid)
    }
}

impl MailExtension {
    fn save_accounts(&self) -> Result<(), String> {
        let storage = self
            .storage
            .as_ref()
            .ok_or_else(|| "邮件设置目录尚未就绪".to_owned())?;
        storage
            .config()
            .write_json(ACCOUNT_FILE, &self.accounts)
            .map_err(|error| error.to_string())
    }

    fn add_account(&mut self) -> Result<(), String> {
        let email = self.email_draft.trim().to_owned();
        if !email.contains('@') || email.chars().any(char::is_whitespace) {
            return Err("请输入有效的邮箱地址".into());
        }
        let server = self.server_draft.trim().to_ascii_lowercase();
        if server.is_empty()
            || server.contains('/')
            || server.contains('@')
            || server.chars().any(char::is_whitespace)
        {
            return Err("请输入有效的 IMAP 服务器地址".into());
        }
        let port = self
            .port_draft
            .trim()
            .parse::<u16>()
            .map_err(|_| "端口需要是 1–65535 之间的数字".to_owned())?;
        let password = self.password_draft.trim().to_owned();
        if password.is_empty() {
            return Err("请输入邮箱密码或应用专用密码".into());
        }
        let id = format!("{}|{}:{}", email.to_ascii_lowercase(), server, port);
        let previous = self
            .accounts
            .iter()
            .find(|account| account.id == id)
            .cloned();
        let secrets = self
            .secrets
            .as_ref()
            .ok_or_else(|| "邮件密钥存储尚未就绪".to_owned())?
            .clone();
        let previous_password = if previous.is_some() {
            secrets
                .get(&id)
                .map_err(|error| format!("无法读取原邮箱密码：{error}"))?
        } else {
            None
        };
        let account = Account {
            id: id.clone(),
            email: email.clone(),
            server,
            port,
            last_uid: previous.as_ref().and_then(|account| account.last_uid),
        };

        let backend = secrets
            .set(&id, &password)
            .map_err(|error| format!("无法保存邮箱密码：{error}"))?;
        if let Some(existing) = self.accounts.iter_mut().find(|existing| existing.id == id) {
            *existing = account.clone();
        } else {
            self.accounts.push(account.clone());
        }
        if let Err(error) = self.save_accounts() {
            if previous.is_none() {
                let _ = secrets.delete(&id);
                self.accounts.retain(|existing| existing.id != id);
            } else if let Some(previous) = previous {
                if let Some(password) = previous_password.as_deref() {
                    let _ = secrets.set(&id, password);
                } else {
                    let _ = secrets.delete(&id);
                }
                *self
                    .accounts
                    .iter_mut()
                    .find(|existing| existing.id == id)
                    .expect("existing account retained") = previous;
            }
            return Err(format!("账号设置保存失败：{error}"));
        }

        self.passwords.insert(id.clone(), password);
        let mailbox = self.mailboxes.entry(id.clone()).or_default();
        mailbox.credential_warning = (backend == SecretBackend::Plaintext)
            .then(|| "邮箱密码以明文保存在当前用户的本地文件中".to_owned());
        self.selected = Some(id.clone());
        self.email_draft.clear();
        self.password_draft.clear();
        self.adding = false;
        self.request_refresh(&account);
        Ok(())
    }

    fn remove_account(&mut self, id: &str) -> Result<(), String> {
        let Some(index) = self.accounts.iter().position(|account| account.id == id) else {
            return Ok(());
        };
        let was_selected = self.selected.as_deref() == Some(id);
        let account = self.accounts.remove(index);
        if let Err(error) = self.save_accounts() {
            self.accounts.insert(index, account);
            return Err(format!("账号删除失败：{error}"));
        }
        self.passwords.remove(id);
        self.mailboxes.remove(id);
        let secret_cleanup_error = self
            .secrets
            .as_ref()
            .and_then(|secrets| secrets.delete(id).err())
            .map(|error| format!("账号已移除，但密码清理失败：{error}"));
        if was_selected {
            self.selected = self.accounts.first().map(|account| account.id.clone());
            if let Some(account) = self
                .selected
                .as_ref()
                .and_then(|id| self.accounts.iter().find(|account| &account.id == id))
                .cloned()
            {
                self.request_refresh(&account);
            }
        }
        secret_cleanup_error.map_or(Ok(()), Err)
    }

    fn request_refresh(&mut self, account: &Account) {
        let Some(mailbox) = self.mailboxes.get_mut(&account.id) else {
            return;
        };
        if mailbox.pending {
            return;
        }
        let Some(password) = self.passwords.get(&account.id).cloned() else {
            mailbox.error = Some("请重新添加账号并输入应用专用密码".into());
            return;
        };
        let Some(worker) = self.worker.as_ref() else {
            mailbox.error = Some("邮件同步服务尚未启动".into());
            return;
        };
        match worker.requests.send(WorkerRequest::Refresh {
            account: account.clone(),
            password,
        }) {
            Ok(()) => {
                mailbox.pending = true;
                mailbox.error = None;
            }
            Err(error) => mailbox.error = Some(format!("无法开始邮件同步：{error}")),
        }
    }

    fn refresh_accounts(&mut self) {
        let accounts = self.accounts.clone();
        for account in &accounts {
            self.request_refresh(account);
        }
    }

    fn refresh_selected(&mut self) {
        let account = self
            .selected
            .as_ref()
            .and_then(|id| self.accounts.iter().find(|account| &account.id == id))
            .cloned();
        if let Some(account) = account {
            self.request_refresh(&account);
        }
    }

    fn queue_open(&mut self, account_id: &str, uid: u32) -> Result<(), String> {
        if self
            .opening
            .as_ref()
            .is_some_and(|(id, pending_uid)| id == account_id && *pending_uid == uid)
        {
            return Ok(());
        }
        let account = self
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
            .ok_or_else(|| "这个邮件账号已经删除".to_owned())?;
        let password = self
            .passwords
            .get(account_id)
            .cloned()
            .ok_or_else(|| "请重新添加账号并输入应用专用密码".to_owned())?;
        let worker = self
            .widget_worker
            .as_ref()
            .ok_or_else(|| "邮件同步服务尚未启动".to_owned())?;
        worker
            .requests
            .send(WorkerRequest::Open {
                account,
                password,
                uid,
            })
            .map_err(|error| format!("无法打开邮件：{error}"))?;
        self.opening = Some((account_id.into(), uid));
        self.error = None;
        Ok(())
    }

    fn target_data(account_id: &str, uid: u32) -> Option<String> {
        serde_json::to_string(&NotificationTarget {
            account_id: account_id.into(),
            uid,
        })
        .ok()
    }

    fn notification(account: &Account, message: &EmailMessage) -> Notification {
        let body = message
            .verification_code
            .as_ref()
            .map(|code| format!("验证码：{code}"))
            .unwrap_or_else(|| message.subject.clone());
        let action = if let Some(code) = &message.verification_code {
            NotificationAction {
                id: "copy-code".into(),
                label: "复制验证码".into(),
                data: Some(code.clone()),
            }
        } else {
            NotificationAction {
                id: "view".into(),
                label: "查看邮件".into(),
                data: Self::target_data(&account.id, message.uid),
            }
        };
        Notification {
            source_id: ExtensionId::builtin(NOTIFICATION_ID),
            id: format!("mail-{}-{}", account.id, message.uid),
            title: format!("{} 的新邮件", message.from),
            body,
            icon: Some("mail".into()),
            priority: NotificationPriority::Normal,
            actions: vec![action],
            created_at: SystemTime::now(),
        }
    }

    fn receive(&mut self, result: WorkerResult) -> bool {
        match result {
            WorkerResult::Refreshed { account_id, result } => {
                let Some(account_index) = self
                    .accounts
                    .iter()
                    .position(|account| account.id == account_id)
                else {
                    return false;
                };
                let Some(mailbox) = self.mailboxes.get_mut(&account_id) else {
                    return false;
                };
                mailbox.pending = false;
                match result {
                    Ok(snapshot) => {
                        let account = &mut self.accounts[account_index];
                        account.last_uid = snapshot.next_uid;
                        let mut merged: HashMap<u32, EmailMessage> = mailbox
                            .messages
                            .drain(..)
                            .map(|message| (message.uid, message))
                            .collect();
                        for message in snapshot.messages {
                            merged.insert(message.uid, message);
                        }
                        mailbox.messages = merged.into_values().collect();
                        mailbox.messages.sort_by_key(|message| {
                            (
                                std::cmp::Reverse(message.timestamp),
                                std::cmp::Reverse(message.uid),
                            )
                        });
                        mailbox.messages.truncate(MAX_MESSAGES);
                        mailbox.unread_count = snapshot.unread_count;
                        mailbox.error = None;
                        let notifications = if self.notify_new.load(Ordering::Relaxed) {
                            snapshot
                                .new_messages
                                .iter()
                                .map(|message| Self::notification(account, message))
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        };
                        self.pending_notifications.extend(notifications);
                        if self.save_accounts().is_err() {
                            self.error = Some("邮件同步游标保存失败".into());
                        }
                    }
                    Err(error) => mailbox.error = Some(error),
                }
                true
            }
            WorkerResult::Opened {
                account_id,
                uid,
                result,
            } => {
                if self
                    .opening
                    .as_ref()
                    .is_some_and(|(id, open_uid)| id == &account_id && *open_uid == uid)
                {
                    self.opening = None;
                }
                match result {
                    Ok(opened) => {
                        if let Some(mailbox) = self.mailboxes.get_mut(&account_id) {
                            if let Some(message) = mailbox
                                .messages
                                .iter_mut()
                                .find(|message| message.uid == uid)
                            {
                                message.is_read = true;
                            }
                            mailbox.unread_count = opened.unread_count;
                            mailbox.error = None;
                        }
                        match self.show_message(opened) {
                            Ok(()) => self.error = None,
                            Err(error) => self.error = Some(error),
                        }
                    }
                    Err(error) => {
                        if let Some(mailbox) = self.mailboxes.get_mut(&account_id) {
                            mailbox.error = Some(error.clone());
                        }
                        self.error = Some(error);
                    }
                }
                true
            }
        }
    }

    fn show_message(&mut self, message: OpenedMessage) -> Result<(), String> {
        if self.reader.as_ref().is_none_or(reader::Reader::is_closed) {
            self.reader = None;
            self.reader = Some(reader::Reader::new()?);
        }
        let body = ammonia::Builder::default()
            .rm_tags(&["img", "iframe", "video", "audio", "form", "input", "style"])
            .clean(&message.body)
            .to_string();
        let html = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>body{{font:16px/1.65 system-ui,-apple-system,sans-serif;color:#202124;background:#fff;margin:0;padding:36px;max-width:900px}}header{{border-bottom:1px solid #ddd;padding-bottom:18px;margin-bottom:26px}}h1{{font-size:24px;line-height:1.3;margin:0 0 14px}}.meta{{color:#687078;font-size:14px}}pre{{white-space:pre-wrap;word-break:break-word}}</style></head><body><header><h1>{}</h1><div class=\"meta\">发件人：{}<br>{}</div></header><main>{}</main></body></html>",
            escape_html(&message.subject),
            escape_html(&message.subject),
            escape_html(&message.from),
            escape_html(&message.date),
            body
        );
        self.reader
            .as_ref()
            .expect("reader initialized")
            .show_html(&message.subject, &html)
    }

    fn set_provider(&mut self, host: &str) {
        self.server_draft = host.into();
        self.port_draft = "993".into();
    }

    fn invalidated(&self) -> ExtensionOutput {
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            ..Default::default()
        }
    }

    fn configure(&mut self, settings: &Settings) {
        if let Some(SettingValue::Integer(minutes)) = settings.get(REFRESH_MINUTES_KEY) {
            self.refresh_interval = Duration::from_secs((*minutes).clamp(1, 60) as u64 * 60);
            self.next_refresh = Instant::now() + self.refresh_interval;
        }
    }

    fn poll_worker(&mut self) -> bool {
        let results: Vec<_> = self
            .worker
            .iter()
            .chain(&self.widget_worker)
            .flat_map(|worker| worker.results.try_iter())
            .collect();
        let mut changed = false;
        for result in results {
            changed |= self.receive(result);
        }
        changed
    }

    fn load_accounts(
        &mut self,
        storage: ExtensionStorage,
        secrets: SecretStore,
    ) -> Result<(), String> {
        let configured = storage
            .config()
            .read_json(ACCOUNT_FILE)
            .map_err(|error| error.to_string())?;
        self.accounts = configured.unwrap_or_default();
        self.selected = self.accounts.first().map(|account| account.id.clone());
        self.mailboxes.clear();
        self.passwords.clear();
        for account in &self.accounts {
            let mailbox = self.mailboxes.entry(account.id.clone()).or_default();
            match secrets.get(&account.id) {
                Ok(Some(password)) => {
                    self.passwords.insert(account.id.clone(), password);
                    if secrets.backend(&account.id).ok().flatten() == Some(SecretBackend::Plaintext)
                    {
                        mailbox.credential_warning =
                            Some("邮箱密码以明文保存在当前用户的本地文件中".into());
                    }
                }
                Ok(None) => {
                    mailbox.error = Some("本地 Secret 文件中没有该账号密码，请重新添加账号".into())
                }
                Err(error) => mailbox.error = Some(format!("读取邮箱密码失败：{error}")),
            }
        }
        self.storage = Some(storage);
        self.secrets = Some(secrets);
        Ok(())
    }
}

impl RuntimeExtension for MailExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        let widget = ContributionDescriptor::builtin(
            WIDGET_ID,
            ContributionKind::Widget,
            "邮件",
            "mail",
            true,
            9,
        )
        .with_size_hint(720.0, 450.0)
        .with_widget_bounds(WidgetBounds {
            min: [500.0, 300.0],
            max: [Some(900.0), Some(720.0)],
            resizable: true,
            ..Default::default()
        });
        let notifications = ContributionDescriptor::builtin(
            NOTIFICATION_ID,
            ContributionKind::Notification,
            "新邮件提醒",
            "mail",
            true,
            9,
        )
        .with_capabilities(vec![
            crate::extension::Capability::Event,
            crate::extension::Capability::Action,
        ]);
        let mut descriptor = ExtensionDescriptor::builtin(
            ID,
            "邮件",
            "IMAP 邮箱分账号阅读、已读同步与新邮件提醒",
            vec![
                widget
                    .with_dependency(ExtensionId::builtin(SERVICE_ID))
                    .with_execution_policy(crate::extension::ExecutionPolicy::Interval(
                        Duration::from_millis(250),
                    )),
                notifications.with_dependency(ExtensionId::builtin(SERVICE_ID)),
                ContributionDescriptor::builtin(
                    SERVICE_ID,
                    ContributionKind::BackgroundService,
                    "IMAP 同步",
                    "mail",
                    true,
                    0,
                )
                .with_execution_policy(
                    crate::extension::ExecutionPolicy::Interval(Duration::from_millis(250)),
                ),
            ],
        );
        descriptor.execution_policy = crate::extension::ExecutionPolicy::EventDriven;
        descriptor.settings_version = 2;
        descriptor.settings = vec![SettingSchema::new(
            REFRESH_MINUTES_KEY,
            SettingValue::Integer(DEFAULT_REFRESH_MINUTES),
            "邮件刷新间隔（分钟）",
        )
        .range(1.0, 60.0)];
        descriptor
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        let storage = ctx.sdk().storage();
        let secrets = ctx.sdk().secrets();
        self.load_accounts(storage, secrets)?;
        self.next_refresh = Instant::now();
        Ok(())
    }

    fn migrate_settings(
        &self,
        _: &ExtensionId,
        old_version: u32,
        mut values: Settings,
    ) -> Result<Settings, String> {
        if old_version != 1 {
            return Err(format!("unsupported mail settings version {old_version}"));
        }
        values.remove(NOTIFY_NEW_KEY);
        Ok(values)
    }

    fn contribution_preference_migration(
        &self,
        values: &Settings,
    ) -> std::collections::BTreeMap<ExtensionId, bool> {
        if let Some(SettingValue::Boolean(enabled)) = values.get(NOTIFY_NEW_KEY) {
            return [(ExtensionId::builtin(NOTIFICATION_ID), *enabled)].into();
        }
        Default::default()
    }

    fn on_contribution_start(
        &mut self,
        ctx: &ExtensionContext,
        id: &ExtensionId,
    ) -> Result<(), String> {
        match id.as_str() {
            SERVICE_ID => {
                self.worker = Some(Worker::start(ctx.tasks(), self.notify_new.clone())?);
                self.next_refresh = Instant::now();
            }
            WIDGET_ID => {
                self.widget_worker = Some(Worker::start(ctx.tasks(), self.notify_new.clone())?)
            }
            _ => {}
        }
        Ok(())
    }

    fn on_contribution_stop(
        &mut self,
        _: &ExtensionContext,
        id: &ExtensionId,
    ) -> Result<(), String> {
        match id.as_str() {
            SERVICE_ID => {
                self.worker = None;
                for mailbox in self.mailboxes.values_mut() {
                    mailbox.pending = false;
                }
            }
            WIDGET_ID => {
                self.widget_worker = None;
                self.reader = None;
                self.opening = None;
            }
            NOTIFICATION_ID => self.pending_notifications.clear(),
            _ => {}
        }
        Ok(())
    }

    fn on_contribution_state(
        &mut self,
        _: &ExtensionContext,
        active: &std::collections::BTreeMap<ExtensionId, Settings>,
    ) {
        self.notify_new.store(
            active.contains_key(&ExtensionId::builtin(NOTIFICATION_ID)),
            Ordering::Relaxed,
        );
    }

    fn widget_view(&self, id: &ExtensionId) -> Option<WidgetView> {
        (id.as_str() == WIDGET_ID).then(|| view::build(self))
    }

    fn on_contribution_update(
        &mut self,
        _: &ExtensionContext,
        id: &ExtensionId,
        now: Instant,
    ) -> ExtensionOutput {
        let mut changed = self.poll_worker();
        if id.as_str() == SERVICE_ID && now >= self.next_refresh {
            self.refresh_accounts();
            self.next_refresh = now + self.refresh_interval;
            changed |= !self.accounts.is_empty();
        }
        if changed {
            let mut output = self.invalidated();
            output.notifications = std::mem::take(&mut self.pending_notifications);
            output
        } else {
            ExtensionOutput::default()
        }
    }

    fn on_action(&mut self, action: Action, now: Instant) -> ExtensionOutput {
        self.error = None;
        match action {
            Action::SetText { key, value } => match key.as_str() {
                "mail-email" => self.email_draft = value,
                "mail-server" => self.server_draft = value,
                "mail-port" => self.port_draft = value,
                "mail-password" => self.password_draft = value,
                _ => return ExtensionOutput::default(),
            },
            Action::Select(id) if self.accounts.iter().any(|account| account.id == id) => {
                self.selected = Some(id.clone());
                if let Some(account) = self
                    .accounts
                    .iter()
                    .find(|account| account.id == id)
                    .cloned()
                {
                    self.request_refresh(&account);
                }
            }
            Action::Select(provider) if provider.starts_with("provider:") => {
                match &provider[9..] {
                    "gmail" => self.set_provider("imap.gmail.com"),
                    "outlook" => self.set_provider("outlook.office365.com"),
                    "yahoo" => self.set_provider("imap.mail.yahoo.com"),
                    "icloud" => self.set_provider("imap.mail.me.com"),
                    "qq" => self.set_provider("imap.qq.com"),
                    "163" => self.set_provider("imap.163.com"),
                    "126" => self.set_provider("imap.126.com"),
                    "custom" => {
                        self.server_draft.clear();
                        self.port_draft = "993".into();
                    }
                    _ => {}
                }
                self.adding = true;
            }
            Action::Advance => {
                self.refresh_selected();
                self.next_refresh = now + self.refresh_interval;
            }
            Action::Custom(key) if key == "add" => self.adding = true,
            Action::Custom(key) if key == "cancel" => {
                self.adding = false;
                self.password_draft.clear();
            }
            Action::Custom(key) if key == "save-account" => {
                self.error = self.add_account().err();
            }
            Action::Custom(key) if key.starts_with("remove-account:") => {
                self.error = self.remove_account(&key["remove-account:".len()..]).err();
            }
            Action::Custom(key) if key.starts_with("open:") => {
                match serde_json::from_str::<NotificationTarget>(&key[5..]) {
                    Ok(target) => {
                        self.error = self.queue_open(&target.account_id, target.uid).err()
                    }
                    Err(error) => self.error = Some(format!("邮件信息无效：{error}")),
                }
            }
            _ => return ExtensionOutput::default(),
        }
        self.invalidated()
    }

    fn on_settings_changed(
        &mut self,
        _: &ExtensionContext,
        _: &ExtensionId,
        settings: &Settings,
        _: ReloadPolicy,
    ) -> ExtensionOutput {
        self.configure(settings);
        self.invalidated()
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
