use super::{ExtensionDescriptor, ExtensionId, ExtensionKind};
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

    pub fn widget_list(&self) -> Vec<&ExtensionDescriptor> {
        let mut widgets: Vec<_> = self
            .descriptors
            .iter()
            .filter(|descriptor| {
                descriptor.kind == ExtensionKind::Widget
                    && (descriptor.default_enabled
                        || descriptor.id.as_str() == NOTIFICATION_CENTER_ID)
            })
            .collect();
        widgets.sort_by_key(|descriptor| {
            (
                if descriptor.id.as_str() == NOTIFICATION_CENTER_ID {
                    0
                } else {
                    descriptor.default_order.saturating_add(1)
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

#[cfg(test)]
mod tests {
    use super::{ExtensionDescriptor, ExtensionKind, ExtensionRegistry, NOTIFICATION_CENTER_ID};

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

    #[test]
    fn fixed_notification_center_is_included_even_when_disabled() {
        let mut registry = ExtensionRegistry::default();
        registry
            .register(ExtensionDescriptor::builtin(
                NOTIFICATION_CENTER_ID,
                ExtensionKind::Widget,
                "通知中心",
                "bell",
                false,
                99,
            ))
            .unwrap();
        registry
            .register(ExtensionDescriptor::builtin(
                "builtin.test/widget",
                ExtensionKind::Widget,
                "Test",
                "test",
                true,
                0,
            ))
            .unwrap();
        let widgets = registry.widget_list();
        assert_eq!(widgets[0].id.as_str(), NOTIFICATION_CENTER_ID);
        assert_eq!(widgets.len(), 2);
    }
}
