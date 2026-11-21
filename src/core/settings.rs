//! Versioned, atomic host storage for ordinary extension settings.
use crate::extension::{
    ContributionDescriptor, ExtensionDescriptor, ExtensionId, ReloadPolicy, SettingSchema, Settings,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Record {
    version: u32,
    values: Settings,
}

pub(crate) struct SettingsSpec<'a> {
    id: &'a ExtensionId,
    version: u32,
    schema: &'a [SettingSchema],
}

impl<'a> From<&'a ExtensionDescriptor> for SettingsSpec<'a> {
    fn from(descriptor: &'a ExtensionDescriptor) -> Self {
        Self {
            id: &descriptor.id,
            version: descriptor.settings_version,
            schema: &descriptor.settings,
        }
    }
}

impl<'a> From<&'a ContributionDescriptor> for SettingsSpec<'a> {
    fn from(descriptor: &'a ContributionDescriptor) -> Self {
        Self {
            id: &descriptor.id,
            version: descriptor.settings_version,
            schema: &descriptor.settings,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    records: BTreeMap<String, Record>,
    writable: bool,
}

impl SettingsStore {
    pub fn open(path: PathBuf) -> Self {
        let loaded = match fs::read(&path) {
            Ok(data) => serde_json::from_slice(&data).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(error) => Err(error),
        };
        let (records, writable) = match loaded {
            Ok(records) => (records, true),
            Err(error) => {
                eprintln!("Unable to read extension settings: {error}");
                (BTreeMap::new(), false)
            }
        };
        Self {
            path,
            records,
            writable,
        }
    }

    pub(crate) fn defaults<'a>(descriptor: impl Into<SettingsSpec<'a>>) -> Settings {
        Self::default_values(&descriptor.into())
    }

    fn default_values(descriptor: &SettingsSpec<'_>) -> Settings {
        descriptor
            .schema
            .iter()
            .filter_map(|field| {
                field
                    .default
                    .clone()
                    .map(|default| (field.key.clone(), default))
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn load<'a>(
        &mut self,
        descriptor: impl Into<SettingsSpec<'a>>,
    ) -> Result<Settings, String> {
        self.load_with_migration(descriptor, |old_version, _| {
            Err(format!("no migration for settings version {old_version}"))
        })
    }

    pub(crate) fn load_with_migration<'a>(
        &mut self,
        descriptor: impl Into<SettingsSpec<'a>>,
        migrate: impl FnOnce(u32, Settings) -> Result<Settings, String>,
    ) -> Result<Settings, String> {
        let descriptor = descriptor.into();
        let stored = self.records.get(descriptor.id.as_str());
        let seed = match stored {
            None if !self.writable => return Err("settings file is unreadable".into()),
            None => Self::default_values(&descriptor),
            Some(record) if record.version != descriptor.version => {
                migrate(record.version, record.values.clone())?
            }
            Some(record) => record.values.clone(),
        };
        let values = Self::normalized(&descriptor, seed)?;
        self.commit(&descriptor, values.clone())?;
        Ok(values)
    }

    pub(crate) fn update<'a>(
        &mut self,
        descriptor: impl Into<SettingsSpec<'a>>,
        updates: Settings,
    ) -> Result<Vec<(String, ReloadPolicy)>, String> {
        let descriptor = descriptor.into();
        let record = self
            .records
            .get(descriptor.id.as_str())
            .ok_or_else(|| "settings have not been initialized".to_owned())?;
        if record.version != descriptor.version {
            return Err("settings version does not match".into());
        }
        let mut candidate = record.values.clone();
        candidate.extend(updates);
        let candidate = Self::normalized(&descriptor, candidate)?;
        let changed = descriptor
            .schema
            .iter()
            .filter(|field| record.values.get(&field.key) != candidate.get(&field.key))
            .map(|field| (field.key.clone(), field.reload_policy))
            .collect();
        self.commit(&descriptor, candidate)?;
        Ok(changed)
    }

    pub(crate) fn retain_contributions(
        &mut self,
        registry: &crate::extension::ExtensionRegistry,
    ) -> Result<(), String> {
        let mut next = self.records.clone();
        next.retain(|id, _| {
            ExtensionId::new(id)
                .ok()
                .is_some_and(|id| registry.get(&id).is_some())
        });
        if next != self.records {
            self.write(&next).map_err(|error| error.to_string())?;
            self.records = next;
        }
        Ok(())
    }

    pub(crate) fn snapshot(&self, id: &ExtensionId) -> Option<(u32, Settings)> {
        self.records
            .get(id.as_str())
            .map(|record| (record.version, record.values.clone()))
    }

    pub fn values(&self, id: &ExtensionId) -> Option<&Settings> {
        self.records.get(id.as_str()).map(|record| &record.values)
    }

    fn normalized(descriptor: &SettingsSpec, values: Settings) -> Result<Settings, String> {
        let mut complete = Self::default_values(descriptor);
        for (key, value) in values {
            let field = descriptor
                .schema
                .iter()
                .find(|field| field.key == key)
                .ok_or_else(|| format!("unknown setting {key} in {}", descriptor.id))?;
            field.validate(&value)?;
            complete.insert(key, value);
        }
        Ok(complete)
    }

    fn commit(&mut self, descriptor: &SettingsSpec, values: Settings) -> Result<(), String> {
        let id = descriptor.id.as_str();
        let record = Record {
            version: descriptor.version,
            values,
        };
        if self.records.get(id) == Some(&record) {
            return Ok(());
        }
        if !self.writable {
            return Err("extension settings file is unreadable".into());
        }
        let mut next = self.records.clone();
        next.insert(id.to_owned(), record);
        self.write(&next).map_err(|error| error.to_string())?;
        self.records = next;
        Ok(())
    }

    fn write(&self, records: &BTreeMap<String, Record>) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("missing settings directory"))?;
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temp, records)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{ContributionKind, ExtensionRegistry, SettingSchema, SettingValue as V};

    fn descriptor(version: u32, key: &str) -> ExtensionDescriptor {
        let contribution = crate::extension::ContributionDescriptor::builtin(
            "test.settings/widget",
            ContributionKind::Widget,
            "Test",
            "",
            true,
            0,
        );
        ExtensionDescriptor::builtin("test.settings", "Test", "", vec![contribution]).with_settings(
            version,
            vec![
                SettingSchema::new(key, V::Integer(1000), "Interval").range(250.0, 10_000.0),
                SettingSchema {
                    reload_policy: ReloadPolicy::Restart,
                    ..SettingSchema::new("startup_mode", V::Boolean(false), "Startup mode")
                },
                SettingSchema {
                    options: vec!["compact".into(), "detailed".into()],
                    reload_policy: ReloadPolicy::Reload,
                    ..SettingSchema::new("mode", V::Enum("compact".into()), "Appearance")
                },
            ],
        )
    }

    #[test]
    fn seeds_once_validates_atomically_and_reloads_all_policies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let descriptor = descriptor(1, "interval");
        let mut host = SettingsStore::open(path.clone());
        let values = host.load(&descriptor).unwrap();
        assert_eq!(values["mode"], V::Enum("compact".into()));
        assert_eq!(values["interval"], V::Integer(1000));
        let before = fs::read(&path).unwrap();
        assert!(host
            .update(
                &descriptor,
                [
                    ("interval".into(), V::Integer(249)),
                    ("startup_mode".into(), V::Boolean(true))
                ]
                .into()
            )
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(host.values(&descriptor.id).unwrap(), &values);
        assert_eq!(
            host.update(
                &descriptor,
                [
                    ("interval".into(), V::Integer(2000)),
                    ("startup_mode".into(), V::Boolean(true)),
                    ("mode".into(), V::Enum("detailed".into())),
                ]
                .into()
            )
            .unwrap(),
            [
                ("interval".into(), ReloadPolicy::Live),
                ("startup_mode".into(), ReloadPolicy::Restart),
                ("mode".into(), ReloadPolicy::Reload),
            ]
        );
        let values = SettingsStore::open(path).load(&descriptor).unwrap();
        assert_eq!(values["interval"], V::Integer(2000));
        assert_eq!(values["mode"], V::Enum("detailed".into()));
        assert_eq!(values["startup_mode"], V::Boolean(true));
    }

    #[test]
    fn settings_version_change_requires_migration_and_preserves_original_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut host = SettingsStore::open(path.clone());
        let old = descriptor(1, "interval");
        let new = descriptor(2, "refresh_interval");
        host.load(&old).unwrap();
        host.update(&old, [("interval".into(), V::Integer(2500))].into())
            .unwrap();
        let initial = fs::read(&path).unwrap();
        assert!(host.load(&new).is_err());
        assert_eq!(fs::read(&path).unwrap(), initial);
        let values = host
            .load_with_migration(&new, |version, mut values| {
                assert_eq!(version, 1);
                let interval = values.remove("interval").unwrap();
                values.insert("refresh_interval".into(), interval);
                Ok(values)
            })
            .unwrap();
        assert_eq!(values["refresh_interval"], V::Integer(2500));
        let upgraded = fs::read(&path).unwrap();
        assert_ne!(upgraded, initial);
        assert!(host.update(&old, Settings::new()).is_err());
        assert_eq!(fs::read(path).unwrap(), upgraded);
    }

    #[test]
    fn unreadable_file_is_never_overwritten_and_bad_defaults_fail_registration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{damaged").unwrap();
        let mut host = SettingsStore::open(path.clone());
        let mut descriptor = descriptor(1, "interval");
        assert!(host.load(&descriptor).is_err());
        assert_eq!(fs::read(path).unwrap(), b"{damaged");
        descriptor.settings[0].default = Some(V::Float(f64::NAN));
        assert!(ExtensionRegistry::default().register(descriptor).is_err());
    }
}
