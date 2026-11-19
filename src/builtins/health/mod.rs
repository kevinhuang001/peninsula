mod reminder;
mod view;

use crate::extension::{
    Action, Capability, ContributionDescriptor, ContributionKind, Event, ExecutionPolicy,
    ExtensionContext, ExtensionDescriptor, ExtensionId, ExtensionOutput, Notification,
    NotificationAction, NotificationKey, NotificationPresentationProvider, NotificationPriority,
    ReloadPolicy, RuntimeExtension, ScheduledTask, SettingSchema, SettingValue, Settings,
    WidgetView,
};
use reminder::{local_time, next_midnight, Reminder};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    time::{Duration, Instant, SystemTime},
};

const ID: &str = "builtin.health";
const WIDGET_ID: &str = "builtin.health/overview";
const NOTIFICATION_ID: &str = "builtin.health/reminders";
const MIDNIGHT_TASK: &str = "health-midnight";
const SYNC_TASK: &str = "health-sync";
const WATER: usize = 0;
const STAND: usize = 1;
const TASKS: [&str; 2] = ["health-water", "health-stand"];
const ACTIONS: [&str; 2] = ["drink", "stand"];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedState {
    reminders: [Reminder; 2],
    day: String,
    water_ml: u64,
    #[serde(default)]
    pending_reminders: [Vec<String>; 2],
}

impl SavedState {
    fn new(now: SystemTime) -> Self {
        Self {
            reminders: [Reminder::new(now), Reminder::new(now)],
            day: local_time(now).date().to_string(),
            water_ml: 0,
            pending_reminders: Default::default(),
        }
    }

    fn roll_day(&mut self, now: SystemTime) -> bool {
        let day = local_time(now).date().to_string();
        if self.day == day {
            return false;
        }
        self.day = day;
        self.water_ml = 0;
        true
    }

    fn water_today(&self, now: SystemTime) -> u64 {
        if self.day == local_time(now).date().to_string() {
            self.water_ml
        } else {
            0
        }
    }
}

pub struct HealthExtension {
    state: SavedState,
    intervals: [Duration; 2],
    daily_goal_ml: u64,
    sip_ml: u64,
    context: Option<ExtensionContext>,
    storage_error: Option<String>,
    resolved_notifications: Vec<NotificationKey>,
}

impl Default for HealthExtension {
    fn default() -> Self {
        Self {
            state: SavedState::new(SystemTime::now()),
            intervals: [Duration::from_secs(30 * 60), Duration::from_secs(60 * 60)],
            daily_goal_ml: 2000,
            sip_ml: 100,
            context: None,
            storage_error: None,
            resolved_notifications: Vec::new(),
        }
    }
}

impl HealthExtension {
    fn goal_met(&self, now: SystemTime) -> bool {
        self.state.water_today(now) >= self.daily_goal_ml
    }

    fn save(&self) -> Result<(), String> {
        let Some(ctx) = &self.context else {
            return Ok(());
        };
        let directory = ctx.extension_data_dir(&ExtensionId::builtin(ID));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let mut file =
            tempfile::NamedTempFile::new_in(&directory).map_err(|error| error.to_string())?;
        serde_json::to_writer(&mut file, &self.state).map_err(|error| error.to_string())?;
        file.flush().map_err(|error| error.to_string())?;
        file.as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        file.persist(directory.join("health.json"))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn save_status(&mut self) {
        self.storage_error = self
            .save()
            .err()
            .map(|error| format!("记录保存失败：{error}"));
    }

    fn schedule(&mut self, now: SystemTime, instant: Instant) -> ExtensionOutput {
        let mut schedule = vec![ScheduledTask {
            task_id: MIDNIGHT_TASK.into(),
            deadline: instant + next_midnight(now).duration_since(now).unwrap_or_default(),
        }];
        for (index, task) in TASKS.iter().enumerate() {
            if index == WATER && self.goal_met(now) {
                continue;
            }
            schedule.push(ScheduledTask {
                task_id: (*task).into(),
                deadline: instant
                    + self.state.reminders[index].remaining(now, self.intervals[index]),
            });
        }
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            cancel_tasks: self
                .goal_met(now)
                .then(|| TASKS[WATER].to_owned())
                .into_iter()
                .collect(),
            schedule,
            resolved_notifications: std::mem::take(&mut self.resolved_notifications),
            ..Default::default()
        }
    }

