use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadState {
    Active,
    Complete,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub id: String,
    pub browser: &'static str,
    pub path: PathBuf,
    pub received: u64,
    pub bytes_per_second: f64,
    pub state: DownloadState,
}

impl Download {
    pub fn filename(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "下载文件".into())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub active: Vec<Download>,
    pub completed: Vec<Download>,
}

impl Snapshot {
    pub fn preferred_active(&self) -> Option<&Download> {
        self.active.first()
    }
}

#[cfg(test)]
mod tests {
    use super::{Download, DownloadState, Snapshot};
    use std::path::PathBuf;

    fn active(id: &str) -> Download {
        Download {
            id: id.into(),
            browser: "Chrome",
            path: PathBuf::from(format!("/tmp/{id}")),
            received: 100,
            bytes_per_second: 1.0,
            state: DownloadState::Active,
        }
    }

    #[test]
    fn capsule_download_uses_the_first_active_download() {
        let snapshot = Snapshot {
            active: vec![active("partial"), active("browser-row")],
            completed: Vec::new(),
        };
        assert_eq!(snapshot.preferred_active().unwrap().id, "partial");
    }

    #[test]
    fn capsule_download_returns_none_when_nothing_is_active() {
        let snapshot = Snapshot {
            active: vec![active("partial")],
            completed: Vec::new(),
        };
        assert!(Snapshot::default().preferred_active().is_none());
    }
}
