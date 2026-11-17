use crate::extension::{
    ColorRole, CompactNode, CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind,
    RuntimeExtension,
};

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

    fn compact_views(&self) -> Vec<CompactView> {
        let time =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        vec![
            CompactView {
                id: ExtensionId::builtin(MASCOT_ID),
                source_extension_id: ExtensionId::builtin(MASCOT_ID),
                content: CompactNode::Sprite {
                    asset: "mascot".into(),
                    size: [38.0, 28.0],
                },
                min_width: 38.0,
                preferred_width: 38.0,
                priority: 0,
                tooltip: Some("灵动岛吉祥物".into()),
            },
            CompactView {
                id: ExtensionId::builtin(CLOCK_ID),
                source_extension_id: ExtensionId::builtin(CLOCK_ID),
                content: CompactNode::IconText {
                    icon: "clock".into(),
                    text: format!("{:02}:{:02}", time.hour(), time.minute()),
                    color: ColorRole::Primary,
                },
                min_width: 56.0,
                preferred_width: 56.0,
                priority: 0,
                tooltip: None,
            },
        ]
    }
}
