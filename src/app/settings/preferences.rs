use super::{
    quiet_hours::{QuietHours, QuietPeriod, MAX_QUIET_PERIODS},
    theme::{parse_accent, WidgetTheme, DEFAULT_WIDGET_ACCENT},
};
use crate::{
    builtins::registry,
    extension::{ContributionKind, ExtensionDescriptor, ExtensionId, Settings},
};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::PathBuf,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactItemPreference {
    pub id: ExtensionId,
    pub enabled: bool,
    pub order: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionPage {
    pub descriptor: ExtensionDescriptor,
    pub values: Settings,
    pub enabled: bool,
    pub running: bool,
    pub status: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CompactPreferences {
    pub compact_items: Vec<CompactItemPreference>,
    pub reduced_motion: bool,
    pub motion_off: bool,
    pub mascot_enabled: bool,
    pub mascot_size: u16,
    pub mascot_motion: String,
    pub quiet_hours: QuietHours,
    pub widget_theme: WidgetTheme,
    pub widget_accent: [u8; 3],
    pub widget_order: Vec<ExtensionId>,
    pub pages: Vec<ExtensionPage>,
    migration_complete: bool,
}
impl Default for CompactPreferences {
    fn default() -> Self {
        let registry = registry();
        Self {
            compact_items: registry
                .by_kind(ContributionKind::Compact)
                .into_iter()
                .map(|d| CompactItemPreference {
                    id: d.id.clone(),
                    enabled: d.default_enabled,
                    order: d.default_order,
                })
                .collect(),
            reduced_motion: false,
            motion_off: false,
            mascot_enabled: true,
            mascot_size: 38,
            mascot_motion: "Full".into(),
            quiet_hours: QuietHours::default(),
            widget_theme: WidgetTheme::default(),
            widget_accent: DEFAULT_WIDGET_ACCENT,
            widget_order: registry
                .by_kind(ContributionKind::Widget)
                .into_iter()
                .map(|d| d.id.clone())
                .collect(),
            migration_complete: true,
            pages: registry
                .extensions()
                .iter()
                .map(|d| ExtensionPage {
                    descriptor: d.clone(),
                    values: crate::core::settings::SettingsStore::defaults(d),
                    enabled: d.default_enabled,
                    running: false,
                    status: "已注册".into(),
                })
                .collect(),
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
                "bilibili_enabled" => {
                    if let Some(page) = settings
                        .pages
                        .iter_mut()
                        .find(|p| p.descriptor.id.as_str() == "builtin.bilibili")
                    {
                        page.enabled = parse_bool(value).unwrap_or(false);
                    }
                }
                "reduced_motion" => settings.reduced_motion = value.trim() == "true",
                "motion_off" => settings.motion_off = value.trim() == "true",
                "mascot" | "mascot_enabled" => {
                    if let Some(v) = parse_bool(value) {
                        settings.mascot_enabled = v;
                    }
                }
                "mascot_size" => {
                    if let Ok(v) = value.trim().parse::<u16>() {
                        settings.mascot_size = v.clamp(24, 64);
                    }
                }
                "mascot_motion" => {
                    if ["Full", "Reduced", "Off"].contains(&value.trim()) {
                        settings.mascot_motion = value.trim().into();
                    }
                }
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
                "widget_order" => {
                    settings.widget_order = value
                        .split(',')
                        .filter_map(|v| ExtensionId::new(v.trim()).ok())
                        .filter(|id| {
                            registry()
                                .get(id)
                                .is_some_and(|d| d.kind == ContributionKind::Widget)
                        })
                        .collect()
                }
                "extension" => {
                    let fields: Vec<_> = value.split('|').collect();
                    if fields.len() != 3 {
                        continue;
                    }
                    let (Ok(mut id), Some(enabled), Ok(order)) = (
                        ExtensionId::new(fields[0].trim()),
                        parse_bool(fields[1]),
                        fields[2].trim().parse::<u32>(),
                    ) else {
                        continue;
                    };
                    if id.as_str() == "builtin.mascot/face" {
                        settings.mascot_enabled = enabled;
                        continue;
                    }
                    if id.as_str() == "builtin.system-monitor/battery" {
                        id = ExtensionId::builtin("builtin.battery/status");
                    }
                    explicit_ids.insert(id.clone());
                    if let Some(item) = settings.compact_items.iter_mut().find(|item| item.id == id)
                    {
                        item.enabled = enabled;
                        item.order = order;
                    }
                }
                "compact_order" => legacy_order = Some(parse_legacy_order(value)),
                "quiet_periods" => {
                    let manual = settings.quiet_hours.manual_enabled;
                    settings.quiet_hours = parse_periods(value);
                    settings.quiet_hours.manual_enabled = manual;
                }
                "quiet_manual" => {
                    settings.quiet_hours.manual_enabled = parse_bool(value).unwrap_or(false)
                }
                key => {
                    if let (Some(id), Some(v)) = (legacy_compact_id(key), parse_bool(value)) {
                        legacy_values.insert(id, v);
                    }
                }
            }
        }
        for (id, enabled) in legacy_values {
            if let Some(item) = settings
                .compact_items
                .iter_mut()
                .find(|item| item.id == id && !explicit_ids.contains(&id))
            {
                item.enabled = enabled;
            }
        }
        if let Some(ids) = legacy_order {
            for (order, id) in ids.into_iter().enumerate() {
                if let Some(item) = settings
                    .compact_items
                    .iter_mut()
                    .find(|item| item.id == id && !explicit_ids.contains(&id))
                {
                    item.order = order as u32;
                }
            }
        }
        // A hidden legacy clock had no other contribution; avoid starting it after upgrade.
        if let Some(clock) = settings
            .pages
            .iter_mut()
            .find(|p| p.descriptor.id.as_str() == "builtin.clock")
        {
            clock.enabled = settings
                .compact_items
                .iter()
                .any(|i| i.id.as_str() == "builtin.clock/time" && i.enabled);
        }
        settings.normalize_order();
        settings
    }
    pub fn encode(&self) -> String {
        let mut lines: Vec<_> = self
            .ordered_compact_items()
            .into_iter()
            .map(|i| format!("extension={}|{}|{}", i.id, i.enabled, i.order))
            .collect();
        lines.extend([
            format!("reduced_motion={}", self.reduced_motion),
            format!("motion_off={}", self.motion_off),
            format!("mascot_enabled={}", self.mascot_enabled),
            format!("mascot_size={}", self.mascot_size),
            format!("mascot_motion={}", self.mascot_motion),
            format!("widget_theme={}", self.widget_theme.name()),
            format!(
                "widget_accent=#{:02x}{:02x}{:02x}",
                self.widget_accent[0], self.widget_accent[1], self.widget_accent[2]
            ),
            format!("quiet_manual={}", self.quiet_hours.manual_enabled),
            format!("quiet_periods={}", encode_periods(&self.quiet_hours)),
            format!(
                "widget_order={}",
                self.widget_order
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ]);
        format!("{}\n", lines.join("\n"))
    }
    pub fn load() -> (Self, HashMap<ExtensionId, Settings>) {
        let source = preference_path()
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default();
        (
            Self::parse(&source),
            super::migration::legacy_settings(&source),
        )
    }
    pub fn refresh_status(&mut self, host: &crate::core::extension_manager::ExtensionManager) {
        for page in &mut self.pages {
            page.running = host.running(&page.descriptor.id);
            page.status = host.status(&page.descriptor.id);
        }
    }
    pub fn read_extension_settings(
        &mut self,
        host: &crate::core::extension_manager::ExtensionManager,
    ) {
        self.migration_complete = self
            .pages
            .iter()
            .all(|p| host.settings_persisted(&p.descriptor.id));
        for page in &mut self.pages {
            if let Some(values) = host.configured_settings(&page.descriptor.id) {
                page.values = values.clone();
            }
            page.running = host.running(&page.descriptor.id);
            page.enabled = page.running;
            page.status = host.status(&page.descriptor.id);
        }
    }
    pub fn save(&self) -> io::Result<()> {
        self.save_to(
            &preference_path()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "无法找到设置目录"))?,
        )
    }
    fn save_to(&self, path: &std::path::Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        use std::io::Write;
        let mut encoded = self.encode();
        if !self.migration_complete {
            if let Ok(previous) = fs::read_to_string(path) {
                for line in previous
                    .lines()
                    .filter(|line| super::migration::is_legacy_setting(line))
                {
                    encoded.push_str(line);
                    encoded.push('\n');
                }
            }
        }
        file.write_all(encoded.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn is_enabled(&self, id: &ExtensionId) -> bool {
        self.compact_items
            .iter()
            .find(|item| &item.id == id)
            .is_some_and(|item| item.enabled)
    }

    #[cfg(test)]
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

fn legacy_compact_id(key: &str) -> Option<ExtensionId> {
    Some(ExtensionId::builtin(match key {
        "mascot" => "builtin.mascot/face",
        "battery" => "builtin.battery/status",
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
        "builtin.battery/status",
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
