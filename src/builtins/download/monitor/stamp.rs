use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Stamp(Vec<Option<(u64, Option<SystemTime>)>>);

pub fn capture(path: &Path) -> Stamp {
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    let store = path
        .parent()
        .unwrap_or(Path::new("."))
        .join("downloads.json");
    Stamp(
        [path.to_path_buf(), PathBuf::from(wal), store]
            .iter()
            .map(|file| {
                fs::metadata(file)
                    .ok()
                    .map(|metadata| (metadata.len(), metadata.modified().ok()))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changes_when_database_or_its_wal_changes() {
        let folder = tempfile::tempdir().unwrap();
        let db = folder.path().join("History");
        fs::write(&db, b"old").unwrap();
        let before = capture(&db);
        fs::write(folder.path().join("History-wal"), b"transaction").unwrap();
        assert_ne!(capture(&db), before);
    }
}
