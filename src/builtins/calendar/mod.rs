mod clock;
mod holiday;
mod model;
mod view;

use chrono::{Datelike, NaiveDate, Timelike, Utc};
use crate::{
    extension::{
        Action, Capability, CompactView, ContributionDescriptor, ContributionKind, Event,
        ExecutionPolicy, ExtensionContext, ExtensionDescriptor, ExtensionId, ExtensionOutput,
        Notification, NotificationPresentationProvider, NotificationPriority, ReloadPolicy,
        RuntimeExtension, SettingSchema, SettingValue, Settings, WidgetView,
    },
    ui::declarative::{ColorSpec, CompactLayout, ViewNode},
};
use model::CalendarSettings;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io::Write,
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

const ID: &str = "builtin.calendar";
const WIDGET_ID: &str = "builtin.calendar/month";
const COMPACT_ID: &str = "builtin.calendar/today";
const NOTIFICATION_ID: &str = "builtin.calendar/holidays";
const TICK: &str = "calendar-reminder-check";
const VIEW_TICK: &str = "calendar-view-day-change";

fn enum_field(key: &str, label: &str, default: &str, choices: &[&str]) -> SettingSchema {
    let mut field = SettingSchema::new(
        key, SettingValue::Enum(default.to_owned()), label,
    );
    field.options = choices.iter().map(|choice| (*choice).into()).collect();
    field
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SentHistory {
    #[serde(default)]
    sent: BTreeSet<String>,
}

/// One extension owns all date views and the optional holiday reminder.
/// No timer or background service remains active when its contribution stops.
#[derive(Default)]
pub struct CalendarExtension {
    settings: CalendarSettings,
    month: Option<NaiveDate>,
    selected: Option<NaiveDate>,
    last_view_day: Option<NaiveDate>,
    twelve_hour: bool,
    show_date: bool,
    reminder_settings: Settings,
    reminder_context: Option<ExtensionContext>,
    sent: SentHistory,
    store: Option<PathBuf>,
}

impl CalendarExtension {
    fn persist(&self) -> Result<(), String> {
        let Some(path) = &self.store else { return Ok(()); };
        let parent = path.parent().ok_or("invalid calendar data path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut temp, &self.sent).map_err(|e| e.to_string())?;
        temp.flush().map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.error.to_string())?;
        Ok(())
    }

    fn remind_mode(&self) -> bool {
        matches!(self.reminder_settings.get("when"),
            Some(SettingValue::Enum(value)) if value == "前一天")
    }

    fn reminder_hour_minute(&self) -> (u32, u32) {
        let hour = match self.reminder_settings.get("hour") {
            Some(SettingValue::Integer(value)) => (*value).clamp(0, 23) as u32,
            _ => 9,
        };
        let minute = match self.reminder_settings.get("minute") {
            Some(SettingValue::Integer(value)) => (*value).clamp(0, 59) as u32,
            _ => 0,
        };
        (hour, minute)
    }

    fn check_due(&mut self, now: chrono::DateTime<chrono::Utc>) -> Vec<Notification> {
        let civil = now.with_timezone(&self.settings.timezone());
        let (hour, minute) = self.reminder_hour_minute();
        if civil.hour() != hour || civil.minute() != minute {
            return vec![];
        }
        let advance = self.remind_mode();
        let target = civil.date_naive() + chrono::Duration::days(if advance { 1 } else { 0 });
        let festivals = holiday::festivals(target, &self.settings);
        let mut emitted = vec![];
        for name in festivals {
            let key = format!("{}:{}:{}", target, if advance { "eve" } else { "day" }, name);
            if !self.sent.sent.insert(key.clone()) {
                continue;
            }
            emitted.push(Notification {
                source_id: ExtensionId::builtin(NOTIFICATION_ID),
                id: key,
                title: if advance { format!("明天是{name}") } else { format!("今天是{name}") },
                body: format!("{} · {}。祝你节日快乐！", target, self.settings.country_label()),
                icon: Some("calendar".into()),
                priority: NotificationPriority::Normal,
                actions: vec![],
                created_at: SystemTime::now(),
            });
        }
        // Keep one year of de-duplication keys, including over app restarts.
        let cutoff = (target - chrono::Duration::days(370)).format("%Y-%m-%d").to_string();
        self.sent.sent.retain(|key| key.as_str() >= cutoff.as_str());
        if !emitted.is_empty() {
            if let Err(error) = self.persist() {
                eprintln!("calendar reminder history not saved: {error}");
            }
        }
        emitted
    }

    fn schedule_next(context: &ExtensionContext) {
        // Twenty seconds safely catches a target minute despite scheduling
        // jitter and survives a daylight-saving offset transition.
        context.schedule_at(TICK, Instant::now() + Duration::from_secs(20));
    }
}

impl NotificationPresentationProvider for CalendarExtension {
    fn handles_notification(&self, id: &ExtensionId) -> bool {
        id.as_str() == NOTIFICATION_ID
    }
}

impl RuntimeExtension for CalendarExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        let widget = ContributionDescriptor::builtin(
            WIDGET_ID, ContributionKind::Widget, "日历", "calendar", true, 4,
        )
        .with_size_hint(800.0, 320.0)
        .with_widget_bounds(crate::ui::declarative::WidgetBounds {
            min: [420.0, 280.0],
            max: [Some(1050.0), None],
            fit_height: true, resizable: true,
            ..Default::default()
        });
        let compact = ContributionDescriptor::builtin(
            COMPACT_ID, ContributionKind::Compact, "日期", "calendar", false, 9,
        ).with_size_hint(105.0, 28.0);
        let reminders = ContributionDescriptor::builtin(
            NOTIFICATION_ID, ContributionKind::Notification, "节日提醒", "calendar", false, 9,
        )
        .with_capabilities(vec![Capability::Event, Capability::Schedule])
        .with_settings(1, vec![
            enum_field("when", "提醒时间", "当天", &["前一天", "当天"]),
            SettingSchema::new("hour", SettingValue::Integer(9), "小时（24 小时制）").range(0.0, 23.0),
            SettingSchema::new("minute", SettingValue::Integer(0), "分钟").range(0.0, 59.0),
        ]);
        let mut descriptor = ExtensionDescriptor::builtin(
            ID, "日历与时间", "本地时间、公历、农历、节日与提醒",
            vec![widget, compact, clock::clock_descriptor(), reminders],
        );
        descriptor.execution_policy = ExecutionPolicy::EventDriven;
        descriptor.settings_version = 2;
        descriptor.settings = vec![
            enum_field("country", "国家（ISO 3166）", "CN",
                &["CN", "US", "GB", "DE", "FR", "CA", "AU", "ID"]),
            enum_field("region", "地区／时区", "CN-BJ", &[
                "默认", "CN-BJ", "CN-SH", "CN-HK", "CN-TW",
                "US-NY", "US-CA", "CA-ON", "AU-NSW",
            ]),
            SettingSchema::new("show_lunar", SettingValue::Boolean(true), "中国地区显示农历"),
            SettingSchema::new("twelve_hour", SettingValue::Boolean(false), "12 小时制"),
            SettingSchema::new("show_date", SettingValue::Boolean(false), "时间旁显示日期"),
        ];
        descriptor
    }

    fn migrate_settings(
        &self, id: &ExtensionId, old_version: u32, mut values: Settings,
    ) -> Result<Settings, String> {
        // Preserve country/region while adding the clock-format options.
        if id.as_str() == ID && old_version == 1 {
            return Ok(values);
        }
        Err(format!("Unsupported settings version {old_version}"))
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        let path = ctx.extension_data_dir(&ExtensionId::builtin(ID)).join("calendar-reminders.json");
        match fs::read(&path) {
            Ok(bytes) => {
                self.sent = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        self.store = Some(path);
        Ok(())
    }

    fn on_stop(&mut self, _: &ExtensionContext) -> Result<(), String> {
        let saved = self.persist();
        self.reminder_context = None;
        self.store = None;
        saved
    }

    fn widget_view(&self, id: &ExtensionId) -> Option<WidgetView> {
        (id.as_str() == WIDGET_ID).then(|| view::build(self))
    }

    fn compact_refresh_interval(&self) -> Option<Duration> {
        Some(Duration::from_secs(1))
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let today = self.settings.today();
        let label = holiday::day_subtitle(today, &self.settings);
        let text = label.map_or_else(|| today.format("%m/%d").to_string(),
            |event| format!("{}/{}  {event}", today.month(), today.day()));
        let date = CompactView {
            id: ExtensionId::builtin(COMPACT_ID),
            source_extension_id: ExtensionId::builtin(COMPACT_ID),
            content: ViewNode::compact(
                ViewNode::icon("calendar", 17.0).color(ColorSpec::Accent),
                super::compact_value(text, false),
                CompactLayout { leading_size: [18.0, 20.0], gap: 5.0, ..Default::default() },
            ),
            visibility: Default::default(),
            min_width: 80.0,
            preferred_width: 122.0,
            fit_width: true,
            priority: 0,
            tooltip: Some(format!("{} · {}", today, self.settings.country_label())),
        };
        vec![clock::compact_view(self.twelve_hour, self.show_date), date]
    }

    fn on_action(&mut self, action: Action, _: Instant) -> ExtensionOutput {
        let today = self.settings.today();
        let current = self.month.unwrap_or_else(|| model::start_month(today));
        match action {
            Action::Custom(ref s) if s == "calendar:previous" => {
                let next = model::shift_month(current, -1);
                self.month = Some(next);
                self.selected = Some(next);
            }
            Action::Custom(ref s) if s == "calendar:next" => {
                let next = model::shift_month(current, 1);
                self.month = Some(next);
                self.selected = Some(next);
            }
            Action::Custom(ref s) if s == "calendar:today" => {
                self.month = Some(model::start_month(today));
                self.selected = Some(today);
            }
            Action::Custom(ref s) if s.starts_with("calendar:select:") => {
                if let Ok(date) = NaiveDate::parse_from_str(&s["calendar:select:".len()..], "%Y-%m-%d") {
                    self.selected = Some(date);
                    // Adjacent-month dates in the complete calendar grid
                    // should navigate to their own month when selected.
                    self.month = Some(model::start_month(date));
                }
            }
            _ => return ExtensionOutput::default(),
        }
        ExtensionOutput { invalidated: vec![ExtensionId::builtin(WIDGET_ID)], ..Default::default() }
    }

    fn on_settings_changed(&mut self, _: &ExtensionContext, _: &ExtensionId,
        values: &Settings, _: ReloadPolicy) -> ExtensionOutput {
        self.settings.apply(values);
        self.twelve_hour = matches!(values.get("twelve_hour"), Some(SettingValue::Boolean(true)));
        self.show_date = matches!(values.get("show_date"), Some(SettingValue::Boolean(true)));
        if let Some(context) = &self.reminder_context {
            context.schedule_at(TICK, Instant::now());
        }
        ExtensionOutput { invalidated: vec![
            ExtensionId::builtin(WIDGET_ID), ExtensionId::builtin(COMPACT_ID),
            ExtensionId::builtin(clock::CLOCK_ID)
        ], ..Default::default() }
    }

    fn on_contribution_start(&mut self, ctx: &ExtensionContext, id: &ExtensionId) -> Result<(), String> {
        if id.as_str() == WIDGET_ID {
            self.last_view_day = Some(self.settings.today());
            ctx.schedule_at(VIEW_TICK, Instant::now() + Duration::from_secs(60));
        }
        if id.as_str() == NOTIFICATION_ID {
            self.reminder_context = Some(ctx.clone());
            ctx.schedule_at(TICK, Instant::now());
        }
        Ok(())
    }

    fn on_contribution_stop(&mut self, ctx: &ExtensionContext, id: &ExtensionId) -> Result<(), String> {
        if id.as_str() == WIDGET_ID {
            ctx.cancel_task(VIEW_TICK);
            self.last_view_day = None;
        }
        if id.as_str() == NOTIFICATION_ID {
            ctx.cancel_task(TICK);
            self.reminder_context = None;
        }
        Ok(())
    }

    fn on_contribution_state(&mut self, _: &ExtensionContext,
        active: &BTreeMap<ExtensionId, Settings>) {
        let id = ExtensionId::builtin(NOTIFICATION_ID);
        self.reminder_settings = active.get(&id).cloned().unwrap_or_default();
        if let Some(ctx) = &self.reminder_context {
            ctx.schedule_at(TICK, Instant::now());
        }
    }

    fn on_contribution_event(&mut self, ctx: &ExtensionContext,
        id: &ExtensionId, event: &Event, _: Instant) -> ExtensionOutput {
        if id.as_str() == WIDGET_ID && event == &Event::Custom(VIEW_TICK.to_owned()) {
            ctx.schedule_at(VIEW_TICK, Instant::now() + Duration::from_secs(60));
            let today = self.settings.today();
            let changed = self.last_view_day.replace(today) != Some(today);
            return ExtensionOutput {
                invalidated: if changed { vec![ExtensionId::builtin(WIDGET_ID)] } else { vec![] },
                ..Default::default()
            };
        }
        if id.as_str() != NOTIFICATION_ID || event != &Event::Custom(TICK.to_owned()) {
            return ExtensionOutput::default();
        }
        let notifications = self.check_due(Utc::now());
        Self::schedule_next(ctx);
        ExtensionOutput { notifications, ..Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_has_independent_widget_compact_and_notification_controls() {
        let d = CalendarExtension::default().descriptor();
        assert_eq!(d.contributions.len(), 4);
        assert!(d.contributions.iter().any(|entry|
            entry.id.as_str() == "builtin.calendar/time"
            && entry.kind == ContributionKind::Compact));
        assert_eq!(d.contributions[3].kind, ContributionKind::Notification);
        assert_eq!(d.contributions[3].settings.len(), 3);
        assert_eq!(d.settings.len(), 5);
        assert!(d.settings.iter().any(|field| field.key == "country"));
        assert!(d.settings.iter().any(|field| field.key == "region"));
    }
    #[test]
    fn festival_reminder_fires_on_local_clock_the_day_before_and_deduplicates() {
        use chrono::TimeZone;
        let mut calendar = CalendarExtension::default();
        calendar.reminder_settings.insert("when".into(), SettingValue::Enum("前一天".into()));
        calendar.reminder_settings.insert("hour".into(), SettingValue::Integer(18));
        calendar.reminder_settings.insert("minute".into(), SettingValue::Integer(30));
        calendar.settings = CalendarSettings::default();
        // 2026-02-16 10:30 UTC is 18:30 in China, one day before 春节.
        let now = Utc.with_ymd_and_hms(2026, 2, 16, 10, 30, 0).single().unwrap();
        let alerts = calendar.check_due(now);
        assert!(alerts.iter().any(|alert| alert.title.contains("春节")));
        assert!(calendar.check_due(now).is_empty());
        assert!(calendar.check_due(now + chrono::Duration::minutes(1)).is_empty());
    }

    #[test]
    fn mode_and_time_are_independent_of_calendar_navigation() {
        let mut calendar = CalendarExtension::default();
        calendar.reminder_settings.insert("when".into(), SettingValue::Enum("前一天".into()));
        calendar.reminder_settings.insert("hour".into(), SettingValue::Integer(18));
        calendar.reminder_settings.insert("minute".into(), SettingValue::Integer(30));
        calendar.on_action(Action::Custom("calendar:next".into()), Instant::now());
        assert!(calendar.remind_mode());
        assert_eq!(calendar.reminder_hour_minute(), (18, 30));
    }
}
