use super::{model::BilibiliEvent, poller, state::SyncState};
use crate::{
    core::external::opencli::OpenCli,
    extension::{
        ExtensionContext, ExtensionDescriptor, ExtensionId, ExtensionKind, ExtensionOutput,
        Notification, NotificationAction, NotificationAlertView, NotificationCenterItemView,
        NotificationPresentationProvider, NotificationPriority, RuntimeExtension,
    },
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

pub const NOTIFICATION_ID: &str = "builtin.bilibili/notifications";
const POLL_SECS: u64 = 60;

enum PollMessage {
    Events(Vec<BilibiliEvent>, mpsc::Sender<String>),
    Checked(String),
    Error(String),
}

pub struct BilibiliExtension {
    receiver: Option<Receiver<PollMessage>>,
    stop: Option<Arc<AtomicBool>>,
    worker: Option<JoinHandle<()>>,
    check_sender: Option<mpsc::Sender<()>>,
    last_error: Option<String>,
    current_account: Option<String>,
    last_check: Option<SystemTime>,
    opencli_path: String,
    dm_enabled: bool,
    likes_enabled: bool,
    show_dm_summary: bool,
    poll_seconds: u16,
    enabled: bool,
    pending_commits: HashMap<String, mpsc::Sender<String>>,
}

impl Default for BilibiliExtension {
    fn default() -> Self {
        Self {
            receiver: None,
            stop: None,
            worker: None,
            check_sender: None,
            last_error: None,
            current_account: None,
            last_check: None,
            opencli_path: String::new(),
            dm_enabled: true,
            likes_enabled: true,
            show_dm_summary: true,
            poll_seconds: 60,
            enabled: false,
            pending_commits: HashMap::new(),
        }
    }
}

impl BilibiliExtension {
    fn prepare_plugin(&self, ctx: &ExtensionContext) -> Result<(), String> {
        let root = ctx.extension_data_dir(&ExtensionId::builtin(NOTIFICATION_ID));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        poller::copy_plugin(&root.join("plugin"))
    }

    fn begin_polling(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        if self.worker.is_some() {
            return Ok(());
        }
        let root = ctx.extension_data_dir(&ExtensionId::builtin(NOTIFICATION_ID));
        let (sender, receiver) = mpsc::channel();
        let (check_sender, check_receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let state_path = root.join("sync-state.tsv");
        let configured =
            (!self.opencli_path.trim().is_empty()).then(|| PathBuf::from(self.opencli_path.trim()));
        let dm_enabled = self.dm_enabled;
        let likes_enabled = self.likes_enabled;
        let show_dm_summary = self.show_dm_summary;
        let poll_seconds = self.poll_seconds.clamp(15, 3600);
        let worker = thread::spawn(move || {
            let mut state = SyncState::load(&state_path);
            let mut backoff = Duration::from_secs(POLL_SECS);
            let cli = match OpenCli::resolve(configured.as_deref()) {
                Ok(cli) => cli,
                Err(error) => {
                    let _ = sender.send(PollMessage::Error(error));
                    return;
                }
            };
            if let Err(error) = ensure_opencli_plugin(&cli, &root) {
                let _ = sender.send(PollMessage::Error(error));
                return;
            }
            while !worker_stop.load(Ordering::Relaxed) {
                let pass = poller::account(&cli).and_then(|account| {
                    if state.account != account {
                        state.account = account.clone();
                        state.seen.clear();
                        state.initialized = false;
                    }
                    let events = poller::collect(&cli, &account, dm_enabled, likes_enabled)?;
                    let was_initialized = state.initialized;
                    let mut fresh = if was_initialized {
                        events
                            .into_iter()
                            .filter(|event| !state.seen.contains(&event.id))
                            .collect()
                    } else {
                        state.seen.extend(events.into_iter().map(|event| event.id));
                        Vec::new()
                    };
                    if !show_dm_summary {
                        for event in &mut fresh {
                            if event.kind == super::model::EventKind::Message {
                                event.body = "收到一条新私信".into();
                            }
                        }
                    }
                    state.initialized = true;
                    Ok((account, fresh, was_initialized))
                });
                match pass {
                    Ok((account, events, was_initialized)) => {
                        backoff = Duration::from_secs(u64::from(poll_seconds));
                        if sender.send(PollMessage::Checked(account)).is_err() {
                            break;
                        }
                        if was_initialized {
                            if !events.is_empty() {
                                let ids: HashSet<_> =
                                    events.iter().map(|event| event.id.clone()).collect();
                                let (ack_sender, ack_receiver) = mpsc::channel();
                                if sender
                                    .send(PollMessage::Events(events, ack_sender))
                                    .is_err()
                                {
                                    break;
                                }
                                let deadline = std::time::Instant::now() + Duration::from_secs(30);
                                let mut acknowledged = HashSet::new();
                                while acknowledged.len() < ids.len()
                                    && std::time::Instant::now() < deadline
                                    && !worker_stop.load(Ordering::Relaxed)
                                {
                                    if let Ok(id) =
                                        ack_receiver.recv_timeout(Duration::from_millis(250))
                                    {
                                        if ids.contains(&id) {
                                            acknowledged.insert(id);
                                        }
                                    }
                                }
                                if acknowledged.len() == ids.len() {
                                    state.seen.extend(ids);
                                    if let Err(error) = state.save(&state_path) {
                                        let _ = sender.send(PollMessage::Error(format!(
                                            "unable to persist Bilibili sync state: {error}"
                                        )));
                                    }
                                }
                            }
                        } else if let Err(error) = state.save(&state_path) {
                            let _ = sender.send(PollMessage::Error(format!(
                                "unable to persist Bilibili sync state: {error}"
                            )));
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(PollMessage::Error(error));
                        backoff = (backoff * 2).min(Duration::from_secs(600));
                    }
                }
                let end = std::time::Instant::now() + backoff;
                while std::time::Instant::now() < end && !worker_stop.load(Ordering::Relaxed) {
                    if check_receiver
                        .recv_timeout(Duration::from_millis(200))
                        .is_ok()
                    {
                        break;
                    }
                }
            }
        });
        self.receiver = Some(receiver);
        self.check_sender = Some(check_sender);
        self.stop = Some(stop);
        self.worker = Some(worker);
        Ok(())
    }
}

impl RuntimeExtension for BilibiliExtension {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        vec![ExtensionDescriptor::builtin(
            NOTIFICATION_ID,
            ExtensionKind::Notification,
            "Bilibili 通知",
            "bell",
            false,
            20,
        )
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_setup(true)]
    }
    fn on_install(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.prepare_plugin(ctx)
    }
    fn on_upgrade(&mut self, ctx: &ExtensionContext, _old_version: &str) -> Result<(), String> {
        self.prepare_plugin(ctx)
    }
    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.enabled = true;
        self.begin_polling(ctx)
    }
    fn configure_bilibili(
        &mut self,
        _ctx: &ExtensionContext,
        opencli_path: &str,
        dm_enabled: bool,
        likes_enabled: bool,
        show_dm_summary: bool,
        poll_seconds: u16,
    ) -> Result<(), String> {
        let changed = self.opencli_path != opencli_path
            || self.dm_enabled != dm_enabled
            || self.likes_enabled != likes_enabled
            || self.show_dm_summary != show_dm_summary
            || self.poll_seconds != poll_seconds;
        if !changed {
            return Ok(());
        }
        self.opencli_path = opencli_path.trim().to_owned();
        self.dm_enabled = dm_enabled;
        self.likes_enabled = likes_enabled;
        self.show_dm_summary = show_dm_summary;
        self.poll_seconds = poll_seconds.clamp(15, 3600);
        // The worker snapshots its settings when it starts. Applying changed
        // CLI options takes effect on the next start without blocking egui.
        Ok(())
    }
    fn on_stop(&mut self, _ctx: &ExtensionContext) -> Result<(), String> {
        self.enabled = false;
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        // The worker checks this flag between bounded CLI calls. Dropping the
        // handle detaches it so a settings click never waits for a subprocess.
        drop(self.worker.take());
        self.receiver = None;
        self.check_sender = None;
        Ok(())
    }
    fn on_uninstall(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.on_stop(ctx)?;
        let root = ctx.extension_data_dir(&ExtensionId::builtin(NOTIFICATION_ID));
        let marker = root.join("managed-plugin");
        if marker.is_file() {
            let configured = (!self.opencli_path.trim().is_empty())
                .then(|| PathBuf::from(self.opencli_path.trim()));
            let cli = OpenCli::resolve(configured.as_deref())?;
            let output = cli.run(
                &["plugin", "uninstall", "peninsula-bilibili"],
                Duration::from_secs(30),
            )?;
            if output.status != 0 {
                return Err(format!(
                    "unable to uninstall Peninsula plugin: {}",
                    output.stderr.trim()
                ));
            }
        }
        fs::remove_dir_all(root).map_err(|error| error.to_string())
    }
    fn update(&mut self, _now: std::time::Instant) -> ExtensionOutput {
        let Some(receiver) = self.receiver.as_ref() else {
            return ExtensionOutput::default();
        };
        let mut messages = Vec::new();
        while let Ok(message) = receiver.try_recv() {
            messages.push(message);
        }
        let mut notifications = Vec::new();
        for message in messages {
            match message {
                PollMessage::Checked(account) => {
                    self.current_account = Some(account);
                    self.last_check = Some(SystemTime::now());
                    self.last_error = None;
                }
                PollMessage::Events(events, ack_sender) => {
                    for event in events {
                        let notification = to_notification(event);
                        self.pending_commits
                            .insert(notification.id.clone(), ack_sender.clone());
                        notifications.push(notification);
                    }
                }
                PollMessage::Error(error) => {
                    if self.last_error.as_deref() != Some(&error) {
                        eprintln!("Bilibili extension: {error}");
                    }
                    self.last_error = Some(error);
                }
            }
        }
        ExtensionOutput {
            notifications,
            ..ExtensionOutput::default()
        }
    }

    fn status_message(&self) -> Option<String> {
        if let Some(error) = &self.last_error {
            let short = error.chars().take(36).collect::<String>();
            return Some(format!("连接异常：{short}"));
        }
        if let Some(account) = &self.current_account {
            let age = self
                .last_check
                .map(|time| {
                    SystemTime::now()
                        .duration_since(time)
                        .unwrap_or_default()
                        .as_secs()
                })
                .unwrap_or_default();
            return Some(format!("已连接账号 {account} · {age} 秒前检查"));
        }
        Some(if self.worker.is_some() {
            "正在连接 OpenCLI 与 Bilibili…".into()
        } else {
            "尚未启用".into()
        })
    }

    fn on_notifications_committed(&mut self, notifications: &[Notification]) {
        for notification in notifications {
            if notification.source_id.as_str() != NOTIFICATION_ID {
                continue;
            }
            if let Some(sender) = self.pending_commits.remove(&notification.id) {
                let _ = sender.send(notification.id.clone());
            }
        }
    }

    fn request_bilibili_check(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        if !self.enabled {
            return Err("Bilibili notifications are not enabled".into());
        }
        if self.worker.is_some() {
            self.on_stop(ctx)?;
            self.enabled = true;
            self.begin_polling(ctx)?;
        }
        self.check_sender
            .as_ref()
            .ok_or_else(|| "Bilibili notifications are not enabled".to_owned())?
            .send(())
            .map_err(|_| "Bilibili poller is no longer running".to_owned())
    }
}

impl NotificationPresentationProvider for BilibiliExtension {
    fn alert_view(&self, notification: &Notification) -> NotificationAlertView {
        NotificationAlertView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("bell".into()),
            actions: notification.actions.clone(),
        }
    }
    fn center_item_view(&self, notification: &Notification) -> NotificationCenterItemView {
        NotificationCenterItemView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("bell".into()),
        }
    }
    fn on_notification_action(
        &mut self,
        _notification: &Notification,
        action: &NotificationAction,
    ) -> Result<(), String> {
        if action.id != "view" {
            return Ok(());
        }
        let Some(url) = action
            .data
            .as_deref()
            .filter(|url| url.starts_with("https://"))
        else {
            return Err("Bilibili notification did not contain a valid HTTPS URL".into());
        };
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(url).status();
        #[cfg(target_os = "linux")]
        let result = std::process::Command::new("xdg-open").arg(url).status();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status();
        result
            .map(|_| ())
            .map_err(|error| format!("unable to open Bilibili URL: {error}"))
    }
    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == NOTIFICATION_ID
    }
}

