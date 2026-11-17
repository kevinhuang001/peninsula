use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::PathBuf,
};

use crate::{
    builtins::{registry, weather::WeatherPreferences},
    extension::{ExtensionId, ExtensionKind},
};

use super::quiet_hours::{QuietHours, QuietPeriod, MAX_QUIET_PERIODS};

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
    pub weather: WeatherPreferences,
    pub pomodoro_focus_minutes: u16,
    pub pomodoro_break_minutes: u16,
    pub bilibili_enabled: bool,
    pub bilibili_dm_enabled: bool,
    pub bilibili_likes_enabled: bool,
    pub bilibili_show_dm_summary: bool,
    pub bilibili_poll_seconds: u16,
    pub opencli_path: String,
    pub bilibili_check_requested: bool,
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
            weather: WeatherPreferences::default(),
            pomodoro_focus_minutes: 25,
            pomodoro_break_minutes: 5,
            bilibili_enabled: false,
            bilibili_dm_enabled: true,
            bilibili_likes_enabled: true,
            bilibili_show_dm_summary: true,
            bilibili_poll_seconds: 60,
            opencli_path: String::new(),
            bilibili_check_requested: false,
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
                "bilibili_dm_enabled" => {
                    settings.bilibili_dm_enabled = parse_bool(value).unwrap_or(true)
                }
                "bilibili_likes_enabled" => {
                    settings.bilibili_likes_enabled = parse_bool(value).unwrap_or(true)
                }
                "bilibili_show_dm_summary" => {
                    settings.bilibili_show_dm_summary = parse_bool(value).unwrap_or(true)
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
        lines.extend(self.weather.encode_lines());
        lines.push(format!(
            "pomodoro_focus_minutes={}",
            self.pomodoro_focus_minutes
        ));
        lines.push(format!(
            "pomodoro_break_minutes={}",
            self.pomodoro_break_minutes
        ));
        lines.push(format!("bilibili_enabled={}", self.bilibili_enabled));
        lines.push(format!("bilibili_dm_enabled={}", self.bilibili_dm_enabled));
        lines.push(format!(
            "bilibili_likes_enabled={}",
            self.bilibili_likes_enabled
        ));
        lines.push(format!(
            "bilibili_show_dm_summary={}",
            self.bilibili_show_dm_summary
        ));
        lines.push(format!(
            "bilibili_poll_seconds={}",
            self.bilibili_poll_seconds
        ));
        lines.push(format!(
            "opencli_path={}",
            self.opencli_path.replace(['\n', '\r'], "")
        ));
        format!("{}\n", lines.join("\n"))
    }

    pub fn load() -> Self {
        preference_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map_or_else(Self::default, |source| Self::parse(&source))
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
        fs::write(&temporary, self.encode())?;
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
