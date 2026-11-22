use crate::extension::{
    CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind, RuntimeExtension,
};
use crate::ui::declarative::{ColorSpec, Node, ViewNode};

const MASCOT_ID: &str = "builtin.mascot/face";
const CLOCK_ID: &str = "builtin.clock/time";

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(MASCOT_ID, ExtensionKind::Compact, "表情", "mascot", true, 0)
            .with_size_hint(38.0, 28.0),
        ExtensionDescriptor::builtin(CLOCK_ID, ExtensionKind::Compact, "时间", "clock", false, 6)
            .with_size_hint(56.0, 28.0),
    ]
}

pub struct CompactExtras;

impl crate::extension::NotificationPresentationProvider for CompactExtras {}

impl RuntimeExtension for CompactExtras {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        descriptors()
    }

    fn compact_refresh_interval(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(1))
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let time =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        vec![
            CompactView {
                id: ExtensionId::builtin(MASCOT_ID),
                source_extension_id: ExtensionId::builtin(MASCOT_ID),
                content: ViewNode::new(Node::Spacer),
                visibility: Default::default(),
                min_width: 38.0,
                preferred_width: 38.0,
                fit_width: false,
                priority: 0,
                tooltip: Some("灵动岛吉祥物".into()),
            },
            CompactView {
                id: ExtensionId::builtin(CLOCK_ID),
                source_extension_id: ExtensionId::builtin(CLOCK_ID),
                content: super::icon_text(
                    "clock",
                    format!("{:02}:{:02}", time.hour(), time.minute()),
                    ColorSpec::PrimaryText,
                    16.0,
                ),
                visibility: Default::default(),
                min_width: 56.0,
                preferred_width: 56.0,
                fit_width: false,
                priority: 0,
                tooltip: None,
            },
        ]
    }
}
