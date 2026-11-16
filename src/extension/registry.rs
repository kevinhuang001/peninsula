use super::{Capability, ExtensionDescriptor, ExtensionId, ExtensionKind};
use std::{collections::HashSet, fmt};

#[derive(Clone, Debug, Default)]
pub struct ExtensionRegistry {
    descriptors: Vec<ExtensionDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationError(pub ExtensionId);

impl fmt::Display for RegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "duplicate extension ID: {}", self.0)
    }
}

impl std::error::Error for RegistrationError {}

impl ExtensionRegistry {
    pub fn register(&mut self, descriptor: ExtensionDescriptor) -> Result<(), RegistrationError> {
        if self.get(&descriptor.id).is_some() {
            return Err(RegistrationError(descriptor.id));
        }
        self.descriptors.push(descriptor);
        Ok(())
    }

    pub fn get(&self, id: &ExtensionId) -> Option<&ExtensionDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| &descriptor.id == id)
    }

    pub fn by_kind(&self, kind: ExtensionKind) -> Vec<&ExtensionDescriptor> {
        let mut result: Vec<_> = self
            .descriptors
            .iter()
            .filter(|descriptor| descriptor.kind == kind)
            .collect();
        result.sort_by_key(|descriptor| (descriptor.default_order, descriptor.id.clone()));
        result
    }

    pub fn widget_list(&self, notification_unread: bool) -> Vec<&ExtensionDescriptor> {
        let mut widgets: Vec<_> = self
            .descriptors
            .iter()
            .filter(|descriptor| {
                descriptor.kind == ExtensionKind::Widget && descriptor.default_enabled
            })
            .collect();
        if notification_unread {
            widgets.retain(|descriptor| descriptor.id.as_str() != NOTIFICATION_CENTER_ID);
            if let Some(center) = self.get(&ExtensionId::builtin(NOTIFICATION_CENTER_ID)) {
                widgets.insert(0, center);
            }
        }
        widgets.sort_by_key(|descriptor| {
            (
                if notification_unread && descriptor.id.as_str() == NOTIFICATION_CENTER_ID {
                    0
                } else {
                    descriptor.default_order + 1
                },
                descriptor.id.clone(),
            )
        });
        widgets
    }

    pub fn validate(&self) -> bool {
        let mut ids = HashSet::with_capacity(self.descriptors.len());
        self.descriptors
            .iter()
            .all(|descriptor| descriptor.schema_version == 1 && ids.insert(descriptor.id.clone()))
    }
}

pub const NOTIFICATION_CENTER_ID: &str = "builtin.notifications/center";

pub fn builtins() -> ExtensionRegistry {
    use ExtensionKind::{Compact, Notification, Widget};
    let mut registry = ExtensionRegistry::default();
    let entries = [
        ExtensionDescriptor::builtin(
            "builtin.system-monitor/overview",
            Widget,
            "性能监控",
            "activity",
            true,
            0,
        )
        .with_size_hint(300.0, 76.0),
        ExtensionDescriptor::builtin("builtin.pomodoro/timer", Widget, "番茄钟", "timer", true, 1)
            .with_size_hint(300.0, 126.0)
            .with_capabilities(vec![
                Capability::View,
                Capability::Action,
                Capability::Schedule,
            ]),
        ExtensionDescriptor::builtin(NOTIFICATION_CENTER_ID, Widget, "通知中心", "bell", false, 2)
            .with_size_hint(300.0, 170.0),
        ExtensionDescriptor::builtin("builtin.mascot/face", Compact, "表情", "mascot", true, 0)
            .with_size_hint(38.0, 28.0),
        ExtensionDescriptor::builtin(
            "builtin.system-monitor/battery",
            Compact,
            "电池",
            "battery",
            true,
            1,
        )
        .with_size_hint(54.0, 28.0),
        ExtensionDescriptor::builtin(
            "builtin.system-monitor/cpu",
            Compact,
            "CPU",
            "cpu",
            false,
            2,
        )
        .with_size_hint(56.0, 28.0),
        ExtensionDescriptor::builtin(
            "builtin.system-monitor/gpu",
            Compact,
            "GPU",
            "gpu",
            false,
            3,
        )
        .with_size_hint(56.0, 28.0),
        ExtensionDescriptor::builtin(
            "builtin.system-monitor/network",
            Compact,
            "网络",
            "network",
            false,
            4,
        )
        .with_size_hint(80.0, 28.0),
        ExtensionDescriptor::builtin(
            "builtin.downloads/status",
            Compact,
            "下载",
            "download",
            false,
            5,
        )
        .with_size_hint(88.0, 28.0),
        ExtensionDescriptor::builtin("builtin.clock/time", Compact, "时间", "clock", false, 6)
            .with_size_hint(56.0, 28.0),
        ExtensionDescriptor::builtin(
            "builtin.downloads/notifications",
            Notification,
            "下载提醒",
            "bell",
            true,
            0,
        ),
    ];
    for entry in entries {
        registry
            .register(entry)
            .expect("built-in extension IDs must be unique");
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::{builtins, ExtensionDescriptor, ExtensionKind, ExtensionRegistry};

    #[test]
    fn built_in_registry_has_stable_unique_descriptors_and_order() {
        let registry = builtins();
        assert!(registry.validate());
        assert_eq!(registry.by_kind(ExtensionKind::Widget).len(), 3);
        assert_eq!(registry.by_kind(ExtensionKind::Compact).len(), 7);
        assert_eq!(registry.by_kind(ExtensionKind::Notification).len(), 1);
        let ids: Vec<_> = registry
            .widget_list(true)
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(ids[0], super::NOTIFICATION_CENTER_ID);
    }

    #[test]
    fn duplicate_ids_fail_registration_instead_of_replacing_entries() {
        let mut registry = ExtensionRegistry::default();
        let entry = ExtensionDescriptor::builtin(
            "builtin.test/item",
            ExtensionKind::Compact,
            "Test",
            "test",
            true,
            0,
        );
        registry.register(entry.clone()).unwrap();
        assert!(registry.register(entry).is_err());
        assert_eq!(registry.by_kind(ExtensionKind::Compact).len(), 1);
    }
}
