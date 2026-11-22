//! Input-only migration of pre-V2 business settings. New writes use the schema store.
use crate::{
    builtins::weather::WeatherPreferences,
    extension::{ExtensionId, SettingValue as V, Settings},
};
use std::collections::HashMap;
pub fn legacy_settings(source: &str) -> HashMap<ExtensionId, Settings> {
    let mut values: HashMap<ExtensionId, Settings> = HashMap::new();
    let mut weather = WeatherPreferences::default();
    let mut has_weather = false;
    for line in source.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        has_weather |= weather.apply_line(key, value);
        let (id, name, parsed) = match key.trim() {
            "pomodoro_focus_minutes" => (
                "builtin.pomodoro",
                "focus_minutes",
                value.parse::<i64>().ok().map(V::Integer),
            ),
            "pomodoro_break_minutes" => (
                "builtin.pomodoro",
                "break_minutes",
                value.parse::<i64>().ok().map(V::Integer),
            ),
            "bilibili_poll_seconds" => (
                "builtin.bilibili",
                "poll_interval",
                value
                    .parse::<u64>()
                    .ok()
                    .map(|v| V::Duration(v.saturating_mul(1000))),
            ),
            "opencli_path" => (
                "builtin.bilibili",
                "opencli_path",
                Some(V::String(value.into())),
            ),
            "kuaidi_refresh_minutes" => (
                "builtin.kuaidi",
                "refresh_minutes",
                value.parse::<i64>().ok().map(V::Integer),
            ),
            "kuaidi_change_notifications" => (
                "builtin.kuaidi",
                "change_notifications",
                value.parse::<bool>().ok().map(V::Boolean),
            ),
            "kuaidi_pickup_notifications" => (
                "builtin.kuaidi",
                "pickup_notifications",
                value.parse::<bool>().ok().map(V::Boolean),
            ),
            _ => continue,
        };
        if let Some(v) = parsed {
            values
                .entry(ExtensionId::builtin(id))
                .or_default()
                .insert(name.into(), v);
        }
    }
    if has_weather {
        values.insert(ExtensionId::builtin("builtin.weather"), weather.settings());
    }
    values
}

pub fn is_legacy_setting(line: &str) -> bool {
    line.split_once('=').is_some_and(|(key, _)| {
        key.starts_with("weather_")
            || key.starts_with("pomodoro_")
            || key.starts_with("kuaidi_")
            || matches!(key, "opencli_path" | "bilibili_poll_seconds")
    })
}
