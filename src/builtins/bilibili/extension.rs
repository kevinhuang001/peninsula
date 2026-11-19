use super::{poller, state::SyncState, state::UnreadIncrease};
use crate::{
    extension::{
        ContributionDescriptor, ContributionKind, ExtensionContext, ExtensionId, ExtensionOutput,
        Notification, NotificationAction, NotificationAlertView, NotificationCenterItemView,
        NotificationPresentationProvider, NotificationPriority, ReloadPolicy, RuntimeExtension,
        SettingSchema, SettingValue, Settings,
    },
    sdk::process::{ProcessExecutor, ProcessSpec},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const NOTIFICATION_ID: &str = "builtin.bilibili/notifications";
const PLUGIN_SPEC: &str = "github:kevinhuang001/opencli-bilibili-unread";
const PLUGIN_NAME: &str = "bilibili-unread";
const DEFAULT_POLL_SECS: u64 = 60;
const OPEN_MESSAGES_ACTION: &str = "open-messages";
const MESSAGES_URL: &str = "https://message.bilibili.com/";

enum PollMessage {
    Increases(Vec<UnreadIncrease>),
    Checked,
    Error(String),
}

pub struct BilibiliExtension {
    receiver: Option<Receiver<PollMessage>>,
    polling: bool,
    last_error: Option<String>,
    last_check: Option<SystemTime>,
    opencli_path: String,
    poll_interval: Duration,
    enabled: bool,
    notification_sequence: u128,
}

impl Default for BilibiliExtension {
    fn default() -> Self {
        Self {
            receiver: None,
            polling: false,
            last_error: None,
            last_check: None,
            opencli_path: String::new(),
            poll_interval: Duration::from_secs(DEFAULT_POLL_SECS),
            enabled: false,
            notification_sequence: 0,
        }
    }
}

impl BilibiliExtension {
    fn begin_polling(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        if self.polling {
            return Ok(());
        }
        let storage = ctx.sdk().storage();
        let state_storage = storage.state().clone();
        let (sender, receiver) = mpsc::channel();
        let program = if self.opencli_path.trim().is_empty() {
            PathBuf::from("opencli")
        } else {
            PathBuf::from(self.opencli_path.trim())
        };
        let poll_interval = self.poll_interval;
        ctx.sdk()
            .tasks()
            .spawn("bilibili-unread", move |cancel| {
                let mut state = match SyncState::load_storage(&state_storage) {
                    Ok(state) => state,
                    Err(error) => {
                        let _ = sender.send(PollMessage::Error(error.to_string()));
                        return;
                    }
                };
                let mut backoff = Duration::from_secs(DEFAULT_POLL_SECS);
                if let Err(error) = ensure_opencli_plugin(&program, storage.data()) {
                    let _ = sender.send(PollMessage::Error(error));
                    return;
                }
                while !cancel.is_cancelled() {
                    match poller::collect(&program) {
                        Ok(counts) => {
                            let increases = state.observe(counts);
                            let save_error = state.save_storage(&state_storage).err();
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
                            backoff = poll_interval;
                        }
                        Err(error) => {
                            let _ = sender.send(PollMessage::Error(error));
                            backoff = (backoff * 2).min(Duration::from_secs(600));
                        }
                    }
                    let deadline = std::time::Instant::now() + backoff;
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if !remaining.is_zero() && cancel.wait(remaining) {
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        self.receiver = Some(receiver);
        self.polling = true;
        Ok(())
    }

    fn unread_notification(&mut self, increases: &[UnreadIncrease]) -> Notification {
        self.notification_sequence += 1;
        let mut body = increases
            .iter()
            .map(|item| {
                let (category, suffix) = match item.category.as_str() {
                    "回复我的" => ("回复", "消息"),
                    "@我的" => ("@我的", "消息"),
                    "收到的赞" => ("点赞", "消息"),
                    category if category.ends_with("消息") || category.ends_with("通知") => {
                        (category, "")
                    }
                    category => (category, "消息"),
                };
                format!("新增{}条{category}{suffix}", item.increase)
            })
            .collect::<Vec<_>>()
            .join("，");
        body.push('。');
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
            actions: vec![NotificationAction {
                id: OPEN_MESSAGES_ACTION.into(),
                label: "查看消息".into(),
                data: None,
            }],
            created_at: SystemTime::now(),
        }
    }
}

impl BilibiliExtension {
    fn descriptor(&self) -> crate::extension::ExtensionDescriptor {
        crate::extension::ExtensionDescriptor::builtin(
            "builtin.bilibili",
            "Bilibili",
            "动态与消息通知",
            self.contributions(),
        )
    }
}

impl BilibiliExtension {
    fn contributions(&self) -> Vec<ContributionDescriptor> {
        vec![ContributionDescriptor::builtin(
            NOTIFICATION_ID,
            ContributionKind::Notification,
            "Bilibili 通知",
            "bell",
            false,
            20,
        )
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_setup(true)
        .with_settings(
            1,
            vec![
                SettingSchema {
                    reload_policy: ReloadPolicy::Reload,
                    ..SettingSchema::new(
                        "opencli_path",
                        SettingValue::String(String::new()),
                        "OpenCLI 可执行文件路径",
                    )
                },
                SettingSchema {
                    reload_policy: ReloadPolicy::Reload,
                    ..SettingSchema::new(
                        "poll_interval",
                        SettingValue::Duration(60_000),
                        "后台轮询间隔",
                    )
                    .range(15_000.0, 3_600_000.0)
                },
            ],
        )]
    }
}

impl RuntimeExtension for BilibiliExtension {
    fn descriptor(&self) -> crate::extension::ExtensionDescriptor {
        BilibiliExtension::descriptor(self)
    }
    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.enabled = true;
        self.begin_polling(ctx)
    }

    fn on_settings_changed(
        &mut self,
        ctx: &ExtensionContext,
        id: &ExtensionId,
        settings: &Settings,
        _policy: ReloadPolicy,
    ) -> ExtensionOutput {
        let (Some(SettingValue::String(path)), Some(SettingValue::Duration(ms))) =
            (settings.get("opencli_path"), settings.get("poll_interval"))
        else {
            return ExtensionOutput::default();
        };
        let interval = Duration::from_millis(*ms);
        let changed = self.opencli_path != path.trim() || self.poll_interval != interval;
        self.opencli_path = path.trim().to_owned();
        self.poll_interval = interval;
        if changed && self.enabled && self.polling {
            ctx.shutdown_tasks();
            self.receiver = None;
            self.polling = false;
            let owner = ctx
                .extension_id()
                .cloned()
                .unwrap_or_else(|| ExtensionId::builtin("builtin.bilibili"));
            let restarted = ctx.scoped_at(&owner, ctx.generation());
            if let Err(error) = self.begin_polling(&restarted) {
                ctx.log(id, &error);
                self.last_error = Some(error);
            }
        }
        ExtensionOutput::default()
    }

    fn on_stop(&mut self, _ctx: &ExtensionContext) -> Result<(), String> {
        self.enabled = false;
        self.receiver = None;
        self.polling = false;
        Ok(())
    }

    fn on_uninstall(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.on_stop(ctx)?;
        let storage = ctx.sdk().storage();
        let marker = storage.data().root().join("managed-unread-plugin");
        if marker.is_file() {
            let configured = (!self.opencli_path.trim().is_empty())
                .then(|| PathBuf::from(self.opencli_path.trim()));
            let program = configured.unwrap_or_else(|| PathBuf::from("opencli"));
            if poller::is_installed(&program)? {
                let output = ProcessExecutor
                    .run(
                        ProcessSpec::new(&program)
                            .args(["plugin", "uninstall", PLUGIN_NAME])
                            .timeout(Duration::from_secs(30)),
                    )
                    .map_err(|error| format!("OpenCLI process failed: {error}"))?;
                if !output.success() {
                    return Err(format!(
                        "unable to uninstall OpenCLI plugin {PLUGIN_NAME} ({}): {}",
                        output.status.code().unwrap_or(-1),
                        output.stderr_text().trim()
                    ));
                }
            }
        }
        if storage.data().root().exists() {
            fs::remove_dir_all(storage.data().root()).map_err(|error| error.to_string())?;
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
        Some(if self.polling {
            "正在连接 OpenCLI 与 Bilibili…".into()
        } else {
            "尚未启用".into()
        })
    }

    fn doctor(&self) -> Vec<crate::extension::DiagnosticCheck> {
        use crate::extension::{DiagnosticCheck as Check, DiagnosticStatus as Status};
        let Some(error) = self.last_error.as_deref() else {
            let status = if self.last_check.is_some() {
                Status::Passed
            } else {
                Status::Unknown
            };
            let detail = if self.last_check.is_some() {
                "最近一次 OpenCLI 与 Bilibili 检查成功。"
            } else {
                "等待 OpenCLI 完成首次检查。"
            };
            return [
                ("opencli-command", "OpenCLI 命令"),
                ("opencli-plugin", "未读插件"),
                ("browser-bridge", "浏览器连接"),
                ("bilibili-login", "Bilibili 登录"),
            ]
            .into_iter()
            .map(|(id, title)| Check::new(id, title, status, detail, None))
            .collect();
        };

        let lower = error.to_ascii_lowercase();
        let missing_cli = [
            "no such file",
            "program not found",
            "executable not found",
            "cannot find the file specified",
            "系统找不到指定的文件",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let login = ["auth_required", "login", "登录", "cookie", "未登录"]
            .iter()
            .any(|needle| lower.contains(needle));
        let browser = ["browser", "bridge", "daemon_unavailable", "浏览器"]
            .iter()
            .any(|needle| lower.contains(needle));
        let plugin = ["plugin", "unknown command", "插件"]
            .iter()
            .any(|needle| lower.contains(needle));
        let check = |id, title, matched, remediation| {
            if matched {
                Check::new(id, title, Status::Failed, error, Some(remediation))
            } else if self.last_check.is_some() {
                Check::passed(id, title, "最近一次检查成功。")
            } else {
                Check::unknown(id, title, "尚未通过首次连接检查。")
            }
        };
        let mut checks = vec![
            check(
                "opencli-command",
                "OpenCLI 命令",
                missing_cli,
                "安装 OpenCLI，或在设置中填写可执行文件路径。",
            ),
            check(
                "opencli-plugin",
                "未读插件",
                plugin,
                "检查 OpenCLI 插件安装与版本，然后重新启用扩展。",
            ),
            check(
                "browser-bridge",
                "浏览器连接",
                browser,
                "启动浏览器并连接 OpenCLI Browser Bridge。",
            ),
            check(
                "bilibili-login",
                "Bilibili 登录",
                login,
                "在浏览器中登录 Bilibili 后重新检查。",
            ),
        ];
        if !missing_cli && !plugin && !browser && !login {
            checks.push(Check::new(
                "unread-query",
                "未读查询",
                Status::Failed,
                error,
                Some("检查 OpenCLI 输出和扩展设置中的命令路径后重试。"),
            ));
        }
        checks
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
        if action.id != OPEN_MESSAGES_ACTION {
            return Err(format!(
                "unknown Bilibili notification action: {}",
                action.id
            ));
        }
        webbrowser::open(MESSAGES_URL).map_err(|error| error.to_string())
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

fn ensure_opencli_plugin(
    program: &Path,
    storage: &crate::sdk::storage::StorageArea,
) -> Result<(), String> {
    let version_output = ProcessExecutor
        .run(
            ProcessSpec::new(program)
                .arg("--version")
                .timeout(Duration::from_secs(20)),
        )
        .map_err(|error| format!("OpenCLI process failed: {error}"))?;
    if !version_output.success() {
        return Err(format!(
            "OpenCLI version query failed ({}): {}",
            version_output.status.code().unwrap_or(-1),
            version_output.stderr_text().trim()
        ));
    }
    let version = version_output.stdout_text();
    let version = version.trim();
    if !opencli_version_supported(version) {
        return Err(format!("OpenCLI >=2.0.0 is required, found {version}"));
    }

    if poller::is_installed(program)? {
        let update = ProcessExecutor
            .run(
                ProcessSpec::new(program)
                    .args(["plugin", "update", PLUGIN_NAME])
                    .timeout(Duration::from_secs(120)),
            )
            .map_err(|error| format!("OpenCLI process failed: {error}"))?;
        if !update.success() {
            return Err(format!(
                "unable to update OpenCLI plugin {PLUGIN_NAME} ({}): {}",
                update.status.code().unwrap_or(-1),
                update.stderr_text().trim()
            ));
        }
    } else {
        let install = ProcessExecutor
            .run(
                ProcessSpec::new(program)
                    .args(["plugin", "install", PLUGIN_SPEC])
                    .timeout(Duration::from_secs(120)),
            )
            .map_err(|error| format!("OpenCLI process failed: {error}"))?;
        if !install.success() {
            return Err(format!(
                "unable to install OpenCLI plugin {PLUGIN_NAME} ({}): {}",
                install.status.code().unwrap_or(-1),
                install.stderr_text().trim()
            ));
        }
        storage
            .write_bytes("managed-unread-plugin", b"Peninsula-managed\n")
            .map_err(|error| error.to_string())?;
    }
    if !poller::is_installed(program)? {
        return Err(format!("OpenCLI did not register {PLUGIN_NAME}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        opencli_version_supported, BilibiliExtension, NotificationPresentationProvider,
        UnreadIncrease, OPEN_MESSAGES_ACTION,
    };

    #[test]
    fn unread_plugin_requires_opencli_two_or_newer() {
        assert!(!opencli_version_supported("1.9.9"));
        assert!(opencli_version_supported("2.0.0"));
        assert!(opencli_version_supported("OpenCLI 2.1.3"));
    }
    #[test]
    fn unread_alert_lists_new_items_as_one_sentence() {
        let mut extension = BilibiliExtension::default();
        let increases = [
            UnreadIncrease {
                category: "我的消息".into(),
                increase: 1,
            },
            UnreadIncrease {
                category: "回复我的".into(),
                increase: 2,
            },
            UnreadIncrease {
                category: "收到的赞".into(),
                increase: 3,
            },
            UnreadIncrease {
                category: "系统通知".into(),
                increase: 4,
            },
        ];
        assert_eq!(
            extension.unread_notification(&increases[..1]).body,
            "新增1条我的消息。"
        );
        let notification = extension.unread_notification(&increases);
        assert_eq!(
            notification.body,
            "新增1条我的消息，新增2条回复消息，新增3条点赞消息，新增4条系统通知。"
        );
        let alert = extension.alert_view(&notification);
        assert_eq!(alert.actions[0].id, OPEN_MESSAGES_ACTION);
        assert_eq!(alert.actions[0].label, "查看消息");
    }
}
