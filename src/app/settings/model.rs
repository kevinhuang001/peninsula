use super::CompactPreferences;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) enum Section {
    #[default]
    Compact,
    QuietHours,
    About,
}

impl Section {
    pub(super) const ALL: [Self; 3] = [Self::Compact, Self::QuietHours, Self::About];

    pub(super) fn extra_height(self) -> f32 {
        match self {
            Self::Compact => 270.0,
            Self::QuietHours => 220.0,
            Self::About => 156.0,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Compact => "胶囊",
            Self::QuietHours => "免打扰",
            Self::About => "关于",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Compact => "胶囊显示内容",
            Self::QuietHours => "免打扰计划",
            Self::About => "灵动岛",
        }
    }

    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Compact => "开启项目，并调整收缩状态的显示顺序",
            Self::QuietHours => "",
            Self::About => "轻量的桌面灵动岛",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OptionItem {
    Mascot,
    Battery,
    Cpu,
    Gpu,
    Network,
    Downloads,
    Time,
}

impl OptionItem {
    pub const ALL: [Self; 7] = [
        Self::Mascot,
        Self::Battery,
        Self::Cpu,
        Self::Gpu,
        Self::Network,
        Self::Downloads,
        Self::Time,
    ];

    pub fn from_id(id: u8) -> Option<Self> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mascot => "表情",
            Self::Battery => "电池",
            Self::Cpu => "CPU",
            Self::Gpu => "GPU",
            Self::Network => "网络",
            Self::Downloads => "下载",
            Self::Time => "时间",
        }
    }

    pub fn enabled(self, preferences: &CompactPreferences) -> bool {
        match self {
            Self::Mascot => preferences.mascot,
            Self::Battery => preferences.battery,
            Self::Cpu => preferences.cpu,
            Self::Gpu => preferences.gpu,
            Self::Network => preferences.network,
            Self::Downloads => preferences.downloads,
            Self::Time => preferences.time,
        }
    }

    pub fn toggle(self, preferences: &mut CompactPreferences) {
        match self {
            Self::Mascot => preferences.mascot = !preferences.mascot,
            Self::Battery => preferences.battery = !preferences.battery,
            Self::Cpu => preferences.cpu = !preferences.cpu,
            Self::Gpu => preferences.gpu = !preferences.gpu,
            Self::Network => preferences.network = !preferences.network,
            Self::Downloads => preferences.downloads = !preferences.downloads,
            Self::Time => preferences.time = !preferences.time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OptionItem, Section};
    use crate::app::settings::CompactPreferences;

    #[test]
    fn all_compact_options_live_in_one_settings_tab() {
        assert_eq!(
            Section::ALL,
            [Section::Compact, Section::QuietHours, Section::About]
        );
        assert_eq!(OptionItem::ALL.len(), 7);
    }

    #[test]
    fn every_compact_setting_toggle_changes_its_preference() {
        let mut preferences = CompactPreferences::default();
        for option in OptionItem::ALL {
            let old = option.enabled(&preferences);
            option.toggle(&mut preferences);
            assert_ne!(option.enabled(&preferences), old, "{option:?}");
        }
    }
}
