use crate::sdk::storage::StorageArea;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnreadIncrease {
    pub category: String,
    pub increase: u64,
}

#[derive(Default, Deserialize, Serialize)]
pub struct SyncState {
    counts: BTreeMap<String, u64>,
    initialized: bool,
}

impl SyncState {
    pub fn load_storage(storage: &StorageArea) -> Result<Self, crate::sdk::storage::StorageError> {
        Ok(storage.read_json("unread-counts.json")?.unwrap_or_default())
    }

    pub fn save_storage(
        &self,
        storage: &StorageArea,
    ) -> Result<(), crate::sdk::storage::StorageError> {
        storage.write_json("unread-counts.json", self)
    }

    /// Replaces the last known counts and returns only categories that grew.
    /// Decreases and unchanged snapshots are still retained and persisted.
    pub fn observe(&mut self, counts: BTreeMap<String, u64>) -> Vec<UnreadIncrease> {
        let increases = if self.initialized {
            counts
                .iter()
                .filter_map(|(category, count)| {
                    let previous = self.counts.get(category).copied().unwrap_or(0);
                    (*count > previous).then(|| UnreadIncrease {
                        category: category.clone(),
                        increase: *count - previous,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        self.counts = counts;
        self.initialized = true;
        increases
    }
}

#[cfg(test)]
mod tests {
    use super::SyncState;
    use crate::sdk::storage::{AreaKind, StorageArea};
    use std::collections::BTreeMap;

    fn counts(values: &[(&str, u64)]) -> BTreeMap<String, u64> {
        values
            .iter()
            .map(|(category, count)| ((*category).into(), *count))
            .collect()
    }

    #[test]
    fn first_snapshot_is_baseline_and_only_increases_are_reported() {
        let mut state = SyncState::default();
        assert!(state
            .observe(counts(&[("回复我的", 2), ("收到的赞", 4)]))
            .is_empty());
        let increases = state.observe(counts(&[("回复我的", 3), ("收到的赞", 1), ("@我的", 1)]));
        assert_eq!(increases.len(), 2);
        assert_eq!(increases[0].category, "@我的");
        assert_eq!(increases[0].increase, 1);
        assert_eq!(increases[1].increase, 1);
    }

    #[test]
    fn latest_decreased_counts_are_persisted_as_the_next_comparison_baseline() {
        let root = tempfile::tempdir().unwrap();
        let storage = StorageArea::from_root(AreaKind::State, root.path());
        let mut state = SyncState::default();
        state.observe(counts(&[("我的消息", 5)]));
        state.save_storage(&storage).unwrap();

        let mut loaded = SyncState::load_storage(&storage).unwrap();
        assert!(loaded.observe(counts(&[("我的消息", 2)])).is_empty());
        loaded.save_storage(&storage).unwrap();
        let mut loaded = SyncState::load_storage(&storage).unwrap();
        let increases = loaded.observe(counts(&[("我的消息", 3)]));
        assert_eq!(increases.len(), 1);
        assert_eq!(increases[0].increase, 1);
    }
}
