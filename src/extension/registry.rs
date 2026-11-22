use super::{ContributionDescriptor, ContributionKind, ExtensionDescriptor, ExtensionId};
use std::{
    collections::{BTreeMap, HashSet},
    fmt,
};
#[derive(Clone, Debug, Default)]
pub struct ExtensionRegistry {
    descriptors: Vec<ExtensionDescriptor>,
    static_counts: BTreeMap<ExtensionId, usize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    Duplicate(ExtensionId),
    UnknownOwner(ExtensionId),
    InvalidSchema(ExtensionId, String),
}
impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "duplicate extension or contribution: {id}"),
            Self::UnknownOwner(id) => write!(f, "unknown extension: {id}"),
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
        for contribution in &descriptor.contributions {
            validate_contribution(contribution)?;
            validate_dependencies(contribution, &descriptor.contributions)?;
        }
        self.static_counts
            .insert(descriptor.id.clone(), descriptor.contributions.len());
        self.descriptors.push(descriptor);
        Ok(())
    }
    /// Replace only the dynamic part of the owner's registrations. The existing
    /// snapshot remains visible if any entry fails validation.
    pub fn sync_dynamic(
        &mut self,
        owner: &ExtensionId,
        snapshot: Vec<ContributionDescriptor>,
    ) -> Result<bool, RegistrationError> {
        let index = self
            .descriptors
            .iter()
            .position(|descriptor| &descriptor.id == owner)
            .ok_or_else(|| RegistrationError::UnknownOwner(owner.clone()))?;
        let static_count = self.static_counts[owner];
        let previous = &self.descriptors[index].contributions[static_count..];
        if previous == snapshot {
            return Ok(false);
        }
        let prefix = format!("{owner}/");
        let static_ids: HashSet<_> = self.descriptors[index].contributions[..static_count]
            .iter()
            .map(|entry| &entry.id)
            .collect();
        let mut ids = HashSet::new();
        for entry in &snapshot {
            if !entry
                .id
                .as_str()
                .strip_prefix(&prefix)
                .is_some_and(|local| !local.is_empty())
            {
                return Err(RegistrationError::InvalidSchema(
                    entry.id.clone(),
                    format!("dynamic contribution must belong to {owner}"),
                ));
            }
            if entry.id.as_str() == NOTIFICATION_CENTER_ID
                || !ids.insert(&entry.id)
                || static_ids.contains(&entry.id)
                || self.extension(&entry.id).is_some()
                || self
                    .owner(&entry.id)
                    .is_some_and(|descriptor| &descriptor.id != owner)
            {
                return Err(RegistrationError::Duplicate(entry.id.clone()));
            }
            if previous
                .iter()
                .any(|old| old.id == entry.id && old.kind != entry.kind)
            {
                return Err(RegistrationError::InvalidSchema(
                    entry.id.clone(),
                    "a contribution ID cannot change kind".into(),
                ));
            }
            validate_contribution(entry)?;
            validate_dependencies(
                entry,
                &self.descriptors[index].contributions[..static_count],
            )?;
        }
        self.descriptors[index]
            .contributions
            .splice(static_count.., snapshot);
        Ok(true)
    }

    pub(crate) fn dynamic_entries(&self, owner: &ExtensionId) -> &[ContributionDescriptor] {
        self.extension(owner).map_or(&[], |descriptor| {
            &descriptor.contributions[self.static_counts[owner]..]
        })
    }

    pub(crate) fn dynamic_snapshots(&self) -> BTreeMap<ExtensionId, Vec<ContributionDescriptor>> {
        self.descriptors
            .iter()
            .filter_map(|descriptor| {
                let entries = &descriptor.contributions[self.static_counts[&descriptor.id]..];
                (!entries.is_empty()).then(|| (descriptor.id.clone(), entries.to_vec()))
            })
            .collect()
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
fn validate_contribution(entry: &ContributionDescriptor) -> Result<(), RegistrationError> {
    let invalid = |error: String| RegistrationError::InvalidSchema(entry.id.clone(), error);
    if entry.schema_version != 1 || entry.settings_version == 0 {
        return Err(invalid("unsupported schema or settings version".into()));
    }
    if entry.title.trim().is_empty() {
        return Err(invalid("contribution title is empty".into()));
    }
    super::validate_schema(&entry.settings).map_err(invalid)
}

fn validate_dependencies(
    entry: &ContributionDescriptor,
    candidates: &[ContributionDescriptor],
) -> Result<(), RegistrationError> {
    let mut seen = HashSet::new();
    if (!entry.dependencies.is_empty() && entry.kind == ContributionKind::BackgroundService)
        || entry.dependencies.iter().any(|id| {
            !seen.insert(id)
                || !candidates.iter().any(|resource| {
                    &resource.id == id && resource.kind == ContributionKind::BackgroundService
                })
        })
    {
        return Err(RegistrationError::InvalidSchema(
            entry.id.clone(),
            "dependencies must reference unique background services declared by the owner".into(),
        ));
    }
    Ok(())
}

pub const NOTIFICATION_CENTER_ID: &str = "builtin.notifications/center";
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_snapshots_are_atomic_and_cannot_steal_an_id() {
        let owner = ExtensionId::builtin("test.catalog");
        let mut registry = ExtensionRegistry::default();
        registry
            .register(ExtensionDescriptor::builtin(
                "test.catalog",
                "Catalog",
                "",
                vec![],
            ))
            .unwrap();
        registry
            .register(ExtensionDescriptor::builtin(
                "test.other",
                "Other",
                "",
                vec![],
            ))
            .unwrap();
        let make = |suffix, kind| {
            ContributionDescriptor::new(
                ExtensionId::new(format!("{owner}/{suffix}")).unwrap(),
                kind,
                suffix,
                "test",
                false,
                0,
            )
        };
        let entries = vec![
            make("first", ContributionKind::Compact),
            make("second", ContributionKind::Widget),
            make("third", ContributionKind::Notification),
        ];
        assert!(registry.sync_dynamic(&owner, entries.clone()).unwrap());
        assert!(!registry.sync_dynamic(&owner, entries.clone()).unwrap());
        let mut duplicate = entries.clone();
        duplicate.push(entries[0].clone());
        assert!(registry.sync_dynamic(&owner, duplicate).is_err());
        assert!(registry
            .sync_dynamic(&ExtensionId::builtin("test.other"), entries.clone())
            .is_err());
        assert!(registry
            .sync_dynamic(&owner, vec![make("first", ContributionKind::Widget)])
            .is_err());
        assert_eq!(registry.dynamic_entries(&owner), entries);
        assert!(registry.sync_dynamic(&owner, vec![]).unwrap());
        assert!(entries
            .iter()
            .all(|entry| registry.owner(&entry.id).is_none()));
    }

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
