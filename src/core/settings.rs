//! Versioned, atomic host storage for ordinary extension settings.
use crate::extension::{ExtensionDescriptor, ExtensionId, ReloadPolicy, Settings};
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

    pub fn defaults(descriptor: &ExtensionDescriptor) -> Settings {
        descriptor
            .settings
            .iter()
            .filter_map(|field| {
                field
                    .default
                    .clone()
                    .map(|default| (field.key.clone(), default))
            })
            .collect()
    }

    pub fn load(
        &mut self,
        descriptor: &ExtensionDescriptor,
        legacy: Settings,
        migrate: impl FnOnce(u32, &mut Settings) -> Result<(), String>,
    ) -> Result<Settings, String> {
        let stored = self.records.get(descriptor.id.as_str());
        let mut legacy = legacy;
        for contribution in &descriptor.contributions {
            if let Some(record) = self.records.get(contribution.id.as_str()) {
                legacy.extend(record.values.clone());
            }
        }
        let seed = match stored {
            None if !self.writable => return Err("extension settings file is unreadable".into()),
            None => legacy,
            Some(record) if record.version > descriptor.settings_version => {
                return Err(format!(
                    "settings for {} were saved by a newer extension",
                    descriptor.id
                ))
            }
            Some(record) if record.version < descriptor.settings_version => {
                let mut values = record.values.clone();
                migrate(record.version, &mut values)?;
                values.retain(|key, _| descriptor.settings.iter().any(|field| field.key == *key));
                values
            }
            Some(record) => record.values.clone(),
        };
        let values = Self::normalized(descriptor, seed)?;
        self.commit(descriptor, values.clone())?;
        Ok(values)
    }

    pub fn update(
        &mut self,
        descriptor: &ExtensionDescriptor,
        updates: Settings,
    ) -> Result<Vec<(String, ReloadPolicy)>, String> {
        let record = self
            .records
            .get(descriptor.id.as_str())
            .ok_or_else(|| "settings have not been initialized".to_owned())?;
        if record.version != descriptor.settings_version {
            return Err("settings version does not match".into());
        }
        let mut candidate = record.values.clone();
        candidate.extend(updates);
        let candidate = Self::normalized(descriptor, candidate)?;
        let changed = descriptor
            .settings
            .iter()
            .filter(|field| record.values.get(&field.key) != candidate.get(&field.key))
            .map(|field| (field.key.clone(), field.reload_policy))
            .collect();
        self.commit(descriptor, candidate)?;
        Ok(changed)
    }

    pub fn is_current(&self, descriptor: &ExtensionDescriptor) -> bool {
        self.records.get(descriptor.id.as_str()).is_some_and(|r| {
            r.version == descriptor.settings_version
                && Self::normalized(descriptor, r.values.clone())
                    .is_ok_and(|normalized| normalized == r.values)
        })
    }
    pub fn values(&self, id: &ExtensionId) -> Option<&Settings> {
        self.records.get(id.as_str()).map(|record| &record.values)
    }

    fn normalized(descriptor: &ExtensionDescriptor, values: Settings) -> Result<Settings, String> {
        let mut complete = Self::defaults(descriptor);
        for (key, value) in values {
            let field = descriptor
                .settings
                .iter()
                .find(|field| field.key == key)
                .ok_or_else(|| format!("unknown setting {key} in {}", descriptor.id))?;
            field.validate(&value)?;
            complete.insert(key, value);
        }
        Ok(complete)
    }

    fn commit(&mut self, descriptor: &ExtensionDescriptor, values: Settings) -> Result<(), String> {
        let id = descriptor.id.as_str();
        let record = Record {
            version: descriptor.settings_version,
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
        for contribution in &descriptor.contributions {
            next.remove(contribution.id.as_str());
        }
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
        )
        .with_settings(
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
        );
        let mut descriptor =
            ExtensionDescriptor::builtin("test.settings", "Test", "", vec![contribution]);
        descriptor.settings_version = version;
        descriptor
    }

    #[test]
    fn seeds_once_validates_atomically_and_reloads_all_policies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let descriptor = descriptor(1, "interval");
        let mut host = SettingsStore::open(path.clone());
        let values = host
            .load(
                &descriptor,
                [("interval".into(), V::Integer(500))].into(),
                |_, _| unreachable!(),
            )
            .unwrap();
        assert_eq!(values["mode"], V::Enum("compact".into()));
        assert_eq!(values["interval"], V::Integer(500));
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
        let values = SettingsStore::open(path)
            .load(
                &descriptor,
                [("interval".into(), V::Integer(500))].into(),
                |_, _| unreachable!(),
            )
            .unwrap();
        assert_eq!(values["interval"], V::Integer(2000));
        assert_eq!(values["mode"], V::Enum("detailed".into()));
        assert_eq!(values["startup_mode"], V::Boolean(true));
    }

    #[test]
    fn failed_migration_and_downgrade_leave_storage_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut host = SettingsStore::open(path.clone());
        let old = descriptor(1, "interval");
        let new = descriptor(2, "refresh_interval");
        host.load(
            &old,
            [("interval".into(), V::Integer(2500))].into(),
            |_, _| unreachable!(),
        )
        .unwrap();
        let initial = fs::read(&path).unwrap();
        assert!(host
            .load(&new, Settings::new(), |_, _| Err("migration failed".into()))
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), initial);
        let values = host
            .load(&new, Settings::new(), |version, values| {
                assert_eq!(version, 1);
                let interval = values.remove("interval").unwrap();
                values.insert("refresh_interval".into(), interval);
                Ok(())
            })
            .unwrap();
        assert_eq!(values["refresh_interval"], V::Integer(2500));
        let upgraded = fs::read(&path).unwrap();
        assert!(host
            .load(&old, Settings::new(), |_, _| unreachable!())
            .is_err());
        assert!(host.update(&old, Settings::new()).is_err());
        assert_eq!(fs::read(&path).unwrap(), upgraded);
    }

    #[test]
    fn unreadable_file_is_never_overwritten_and_bad_defaults_fail_registration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{damaged").unwrap();
        let mut host = SettingsStore::open(path.clone());
        let mut descriptor = descriptor(1, "interval");
        assert!(host
            .load(&descriptor, Settings::new(), |_, _| unreachable!())
            .is_err());
        assert_eq!(fs::read(path).unwrap(), b"{damaged");
        descriptor.settings[0].default = Some(V::Float(f64::NAN));
        assert!(ExtensionRegistry::default().register(descriptor).is_err());
    }
}
