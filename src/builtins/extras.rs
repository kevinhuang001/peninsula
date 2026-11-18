use crate::extension::{
    CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind, RuntimeExtension,
};
use crate::ui::declarative::{ColorSpec, CompactLayout, ViewNode};

pub(crate) mod mascot;

const CLOCK_ID: &str = "builtin.clock/time";

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![mascot::descriptor(), clock_descriptor()]
}

fn clock_descriptor() -> ExtensionDescriptor {
    ExtensionDescriptor::builtin(CLOCK_ID, ExtensionKind::Compact, "时间", "clock", false, 6)
        .with_size_hint(64.0, 28.0)
}

pub struct CompactExtras;

impl crate::extension::NotificationPresentationProvider for CompactExtras {}

impl RuntimeExtension for CompactExtras {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        vec![clock_descriptor()]
    }

    fn compact_refresh_interval(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_millis(250))
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let time =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        vec![CompactView {
            id: ExtensionId::builtin(CLOCK_ID),
            source_extension_id: ExtensionId::builtin(CLOCK_ID),
            content: ViewNode::compact(
                ViewNode::icon("clock-live", 18.0).color(ColorSpec::SecondaryText),
                super::compact_value(format!("{:02}:{:02}", time.hour(), time.minute()), true),
                CompactLayout {
                    leading_size: [18.0, 20.0],
                    gap: 4.0,
                    ..Default::default()
                },
            ),
            visibility: Default::default(),
            min_width: 64.0,
            preferred_width: 64.0,
            fit_width: false,
            priority: 0,
            tooltip: None,
        }]
    }
}