fn to_notification(event: BilibiliEvent) -> Notification {
    Notification {
        source_id: ExtensionId::builtin(NOTIFICATION_ID),
        id: event.id,
        title: event.title,
        body: event.body,
        icon: Some("bell".into()),
        priority: NotificationPriority::Normal,
        actions: vec![NotificationAction {
            id: "view".into(),
            label: "查看".into(),
            data: Some(event.url),
        }],
        created_at: SystemTime::now(),
    }
}

fn opencli_version_supported(version: &str) -> bool {
    let Some((major, _minor)) = version
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .find(|part| part.chars().any(|character| character.is_ascii_digit()))
        .and_then(|part| part.trim_start_matches('.').split_once('.'))
    else {
        return false;
    };
    major.parse::<u32>().is_ok_and(|major| major >= 1)
}

fn ensure_opencli_plugin(cli: &OpenCli, root: &std::path::Path) -> Result<(), String> {
    let version = cli.version(Duration::from_secs(20))?;
    if !opencli_version_supported(&version) {
        return Err(format!("OpenCLI >=1.0.0 is required, found {version}"));
    }
    let marker = root.join("managed-plugin");
    let installed = poller::is_installed(cli)?;
    if installed && !marker.is_file() {
        return Err(
            "peninsula-bilibili is already installed and is not managed by Peninsula".into(),
        );
    }
    if installed {
        let recorded = fs::read_to_string(&marker).unwrap_or_default();
        if recorded.trim() == format!("Peninsula-managed {}", env!("CARGO_PKG_VERSION")) {
            return Ok(());
        }
        let removed = cli.run(
            &["plugin", "uninstall", "peninsula-bilibili"],
            Duration::from_secs(30),
        )?;
        if removed.status != 0 {
            return Err(format!(
                "unable to update Peninsula OpenCLI plugin: {}",
                removed.stderr.trim()
            ));
        }
    }
    let source = root.join("plugin");
    let source_arg = source
        .to_str()
        .ok_or_else(|| "OpenCLI plugin path is not valid UTF-8".to_owned())?;
    let output = cli.run(&["plugin", "install", source_arg], Duration::from_secs(120))?;
    if output.status != 0 {
        return Err(format!(
            "unable to install OpenCLI plugin: {}",
            output.stderr.trim()
        ));
    }
    if !poller::is_installed(cli)? {
        return Err("OpenCLI did not register peninsula-bilibili after installation".into());
    }
    fs::write(
        marker,
        format!("Peninsula-managed {}\n", env!("CARGO_PKG_VERSION")),
    )
    .map_err(|error| error.to_string())
}
