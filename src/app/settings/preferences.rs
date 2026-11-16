use std::{fs, io, path::PathBuf};

use super::{
    model::OptionItem,
    quiet_hours::{QuietHours, QuietPeriod, MAX_QUIET_PERIODS},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactPreferences {
    pub mascot: bool,
    pub battery: bool,
    pub cpu: bool,
    pub gpu: bool,
    pub network: bool,
    pub downloads: bool,
    pub time: bool,
    pub compact_order: [u8; 7],
    pub quiet_hours: QuietHours,
}

impl Default for CompactPreferences {
    fn default() -> Self {
        Self {
            mascot: true,
            battery: true,
            cpu: false,
            gpu: false,
            network: false,
            downloads: false,
            time: false,
            compact_order: std::array::from_fn(|index| index as u8),
            quiet_hours: QuietHours::default(),
        }
    }
}

impl CompactPreferences {
    pub fn parse(source: &str) -> Self {
        let mut settings = Self::default();
        for line in source.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "compact_order" => settings.compact_order = parse_order(value),
                "quiet_periods" => {
                    let manual = settings.quiet_hours.manual_enabled;
                    settings.quiet_hours = parse_periods(value);
                    settings.quiet_hours.manual_enabled = manual;
                }
                key => {
                    let Some(value) = parse_bool(value) else {
                        continue;
                    };
                    match key {
                        "mascot" => settings.mascot = value,
                        "battery" => settings.battery = value,
                        "cpu" => settings.cpu = value,
                        "gpu" => settings.gpu = value,
                        "network" => settings.network = value,
                        "downloads" => settings.downloads = value,
                        "time" => settings.time = value,
                        "quiet_manual" => settings.quiet_hours.manual_enabled = value,
                        _ => {}
                    }
                }
            }
        }
        settings
    }

    pub fn encode(self) -> String {
        format!(
            "mascot={}\nbattery={}\ncpu={}\ngpu={}\nnetwork={}\ndownloads={}\ntime={}\ncompact_order={}\nquiet_manual={}\nquiet_periods={}\n",
            self.mascot,
            self.battery,
            self.cpu,
            self.gpu,
            self.network,
            self.downloads,
            self.time,
            self.compact_order.map(|id| id.to_string()).join(","),
            self.quiet_hours.manual_enabled,
            encode_periods(&self.quiet_hours)
        )
    }

    pub fn load() -> Self {
        preference_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map_or_else(Self::default, |source| Self::parse(&source))
    }

    pub fn save(self) -> io::Result<()> {
        let Some(path) = preference_path() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "无法找到设置目录"));
        };
        self.save_to(&path)
    }

    fn save_to(self, path: &std::path::Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, self.encode())?;
        fs::rename(&temporary, path)
    }

    pub fn compact_width(self, downloading: bool) -> f32 {
        super::super::compact_layout::width(&self, downloading)
    }

    pub fn ordered_compact_items(self) -> [OptionItem; 7] {
        std::array::from_fn(|index| OptionItem::from_id(self.compact_order[index]).unwrap())
    }

    pub fn move_compact_item(&mut self, item: OptionItem, offset: isize) -> bool {
        let Some(index) = self
            .ordered_compact_items()
            .iter()
            .position(|candidate| *candidate == item)
        else {
            return false;
        };
        let target = index
            .saturating_add_signed(offset)
            .min(self.compact_order.len() - 1);
        if target == index {
            return false;
        }
        self.compact_order.swap(index, target);
        true
    }
}

fn parse_order(value: &str) -> [u8; 7] {
    let mut ids = Vec::with_capacity(7);
    for id in value
        .split(',')
        .filter_map(|part| part.trim().parse::<u8>().ok())
    {
        if usize::from(id) < 7 && !ids.contains(&id) {
            ids.push(id);
        }
    }
    for id in 0..7 {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    std::array::from_fn(|index| ids[index])
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
