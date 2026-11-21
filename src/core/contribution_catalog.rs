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
        Ok(contents) => serde_json::from_slice(&contents).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SavedCatalog::default()),
        Err(error) => Err(error),
    }
}