    fn record(&mut self, index: usize, now: SystemTime) {
        self.state.roll_day(now);
        self.state.reminders[index] = Reminder::new(now);
        self.resolve_reminders(index);
        if index == WATER {
            self.state.water_ml = self.state.water_ml.saturating_add(self.sip_ml);
        }
        self.save_status();
    }

    fn resolve_reminders(&mut self, index: usize) {
        self.resolved_notifications
            .extend(
                self.state.pending_reminders[index]
                    .drain(..)
                    .map(|id| NotificationKey {
                        source_extension_id: ExtensionId::builtin(NOTIFICATION_ID),
                        notification_id: id,
                    }),
            );
    }

    fn notification(&self, index: usize, now: SystemTime) -> Notification {
        let (title, body, icon, label) = if index == WATER {
            (
                "喝水时间到",
                "喝一口水，给自己补充一点水分。",
                "droplet",
                "喝一口水",
            )
        } else {
            (
                "起来活动一下",
                "离上次起身已经有一会儿了，站起来走动一下吧。",
                "health/stand",
                "起身",
            )
        };
        Notification {
            source_id: ExtensionId::builtin(NOTIFICATION_ID),
            id: format!(
                "{}-{}",
                TASKS[index],
                now.duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            title: title.into(),
            body: body.into(),
            icon: Some(icon.into()),
            priority: NotificationPriority::Normal,
            actions: vec![NotificationAction {
                id: ACTIONS[index].into(),
                label: label.into(),
                data: None,
            }],
            created_at: now,
        }
    }

    fn remind(&mut self, now: SystemTime, instant: Instant) -> ExtensionOutput {
        let mut changed = self.state.roll_day(now);
        let mut notifications = Vec::new();
        for index in [WATER, STAND] {
            if index == WATER && self.goal_met(now) {
                continue;
            }
            if self.state.reminders[index].notify_if_due(now, self.intervals[index]) {
                self.resolve_reminders(index);
                let notification = self.notification(index, now);
                self.state.pending_reminders[index].push(notification.id.clone());
                notifications.push(notification);
                changed = true;
            }
        }
        if changed {
            self.save_status();
        }
        let mut output = self.schedule(now, instant);
        output.notifications = notifications;
        output
    }
}

impl RuntimeExtension for HealthExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        let widget = ContributionDescriptor::builtin(
            WIDGET_ID,
            ContributionKind::Widget,
            "健康",
            "health/heart",
            true,
            7,
        )
        .with_size_hint(440.0, 280.0)
        .with_widget_bounds(crate::ui::declarative::WidgetBounds {
            min: [320.0, 220.0],
            max: [Some(620.0), Some(420.0)],
            resizable: true,
            fit_height: true,
            ..Default::default()
        })
        .with_capabilities(vec![
            Capability::View,
            Capability::Action,
            Capability::Schedule,
            Capability::Event,
        ]);
        let notifications = ContributionDescriptor::builtin(
            NOTIFICATION_ID,
            ContributionKind::Notification,
            "健康提醒",
            "health/heart",
            true,
            7,
        );
        let mut descriptor = ExtensionDescriptor::builtin(
            ID,
            "健康",
            "喝水记录与久坐提醒",
            vec![widget, notifications],
        );
        descriptor.execution_policy = ExecutionPolicy::EventDriven;
        descriptor.settings = vec![
            SettingSchema::new(
                "daily_goal_ml",
                SettingValue::Integer(2000),
                "每日饮水目标（ml）",
            )
            .range(100.0, 10_000.0),
            SettingSchema::new("sip_ml", SettingValue::Integer(100), "每次喝一口水（ml）")
                .range(10.0, 2000.0),
            SettingSchema::new(
                "water_minutes",
                SettingValue::Integer(30),
                "喝水提醒间隔（分钟）",
            )
            .range(1.0, 240.0),
            SettingSchema::new(
                "stand_minutes",
                SettingValue::Integer(60),
                "久坐提醒间隔（分钟）",
            )
            .range(1.0, 240.0),
        ];
        descriptor
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        let now = SystemTime::now();
        let path = ctx
            .extension_data_dir(&ExtensionId::builtin(ID))
            .join("health.json");
        match std::fs::File::open(path) {
            Ok(file) => {
                self.state = serde_json::from_reader(file).map_err(|error| error.to_string())?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.state = SavedState::new(now)
            }
            Err(error) => return Err(error.to_string()),
        }
        self.state.roll_day(now);
        for reminder in &mut self.state.reminders {
            reminder.clamp_future_times(now);
        }
        self.context = Some(ctx.clone());
        if self.goal_met(now) {
            self.resolve_reminders(WATER);
        }
        self.save_status();
        for task in self.schedule(now, Instant::now()).schedule {
            ctx.schedule_at(task.task_id, task.deadline);
        }
        Ok(())
    }

    fn on_stop(&mut self, _: &ExtensionContext) -> Result<(), String> {
        let result = self.save();
        self.context = None;
        result
    }

    fn widget_view(&self, id: &ExtensionId) -> Option<WidgetView> {
        (id.as_str() == WIDGET_ID).then(|| view::build(self, SystemTime::now()))
    }

    fn on_action(&mut self, action: Action, instant: Instant) -> ExtensionOutput {
        let Action::Custom(key) = action else {
            return ExtensionOutput::default();
        };
        let Some(index) = ACTIONS.iter().position(|action| *action == key) else {
            return ExtensionOutput::default();
        };
        let now = SystemTime::now();
        self.record(index, now);
        self.schedule(now, instant)
    }

    fn on_event(&mut self, event: &Event, instant: Instant) -> ExtensionOutput {
        match event {
            Event::Custom(task)
                if TASKS.contains(&task.as_str()) || task == MIDNIGHT_TASK || task == SYNC_TASK =>
            {
                self.remind(SystemTime::now(), instant)
            }
            _ => ExtensionOutput::default(),
        }
    }

    fn on_settings_changed(
        &mut self,
        _: &ExtensionContext,
        _: &ExtensionId,
        settings: &Settings,
        _: ReloadPolicy,
    ) -> ExtensionOutput {
        for (index, key) in ["water_minutes", "stand_minutes"].into_iter().enumerate() {
            if let Some(SettingValue::Integer(minutes)) = settings.get(key) {
                self.intervals[index] = Duration::from_secs(*minutes as u64 * 60);
            }
        }
        if let Some(SettingValue::Integer(ml)) = settings.get("daily_goal_ml") {
            self.daily_goal_ml = *ml as u64;
        }
        if let Some(SettingValue::Integer(ml)) = settings.get("sip_ml") {
            self.sip_ml = *ml as u64;
        }
        if self.goal_met(SystemTime::now()) {
            self.resolve_reminders(WATER);
            self.save_status();
        }
        // Initial configuration precedes on_start and restoration of saved state.
        if self.context.is_some() {
            self.schedule(SystemTime::now(), Instant::now())
        } else {
            ExtensionOutput::default()
        }
    }

    fn status_message(&self) -> Option<String> {
        self.storage_error.clone()
    }
}

