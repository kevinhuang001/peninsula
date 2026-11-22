use super::monitor::{self, DownloadState, Snapshot};
use crate::extension::{
    CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind, ExtensionOutput, Notification,
    NotificationAction, NotificationAlertView, NotificationCenterItemView,
    NotificationPresentationProvider, NotificationPriority, ReloadPolicy, RuntimeExtension,
    SettingSchema, SettingValue, Settings, ViewNode,
};
use crate::ui::declarative::{Align, ColorSpec, Flow, Size};
use std::{path::Path, sync::mpsc::Receiver, time::SystemTime};

pub const NOTIFICATION_ID: &str = "builtin.downloads/notifications";
pub const COMPACT_ID: &str = "builtin.downloads/status";

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(
            COMPACT_ID,
            ExtensionKind::Compact,
            "下载",
            "download",
            false,
            5,
        )
        .with_size_hint(88.0, 28.0)
        .with_settings(
            1,
            vec![SettingSchema::new(
                "show_rate",
                SettingValue::Boolean(true),
                "在胶囊显示下载速率",
            )],
        ),
        ExtensionDescriptor::builtin(
            NOTIFICATION_ID,
            ExtensionKind::Notification,
            "下载提醒",
            "bell",
            true,
            0,
        )
        .with_capabilities(vec![
            crate::extension::Capability::Event,
            crate::extension::Capability::Action,
        ]),
    ]
}

pub struct DownloadExtension {
    updates: Receiver<Snapshot>,
    snapshot: Snapshot,
    show_rate: bool,
    notification_sequence: u64,
}

impl DownloadExtension {
    pub fn new() -> Self {
        Self {
            updates: monitor::start(),
            snapshot: Snapshot::default(),
            show_rate: true,
            notification_sequence: 0,
        }
    }
}

impl Default for DownloadExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeExtension for DownloadExtension {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        descriptors()
    }

    fn update(&mut self, _now: std::time::Instant) -> ExtensionOutput {
        let mut output = ExtensionOutput::default();
        while let Ok(mut snapshot) = self.updates.try_recv() {
            for download in snapshot
                .completed
                .drain(..)
                .filter(|download| download.state == DownloadState::Complete)
            {
                self.notification_sequence += 1;
                output
                    .notifications
                    .push(download_notification(download, self.notification_sequence));
            }
            self.snapshot = snapshot;
            output.invalidated.push(ExtensionId::builtin(COMPACT_ID));
        }
        output
    }

    fn on_settings_changed(
        &mut self,
        _ctx: &crate::extension::ExtensionContext,
        _id: &ExtensionId,
        settings: &Settings,
        _policy: ReloadPolicy,
    ) -> ExtensionOutput {
        self.show_rate = settings.get("show_rate") != Some(&SettingValue::Boolean(false));
        ExtensionOutput::default()
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let Some(download) = self.snapshot.preferred_active() else {
            return Vec::new();
        };
        let size = download.total.filter(|total| *total > 0).map_or_else(
            || compact_size(download.received),
            |total| compact_pair(download.received, total),
        );
        let progress = download.progress();
        let mut detail = vec![super::super::compact_text(size)];
        if self.show_rate {
            detail.push(
                super::super::compact_text(crate::builtins::performance::sampler::format_speed(
                    download.bytes_per_second,
                ))
                .color(ColorSpec::Info),
            );
        }
        let content = ViewNode::row(
            vec![
                ViewNode::ring(
                    progress.unwrap_or(0.0),
                    progress.map_or_else(|| "↓".into(), |value| format!("{:.0}%", value * 100.0)),
                    24.0,
                    2.0,
                    6.0,
                )
                .color(ColorSpec::Info),
                ViewNode::column(
                    detail,
                    Flow {
                        gap: Some(0.0),
                        align: Align::Start,
                        ..Default::default()
                    },
                )
                .sized(Size::Fill(1.0), Size::Auto),
            ],
            Flow {
                gap: Some(4.0),
                ..Default::default()
            },
        )
        .sized(Size::Fixed(80.0), Size::Auto);
        vec![CompactView {
            id: ExtensionId::builtin(COMPACT_ID),
            source_extension_id: ExtensionId::builtin(NOTIFICATION_ID),
            content,
            visibility: Default::default(),
            min_width: 88.0,
            preferred_width: 88.0,
            fit_width: false,
            priority: 1,
            tooltip: Some(download.filename()),
        }]
    }
}

impl NotificationPresentationProvider for DownloadExtension {
    fn alert_view(&self, notification: &Notification) -> NotificationAlertView {
        NotificationAlertView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("download".into()),
            actions: notification.actions.clone(),
        }
    }

    fn center_item_view(&self, notification: &Notification) -> NotificationCenterItemView {
        NotificationCenterItemView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("download".into()),
        }
    }

    fn on_notification_action(
        &mut self,
        _notification: &Notification,
        action: &NotificationAction,
    ) -> Result<(), String> {
        let Some(path) = action.data.as_deref() else {
            return Err(format!("download action {} has no file path", action.id));
        };
        match action.id.as_str() {
            "open" => open_file(Path::new(path), false),
            "reveal" => open_file(Path::new(path), true),
            other => Err(format!("unsupported download action: {other}")),
        }
    }

    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == NOTIFICATION_ID
    }
}

fn download_notification(download: super::monitor::Download, sequence: u64) -> Notification {
    let path = download.path.to_string_lossy().into_owned();
    Notification {
        source_id: ExtensionId::builtin(NOTIFICATION_ID),
        id: format!("download-{sequence}"),
        title: "下载完成".into(),
        body: download.filename(),
        icon: Some("download".into()),
        priority: NotificationPriority::Normal,
        actions: vec![
            NotificationAction {
                id: "open".into(),
                label: "打开文件".into(),
                data: Some(path.clone()),
            },
            NotificationAction {
                id: "reveal".into(),
                label: "显示所在位置".into(),
                data: Some(path),
            },
        ],
        created_at: SystemTime::now(),
    }
}

fn open_file(path: &Path, reveal: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut command = crate::core::external::process::command("open");
        if reveal {
            command.arg("-R");
        }
        command
            .arg("--")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(target_os = "linux")]
    {
        let target = if reveal {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        crate::core::external::process::command("xdg-open")
            .arg(target)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = crate::core::external::process::command("explorer.exe");
        if reveal {
            let mut argument = std::ffi::OsString::from("/select,");
            argument.push(path);
            command.arg(argument);
        } else {
            command.arg(path);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn compact_size(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < 3 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1}{}", ["B", "K", "M", "G"][unit])
}

fn compact_pair(received: u64, total: u64) -> String {
    let unit = [received, total]
        .into_iter()
        .max()
        .map(unit_index)
        .unwrap_or(0);
    let divisor = 1024_f64.powi(unit as i32);
    format!(
        "{}/{}{}",
        (received as f64 / divisor).round() as u64,
        (total as f64 / divisor).round() as u64,
        ["B", "K", "M", "G"][unit]
    )
}

fn unit_index(bytes: u64) -> usize {
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024 && unit < 3 {
        value /= 1024;
        unit += 1;
    }
    unit
}
