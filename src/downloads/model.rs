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
    pub total: Option<u64>,
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
    pub fn progress(&self) -> Option<f32> {
        self.total
            .filter(|total| *total > 0)
            .map(|total| (self.received as f32 / total as f32).clamp(0.0, 1.0))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub active: Vec<Download>,
    pub completed: Vec<Download>,
}

impl Snapshot {
    pub fn preferred_active(&self) -> Option<&Download> {
        self.active
            .iter()
            .find(|download| download.total.is_some_and(|total| total > 0))
            .or_else(|| self.active.first())
    }
}

#[cfg(test)]
mod tests {
    use super::{Download, DownloadState, Snapshot};
    use std::path::PathBuf;

    fn active(id: &str, total: Option<u64>) -> Download {
        Download {
            id: id.into(),
            browser: "Chrome",
            path: PathBuf::from(format!("/tmp/{id}")),
            received: 100,
            total,
            bytes_per_second: 1.0,
            state: DownloadState::Active,
        }
    }

    #[test]
    fn compact_download_prefers_browser_metadata_with_a_known_total() {
        let snapshot = Snapshot {
            active: vec![active("partial", None), active("browser-row", Some(400))],
            completed: Vec::new(),
        };
        assert_eq!(snapshot.preferred_active().unwrap().id, "browser-row");
    }

    #[test]
    fn compact_download_falls_back_to_partial_metadata_when_total_is_unknown() {
        let snapshot = Snapshot {
            active: vec![active("partial", None)],
            completed: Vec::new(),
        };
        assert_eq!(snapshot.preferred_active().unwrap().total, None);
        assert!(Snapshot::default().preferred_active().is_none());
    }
}
