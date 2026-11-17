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

#[cfg(test)]
mod tests {
    use super::Section;
    use crate::{builtins::registry, extension::ExtensionKind};

    #[test]
    fn all_compact_options_live_in_one_settings_tab() {
        assert_eq!(
            Section::ALL,
            [Section::Compact, Section::QuietHours, Section::About]
        );
        assert_eq!(registry().by_kind(ExtensionKind::Compact).len(), 7);
    }
}
