use super::{
    completion::file_identity,
    model::{Download, DownloadState},
};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn default_directory() -> Option<PathBuf> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    #[cfg(target_os = "linux")]
    {
        let config = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        if let Ok(content) = fs::read_to_string(config.join("user-dirs.dirs")) {
            for line in content.lines() {
                if let Some(value) = line.trim().strip_prefix("XDG_DOWNLOAD_DIR=") {
                    let value = value
                        .trim()
                        .trim_matches('"')
                        .replace("$HOME", &home.to_string_lossy());
                    let path = PathBuf::from(value);
                    if path.is_absolute() {
                        return Some(path);
                    }
                }
            }
        }
    }
    Some(home.join("Downloads"))
}

pub fn scan(directories: &HashSet<PathBuf>) -> Vec<Download> {
    let mut downloads = Vec::new();
    for directory in directories.iter().take(32) {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "crdownload") {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            if metadata
                .modified()
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age > Duration::from_secs(60))
            {
                continue;
            }
            let target = remove_extension(&path);
            downloads.push(Download {
                id: format!("partial:{}", path.display()),
                browser: "Chromium",
                path: target,
                received: metadata.len(),
                total: None,
                bytes_per_second: 0.0,
                state: DownloadState::Active,
            });
        }
    }
    downloads
}

fn remove_extension(path: &Path) -> PathBuf {
    let mut result = path.to_path_buf();
    result.set_extension("");
    result
}

#[derive(Default)]
pub struct CompletionTracker {
    previous: std::collections::HashMap<(u64, u64), Download>,
}

impl CompletionTracker {
    pub fn update(&mut self, active: &[Download]) -> Vec<Download> {
        let mut current = std::collections::HashMap::new();
        for item in active {
            let partial = item.path.with_extension(match item.path.extension() {
                Some(ext) => format!("{}.crdownload", ext.to_string_lossy()),
                None => "crdownload".into(),
            });
            if let Ok(metadata) = fs::metadata(&partial) {
                if let Some(key) = file_identity(&metadata) {
                    current.insert(key, item.clone());
                }
            }
        }
        let mut completed = Vec::new();
        for (key, item) in &self.previous {
            if current.contains_key(key) {
                continue;
            }
            if fs::metadata(&item.path)
                .is_ok_and(|metadata| metadata.is_file() && file_identity(&metadata) == Some(*key))
            {
                let mut done = item.clone();
                done.state = DownloadState::Complete;
                completed.push(done);
                continue;
            }
            let Some(parent) = item.path.parent() else {
                continue;
            };
            let Ok(entries) = fs::read_dir(parent) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "crdownload") {
                    continue;
                }
                if entry
                    .metadata()
                    .ok()
                    .and_then(|m| file_identity(&m))
                    .as_ref()
                    == Some(key)
                    && path.is_file()
                {
                    let mut done = item.clone();
                    done.path = path;
                    done.state = DownloadState::Complete;
                    completed.push(done);
                    break;
                }
            }
        }
        self.previous = current;
        completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn completed_rename_is_detected_even_when_browser_does_not_record_history() {
        let folder = tempfile::tempdir().unwrap();
        let temporary = folder.path().join("Unconfirmed 123.crdownload");
        let final_path = folder.path().join("real-name.zip");
        fs::write(&temporary, b"download").unwrap();
        let directories = HashSet::from([folder.path().to_path_buf()]);
        let mut tracker = CompletionTracker::default();
        assert!(tracker.update(&scan(&directories)).is_empty());
        fs::rename(temporary, &final_path).unwrap();
        let complete = tracker.update(&scan(&directories));
        assert_eq!(complete.len(), 1);
        assert_eq!(complete[0].path, final_path);
        assert!(tracker.update(&[]).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn cancelling_a_partial_does_not_count_an_older_file_at_the_same_path() {
        let folder = tempfile::tempdir().unwrap();
        let temporary = folder.path().join("report.zip.crdownload");
        fs::write(folder.path().join("report.zip"), b"old").unwrap();
        fs::write(&temporary, b"partial").unwrap();
        let dirs = HashSet::from([folder.path().to_path_buf()]);
        let mut tracker = CompletionTracker::default();
        tracker.update(&scan(&dirs));

        fs::remove_file(temporary).unwrap();
        assert!(tracker.update(&scan(&dirs)).is_empty());
    }

    #[test]
    fn detects_live_chromium_partial_files_without_assuming_total_size() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("report.zip.crdownload"), vec![0; 42]).unwrap();
        fs::write(folder.path().join("unrelated.txt"), b"hello").unwrap();
        let downloads = scan(&HashSet::from([folder.path().to_path_buf()]));
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].path, folder.path().join("report.zip"));
        assert_eq!(downloads[0].received, 42);
        assert_eq!(downloads[0].progress(), None);
    }
}
