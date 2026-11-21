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
    storage_error: Option<String>,
}

impl NotificationStore {
    pub fn open(path: std::path::PathBuf) -> Self {
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(records) => Self {
                    records,
                    path: Some(path),
                    storage_error: None,
                },
                Err(error) => {
                    eprintln!("Unable to read notifications; preserving file: {error}");
                    Self {
                        records: vec![],
                        path: Some(path),
                        storage_error: Some(error.to_string()),
                    }
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self {
                records: vec![],
                path: Some(path),
                storage_error: None,
            },
            Err(error) => {
                eprintln!("Unable to read notifications; preserving file: {error}");
                Self {
                    records: vec![],
                    path: Some(path),
                    storage_error: Some(error.to_string()),
                }
            }
        }
    }
    fn save(&self) {
        if let Err(error) = self.save_result() {
            eprintln!("Unable to persist notifications: {error}");
        }
    }
    fn save_result(&self) -> Result<(), String> {
        if let Some(error) = &self.storage_error {
            return Err(format!(
                "notification history is unreadable and was preserved: {error}"
            ));
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        let parent = path
            .parent()
            .ok_or_else(|| "missing notification directory".to_owned())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut file =
            tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
        serde_json::to_writer(&mut file, &self.records).map_err(|error| error.to_string())?;
        file.as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        file.persist(path)
            .map_err(|error| error.error.to_string())?;
        Ok(())
    }
    pub fn try_insert(&mut self, notification: Notification) -> Result<bool, String> {
        let key = notification.key();
        if self.get(&key).is_some() {
            return Ok(false);
        }
        self.records.push(NotificationRecord {
            notification,
            status: NotificationStatus::Pending,
        });
        if let Err(error) = self.save_result() {
            self.records.pop();
            return Err(error);
        }
        Ok(true)
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

    pub fn datasource_alert_history(&self) -> std::collections::BTreeMap<String, u64> {
        let mut history = std::collections::BTreeMap::new();
        for record in &self.records {
            let Some(id) =
                crate::core::datasource::notification_source_local(&record.notification.source_id)
            else {
                continue;
            };
            let Ok(timestamp) = record
                .notification
                .created_at
                .duration_since(std::time::UNIX_EPOCH)
            else {
                continue;
            };
            let timestamp = timestamp.as_millis().min(u128::from(u64::MAX)) as u64;
            history.insert(
                format!("datasource.{id}:{}", record.notification.id),
                timestamp,
            );
        }
        history
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
}
