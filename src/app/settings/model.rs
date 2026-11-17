#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) enum Section {
    #[default]
    Compact,
    Weather,
    QuietHours,
    Pomodoro,
    Bilibili,
    About,
}

impl Section {
    pub(super) const ALL: [Self; 6] = [
        Self::Compact,
        Self::Weather,
        Self::QuietHours,
        Self::Pomodoro,
        Self::Bilibili,
        Self::About,
    ];

    pub(super) fn extra_height(self) -> f32 {
        match self {
            Self::Compact => 270.0,
            Self::Weather => 280.0,
            Self::QuietHours => 220.0,
            Self::Pomodoro => 190.0,
            Self::Bilibili => 306.0,
            Self::About => 156.0,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Compact => "胶囊",
            Self::Weather => "天气",
            Self::QuietHours => "免打扰",
            Self::Pomodoro => "番茄钟",
            Self::Bilibili => "Bilibili",
            Self::About => "关于",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Compact => "胶囊显示内容",
            Self::Weather => "天气与预警",
            Self::QuietHours => "免打扰计划",
            Self::Pomodoro => "番茄钟时长",
            Self::Bilibili => "Bilibili 通知",
            Self::About => "灵动岛",
        }
    }

    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Compact => "开启项目，并调整收缩状态的显示顺序",
            Self::Weather => "搜索城市，或跟随网络位置自动定位",
            Self::QuietHours => "",
            Self::Pomodoro => "设置每轮的专注与休息时间（1–180 分钟）",
            Self::Bilibili => "使用系统 OpenCLI 与浏览器中已有的登录状态",
            Self::About => "轻量的桌面灵动岛",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Section;
    use crate::{builtins::registry, extension::ExtensionKind};

    #[test]
    fn all_compact_options_live_in_one_settings_tab() {
        assert_eq!(
            Section::ALL,
            [
                Section::Compact,
                Section::Weather,
                Section::QuietHours,
                Section::Pomodoro,
                Section::Bilibili,
                Section::About
            ]
        );
        assert_eq!(registry().by_kind(ExtensionKind::Compact).len(), 9);
    }
}
