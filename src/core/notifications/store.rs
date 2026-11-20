use crate::extension::{Notification, NotificationKey, NotificationStatus};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NotificationRecord {
    pub notification: Notification,
    pub status: NotificationStatus,
}

#[derive(Clone, Debug, Default)]
pub struct NotificationStore {
    records: Vec<NotificationRecord>,
    path: Option<std::path::PathBuf>,
}

impl NotificationStore {
    pub fn open(path: std::path::PathBuf) -> Self {
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(records) => Self {
                    records,
                    path: Some(path),
                },
                Err(error) => {
                    eprintln!("Unable to read notifications; preserving file: {error}");
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self {
                records: vec![],
                path: Some(path),
            },
            Err(error) => {
                eprintln!("Unable to read notifications; preserving file: {error}");
                Self::default()
            }
        }
    }
    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let save = || -> std::io::Result<()> {
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::other("missing notification directory"))?;
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer(&mut file, &self.records)?;
            file.as_file().sync_all()?;
            file.persist(path).map_err(|e| e.error)?;
            Ok(())
        };
        if let Err(error) = save() {
            eprintln!("Unable to persist notifications: {error}");
        }
    }
    pub fn insert(&mut self, notification: Notification) -> bool {
        let key = notification.key();
        if self.get(&key).is_some() {
            return false;
        }
        self.records.push(NotificationRecord {
            notification,
            status: NotificationStatus::Pending,
        });
        self.save();
        true
    }

    pub fn get(&self, key: &NotificationKey) -> Option<&NotificationRecord> {
        self.records
            .iter()
            .find(|record| record.notification.key() == *key)
    }

    pub fn get_mut(&mut self, key: &NotificationKey) -> Option<&mut NotificationRecord> {
        self.records
            .iter_mut()
            .find(|record| record.notification.key() == *key)
    }

    pub fn pending(&self) -> impl Iterator<Item = &Notification> {
        self.records
            .iter()
            .filter(|record| record.status == NotificationStatus::Pending)
            .map(|record| &record.notification)
    }

    pub fn pending_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.status == NotificationStatus::Pending)
            .count()
    }

    pub fn acknowledge(&mut self, key: &NotificationKey) -> bool {
        let Some(record) = self.get_mut(key) else {
            return false;
        };
        if record.status == NotificationStatus::Acknowledged {
            return false;
        }
        record.status = NotificationStatus::Acknowledged;
        self.save();
        true
    }

    pub fn acknowledge_all(&mut self) -> usize {
        let mut changed = 0;
        for record in &mut self.records {
            if record.status == NotificationStatus::Pending {
                record.status = NotificationStatus::Acknowledged;
                changed += 1;
            }
        }
        if changed > 0 {
            self.save();
        }
        changed
    }

    pub fn remove_source(&mut self, source: &crate::extension::ExtensionId) -> usize {
        let before = self.records.len();
        self.records
            .retain(|record| &record.notification.source_id != source);
        let removed = before - self.records.len();
        if removed > 0 {
            self.save();
        }
        removed
    }
}
