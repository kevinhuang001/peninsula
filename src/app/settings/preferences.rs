use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::PathBuf,
};

use crate::{
    builtins::{kuaidi, registry, weather::WeatherPreferences},
    extension::{ExtensionId, ExtensionKind, SettingValue, Settings},
};

use super::quiet_hours::{QuietHours, QuietPeriod, MAX_QUIET_PERIODS};
use super::theme::{parse_accent, WidgetTheme, DEFAULT_WIDGET_ACCENT};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactItemPreference {
    pub id: ExtensionId,
    pub enabled: bool,
    pub order: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompactPreferences {
    pub compact_items: Vec<CompactItemPreference>,
    pub quiet_hours: QuietHours,
    pub widget_theme: WidgetTheme,
    pub widget_accent: [u8; 3],
    pub pomodoro_focus_minutes: u16,
    pub pomodoro_break_minutes: u16,
    pub kuaidi_refresh_minutes: u16,
    pub kuaidi_change_notifications: bool,
    pub kuaidi_pickup_notifications: bool,
    pub bilibili_enabled: bool,
    pub bilibili_poll_seconds: u16,
    pub opencli_path: String,
    pub weather: WeatherPreferences,
}

impl Default for CompactPreferences {
    fn default() -> Self {
        let compact_items = registry()
            .by_kind(ExtensionKind::Compact)
            .into_iter()
            .map(|descriptor| CompactItemPreference {
                id: descriptor.id.clone(),
                enabled: descriptor.default_enabled,
                order: descriptor.default_order,
            })
            .collect();
        Self {
            compact_items,
            quiet_hours: QuietHours::default(),
            widget_theme: WidgetTheme::default(),
            widget_accent: DEFAULT_WIDGET_ACCENT,
            pomodoro_focus_minutes: 25,
            pomodoro_break_minutes: 5,
            kuaidi_refresh_minutes: kuaidi::DEFAULT_REFRESH_MINUTES,
            kuaidi_change_notifications: true,
            kuaidi_pickup_notifications: true,
            bilibili_enabled: false,
            bilibili_poll_seconds: 60,
            opencli_path: String::new(),
            weather: WeatherPreferences::default(),
        }
    }
}

impl CompactPreferences {
    pub fn parse(source: &str) -> Self {
        let mut settings = Self::default();
        let mut explicit_ids = HashSet::new();
        let mut legacy_values = HashMap::new();
        let mut legacy_order = None;
        for line in source.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "widget_theme" => {
                    if let Some(theme) = WidgetTheme::parse(value) {
                        settings.widget_theme = theme;
                    }
                }
                "widget_accent" => {
                    if let Some(accent) = parse_accent(value) {
                        settings.widget_accent = accent;
                    }
                }
                "extension" => {
                    let mut fields = value.split('|');
                    let (Some(id), Some(enabled), Some(order)) =
                        (fields.next(), fields.next(), fields.next())
                    else {
                        continue;
                    };
                    let (Ok(id), Some(enabled), Ok(order)) = (
                        ExtensionId::new(id.trim()),
                        parse_bool(enabled),
                        order.trim().parse::<u32>(),
                    ) else {
                        continue;
                    };
                    explicit_ids.insert(id.clone());
                    if let Some(item) = settings.compact_items.iter_mut().find(|item| item.id == id)
                    {
                        item.enabled = enabled;
                        item.order = order;
                    } else {
                        settings
                            .compact_items
                            .push(CompactItemPreference { id, enabled, order });
                    }
                }
                "compact_order" => legacy_order = Some(parse_legacy_order(value)),
                "quiet_periods" => {
                    let manual = settings.quiet_hours.manual_enabled;
                    settings.quiet_hours = parse_periods(value);
                    settings.quiet_hours.manual_enabled = manual;
                }
                "pomodoro_focus_minutes" => {
                    settings.pomodoro_focus_minutes = parse_duration(value, 25);
                }
                "pomodoro_break_minutes" => {
                    settings.pomodoro_break_minutes = parse_duration(value, 5);
                }
                "bilibili_enabled" => {
                    settings.bilibili_enabled = parse_bool(value).unwrap_or(false);
                }
                "bilibili_poll_seconds" => {
                    settings.bilibili_poll_seconds = value
                        .trim()
                        .parse::<u16>()
                        .ok()
                        .filter(|value| (15..=3600).contains(value))
                        .unwrap_or(60)
                }
                "opencli_path" => settings.opencli_path = value.trim().to_owned(),
                key => {
                    if settings.weather.apply_line(key, value) {
                        continue;
                    }
                    let Some(value) = parse_bool(value) else {
                        continue;
                    };
                    if key == "quiet_manual" {
                        settings.quiet_hours.manual_enabled = value;
                    } else if let Some(id) = legacy_compact_id(key) {
                        legacy_values.insert(id, value);
                    }
                }
            }
        }
        for (id, enabled) in legacy_values {
            if let Some(item) = settings
                .compact_items
                .iter_mut()
                .find(|item| item.id == id && !explicit_ids.contains(&item.id))
            {
                item.enabled = enabled;
            }
        }
        if let Some(ids) = legacy_order {
            for (order, id) in ids.into_iter().enumerate() {
                if let Some(item) = settings.compact_items.iter_mut().find(|item| item.id == id) {
                    if !explicit_ids.contains(&id) {
                        item.order = order as u32;
                    }
                }
            }
        }
        settings.normalize_order();
        settings
    }

    pub fn encode(&self) -> String {
        let mut lines: Vec<_> = self
            .ordered_compact_items()
            .into_iter()
            .map(|item| format!("extension={}|{}|{}", item.id, item.enabled, item.order))
            .collect();
        lines.push(format!("quiet_manual={}", self.quiet_hours.manual_enabled));
        lines.push(format!(
            "quiet_periods={}",
            encode_periods(&self.quiet_hours)
        ));
        lines.push(format!("bilibili_enabled={}", self.bilibili_enabled));
        lines.extend(self.weather.encode_lines());
        lines.push(format!("widget_theme={}", self.widget_theme.name()));
        let [r, g, b] = self.widget_accent;
        lines.push(format!("widget_accent=#{r:02x}{g:02x}{b:02x}"));
        format!("{}\n", lines.join("\n"))
    }

    // Existing controls are an adapter; the runtime reads the registered schema.
    pub fn extension_settings(&self) -> HashMap<ExtensionId, Settings> {
        use SettingValue::*;
        [
            (
                ExtensionId::builtin(crate::builtins::pomodoro::WIDGET_ID),
                [
                    (
                        "focus_minutes".into(),
                        Integer(i64::from(self.pomodoro_focus_minutes)),
                    ),
                    (
                        "break_minutes".into(),
                        Integer(i64::from(self.pomodoro_break_minutes)),
                    ),
                ]
                .into(),
            ),
            (
                ExtensionId::builtin(kuaidi::WIDGET_ID),
                [
                    (
                        kuaidi::REFRESH_MINUTES_KEY.into(),
                        Integer(i64::from(self.kuaidi_refresh_minutes)),
                    ),
                    (
                        kuaidi::CHANGE_NOTIFICATIONS_KEY.into(),
                        Boolean(self.kuaidi_change_notifications),
                    ),
                    (
                        kuaidi::PICKUP_NOTIFICATIONS_KEY.into(),
                        Boolean(self.kuaidi_pickup_notifications),
                    ),
                ]
                .into(),
            ),
            (
                ExtensionId::builtin(crate::builtins::bilibili::NOTIFICATION_ID),
                [
                    (
                        "opencli_path".into(),
                        String(self.opencli_path.trim().to_owned()),
                    ),
                    (
                        "poll_interval".into(),
                        Duration(u64::from(self.bilibili_poll_seconds) * 1000),
                    ),
                ]
                .into(),
            ),
        ]
        .into()
    }

    pub fn read_extension_settings(&mut self, host: &crate::builtins::BuiltinExtensions) {
        let pomodoro = ExtensionId::builtin(crate::builtins::pomodoro::WIDGET_ID);
        if let Some(settings) = host.settings(&pomodoro) {
            if let Some(SettingValue::Integer(value)) = settings.get("focus_minutes") {
                self.pomodoro_focus_minutes = *value as u16;
            }
            if let Some(SettingValue::Integer(value)) = settings.get("break_minutes") {
                self.pomodoro_break_minutes = *value as u16;
            }
        }
        if let Some(settings) = host.settings(&ExtensionId::builtin(kuaidi::WIDGET_ID)) {
            if let Some(SettingValue::Integer(minutes)) = settings.get(kuaidi::REFRESH_MINUTES_KEY)
            {
                self.kuaidi_refresh_minutes = *minutes as u16;
            }
            if let Some(SettingValue::Boolean(enabled)) =
                settings.get(kuaidi::CHANGE_NOTIFICATIONS_KEY)
            {
                self.kuaidi_change_notifications = *enabled;
            }
            if let Some(SettingValue::Boolean(enabled)) =
                settings.get(kuaidi::PICKUP_NOTIFICATIONS_KEY)
            {
                self.kuaidi_pickup_notifications = *enabled;
            }
        }
        let bilibili = ExtensionId::builtin(crate::builtins::bilibili::NOTIFICATION_ID);
        if let Some(settings) = host.settings(&bilibili) {
            if let Some(SettingValue::String(value)) = settings.get("opencli_path") {
                self.opencli_path = value.clone();
            }
            if let Some(SettingValue::Duration(value)) = settings.get("poll_interval") {
                self.bilibili_poll_seconds = (*value / 1000) as u16;
            }
        }
    }

    fn legacy_extension_settings(&self, source: &str) -> HashMap<ExtensionId, Settings> {
        let keys: HashSet<_> = source
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, _)| key.trim())
            .collect();
        let mut settings = self.extension_settings();
        for (id, values) in &mut settings {
            values.retain(|key, _| {
                let old = match (id.as_str(), key.as_str()) {
                    (crate::builtins::pomodoro::WIDGET_ID, "focus_minutes") => {
                        "pomodoro_focus_minutes"
                    }
                    (crate::builtins::pomodoro::WIDGET_ID, "break_minutes") => {
                        "pomodoro_break_minutes"
                    }
                    (crate::builtins::bilibili::NOTIFICATION_ID, "poll_interval") => {
                        "bilibili_poll_seconds"
                    }
                    (kuaidi::WIDGET_ID, _) => return false,
                    _ => key.as_str(),
                };
                keys.contains(old)
            });
        }
        settings
    }

    pub fn load() -> (Self, HashMap<ExtensionId, Settings>) {
        let Some(source) = preference_path().and_then(|path| fs::read_to_string(path).ok()) else {
            return (Self::default(), HashMap::new());
        };
        let preferences = Self::parse(&source);
        let legacy = preferences.legacy_extension_settings(&source);
        (preferences, legacy)
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = preference_path() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "无法找到设置目录"));
        };
        self.save_to(&path)
    }

    fn save_to(&self, path: &std::path::Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("tmp");
        let mut encoded = self.encode();
        // Keep the old values available if the separate store could not be
        // opened on this launch. New edits are recorded only by the schema store.
        if let Ok(old) = fs::read_to_string(path) {
            for line in old.lines() {
                if line.split_once('=').is_some_and(|(key, _)| {
                    matches!(
                        key.trim(),
                        "pomodoro_focus_minutes"
                            | "pomodoro_break_minutes"
                            | "bilibili_poll_seconds"
                            | "opencli_path"
                    )
                }) {
                    encoded.push_str(line);
                    encoded.push('\n');
                }
            }
        }
        fs::write(&temporary, encoded)?;
        fs::rename(&temporary, path)
    }

    pub fn is_enabled(&self, id: &ExtensionId) -> bool {
        self.compact_items
            .iter()
            .find(|item| &item.id == id)
            .is_some_and(|item| item.enabled)
    }

    pub fn set_enabled(&mut self, id: &ExtensionId, enabled: bool) {
        if let Some(item) = self.compact_items.iter_mut().find(|item| &item.id == id) {
            item.enabled = enabled;
        }
    }

    pub fn ordered_compact_items(&self) -> Vec<&CompactItemPreference> {
        let mut items: Vec<_> = self.compact_items.iter().collect();
        items.sort_by_key(|item| (item.order, item.id.clone()));
        items
    }

    pub fn move_compact_item(&mut self, id: &ExtensionId, offset: isize) -> bool {
        let ordered: Vec<_> = self
            .ordered_compact_items()
            .into_iter()
            .map(|item| item.id.clone())
            .collect();
        let Some(index) = ordered.iter().position(|candidate| candidate == id) else {
            return false;
        };
        let target = index
            .saturating_add_signed(offset)
            .min(ordered.len().saturating_sub(1));
        if target == index {
            return false;
        }
        let left = ordered[index].clone();
        let right = ordered[target].clone();
        let left_order = self
            .compact_items
            .iter()
            .find(|item| item.id == left)
            .map(|item| item.order)
            .unwrap_or(index as u32);
        let right_order = self
            .compact_items
            .iter()
            .find(|item| item.id == right)
            .map(|item| item.order)
            .unwrap_or(target as u32);
        if let Some(item) = self.compact_items.iter_mut().find(|item| item.id == left) {
            item.order = right_order;
        }
        if let Some(item) = self.compact_items.iter_mut().find(|item| item.id == right) {
            item.order = left_order;
        }
        self.normalize_order();
        true
    }

    fn normalize_order(&mut self) {
        let mut ordered: Vec<_> = self
            .compact_items
            .iter()
            .map(|item| item.id.clone())
            .collect();
        ordered.sort_by_key(|id| {
            self.compact_items
                .iter()
                .find(|item| &item.id == id)
                .map(|item| (item.order, item.id.clone()))
        });
        for (order, id) in ordered.into_iter().enumerate() {
            if let Some(item) = self.compact_items.iter_mut().find(|item| item.id == id) {
                item.order = order as u32;
            }
        }
    }
}

