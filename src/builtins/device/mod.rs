//! Power and USB belong to one device extension with distinct prompt contributions.
//! Both monitors retain their established OS-specific sampling and vector UI.
mod battery;
mod power;
mod usb;

use crate::{
    core::mascot::MascotExpression,
    extension::{
        CompactView, ExtensionContext, ExtensionDescriptor, ExtensionOutput, ExtensionId,
        NotificationPresentationProvider, ReloadPolicy, RuntimeExtension,
        SettingSchema, SettingValue, Settings,
    },
};
use std::time::Instant;

pub struct DeviceExtension {
    battery: battery::BatteryExtension,
    usb: usb::UsbExtension,
    power_prompt_enabled: bool,
    usb_prompt_enabled: bool,
    usb_started: bool,
    running: bool,
}

impl Default for DeviceExtension {
    fn default() -> Self {
        Self {
            battery: battery::BatteryExtension::default(),
            usb: usb::UsbExtension::default(),
            power_prompt_enabled: true,
            usb_prompt_enabled: true,
            usb_started: false,
            running: false,
        }
    }
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
        // Consolidation must not make the two independent prompts inseparable.
        // The battery Compact remains managed by the Components page.
        descriptor.settings = vec![
            SettingSchema::new(
                "power_prompt_enabled",
                SettingValue::Boolean(true),
                "电源连接提示",
            ),
            SettingSchema::new(
                "usb_prompt_enabled",
                SettingValue::Boolean(true),
                "USB 插拔提示",
            ),
        ];
        descriptor
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.battery.on_start(ctx)?;
        if self.usb_prompt_enabled {
            if let Err(error) = self.usb.on_start(ctx) {
                let _ = self.battery.on_stop(ctx);
                return Err(error);
            }
            self.usb_started = true;
        }
        self.running = true;
        Ok(())
    }

    fn on_stop(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.running = false;
        self.usb_started = false;
        let usb = self.usb.on_stop(ctx);
        let battery = self.battery.on_stop(ctx);
        usb.and(battery)
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut power = self.battery.update(now);
        if !self.power_prompt_enabled {
            power.prompts.clear();
        }
        let usb = if self.usb_started {
            self.usb.update(now)
        } else {
            ExtensionOutput::default()
        };
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

    fn on_settings_changed(
        &mut self,
        ctx: &ExtensionContext,
        _: &ExtensionId,
        settings: &Settings,
        _: ReloadPolicy,
    ) -> ExtensionOutput {
        self.power_prompt_enabled = settings.get("power_prompt_enabled")
            != Some(&SettingValue::Boolean(false));
        self.usb_prompt_enabled = settings.get("usb_prompt_enabled")
            != Some(&SettingValue::Boolean(false));
        if self.running && self.usb_prompt_enabled != self.usb_started {
            if self.usb_prompt_enabled {
                match self.usb.on_start(ctx) {
                    Ok(()) => self.usb_started = true,
                    Err(error) => ctx.log(&ExtensionId::builtin("builtin.device"),
                        &format!("USB 监测启动失败: {error}")),
                }
            } else {
                let _ = self.usb.on_stop(ctx);
                self.usb_started = false;
            }
        }
        ExtensionOutput::default()
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
        assert_eq!(descriptor.settings.len(), 2);
        let registry = super::super::registry();
        assert!(registry.extensions().iter().any(|d| d.id == descriptor.id));
        assert!(!registry.extensions().iter().any(|d| {
            d.id.as_str() == "builtin.battery" || d.id.as_str() == "builtin.usb"
        }));
    }

    #[test]
    fn changing_device_prompt_settings_keeps_battery_compact_independent() {
        let ctx = ExtensionContext::new(std::path::PathBuf::new(), std::path::PathBuf::new());
        let mut device = DeviceExtension::default();
        let settings = [
            ("power_prompt_enabled".into(), SettingValue::Boolean(false)),
            ("usb_prompt_enabled".into(), SettingValue::Boolean(false)),
        ].into();
        device.on_settings_changed(
            &ctx,
            &ExtensionId::builtin("builtin.device"),
            &settings,
            ReloadPolicy::Live,
        );
        assert!(!device.power_prompt_enabled);
        assert!(!device.usb_prompt_enabled);
        assert_eq!(device.compact_views().len(), 1);
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
