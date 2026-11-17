use std::{fs, path::Path};

pub fn read_with_fallback<T>(
    path: &Path,
    read: impl Fn(&Path) -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    match read(path) {
        Ok(value) => Ok(value),
        Err(original) if is_locked(&original) => {
            let Ok(folder) = tempfile::Builder::new()
                .prefix("peninsula-downloads-")
                .tempdir()
            else {
                return Err(original);
            };
            let Some(name) = path.file_name() else {
                return Err(original);
            };
            let copy = folder.path().join(name);
            if copy_snapshot_file(path, &copy).is_err() {
                return Err(original);
            }
            for suffix in ["-wal", "-shm"] {
                let mut source = path.as_os_str().to_os_string();
                source.push(suffix);
                let mut dest = copy.as_os_str().to_os_string();
                dest.push(suffix);
                if Path::new(&source).is_file()
                    && copy_snapshot_file(Path::new(&source), Path::new(&dest)).is_err()
                {
                    return Err(original);
                }
            }
            read(&copy)
        }
        Err(error) => Err(error),
    }
}

fn copy_snapshot_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        if let (Ok(source), Ok(destination)) = (
            CString::new(source.as_os_str().as_bytes()),
            CString::new(destination.as_os_str().as_bytes()),
        ) {
            if unsafe { libc::clonefile(source.as_ptr(), destination.as_ptr(), 0) } == 0 {
                return Ok(());
            }
        }
    }
    fs::copy(source, destination).map(|_| ())
}

fn is_locked(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn can_read_snapshot_while_browser_has_exclusive_database_lock() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("History");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE downloads(id INTEGER); INSERT INTO downloads VALUES(42); BEGIN EXCLUSIVE;").unwrap();
        let count = read_with_fallback(&path, |db_path| {
            let db =
                Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            db.query_row("SELECT COUNT(*) FROM downloads", [], |row| {
                row.get::<_, i64>(0)
            })
        })
        .unwrap();
        assert_eq!(count, 1);
    }
}
