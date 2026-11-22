//! Power and USB belong to one device extension with distinct prompt contributions.
//! Both monitors retain their established OS-specific sampling and vector UI.
mod battery;
mod power;
mod usb;

use crate::{
    core::mascot::MascotExpression,
    extension::{
        CompactView, ExtensionContext, ExtensionDescriptor, ExtensionOutput,
        NotificationPresentationProvider, RuntimeExtension,
    },
};
use std::time::Instant;

#[derive(Default)]
pub struct DeviceExtension {
    battery: battery::BatteryExtension,
    usb: usb::UsbExtension,
}

impl NotificationPresentationProvider for DeviceExtension {}

impl RuntimeExtension for DeviceExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        let mut contributions = self.battery.descriptor().contributions;
        contributions.extend(self.usb.descriptor().contributions);
        let mut descriptor = ExtensionDescriptor::builtin(
            "builtin.device",
            "设备",
            "电池状态、充电提示与 USB 插拔提醒",
            contributions,
        );
        // Preserve the two existing monitors' update cadence without polling
        // a dormant extension; the lifecycle host owns start/stop.
        descriptor.execution_policy =
            crate::extension::ExecutionPolicy::Interval(std::time::Duration::from_millis(250));
        descriptor
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.battery.on_start(ctx)?;
        if let Err(error) = self.usb.on_start(ctx) {
            let _ = self.battery.on_stop(ctx);
            return Err(error);
        }
        Ok(())
    }

    fn on_stop(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        let usb = self.usb.on_stop(ctx);
        let battery = self.battery.on_stop(ctx);
        usb.and(battery)
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut power = self.battery.update(now);
        let usb = self.usb.update(now);
        power.invalidated.extend(usb.invalidated);
        power.prompts.extend(usb.prompts);
        power.events.extend(usb.events);
        power.notifications.extend(usb.notifications);
        power.resolved_notifications.extend(usb.resolved_notifications);
        power.schedule.extend(usb.schedule);
        power.cancel_tasks.extend(usb.cancel_tasks);
        power.mascot_events.extend(usb.mascot_events);
        power
    }

    fn compact_views(&self) -> Vec<CompactView> {
        self.battery.compact_views()
    }

    fn mascot_state(&self) -> std::collections::BTreeMap<String, String> {
        self.battery.mascot_state()
    }

    fn mascot_expressions(&self) -> Vec<MascotExpression> {
        self.battery.mascot_expressions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{ContributionKind, ExtensionId};

    #[test]
    fn device_is_one_extension_with_independent_compact_and_two_prompts() {
        let descriptor = DeviceExtension::default().descriptor();
        assert_eq!(descriptor.id, ExtensionId::builtin("builtin.device"));
        assert_eq!(descriptor.name, "设备");
        let compact: Vec<_> = descriptor.contributions.iter()
            .filter(|c| c.kind == ContributionKind::Compact).collect();
        let prompts: Vec<_> = descriptor.contributions.iter()
            .filter(|c| c.kind == ContributionKind::Prompt).collect();
        assert_eq!(compact.len(), 1);
        assert_eq!(prompts.len(), 2);
        assert!(compact.iter().any(|c| c.id.as_str() == "builtin.device/battery"));
        assert!(prompts.iter().any(|c| c.id.as_str() == "builtin.device/power-prompt"));
        assert!(prompts.iter().any(|c| c.id.as_str() == "builtin.device/usb-prompt"));
        let registry = super::super::registry();
        assert!(registry.extensions().iter().any(|d| d.id == descriptor.id));
        assert!(!registry.extensions().iter().any(|d| {
            d.id.as_str() == "builtin.battery" || d.id.as_str() == "builtin.usb"
        }));
    }

    #[test]
    fn usb_prompt_is_owned_by_the_new_device_extension() {
        let old = std::collections::BTreeMap::new();
        let current = [("usb1".into(), "测试 U 盘".into())].into();
        let prompts = usb::changes(&old, &current);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].source_extension_id, ExtensionId::builtin("builtin.device"));
        assert_eq!(prompts[0].contribution_id,
            ExtensionId::builtin("builtin.device/usb-prompt"));
    }
}
