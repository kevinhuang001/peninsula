mod parcels;
mod poller;
mod widget;

use self::{
    parcels::{Parcel, ShipmentChange, Snapshot},
    poller::{Platform, SyncError},
};
use crate::{
    extension::{
        Action, Capability, ContributionDescriptor, ContributionKind, ExtensionContext,
        ExtensionId, ExtensionOutput, Notification, NotificationPresentationProvider,
        NotificationPriority, ReloadPolicy, RuntimeExtension, SettingSchema, SettingValue,
        Settings, WidgetView,
    },
    sdk::{storage::StorageArea, tasks::TaskScope},
    ui::declarative::{Overflow, ScrollAxes, WidgetBounds},
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const WIDGET_ID: &str = "builtin.kuaidi/shipments";
const NOTIFICATION_ID: &str = "builtin.kuaidi/notifications";
pub const DEFAULT_REFRESH_MINUTES: u16 = 20;
pub const REFRESH_MINUTES_KEY: &str = "refresh_minutes";
pub const CHANGE_NOTIFICATIONS_KEY: &str = "change_notifications";
pub const PICKUP_NOTIFICATIONS_KEY: &str = "pickup_notifications";
pub const MAX_REFRESH_MINUTES: u16 = 180;

pub fn descriptors() -> Vec<ContributionDescriptor> {
    vec![
        ContributionDescriptor::builtin(
            WIDGET_ID,
            ContributionKind::Widget,
            "快递",
            "bell",
            true,
            3,
        )
        .with_settings(
            1,
            vec![
                SettingSchema::new(
                    REFRESH_MINUTES_KEY,
                    SettingValue::Integer(i64::from(DEFAULT_REFRESH_MINUTES)),
                    "快递刷新间隔（分钟）",
                )
                .range(1.0, f64::from(MAX_REFRESH_MINUTES)),
                SettingSchema::new(
                    CHANGE_NOTIFICATIONS_KEY,
                    SettingValue::Boolean(true),
                    "快递变更通知",
                ),
                SettingSchema::new(
                    PICKUP_NOTIFICATIONS_KEY,
                    SettingValue::Boolean(true),
                    "取件通知",
                ),
            ],
        )
        .with_capabilities(vec![Capability::Action])
        .with_widget_bounds(WidgetBounds {
            fit_width: true,
            fit_height: true,
            overflow: Overflow::Scroll(ScrollAxes::Vertical),
            ..WidgetBounds::default()
        }),
        ContributionDescriptor::builtin(
            NOTIFICATION_ID,
            ContributionKind::Notification,
            "快递动态",
            "bell",
            true,
            21,
        ),
    ]
}

#[derive(Default)]
struct Source {
    snapshot: Option<Snapshot>,
    error: Option<SyncError>,
}

struct Refresh {
    results: Receiver<(Platform, Result<Vec<Parcel>, SyncError>)>,
}

pub struct KuaidiExtension {
    storage: Option<StorageArea>,
    tasks: Option<TaskScope>,
    sources: [Source; 2],
    refresh: Option<Refresh>,
    last_poll: Option<Instant>,
    poll_interval: Duration,
    change_notifications: bool,
    pickup_notifications: bool,
    sequence: u64,
}

impl Default for KuaidiExtension {
    fn default() -> Self {
        Self {
            storage: None,
            tasks: None,
            sources: Default::default(),
            refresh: None,
            last_poll: None,
            poll_interval: Duration::from_secs(u64::from(DEFAULT_REFRESH_MINUTES) * 60),
            change_notifications: true,
            pickup_notifications: true,
            sequence: 0,
        }
    }
}

impl KuaidiExtension {
    fn begin_refresh(&mut self, now: Instant) {
        let Some(tasks) = self.tasks.as_ref() else {
            return;
        };
        let (sender, results) = mpsc::channel();
        let worker = tasks.spawn("kuaidi-sync", move |cancel| {
            let program = PathBuf::from("opencli");
            let cli = poller::check_plugin(&program).map(|()| program);

            if cancel.is_cancelled() {
                return;
            }
            for platform in Platform::ALL {
                if cancel.is_cancelled() {
                    break;
                }
                let result = match &cli {
                    Ok(program) => platform.collect(program),
                    Err(error) => Err(*error),
                };
                if sender.send((platform, result)).is_err() {
                    break;
                }
            }
        });
        if worker.is_ok() {
            self.refresh = Some(Refresh { results });
        } else {
            for source in &mut self.sources {
                source.error = Some(SyncError::Query);
            }
        }
        self.last_poll = Some(now);
    }

    fn receive(
        &mut self,
        platform: Platform,
        result: Result<Vec<Parcel>, SyncError>,
        notifications: &mut Vec<Notification>,
    ) {
        let source = &mut self.sources[platform as usize];
        let parcels = match result {
            Ok(parcels) => parcels,
            Err(error) => {
                source.error = Some(error);
                return;
            }
        };
        let mut snapshot = Snapshot {
            checked_at: SystemTime::now(),
            parcels,
        };
        let changes = snapshot.changes(source.snapshot.as_ref());
        if let Some(storage) = &self.storage {
            let file = format!("{}.json", platform.command());
            source.error = storage
                .write_json(file, &snapshot)
                .err()
                .map(|_| SyncError::Save);
        }
        for change in changes {
            let (index, pickup) = match change {
                ShipmentChange::ReadyForPickup(index) if self.pickup_notifications => (index, true),
                ShipmentChange::Updated(index) | ShipmentChange::ReadyForPickup(index)
                    if self.change_notifications =>
                {
                    (index, false)
                }
                _ => continue,
            };
            self.sequence = self.sequence.wrapping_add(1);
            notifications.push(shipment_notification(
                platform,
                &snapshot.parcels[index],
                snapshot.checked_at,
                self.sequence,
                pickup,
            ));
        }
        source.snapshot = Some(snapshot);
    }
}

fn invalidated() -> ExtensionOutput {
    ExtensionOutput {
        invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
        ..ExtensionOutput::default()
    }
}

fn shipment_notification(
    platform: Platform,
    parcel: &Parcel,
    now: SystemTime,
    sequence: u64,
    pickup: bool,
) -> Notification {
    let status = if parcel.current_status.is_empty() {
        "物流有更新"
    } else {
        &parcel.current_status
    };
    let label = if parcel.product_name.is_empty() {
        "有一件快递".into()
    } else {
        shorten(&parcel.product_name, 26)
    };
    let body = if pickup {
        let metadata = [&parcel.carrier, &parcel.tracking_no, &parcel.latest_time]
            .into_iter()
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        [
            label.as_str(),
            metadata.as_str(),
            parcel.latest_detail.as_str(),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
    } else if parcel.latest_detail.is_empty() || parcel.latest_detail == status {
        label
    } else {
        format!("{label} · {}", shorten(&parcel.latest_detail, 62))
    };
    let timestamp = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Notification {
        source_id: ExtensionId::builtin(NOTIFICATION_ID),
        id: format!("shipment-{timestamp}-{sequence}"),
        title: if pickup {
            format!("{} · 取件提醒", platform.name())
        } else {
            format!("{} · {}", platform.name(), shorten(status, 18))
        },
        body,
        icon: Some("bell".into()),
        priority: NotificationPriority::Normal,
        actions: vec![],
        created_at: now,
    }
}

fn shorten(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut text: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        text.push('…');
    }
    text
}

impl RuntimeExtension for KuaidiExtension {
    fn descriptor(&self) -> crate::extension::ExtensionDescriptor {
        crate::extension::ExtensionDescriptor::builtin(
            "builtin.kuaidi",
            "快递",
            "物流状态与取件通知",
            descriptors(),
        )
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        let storage = ctx.sdk().storage().data().clone();
        self.tasks = Some(ctx.sdk().tasks());
        for platform in Platform::ALL {
            let name = format!("{}.json", platform.command());
            self.sources[platform as usize].snapshot = storage
                .read_json(&name)
                .map_err(|error| error.to_string())?;
        }
        self.storage = Some(storage);
        self.last_poll = None;
        Ok(())
    }

    fn on_stop(&mut self, _ctx: &ExtensionContext) -> Result<(), String> {
        self.refresh = None;
        self.storage = None;
        self.tasks = None;
        Ok(())
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut output = ExtensionOutput::default();
        if self.storage.is_some()
            && self.refresh.is_none()
            && self
                .last_poll
                .is_none_or(|last| now.saturating_duration_since(last) >= self.poll_interval)
        {
            self.begin_refresh(now);
            output = invalidated();
        }
        while let Some(refresh) = &self.refresh {
            match refresh.results.try_recv() {
                Ok((platform, result)) => {
                    if output.invalidated.is_empty() {
                        output = invalidated();
                    }
                    self.receive(platform, result, &mut output.notifications);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.refresh = None;
                    if output.invalidated.is_empty() {
                        output = invalidated();
                    }
                    break;
                }
            }
        }
        output
    }

    fn on_settings_changed(
        &mut self,
        _ctx: &ExtensionContext,
        id: &ExtensionId,
        settings: &Settings,
        _policy: ReloadPolicy,
    ) -> ExtensionOutput {
        if id.as_str() == WIDGET_ID || id.as_str() == "builtin.kuaidi" {
            if let Some(SettingValue::Integer(minutes)) = settings.get(REFRESH_MINUTES_KEY) {
                self.poll_interval = Duration::from_secs(*minutes as u64 * 60);
            }
            if let Some(SettingValue::Boolean(enabled)) = settings.get(CHANGE_NOTIFICATIONS_KEY) {
                self.change_notifications = *enabled;
            }
            if let Some(SettingValue::Boolean(enabled)) = settings.get(PICKUP_NOTIFICATIONS_KEY) {
                self.pickup_notifications = *enabled;
            }
        }
        ExtensionOutput::default()
    }

    fn on_action(&mut self, action: Action, now: Instant) -> ExtensionOutput {
        if action != Action::Advance || self.storage.is_none() || self.refresh.is_some() {
            return ExtensionOutput::default();
        }
        self.begin_refresh(now);
        invalidated()
    }

    fn widget_view(&self, widget_id: &ExtensionId) -> Option<WidgetView> {
        (widget_id.as_str() == WIDGET_ID)
            .then(|| widget::build(&self.sources, self.refresh.is_some()))
    }

    fn doctor(&self) -> Vec<crate::extension::DiagnosticCheck> {
        use crate::extension::{DiagnosticCheck as Check, DiagnosticStatus as Status};
        let errors: Vec<_> = self
            .sources
            .iter()
            .filter_map(|source| source.error)
            .collect();
        let all_ready = self.sources.iter().all(|source| source.snapshot.is_some());
        let mut checks = Vec::new();
        for (id, title, issue, detail, fix) in [
            (
                "opencli-command",
                "OpenCLI 命令",
                SyncError::OpenCli,
                "无法启动 OpenCLI。",
                "安装 OpenCLI，并确认它位于 Peninsula 可访问的 PATH 中。",
            ),
            (
                "kuaidi-plugin",
                "快递插件",
                SyncError::Plugin,
                "OpenCLI 未找到快递查询插件。",
                "安装 OpenCLI 快递插件后重新启用扩展。",
            ),
            (
                "browser-bridge",
                "浏览器连接",
                SyncError::Browser,
                "没有连接到 Chrome Browser Bridge。",
                "打开 Chrome 并连接 OpenCLI Browser Bridge。",
            ),
            (
                "account-login",
                "电商账号登录",
                SyncError::Login,
                "淘宝或京东需要重新登录。",
                "在 Chrome 中登录对应账号，再刷新快递列表。",
            ),
            (
                "query-result",
                "查询结果",
                SyncError::Query,
                "OpenCLI 查询未能返回可用数据。",
                "检查浏览器登录状态、插件版本和网络后重试。",
            ),
        ] {
            if errors.contains(&issue) {
                checks.push(Check::new(id, title, Status::Failed, detail, Some(fix)));
            } else if all_ready {
                checks.push(Check::passed(id, title, "淘宝和京东最近一次查询均已完成。"));
            } else if self.last_poll.is_some() {
                checks.push(Check::new(
                    id,
                    title,
                    Status::Warning,
                    "最近一次检查未能覆盖所有来源。",
                    Some(fix),
                ));
            } else {
                checks.push(Check::unknown(id, title, "等待首次快递查询完成。"));
            }
        }
        checks
    }
}

impl NotificationPresentationProvider for KuaidiExtension {
    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == NOTIFICATION_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::notifications::NotificationManager;

    #[test]
    fn refresh_button_recovers_when_the_worker_finishes() {
        let mut extension = KuaidiExtension::default();
        let widget = || ExtensionId::builtin(WIDGET_ID);
        let describe = |extension: &KuaidiExtension| {
            serde_json::to_string(&extension.widget_view(&widget()).unwrap().root).unwrap()
        };
        assert!(describe(&extension).contains("立即刷新"));
        let (sender, results) = mpsc::channel();
        extension.refresh = Some(Refresh { results });
        let pending = describe(&extension);
        assert!(pending.contains("刷新中…"));
        assert!(pending.contains("\"enabled\":false"));
        assert!(extension
            .on_action(Action::Advance, Instant::now())
            .invalidated
            .is_empty());
        drop(sender);
        assert_eq!(extension.update(Instant::now()).invalidated, vec![widget()]);
        assert!(describe(&extension).contains("立即刷新"));
    }

    fn row(status: &str, time: &str, detail: &str) -> Vec<Parcel> {
        vec![serde_json::from_value(serde_json::json!({
            "order_id": "001",
            "product_name": "数据线",
            "current_status": status,
            "latest_time": time,
            "latest_detail": detail
        }))
        .unwrap()]
    }

    #[test]
    fn pickup_reminder_takes_priority_and_respects_both_switches() {
        for (change, pickup, expected) in [
            (true, true, Some("淘宝 · 取件提醒")),
            (false, true, Some("淘宝 · 取件提醒")),
            (true, false, Some("淘宝 · 待取件")),
            (false, false, None),
        ] {
            let mut extension = KuaidiExtension {
                change_notifications: change,
                pickup_notifications: pickup,
                ..KuaidiExtension::default()
            };
            let mut pending = Vec::new();
            extension.receive(
                Platform::Taobao,
                Ok(row("运输中", "10:00", "已离开网点")),
                &mut pending,
            );
            assert!(pending.is_empty());

            let detail = "快件已送达菜鸟驿站，请凭取件码 1-2-3456 领取。";
            let mut ready = row("待取件", "11:00", detail);
            ready[0].tracking_no = "TEST001".into();
            ready[0].carrier = "测试快递".into();
            extension.receive(Platform::Taobao, Ok(ready.clone()), &mut pending);
            assert_eq!(pending.len(), usize::from(expected.is_some()));
            if let Some(title) = expected {
                assert_eq!(pending[0].title, title);
                assert!(pending[0].body.contains(detail));
                if pickup {
                    assert!(pending[0].body.contains("测试快递 · TEST001 · 11:00"));
                }
            }
            pending.clear();
            extension.receive(Platform::Taobao, Ok(ready), &mut pending);
            assert!(pending.is_empty());
            extension.receive(
                Platform::Taobao,
                Ok(row("已签收", "12:00", "已完成取件")),
                &mut pending,
            );
            assert_eq!(pending.len(), usize::from(change));
        }
    }

    #[test]
    fn reloads_each_platform_baseline_and_publishes_only_the_changed_logistics() {
        let directory = tempfile::tempdir().unwrap();
        let context = ExtensionContext::new(directory.path().into(), directory.path().into());
        let mut extension = KuaidiExtension::default();
        let mut pending = vec![];
        extension.on_start(&context).unwrap();
        extension.receive(
            Platform::Taobao,
            Ok(row("运输中", "10:00", "已离开网点")),
            &mut pending,
        );
        extension.receive(
            Platform::Jingdong,
            Ok(row("待揽收", "09:00", "仓库已打包")),
            &mut pending,
        );
        assert!(pending.is_empty());

        let mut restarted = KuaidiExtension::default();
        restarted.on_start(&context).unwrap();
        restarted.receive(Platform::Jingdong, Err(SyncError::Login), &mut pending);
        assert_eq!(
            restarted.sources[Platform::Jingdong as usize]
                .snapshot
                .as_ref()
                .unwrap()
                .parcels[0]
                .current_status,
            "待揽收"
        );
        restarted.receive(
            Platform::Taobao,
            Ok(row("运输中", "11:00", "已到达分拨中心")),
            &mut pending,
        );
        restarted.receive(
            Platform::Taobao,
            Ok(row("运输中", "11:00", "已到达分拨中心")),
            &mut pending,
        );
        restarted.receive(
            Platform::Jingdong,
            Ok(row("待揽收", "09:00", "仓库已打包")),
            &mut pending,
        );
        assert_eq!(pending.len(), 1);
        let mut center = NotificationManager::default();
        assert!(center.publish(pending.pop().unwrap()));
        let notification = center.pending().next().unwrap();
        assert_eq!(notification.source_id.as_str(), NOTIFICATION_ID);
        assert_eq!(notification.title, "淘宝 · 运输中");
        assert!(notification.body.contains("已到达分拨中心"));
    }
}
