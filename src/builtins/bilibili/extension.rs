use super::{poller, state::SyncState, state::UnreadIncrease};
use crate::{
    core::external::opencli::OpenCli,
    extension::{
        ExtensionContext, ExtensionDescriptor, ExtensionId, ExtensionKind, ExtensionOutput,
        Notification, NotificationAlertView, NotificationCenterItemView,
        NotificationPresentationProvider, NotificationPriority, RuntimeExtension,
    },
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const NOTIFICATION_ID: &str = "builtin.bilibili/notifications";
const PLUGIN_SPEC: &str = "github:kevinhuang001/opencli-bilibili-unread";
const PLUGIN_NAME: &str = "bilibili-unread";
const LEGACY_PLUGIN_NAME: &str = "peninsula-bilibili";
const DEFAULT_POLL_SECS: u64 = 60;

enum PollMessage {
    Increases(Vec<UnreadIncrease>),
    Checked,
    Error(String),
}

pub struct BilibiliExtension {
    receiver: Option<Receiver<PollMessage>>,
    stop: Option<Arc<AtomicBool>>,
    worker: Option<JoinHandle<()>>,
    check_sender: Option<mpsc::Sender<()>>,
    last_error: Option<String>,
    last_check: Option<SystemTime>,
    opencli_path: String,
    poll_seconds: u16,
    enabled: bool,
    notification_sequence: u128,
}

impl Default for BilibiliExtension {
    fn default() -> Self {
        Self {
            receiver: None,
            stop: None,
            worker: None,
            check_sender: None,
            last_error: None,
            last_check: None,
            opencli_path: String::new(),
            poll_seconds: 60,
            enabled: false,
            notification_sequence: 0,
        }
    }
}

impl BilibiliExtension {
    fn begin_polling(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        if self.worker.is_some() {
            return Ok(());
        }
        let root = ctx.extension_data_dir(&ExtensionId::builtin(NOTIFICATION_ID));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let (sender, receiver) = mpsc::channel();
        let (check_sender, check_receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let state_path = root.join("unread-counts.tsv");
        let configured =
            (!self.opencli_path.trim().is_empty()).then(|| PathBuf::from(self.opencli_path.trim()));
        let poll_seconds = self.poll_seconds.clamp(15, 3600);
        let worker = thread::spawn(move || {
            let mut state = SyncState::load(&state_path);
            let mut backoff = Duration::from_secs(DEFAULT_POLL_SECS);
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
                match poller::collect(&cli) {
                    Ok(counts) => {
                        let increases = state.observe(counts);
                        let save_error = state.save(&state_path).err();
                        if sender.send(PollMessage::Checked).is_err() {
                            break;
                        }
                        if let Some(error) = save_error {
                            let _ = sender.send(PollMessage::Error(format!(
                                "unable to persist Bilibili unread counts: {error}"
                            )));
                        }
                        if !increases.is_empty()
                            && sender.send(PollMessage::Increases(increases)).is_err()
                        {
                            break;
                        }
                        backoff = Duration::from_secs(u64::from(poll_seconds));
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

    fn unread_notification(&mut self, increases: &[UnreadIncrease]) -> Notification {
        self.notification_sequence += 1;
        let body = increases
            .iter()
            .map(|item| {
                format!(
                    "{}：{} 条未读（增加 {} 条）",
                    item.category, item.count, item.increase
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Notification {
            source_id: ExtensionId::builtin(NOTIFICATION_ID),
            id: format!("unread-{nonce}-{}", self.notification_sequence),
            title: "Bilibili 未读消息增加".into(),
            body,
            icon: Some("bell".into()),
            priority: NotificationPriority::Normal,
            actions: Vec::new(),
            created_at: SystemTime::now(),
        }
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

    fn on_install(&mut self, _ctx: &ExtensionContext) -> Result<(), String> {
        Ok(())
    }

    fn on_upgrade(&mut self, _ctx: &ExtensionContext, _old_version: &str) -> Result<(), String> {
        Ok(())
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.enabled = true;
        self.begin_polling(ctx)
    }

    fn configure_bilibili(
        &mut self,
        ctx: &ExtensionContext,
        opencli_path: &str,
        _dm_enabled: bool,
        _likes_enabled: bool,
        _show_dm_summary: bool,
        poll_seconds: u16,
    ) -> Result<(), String> {
        let changed = self.opencli_path != opencli_path.trim()
            || self.poll_seconds != poll_seconds.clamp(15, 3600);
        self.opencli_path = opencli_path.trim().to_owned();
        self.poll_seconds = poll_seconds.clamp(15, 3600);
        if changed && self.enabled && self.worker.is_some() {
            self.on_stop(ctx)?;
            self.enabled = true;
            self.begin_polling(ctx)?;
        }
        Ok(())
    }

    fn on_stop(&mut self, _ctx: &ExtensionContext) -> Result<(), String> {
        self.enabled = false;
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        drop(self.worker.take());
        self.receiver = None;
        self.check_sender = None;
        Ok(())
    }

    fn on_uninstall(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.on_stop(ctx)?;
        let root = ctx.extension_data_dir(&ExtensionId::builtin(NOTIFICATION_ID));
        let marker = root.join("managed-unread-plugin");
        let legacy_marker = root.join("managed-plugin");
        if marker.is_file() || legacy_marker.is_file() {
            let configured = (!self.opencli_path.trim().is_empty())
                .then(|| PathBuf::from(self.opencli_path.trim()));
            let cli = OpenCli::resolve(configured.as_deref())?;
            for (installed, plugin) in [
                (marker.is_file() && poller::is_installed(&cli)?, PLUGIN_NAME),
                (
                    legacy_marker.is_file() && poller::legacy_plugin_is_installed(&cli)?,
                    LEGACY_PLUGIN_NAME,
                ),
            ] {
                if !installed {
                    continue;
                }
                let output = cli.run(&["plugin", "uninstall", plugin], Duration::from_secs(30))?;
                if output.status != 0 {
                    return Err(format!(
                        "unable to uninstall OpenCLI plugin {plugin}: {}",
                        output.stderr.trim()
                    ));
                }
            }
        }
        if root.exists() {
            fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn update(&mut self, _now: std::time::Instant) -> ExtensionOutput {
        let Some(receiver) = self.receiver.as_ref() else {
            return ExtensionOutput::default();
        };
        let messages: Vec<_> = receiver.try_iter().collect();
        let mut notifications = Vec::new();
        for message in messages {
            match message {
                PollMessage::Checked => {
                    self.last_check = Some(SystemTime::now());
                    self.last_error = None;
                }
                PollMessage::Increases(increases) => {
                    notifications.push(self.unread_notification(&increases));
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
        if let Some(last_check) = self.last_check {
            let age = SystemTime::now()
                .duration_since(last_check)
                .unwrap_or_default()
                .as_secs();
            return Some(format!("已连接 OpenCLI · {age} 秒前检查"));
        }
        Some(if self.worker.is_some() {
            "正在连接 OpenCLI 与 Bilibili…".into()
        } else {
            "尚未启用".into()
        })
    }

    fn request_bilibili_check(&mut self, _ctx: &ExtensionContext) -> Result<(), String> {
        if !self.enabled {
            return Err("Bilibili notifications are not enabled".into());
        }
        self.check_sender
            .as_ref()
            .ok_or_else(|| "Bilibili poller is no longer running".to_owned())?
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

    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == NOTIFICATION_ID
    }
}

fn opencli_version_supported(version: &str) -> bool {
    version
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .find(|part| part.chars().any(|character| character.is_ascii_digit()))
        .and_then(|part| part.trim_start_matches('.').split('.').next())
        .and_then(|major| major.parse::<u32>().ok())
        .is_some_and(|major| major >= 2)
}

fn ensure_opencli_plugin(cli: &OpenCli, root: &std::path::Path) -> Result<(), String> {
    let version = cli.version(Duration::from_secs(20))?;
    if !opencli_version_supported(&version) {
        return Err(format!("OpenCLI >=2.0.0 is required, found {version}"));
    }

    let legacy_marker = root.join("managed-plugin");
    if legacy_marker.is_file() {
        if poller::legacy_plugin_is_installed(cli)? {
            let removed = cli.run(
                &["plugin", "uninstall", LEGACY_PLUGIN_NAME],
                Duration::from_secs(30),
            )?;
            if removed.status != 0 {
                return Err(format!(
                    "unable to remove the previous Bilibili plugin: {}",
                    removed.stderr.trim()
                ));
            }
        }
        fs::remove_file(legacy_marker).map_err(|error| error.to_string())?;
        let old_plugin = root.join("plugin");
        if old_plugin.exists() {
            fs::remove_dir_all(old_plugin).map_err(|error| error.to_string())?;
        }
    }

    if poller::is_installed(cli)? {
        let update = cli.run(&["plugin", "update", PLUGIN_NAME], Duration::from_secs(120))?;
        if update.status != 0 {
            return Err(format!(
                "unable to update OpenCLI plugin {PLUGIN_NAME}: {}",
                update.stderr.trim()
            ));
        }
    } else {
        let install = cli.run(
            &["plugin", "install", PLUGIN_SPEC],
            Duration::from_secs(120),
        )?;
        if install.status != 0 {
            return Err(format!(
                "unable to install OpenCLI plugin {PLUGIN_NAME}: {}",
                install.stderr.trim()
            ));
        }
        fs::write(root.join("managed-unread-plugin"), "Peninsula-managed\n")
            .map_err(|error| error.to_string())?;
    }
    if !poller::is_installed(cli)? {
        return Err(format!("OpenCLI did not register {PLUGIN_NAME}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::opencli_version_supported;

    #[test]
    fn unread_plugin_requires_opencli_two_or_newer() {
        assert!(!opencli_version_supported("1.9.9"));
        assert!(opencli_version_supported("2.0.0"));
        assert!(opencli_version_supported("OpenCLI 2.1.3"));
    }
}
