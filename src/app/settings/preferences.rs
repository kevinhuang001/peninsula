use std::{fs, io, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactPreferences {
    pub mascot: bool,
    pub battery: bool,
    pub cpu: bool,
    pub gpu: bool,
    pub network: bool,
}

impl Default for CompactPreferences {
    fn default() -> Self {
        Self {
            mascot: true,
            battery: true,
            cpu: false,
            gpu: false,
            network: false,
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
            let Some(value) = parse_bool(value) else {
                continue;
            };
            match key.trim() {
                "mascot" => settings.mascot = value,
                "battery" => settings.battery = value,
                "cpu" => settings.cpu = value,
                "gpu" => settings.gpu = value,
                "network" => settings.network = value,
                _ => {}
            }
        }
        settings
    }

    pub fn encode(self) -> String {
        format!(
            "mascot={}\nbattery={}\ncpu={}\ngpu={}\nnetwork={}\n",
            self.mascot, self.battery, self.cpu, self.gpu, self.network
        )
    }

    pub fn load() -> Self {
        preference_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map_or_else(Self::default, |source| Self::parse(&source))
    }

    pub fn save(self) -> io::Result<()> {
        let Some(path) = preference_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.encode())
    }

    pub fn compact_width(self) -> f32 {
        let items = [
            (self.mascot, 42.0),
            (self.battery, 54.0),
            (self.cpu, 58.0),
            (self.gpu, 58.0),
            (self.network, 78.0),
        ];
        let count = items.iter().filter(|(enabled, _)| *enabled).count();
        if count == 0 {
            return 148.0;
        }
        (items
            .iter()
            .filter(|(enabled, _)| *enabled)
            .map(|(_, width)| *width)
            .sum::<f32>()
            + (count.saturating_sub(1) as f32) * 10.0
            + 28.0)
            .max(148.0)
    }
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
mod tests {
    use super::CompactPreferences;

    #[test]
    fn compact_preferences_round_trip_and_keep_defaults_for_missing_keys() {
        let prefs = CompactPreferences {
            mascot: false,
            battery: true,
            cpu: true,
            gpu: false,
            network: true,
        };
        assert_eq!(CompactPreferences::parse(&prefs.encode()), prefs);
        assert!(CompactPreferences::parse("cpu=true\n").cpu);
        assert!(CompactPreferences::parse("cpu=true\n").mascot);
    }

    #[test]
    fn compact_width_accounts_for_enabled_content() {
        let default = CompactPreferences::default();
        let wide = CompactPreferences {
            cpu: true,
            network: true,
            ..default
        };
        assert_eq!(default.compact_width(), 148.0);
        assert!(wide.compact_width() > default.compact_width());
    }
}