impl NotificationPresentationProvider for HealthExtension {
    fn handles_notification(&self, id: &ExtensionId) -> bool {
        id.as_str() == NOTIFICATION_ID
    }

    fn on_notification_action(
        &mut self,
        notification: &Notification,
        action: &NotificationAction,
    ) -> Result<(), String> {
        if !self.handles_notification(&notification.source_id) {
            return Err("未知的健康提醒".into());
        }
        let index = ACTIONS
            .iter()
            .position(|id| *id == action.id)
            .ok_or("未知的健康操作")?;
        let now = SystemTime::now();
        self.record(index, now);
        if let Some(ctx) = &self.context {
            ctx.schedule_at(SYNC_TASK, Instant::now());
        }
        self.storage_error.clone().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(root: &std::path::Path) -> ExtensionContext {
        ExtensionContext::new(root.join("data"), root.join("config"))
    }

    #[test]
    fn drinking_updates_volume_and_only_resets_the_water_timer() {
        let mut health = HealthExtension::default();
        let start = SystemTime::now();
        health.state = SavedState::new(start);
        let now = start + Duration::from_secs(600);
        health.record(WATER, now);
        assert_eq!(health.state.water_today(now), 100);
        assert_eq!(
            health.state.reminders[WATER].next_due(health.intervals[WATER]),
            now + Duration::from_secs(1800)
        );
        assert_eq!(health.state.reminders[STAND].last_action, start);
        health.record(STAND, now);
        assert_eq!(health.state.reminders[STAND].last_action, now);
        assert_eq!(health.state.water_today(now), 100);
    }

    #[test]
    fn goal_suppresses_water_notifications_and_midnight_restores_them() {
        let start = SystemTime::now();
        let mut health = HealthExtension {
            state: SavedState::new(start),
            daily_goal_ml: 200,
            ..HealthExtension::default()
        };
        health.record(WATER, start);
        health.record(WATER, start);
        let output = health.remind(start + Duration::from_secs(3600), Instant::now());
        assert!(health.goal_met(start));
        assert_eq!(output.notifications.len(), 1);
        assert_eq!(output.notifications[0].actions[0].id, "stand");
        assert!(!output
            .schedule
            .iter()
            .any(|task| task.task_id == TASKS[WATER]));
        assert!(output.cancel_tasks.contains(&TASKS[WATER].into()));
        assert!(output
            .schedule
            .iter()
            .any(|task| task.task_id == MIDNIGHT_TASK));
        let tomorrow = next_midnight(start) + Duration::from_secs(1);
        health.remind(tomorrow, Instant::now());
        assert_eq!(health.state.water_today(tomorrow), 0);
        assert!(!health.goal_met(tomorrow));
        assert!(health
            .schedule(tomorrow, Instant::now())
            .schedule
            .iter()
            .any(|task| task.task_id == TASKS[WATER]));
    }

    #[test]
    fn state_survives_restart_and_settings_change_relative_to_the_last_action() {
        let root = tempfile::tempdir().unwrap();
        let ctx = context(root.path());
        let mut health = HealthExtension::default();
        health.on_start(&ctx).unwrap();
        health.record(WATER, SystemTime::now());
        let last = health.state.reminders[WATER].last_action;
        health.on_stop(&ctx).unwrap();
        let mut restored = HealthExtension::default();
        restored.on_start(&ctx).unwrap();
        assert_eq!(restored.state.water_ml, 100);
        assert_eq!(restored.state.reminders[WATER].last_action, last);
        let output = restored.on_settings_changed(
            &ctx,
            &ExtensionId::builtin(ID),
            &[
                ("daily_goal_ml".into(), SettingValue::Integer(100)),
                ("sip_ml".into(), SettingValue::Integer(250)),
                ("water_minutes".into(), SettingValue::Integer(15)),
            ]
            .into(),
            ReloadPolicy::Live,
        );
        assert_eq!(
            restored.state.reminders[WATER].next_due(restored.intervals[WATER]),
            last + Duration::from_secs(900)
        );
        assert!(!output
            .schedule
            .iter()
            .any(|task| task.task_id == TASKS[WATER]));
        restored.record(WATER, SystemTime::now());
        assert_eq!(restored.state.water_ml, 350);
    }

    #[test]
    fn notification_buttons_record_actions_and_replace_the_scheduled_deadlines() {
        let root = tempfile::tempdir().unwrap();
        let ctx = context(root.path());
        let mut health = HealthExtension::default();
        health.on_start(&ctx).unwrap();
        ctx.drain_tasks();
        let notification = health.notification(WATER, SystemTime::now());
        health
            .on_notification_action(&notification, &notification.actions[0])
            .unwrap();
        assert_eq!(health.state.water_ml, health.sip_ml);
        let (scheduled, canceled) = ctx.drain_tasks();
        assert!(scheduled.iter().any(|(id, _)| id == SYNC_TASK));
        assert!(!canceled.contains(&TASKS[WATER].into()));
        let output = health.on_event(&Event::Custom(SYNC_TASK.into()), Instant::now());
        assert!(output
            .schedule
            .iter()
            .any(|task| task.task_id == TASKS[WATER]));
    }

    #[test]
    fn reaching_the_goal_resolves_an_already_queued_water_reminder() {
        let now = SystemTime::now();
        let mut health = HealthExtension::default();
        health.daily_goal_ml = health.sip_ml;
        health.state.reminders[WATER] = Reminder::new(now - health.intervals[WATER]);
        let reminder = health.remind(now, Instant::now()).notifications.remove(0);
        health.record(WATER, now);
        let output = health.schedule(now, Instant::now());
        assert_eq!(output.resolved_notifications, [reminder.key()]);
        assert!(!output
            .schedule
            .iter()
            .any(|task| task.task_id == TASKS[WATER]));
    }
}
