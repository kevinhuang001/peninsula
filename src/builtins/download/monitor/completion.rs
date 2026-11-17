use std::{
    collections::HashSet,
    fs::{self, Metadata},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Default)]
pub(super) struct CompletedFiles(HashSet<CompletedFile>);

#[derive(Eq, Hash, PartialEq)]
enum CompletedFile {
    Identified {
        identity: (u64, u64),
        created: Option<SystemTime>,
    },
    Path {
        path: PathBuf,
        created: Option<SystemTime>,
        modified: Option<SystemTime>,
        len: u64,
    },
}

impl CompletedFiles {
    pub fn first_completion(&mut self, path: &Path) -> bool {
        let Ok(metadata) = fs::metadata(path) else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }

        // Browser history and the temporary-file watcher can report the same
        // finished file under different paths or many polls apart.
        let key = if let Some(identity) = file_identity(&metadata) {
            CompletedFile::Identified {
                identity,
                created: metadata.created().or_else(|_| metadata.modified()).ok(),
            }
        } else {
            CompletedFile::Path {
                path: path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
                created: metadata.created().ok(),
                modified: metadata.modified().ok(),
                len: metadata.len(),
            }
        };
        self.0.insert(key)
    }
}

#[cfg(unix)]
pub(super) fn file_identity(metadata: &Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
pub(super) fn file_identity(_metadata: &Metadata) -> Option<(u64, u64)> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::builtins::download::monitor::{
        model::{Download, DownloadState},
        partial::{scan, CompletionTracker},
        tracker::Tracker,
    };

    #[test]
    fn a_partial_rename_and_a_later_browser_record_only_announce_one_download() {
        let folder = tempfile::tempdir().unwrap();
        let temporary = folder.path().join("Unconfirmed 12.crdownload");
        let destination = folder.path().join("report.zip");
        fs::write(&temporary, b"report").unwrap();
        let mut partial = CompletionTracker::default();
        let dirs = HashSet::from([folder.path().to_path_buf()]);
        assert!(partial.update(&scan(&dirs)).is_empty());

        let mut browser = Tracker::default();
        assert!(browser.update("profile", &[]).is_empty());
        fs::rename(&temporary, &destination).unwrap();
        let records = partial.update(&scan(&dirs));
        let mut completed = CompletedFiles::default();
        assert_eq!(records.len(), 1);
        assert!(completed.first_completion(&records[0].path));

        // The history record uses a different name for the containing directory.
        let alias = folder.path().join("alias");
        std::os::unix::fs::symlink(folder.path(), &alias).unwrap();
        let history = Download {
            id: "history:123".into(),
            browser: "Chrome",
            path: alias.join("report.zip"),
            state: DownloadState::Complete,
            ..records[0].clone()
        };
        let history = browser.update("profile", &[history]);
        assert_eq!(history.len(), 1);
        assert!(!completed.first_completion(&history[0].path));
    }

    #[test]
    fn replacing_the_file_at_the_same_destination_can_announce_a_new_download() {
        let folder = tempfile::tempdir().unwrap();
        let destination = folder.path().join("report.zip");
        let replacement = folder.path().join("another.zip");
        fs::write(&destination, b"123").unwrap();
        let mut completed = CompletedFiles::default();
        assert!(completed.first_completion(&destination));
        assert!(!completed.first_completion(&destination));

        fs::write(&replacement, b"456").unwrap();
        fs::rename(&replacement, &destination).unwrap();
        assert!(completed.first_completion(&destination));
        assert!(!completed.first_completion(&destination));
    }
}
