//! Durable snapshots let settings show dynamic contributions without starting their owner.
use crate::extension::{ContributionDescriptor, ExtensionId, ExtensionRegistry};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

type Snapshots = BTreeMap<ExtensionId, Vec<ContributionDescriptor>>;
type MigratedPreferences = BTreeMap<ExtensionId, bool>;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SavedCatalog {
    snapshots: Snapshots,
    #[serde(default)]
    preferences: MigratedPreferences,
}

pub(super) struct ContributionCatalog {
    path: Option<PathBuf>,
    writable: bool,
    preferences: MigratedPreferences,
}

impl ContributionCatalog {
    pub(super) fn load(path: Option<PathBuf>, registry: &mut ExtensionRegistry) -> Self {
        let mut writable = true;
        let mut preferences = BTreeMap::new();
        if let Some(path) = path.as_deref() {
            match read(path) {
                Ok(saved) => {
                    preferences = saved.preferences;
                    for (owner, entries) in saved.snapshots {
                        if registry.extension(&owner).is_some() {
                            if let Err(error) = registry.sync_dynamic(&owner, entries) {
                                eprintln!("Unable to restore contributions for {owner}: {error}");
                                writable = false;
                            }
                        }
                    }
                }
                Err(error) => {
                    eprintln!("Unable to read contribution catalog: {error}");
                    writable = false;
                }
            }
        }
        Self {
            path,
            writable,
            preferences,
        }
    }

    pub(super) fn save(&self, registry: &ExtensionRegistry) -> io::Result<()> {
        if !self.writable {
            return Err(io::Error::other("contribution catalog could not be loaded"));
        }
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("missing catalog directory"))?;
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(
            &mut temp,
            &SavedCatalog {
                snapshots: registry.dynamic_snapshots(),
                preferences: self.preferences.clone(),
            },
        )?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
    pub(super) fn migrated_preference(&self, id: &ExtensionId) -> Option<bool> {
        self.preferences.get(id).copied()
    }

    pub(super) fn import_preferences(
        &mut self,
        registry: &ExtensionRegistry,
        migrated: MigratedPreferences,
    ) -> io::Result<()> {
        if migrated.is_empty() {
            return Ok(());
        }
        let previous = self.preferences.clone();
        for (id, desired) in migrated {
            self.preferences.entry(id).or_insert(desired);
        }
        if self.preferences != previous {
            if let Err(error) = self.save(registry) {
                self.preferences = previous;
                return Err(error);
            }
        }
        Ok(())
    }
}

fn read(path: &Path) -> io::Result<SavedCatalog> {
    match fs::read(path) {
        Ok(contents) => {
            let mut data: serde_json::Value =
                serde_json::from_slice(&contents).map_err(io::Error::other)?;
            if migrate_kind(&mut data) {
                // Best effort atomic one-time write. Failed persistence never discards
                // already loaded dynamic contributions; the retry happens next launch.
                if let Some(parent) = path.parent() {
                    if let Ok(mut temp) = tempfile::NamedTempFile::new_in(parent) {
                        if serde_json::to_writer_pretty(&mut temp, &data).is_ok()
                            && temp.write_all(b"\n").is_ok()
                            && temp.as_file().sync_all().is_ok()
                        {
                            if let Err(error) = temp.persist(path) {
                                eprintln!("Unable to persist contribution catalog migration: {error}");
                            }
                        }
                    }
                }
            }
            serde_json::from_value(data).map_err(io::Error::other)
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SavedCatalog::default()),
        Err(error) => Err(error),
    }
}


/// Older catalogs serialized ContributionKind::Compact under "kind".
/// Rewrite only that enum value, leaving business strings and IDs untouched.
fn migrate_kind(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(properties) => {
            let mut changed = false;
            if properties.get("kind").and_then(serde_json::Value::as_str) == Some("Compact") {
                properties.insert("kind".into(), serde_json::Value::String("Capsule".into()));
                changed = true;
            }
            for child in properties.values_mut() {
                changed |= migrate_kind(child);
            }
            changed
        }
        serde_json::Value::Array(items) => {
            items.iter_mut().fold(false, |changed, item| migrate_kind(item) | changed)
        }
        _ => false,
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn upgrades_kind_without_changing_ids_or_business_labels() {
        let mut json = serde_json::json!({
            "snapshots": {
                "example": [
                    {"kind": "Compact", "id": "test.compact", "title": "Compact sample"},
                    {"kind": "Widget", "id": "test.widget"}
                ]
            }
        });
        assert!(migrate_kind(&mut json));
        assert!(!migrate_kind(&mut json));
        assert_eq!(json["snapshots"]["example"][0]["kind"], "Capsule");
        assert_eq!(json["snapshots"]["example"][0]["id"], "test.compact");
        assert_eq!(json["snapshots"]["example"][0]["title"], "Compact sample");
    }
}
