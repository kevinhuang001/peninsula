use super::{ContributionDescriptor, ContributionKind, ExtensionDescriptor, ExtensionId};
use std::{collections::HashSet, fmt};
#[derive(Clone, Debug, Default)]
pub struct ExtensionRegistry {
    descriptors: Vec<ExtensionDescriptor>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    Duplicate(ExtensionId),
    InvalidSchema(ExtensionId, String),
}
impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "duplicate extension or contribution: {id}"),
            Self::InvalidSchema(id, error) => write!(f, "{id}: {error}"),
        }
    }
}
impl std::error::Error for RegistrationError {}
impl ExtensionRegistry {
    pub fn register(&mut self, descriptor: ExtensionDescriptor) -> Result<(), RegistrationError> {
        let mut local = HashSet::new();
        if self.extension(&descriptor.id).is_some()
            || self.get(&descriptor.id).is_some()
            || descriptor.contributions.iter().any(|c| {
                !local.insert(c.id.clone())
                    || c.id == descriptor.id
                    || self.get(&c.id).is_some()
                    || self.extension(&c.id).is_some()
            })
        {
            return Err(RegistrationError::Duplicate(descriptor.id));
        }
        if descriptor.id.as_str() == NOTIFICATION_CENTER_ID
            || descriptor
                .contributions
                .iter()
                .any(|c| c.id.as_str() == NOTIFICATION_CENTER_ID)
        {
            return Err(RegistrationError::InvalidSchema(
                descriptor.id,
                "Core pages cannot be registered by extensions".into(),
            ));
        }
        if descriptor.settings_version == 0
            || descriptor
                .contributions
                .iter()
                .any(|c| c.schema_version != 1)
        {
            return Err(RegistrationError::InvalidSchema(
                descriptor.id,
                "unsupported schema version".into(),
            ));
        }
        super::validate_schema(&descriptor.settings)
            .map_err(|e| RegistrationError::InvalidSchema(descriptor.id.clone(), e))?;
        self.descriptors.push(descriptor);
        Ok(())
    }
    pub fn extensions(&self) -> &[ExtensionDescriptor] {
        &self.descriptors
    }
    pub fn extension(&self, id: &ExtensionId) -> Option<&ExtensionDescriptor> {
        self.descriptors.iter().find(|d| &d.id == id)
    }
    pub fn owner(&self, id: &ExtensionId) -> Option<&ExtensionDescriptor> {
        self.descriptors
            .iter()
            .find(|d| &d.id == id || d.contributions.iter().any(|c| &c.id == id))
    }
    pub fn get(&self, id: &ExtensionId) -> Option<&ContributionDescriptor> {
        self.descriptors
            .iter()
            .flat_map(|d| &d.contributions)
            .find(|c| &c.id == id)
    }
    pub fn by_kind(&self, kind: ContributionKind) -> Vec<&ContributionDescriptor> {
        let mut result: Vec<_> = self
            .descriptors
            .iter()
            .flat_map(|d| &d.contributions)
            .filter(|c| c.kind == kind)
            .collect();
        result.sort_by_key(|c| (c.default_order, c.id.clone()));
        result
    }
    pub fn validate(&self) -> bool {
        let mut other = Self::default();
        self.descriptors
            .iter()
            .all(|d| other.register(d.clone()).is_ok())
    }
}
pub const NOTIFICATION_CENTER_ID: &str = "builtin.notifications/center";
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_contributions_and_core_pages_are_rejected() {
        let c = ContributionDescriptor::builtin(
            "test/cpu",
            ContributionKind::Compact,
            "CPU",
            "cpu",
            true,
            0,
        );
        let mut d = ExtensionDescriptor::builtin("test", "Test", "", vec![c.clone(), c.clone()]);
        let mut r = ExtensionRegistry::default();
        assert!(r.register(d.clone()).is_err());
        d.contributions.pop();
        r.register(d.clone()).unwrap();
        assert!(r.register(d).is_err());
        let fixed = ContributionDescriptor {
            id: ExtensionId::builtin(NOTIFICATION_CENTER_ID),
            ..c
        };
        assert!(r
            .register(ExtensionDescriptor::builtin("other", "", "", vec![fixed]))
            .is_err());
    }
}
