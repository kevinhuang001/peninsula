use crate::extension::{Notification, NotificationKey, NotificationStatus};

#[derive(Clone, Debug)]
pub struct NotificationRecord {
    pub notification: Notification,
    pub status: NotificationStatus,
}

#[derive(Clone, Debug, Default)]
pub struct NotificationStore {
    records: Vec<NotificationRecord>,
}

impl NotificationStore {
    pub fn insert(&mut self, notification: Notification) -> bool {
        let key = notification.key();
        if self.get(&key).is_some() {
            return false;
        }
        self.records.push(NotificationRecord {
            notification,
            status: NotificationStatus::Pending,
        });
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
        changed
    }
}