fn parse_duration(value: &str, default: u16) -> u16 {
    value
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|minutes| (1..=180).contains(minutes))
        .unwrap_or(default)
}

fn legacy_compact_id(key: &str) -> Option<ExtensionId> {
    Some(ExtensionId::builtin(match key {
        "mascot" => "builtin.mascot/face",
        "battery" => "builtin.system-monitor/battery",
        "cpu" => "builtin.system-monitor/cpu",
        "gpu" => "builtin.system-monitor/gpu",
        "network" => "builtin.system-monitor/network",
        "downloads" => "builtin.downloads/status",
        "time" => "builtin.clock/time",
        _ => return None,
    }))
}

fn parse_legacy_order(value: &str) -> Vec<ExtensionId> {
    let defaults = [
        "builtin.mascot/face",
        "builtin.system-monitor/battery",
        "builtin.system-monitor/cpu",
        "builtin.system-monitor/gpu",
        "builtin.system-monitor/network",
        "builtin.downloads/status",
        "builtin.clock/time",
    ];
    let mut ids = Vec::new();
    for index in value
        .split(',')
        .filter_map(|part| part.trim().parse::<usize>().ok())
    {
        if let Some(id) = defaults.get(index).map(|id| ExtensionId::builtin(id)) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    for id in defaults.map(ExtensionId::builtin) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

fn parse_periods(value: &str) -> QuietHours {
    let mut hours = QuietHours::default();
    for encoded in value.split(';').take(MAX_QUIET_PERIODS) {
        let mut fields = encoded.split(',');
        let (Some(enabled), Some(days), Some(start), Some(end)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let (Some(enabled), Ok(days), Ok(start), Ok(end)) = (
            parse_bool(enabled),
            days.trim().parse::<u8>(),
            start.trim().parse::<u16>(),
            end.trim().parse::<u16>(),
        ) else {
            continue;
        };
        hours.add_period(QuietPeriod {
            enabled,
            days,
            start_minute: start,
            end_minute: end,
        });
    }
    hours
}

fn encode_periods(hours: &QuietHours) -> String {
    hours
        .periods()
        .iter()
        .map(|period| {
            format!(
                "{},{},{},{}",
                period.enabled, period.days, period.start_minute, period.end_minute
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn preference_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("APPDATA").map(PathBuf::from)?;
    #[cfg(not(target_os = "windows"))]
    let root = std::env::var_os("HOME").map(PathBuf::from)?;

    #[cfg(target_os = "macos")]
    return Some(root.join("Library/Application Support/island-lab/preferences.conf"));
    #[cfg(target_os = "windows")]
    return Some(root.join("IslandLab/preferences.conf"));
    #[cfg(target_os = "linux")]
    return Some(root.join(".config/island-lab/preferences.conf"));
    #[allow(unreachable_code)]
    None
}

#[cfg(test)]
mod tests;
