use super::model::{Download, DownloadState};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Tracker {
    sources: HashMap<String, HashMap<String, DownloadState>>,
    notified: HashSet<String>,
}

impl Tracker {
    pub fn update(&mut self, source: &str, items: &[Download]) -> Vec<Download> {
        let Some(previous) = self.sources.get(source) else {
            self.sources.insert(
                source.to_owned(),
                items
                    .iter()
                    .map(|item| (item.id.clone(), item.state))
                    .collect(),
            );
            return Vec::new();
        };
        let completed = items
            .iter()
            .filter(|item| {
                item.state == DownloadState::Complete
                    && previous.get(&item.id) != Some(&DownloadState::Complete)
                    && !self.notified.contains(&item.id)
                    && item.path.is_file()
            })
            .cloned()
            .collect::<Vec<_>>();
        self.notified
            .extend(completed.iter().map(|item| item.id.clone()));
        self.sources.insert(
            source.to_owned(),
            items
                .iter()
                .map(|item| {
                    let state = if item.state == DownloadState::Complete && !item.path.is_file() {
                        DownloadState::Active
                    } else {
                        item.state
                    };
                    (item.id.clone(), state)
                })
                .collect(),
        );
        completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};
    fn item(id: &str, path: PathBuf, state: DownloadState) -> Download {
        Download {
            id: id.into(),
            browser: "测试",
            path,
            received: 1,
            total: Some(2),
            bytes_per_second: 0.0,
            state,
        }
    }
    #[test]
    fn old_records_are_not_announced_but_new_completed_downloads_are_announced_once() {
        let path =
            std::env::temp_dir().join(format!("peninsula-download-test-{}", std::process::id()));
        fs::write(&path, b"ok").unwrap();
        let old = item("old", path.clone(), DownloadState::Complete);
        let active = item("new", path.clone(), DownloadState::Active);
        let complete = item("new", path.clone(), DownloadState::Complete);
        let mut tracker = Tracker::default();
        assert!(tracker.update("profile", &[old.clone(), active]).is_empty());
        assert_eq!(
            tracker
                .update("profile", &[old.clone(), complete.clone()])
                .len(),
            1
        );
        assert!(tracker.update("profile", &[old, complete]).is_empty());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn failed_and_missing_files_do_not_emit_success() {
        let missing = item(
            "missing",
            PathBuf::from("/not-a-real-download"),
            DownloadState::Complete,
        );
        let mut tracker = Tracker::default();
        tracker.update("profile", &[]);
        assert!(tracker.update("profile", &[missing]).is_empty());
    }
}
