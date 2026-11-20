use super::monitor::{self, Download, DownloadState, Snapshot};
use crate::extension::{
    CompactView, ContributionDescriptor, ContributionKind, ExtensionId, ExtensionOutput,
    Notification, NotificationAction, NotificationAlertView, NotificationCenterItemView,
    NotificationPresentationProvider, NotificationPriority, ReloadPolicy, RuntimeExtension,
    SettingSchema, SettingValue, Settings, ViewNode,
};
use crate::ui::declarative::*;
use std::{
    path::Path,
    time::{Duration, Instant, SystemTime},
};

pub const NOTIFICATION_ID: &str = "builtin.downloads/notifications";
pub const COMPACT_ID: &str = "builtin.downloads/status";
const COMPACT_WIDTH: f32 = 124.0;
const COMPLETION_HOLD: Duration = Duration::from_secs(3);

pub fn descriptors() -> Vec<ContributionDescriptor> {
    vec![
        ContributionDescriptor::builtin(
            COMPACT_ID,
            ContributionKind::Compact,
            "下载",
            "download",
            false,
            5,
        )
        .with_size_hint(COMPACT_WIDTH, 28.0)
        .with_settings(
            1,
            vec![SettingSchema::new(
                "show_rate",
                SettingValue::Boolean(true),
                "在胶囊显示下载速率",
            )],
        ),
        ContributionDescriptor::builtin(
            NOTIFICATION_ID,
            ContributionKind::Notification,
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
    updates: Option<crate::core::worker::Worker<Snapshot>>,
    snapshot: Snapshot,
    show_rate: bool,
    notification_sequence: u64,
    recent_completion: Option<(Download, Instant)>,
}

impl DownloadExtension {
    pub fn new() -> Self {
        Self {
            updates: None,
            snapshot: Snapshot::default(),
            show_rate: true,
            notification_sequence: 0,
            recent_completion: None,
        }
    }
}

impl Default for DownloadExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeExtension for DownloadExtension {
    fn descriptor(&self) -> crate::extension::ExtensionDescriptor {
        crate::extension::ExtensionDescriptor::builtin(
            "builtin.downloads",
            "下载",
            "Chromium、Firefox、Safari 下载检测",
            descriptors(),
        )
    }

    fn mascot_expressions(&self) -> Vec<crate::core::mascot::MascotExpression> {
        vec![crate::core::mascot::MascotExpression::event(
            "happy",
            "download.completed",
            3,
        )]
    }
    fn on_start(&mut self, _: &crate::extension::ExtensionContext) -> Result<(), String> {
        self.updates = Some(monitor::start());
        Ok(())
    }
    fn on_stop(&mut self, _: &crate::extension::ExtensionContext) -> Result<(), String> {
        self.updates = None;
        self.snapshot = Snapshot::default();
        self.recent_completion = None;
        Ok(())
    }
    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut output = ExtensionOutput::default();
        if self
            .recent_completion
            .as_ref()
            .is_some_and(|(_, expires)| now >= *expires)
        {
            self.recent_completion = None;
            output.invalidated.push(ExtensionId::builtin(COMPACT_ID));
        }
        while let Some(Ok(mut snapshot)) = self.updates.as_ref().map(|w| w.try_recv()) {
            for download in snapshot
                .completed
                .drain(..)
                .filter(|download| download.state == DownloadState::Complete)
            {
                output.mascot_events.push("download.completed".into());
                self.notification_sequence += 1;
                self.recent_completion = Some((download.clone(), now + COMPLETION_HOLD));
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
        let Some(download) = self.snapshot.preferred_active().or_else(|| {
            self.recent_completion
                .as_ref()
                .map(|(download, _)| download)
        }) else {
            return Vec::new();
        };
        let complete = download.state == DownloadState::Complete;
        let color = if complete {
            ColorSpec::Success
        } else {
            ColorSpec::Cyan
        };
        let size = download.total.filter(|total| *total > 0).map_or_else(
            || compact_size(download.received),
            |total| compact_pair(download.received, total),
        );
        let progress = if complete {
            1.0
        } else {
            download.progress().unwrap_or(0.0)
        };
        let icon = if complete {
            ViewNode::svg("compact/check", 16.0).motion(Motion {
                duration_ms: 300,
                easing: Easing::EaseOut,
                tracks: vec![MotionTrack::new(
                    MotionProperty::Scale,
                    &[(0, 0.65), (160, 1.12), (300, 1.0)],
                )],
                ..Default::default()
            })
        } else {
            ViewNode::svg("lucide/arrow-down", 14.0).motion(Motion {
                duration_ms: 720,
                gap_ms: 180,
                repeat: Repeat::Infinite,
                easing: Easing::EaseInOut,
                tracks: vec![MotionTrack::new(
                    MotionProperty::TranslateY,
                    &[(0, -1.0), (360, 1.0), (720, -1.0)],
                )],
                ..Default::default()
            })
        };
        let mut leading = ViewNode::new(Node::Stack(
            [Align::Center; 2],
            vec![
                ViewNode::ring(progress, "", 24.0, 1.6, 7.0)
                    .motion(Motion::progress(progress, 220)),
                icon,
            ],
        ))
        .sized(Size::Fixed(25.0), Size::Fixed(25.0));
        leading.key = Some(format!("{}:{}", download.id, complete));
        let (title, detail) = if complete {
            ("下载完成".into(), download.filename())
        } else if self.show_rate {
            (
                crate::builtins::performance::sampler::format_speed(download.bytes_per_second),
                format!("下载中 · {size}"),
            )
        } else {
            ("正在下载".into(), size.clone())
        };
        let content = ViewNode::compact(
            leading,
            ViewNode::column(
                vec![
                    super::super::compact_value(title, false).color(ColorSpec::PrimaryText),
                    super::super::compact_detail(detail),
                ],
                Flow {
                    gap: Some(0.0),
                    align: Align::Start,
                    ..Default::default()
                },
            ),
            CompactLayout {
                leading_size: [25.0, 25.0],
                gap: 6.0,
                ..Default::default()
            },
        )
        .color(color);
        vec![CompactView {
            id: ExtensionId::builtin(COMPACT_ID),
            source_extension_id: ExtensionId::builtin(NOTIFICATION_ID),
            content,
            visibility: Default::default(),
            min_width: 86.0,
            preferred_width: COMPACT_WIDTH,
            fit_width: false,
            priority: 1,
            tooltip: Some(if complete {
                download.filename()
            } else {
                format!("{} · {size}", download.filename())
            }),
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
        id: format!(
            "download-{}-{sequence}",
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ),
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

#[cfg(test)]
mod compact_tests {
    use super::*;

    #[test]
    fn completion_switches_the_descriptor_then_expires() {
        let (sender, updates) = std::sync::mpsc::channel();
        let mut extension = DownloadExtension {
            updates: Some(crate::core::worker::Worker::from_receiver(updates)),
            snapshot: Snapshot::default(),
            show_rate: true,
            notification_sequence: 0,
            recent_completion: None,
        };
        let now = Instant::now();
        let download = Download {
            id: "job".into(),
            browser: "Chrome",
            path: "/tmp/example.zip".into(),
            received: 40,
            total: Some(100),
            bytes_per_second: 20.0,
            state: DownloadState::Active,
        };
        sender
            .send(Snapshot {
                active: vec![download.clone()],
                completed: vec![],
            })
            .unwrap();
        extension.update(now);
        let active = serde_json::to_string(&extension.compact_views()[0].content).unwrap();
        assert!(active.contains("lucide/arrow-down"));
        assert!(active.contains("下载中"));

        sender
            .send(Snapshot {
                active: vec![],
                completed: vec![Download {
                    received: 100,
                    state: DownloadState::Complete,
                    ..download
                }],
            })
            .unwrap();
        assert_eq!(extension.update(now).notifications.len(), 1);
        let complete = serde_json::to_string(&extension.compact_views()[0].content).unwrap();
        assert!(complete.contains("compact/check"));
        assert!(complete.contains("下载完成"));

        extension.update(now + COMPLETION_HOLD);
        assert!(extension.compact_views().is_empty());
    }
}
