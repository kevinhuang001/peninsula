use super::{
    quiet_hours::{QuietHours, QuietPeriod, MAX_QUIET_PERIODS},
    theme::{parse_accent_color, AppearanceTheme, DEFAULT_ACCENT_COLOR},
};
use crate::{
    builtins::registry,
    extension::{ContributionKind, DiagnosticCheck, ExtensionDescriptor, ExtensionId, Settings},
};
use std::{collections::BTreeMap, fs, io, path::PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapsuleItemPreference {
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
    pub doctor: Vec<DiagnosticCheck>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AppPreferences {
    pub capsule_items: Vec<CapsuleItemPreference>,
    pub reduced_motion: bool,
    pub motion_off: bool,
    pub mascot_enabled: bool,
    pub mascot_motion: String,
    pub quiet_hours: QuietHours,
    pub appearance_theme: AppearanceTheme,
    pub accent_color: [u8; 3],
    pub capsule_text_size: f32,
    pub capsule_text_color: Option<[u8; 3]>,
    pub widget_order: Vec<ExtensionId>,
    contribution_enabled: BTreeMap<ExtensionId, bool>,
    pub contribution_values: BTreeMap<ExtensionId, Settings>,
    pub pages: Vec<ExtensionPage>,
}
impl Default for AppPreferences {
    fn default() -> Self {
        let registry = registry();
        Self {
            capsule_items: registry
                .by_kind(ContributionKind::Capsule)
                .into_iter()
                .map(|d| CapsuleItemPreference {
                    id: d.id.clone(),
                    enabled: d.default_enabled,
                    order: d.default_order,
                })
                .collect(),
            reduced_motion: false,
            motion_off: false,
            mascot_enabled: true,
            mascot_motion: "Full".into(),
            quiet_hours: QuietHours::default(),
            appearance_theme: AppearanceTheme::default(),
            accent_color: DEFAULT_ACCENT_COLOR,
            capsule_text_size: 11.0,
            capsule_text_color: None,
            widget_order: registry
                .by_kind(ContributionKind::Widget)
                .into_iter()
                .map(|d| d.id.clone())
                .collect(),
            contribution_enabled: BTreeMap::new(),
            contribution_values: BTreeMap::new(),
            pages: registry
                .extensions()
                .iter()
                .map(|d| ExtensionPage {
                    descriptor: d.clone(),
                    values: crate::core::settings::SettingsStore::defaults(d),
                    enabled: d.default_enabled,
                    running: false,
                    status: "已注册".into(),
                    doctor: vec![],
                })
                .collect(),
        }
    }
}
impl AppPreferences {
    pub fn parse(source: &str) -> Self {
        let mut settings = Self::default();
        for line in source.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "reduced_motion" => settings.reduced_motion = value.trim() == "true",
                "motion_off" => settings.motion_off = value.trim() == "true",
                "mascot_enabled" => {
                    if let Some(v) = parse_bool(value) {
                        settings.mascot_enabled = v;
                    }
                }
                "mascot_motion" => {
                    if ["Full", "Reduced", "Off"].contains(&value.trim()) {
                        settings.mascot_motion = value.trim().into();
                    }
                }
                "widget_theme" | "appearance_theme" => {
                    if let Some(theme) = AppearanceTheme::parse(value) {
                        settings.appearance_theme = theme;
                    }
                }
                "widget_accent" | "accent_color" => {
                    if let Some(accent) = parse_accent_color(value) {
                        settings.accent_color = accent;
                    }
                }
                "compact_text_size" | "capsule_text_size" => {
                    if let Ok(size) = value.trim().parse::<f32>() {
                        if size.is_finite() {
                            settings.capsule_text_size = size.clamp(8.0, 18.0);
                        }
                    }
                }
                "compact_text_color" | "capsule_text_color" => {
                    settings.capsule_text_color = if value.trim() == "theme" {
                        None
                    } else {
                        parse_hex_color(value)
                    };
                }
                "widget_order" => {
                    settings.widget_order = value
                        .split(',')
                        .filter_map(|v| ExtensionId::new(v.trim()).ok())
                        .collect()
                }
                // Preserve Widget visibility saved before the contribution lifecycle merge.
                "widget_disabled" => {
                    for value in value.split(',') {
                        if let Ok(id) = ExtensionId::new(value.trim()) {
                            if registry().get(&id).is_some_and(|entry| entry.kind == ContributionKind::Widget) {
                                settings.contribution_enabled.insert(id, false);
                            }
                        }
                    }
                }
                "contribution_enabled" => {
                    if let Some((id, enabled)) = value.split_once('|') {
                        if let (Ok(id), Some(enabled)) =
                            (ExtensionId::new(id.trim()), parse_bool(enabled))
                        {
                            settings.contribution_enabled.insert(id, enabled);
                        }
                    }
                }
                "extension" | "capsule_item" => {
                    let fields: Vec<_> = value.split('|').collect();
                    if fields.len() != 3 {
                        continue;
                    }
                    let (Ok(id), Some(enabled), Ok(order)) = (
                        ExtensionId::new(fields[0].trim()),
                        parse_bool(fields[1]),
                        fields[2].trim().parse::<u32>(),
                    ) else {
                        continue;
                    };
                    if let Some(item) = settings.capsule_items.iter_mut().find(|item| item.id == id)
                    {
                        item.enabled = enabled;
                        item.order = order;
                    } else {
                        settings
                            .capsule_items
                            .push(CapsuleItemPreference { id, enabled, order });
                    }
                }
                "quiet_periods" => {
                    let manual = settings.quiet_hours.manual_enabled;
                    settings.quiet_hours = parse_periods(value);
                    settings.quiet_hours.manual_enabled = manual;
                }
                "quiet_manual" => {
                    settings.quiet_hours.manual_enabled = parse_bool(value).unwrap_or(false)
                }
                _ => {}
            }
        }
        settings.normalize_order();
        settings
    }
    pub fn encode(&self) -> String {
        let mut lines: Vec<_> = self
            .ordered_capsule_items()
            .into_iter()
            .map(|i| format!("capsule_item={}|{}|{}", i.id, i.enabled, i.order))
            .collect();
        lines.extend(
            self.contribution_enabled
                .iter()
                .map(|(id, enabled)| format!("contribution_enabled={id}|{enabled}")),
        );
        lines.extend([
            format!("reduced_motion={}", self.reduced_motion),
            format!("motion_off={}", self.motion_off),
            format!("mascot_enabled={}", self.mascot_enabled),
            format!("mascot_motion={}", self.mascot_motion),
            format!("appearance_theme={}", self.appearance_theme.name()),
            format!(
                "accent_color=#{:02x}{:02x}{:02x}",
                self.accent_color[0], self.accent_color[1], self.accent_color[2]
            ),
            format!("capsule_text_size={:.1}", self.capsule_text_size),
            format!(
                "capsule_text_color={}",
                self.capsule_text_color.map_or_else(
                    || "theme".into(),
                    |color| format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]),
                )
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
    pub fn load() -> Self {
        let source = preference_path()
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default();
        let settings = Self::parse(&source);
        if source.lines().any(|line| {
            ["extension=", "compact_text_size=", "compact_text_color=",
             "widget_theme=", "widget_accent="]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        }) {
            let _ = settings.save(); // Atomic and idempotent one-time preference migration.
        }
        settings
    }
    pub fn refresh_status(&mut self, host: &crate::core::extension_manager::ExtensionManager) {
        for page in &mut self.pages {
            page.running = host.running(&page.descriptor.id);
            page.status = host.status(&page.descriptor.id);
            page.doctor = host.doctor(&page.descriptor.id);
        }
    }
    pub fn read_extension_settings(
        &mut self,
        host: &crate::core::extension_manager::ExtensionManager,
    ) {
        for page in &mut self.pages {
            if let Some(values) = host.configured_settings(&page.descriptor.id) {
                page.values = values.clone();
            }
            page.running = host.running(&page.descriptor.id);
            page.enabled = host.desired_enabled(&page.descriptor.id);
            page.status = host.status(&page.descriptor.id);
            page.doctor = host.doctor(&page.descriptor.id);
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
        let encoded = self.encode();
        file.write_all(encoded.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn is_enabled(&self, id: &ExtensionId) -> bool {
        self.capsule_items
            .iter()
            .find(|item| &item.id == id)
            .is_some_and(|item| item.enabled)
    }

    pub fn contribution_preferences(&self) -> BTreeMap<ExtensionId, bool> {
        let mut enabled = self.contribution_enabled.clone();
        enabled.extend(
            self.capsule_items
                .iter()
                .map(|item| (item.id.clone(), item.enabled)),
        );
        enabled
    }

    pub fn contribution_enabled(
        &self,
        descriptor: &crate::extension::ContributionDescriptor,
    ) -> bool {
        self.contribution_enabled
            .get(&descriptor.id)
            .copied()
            .unwrap_or(descriptor.default_enabled)
    }

    pub fn set_contribution_enabled(&mut self, id: ExtensionId, enabled: bool) {
        self.contribution_enabled.insert(id, enabled);
    }

    pub fn read_contribution_defaults(
        &mut self,
        host: &crate::core::extension_manager::ExtensionManager,
    ) {
        for entry in self
            .pages
            .iter_mut()
            .flat_map(|page| &mut page.descriptor.contributions)
        {
            if let Some(enabled) = host.default_contribution_enabled(&entry.id) {
                entry.default_enabled = enabled;
            }
        }
    }

    pub fn read_contribution_settings(
        &mut self,
        host: &crate::core::extension_manager::ExtensionManager,
    ) {
        self.contribution_values = host
            .registry()
            .extensions()
            .iter()
            .flat_map(|entry| &entry.contributions)
            .filter(|entry| !entry.settings.is_empty())
            .map(|entry| {
                // Dynamic contributions may be visible before their first
                // settings record was written. Seed the edit form from schema,
                // then let the host persist it on the first change.
                let values = host.configured_contribution_settings(&entry.id)
                    .cloned()
                    .unwrap_or_else(|| crate::core::settings::SettingsStore::defaults(entry));
                (entry.id.clone(), values)
            })
            .collect();
    }

    pub fn sync_contributions(&mut self, registry: &crate::extension::ExtensionRegistry) -> bool {
        let old_counts = (
            self.capsule_items.len(),
            self.widget_order.len(),
            self.contribution_enabled.len(),
        );
        self.capsule_items.retain(|item| {
            registry
                .get(&item.id)
                .is_some_and(|c| c.kind == ContributionKind::Capsule)
        });
        self.widget_order.retain(|id| {
            registry
                .get(id)
                .is_some_and(|c| c.kind == ContributionKind::Widget)
        });
        self.contribution_enabled.retain(|id, _| {
            registry.get(id).is_some_and(|c| {
                matches!(
                    c.kind,
                    ContributionKind::Widget | ContributionKind::Notification
                )
            })
        });
        let mut changed = old_counts
            != (
                self.capsule_items.len(),
                self.widget_order.len(),
                self.contribution_enabled.len(),
            );
        let mut order = self
            .capsule_items
            .iter()
            .map(|item| item.order)
            .max()
            .map_or(0, |last| last.saturating_add(1));
        for c in registry.by_kind(ContributionKind::Capsule) {
            if !self.capsule_items.iter().any(|item| item.id == c.id) {
                self.capsule_items.push(CapsuleItemPreference {
                    id: c.id.clone(),
                    enabled: c.default_enabled,
                    order,
                });
                order = order.saturating_add(1);
                changed = true;
            }
        }
        for c in registry.by_kind(ContributionKind::Widget) {
            if !self.widget_order.contains(&c.id) {
                self.widget_order.push(c.id.clone());
                changed = true;
            }
        }
        for page in &mut self.pages {
            if let Some(descriptor) = registry.extension(&page.descriptor.id) {
                if page.descriptor.contributions != descriptor.contributions {
                    page.descriptor
                        .contributions
                        .clone_from(&descriptor.contributions);
                }
            }
        }
        if changed {
            self.normalize_order();
        }
        changed
    }

    pub fn set_capsule_item_enabled(&mut self, id: &ExtensionId, enabled: bool) -> bool {
        let Some(item) = self.capsule_items.iter_mut().find(|item| &item.id == id) else {
            return false;
        };
        if item.enabled == enabled {
            return false;
        }
        item.enabled = enabled;
        true
    }

    #[cfg(test)]
    pub fn set_enabled(&mut self, id: &ExtensionId, enabled: bool) {
        if let Some(item) = self.capsule_items.iter_mut().find(|item| &item.id == id) {
            item.enabled = enabled;
        }
    }

    pub fn ordered_enabled_widgets(&self) -> Vec<ExtensionId> {
        self.widget_order.iter()
            .filter(|id| self.pages.iter().flat_map(|page| &page.descriptor.contributions)
                .find(|entry| &entry.id == *id)
                .is_some_and(|entry| self.contribution_enabled(entry)))
            .cloned()
            .collect()
    }

    pub fn move_widget(&mut self, id: &ExtensionId, offset: isize) -> bool {
        let enabled = self.ordered_enabled_widgets();
        let Some(index) = enabled.iter().position(|entry| entry == id) else {
            return false;
        };
        let next = index.saturating_add_signed(offset).min(enabled.len().saturating_sub(1));
        if next == index {
            return false;
        }
        let left = self.widget_order.iter().position(|entry| entry == id).unwrap();
        let right = self.widget_order.iter().position(|entry| entry == &enabled[next]).unwrap();
        self.widget_order.swap(left, right);
        true
    }

    pub fn ordered_capsule_items(&self) -> Vec<&CapsuleItemPreference> {
        let mut items: Vec<_> = self.capsule_items.iter().collect();
        items.sort_by_key(|item| (item.order, item.id.clone()));
        items
    }

    pub fn move_capsule_item(&mut self, id: &ExtensionId, offset: isize) -> bool {
        let ordered: Vec<_> = self
            .ordered_capsule_items()
            .into_iter()
            .filter(|item| item.enabled)
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
            .capsule_items
            .iter()
            .find(|item| item.id == left)
            .map(|item| item.order)
            .unwrap_or(index as u32);
        let right_order = self
            .capsule_items
            .iter()
            .find(|item| item.id == right)
            .map(|item| item.order)
            .unwrap_or(target as u32);
        if let Some(item) = self.capsule_items.iter_mut().find(|item| item.id == left) {
            item.order = right_order;
        }
        if let Some(item) = self.capsule_items.iter_mut().find(|item| item.id == right) {
            item.order = left_order;
        }
        self.normalize_order();
        true
    }

    fn normalize_order(&mut self) {
        let mut ordered: Vec<_> = self
            .capsule_items
            .iter()
            .map(|item| item.id.clone())
            .collect();
        ordered.sort_by_key(|id| {
            self.capsule_items
                .iter()
                .find(|item| &item.id == id)
                .map(|item| (item.order, item.id.clone()))
        });
        for (order, id) in ordered.into_iter().enumerate() {
            if let Some(item) = self.capsule_items.iter_mut().find(|item| item.id == id) {
                item.order = order as u32;
            }
        }
    }
}

fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let value = value.trim().strip_prefix('#')?;
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
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
