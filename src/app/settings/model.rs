#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) enum Section {
    #[default]
    General,
    Capsule,
    Notifications,
    QuietHours,
    Extensions,
    Secrets,
    About,
}
impl Section {
    pub const ALL: [Self; 7] = [
        Self::General,
        Self::Capsule,
        Self::Notifications,
        Self::QuietHours,
        Self::Extensions,
        Self::Secrets,
        Self::About,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "通用",
            Self::Capsule => "胶囊",
            Self::Notifications => "通知",
            Self::QuietHours => "免打扰",
            Self::Extensions => "扩展",
            Self::Secrets => "秘密",
            Self::About => "关于",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_includes_the_notification_manager() {
        assert_eq!(
            Section::ALL.map(Section::label),
            ["通用", "胶囊", "通知", "免打扰", "扩展", "秘密", "关于"]
        );
    }
}
