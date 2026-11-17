use std::{collections::BTreeMap, fs, io, path::Path};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnreadIncrease {
    pub category: String,
    pub count: u64,
    pub increase: u64,
}

#[derive(Default)]
pub struct SyncState {
    counts: BTreeMap<String, u64>,
    initialized: bool,
}

impl SyncState {
    pub fn load(path: &Path) -> Self {
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        let mut state = Self::default();
        for line in contents.lines() {
            if line == "initialized=true" {
                state.initialized = true;
            } else if let Some((category, count)) = line.split_once('\t') {
                if let Ok(count) = count.parse() {
                    state.counts.insert(category.to_owned(), count);
                }
            }
        }
        state
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
                        count: *count,
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

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut lines = vec![format!("initialized={}", self.initialized)];
        lines.extend(
            self.counts
                .iter()
                .map(|(category, count)| format!("{category}\t{count}")),
        );
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, format!("{}\n", lines.join("\n")))?;
        fs::rename(temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::SyncState;
    use std::{collections::BTreeMap, path::PathBuf};

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
        assert_eq!(increases[0].count, 1);
        assert_eq!(increases[1].increase, 1);
    }

    #[test]
    fn latest_decreased_counts_are_persisted_as_the_next_comparison_baseline() {
        let path: PathBuf = std::env::temp_dir().join(format!(
            "peninsula-bilibili-unread-test-{}.tsv",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut state = SyncState::default();
        state.observe(counts(&[("我的消息", 5)]));
        state.save(&path).unwrap();

        let mut loaded = SyncState::load(&path);
        assert!(loaded.observe(counts(&[("我的消息", 2)])).is_empty());
        loaded.save(&path).unwrap();
        let mut loaded = SyncState::load(&path);
        let increases = loaded.observe(counts(&[("我的消息", 3)]));
        assert_eq!(increases.len(), 1);
        assert_eq!(increases[0].increase, 1);
        std::fs::remove_file(path).unwrap();
    }
}
