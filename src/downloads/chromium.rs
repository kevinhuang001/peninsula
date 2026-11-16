use super::model::{Download, DownloadState};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub fn read(path: &Path, browser: &'static str) -> rusqlite::Result<Vec<Download>> {
    super::database::read_with_fallback(path, |snapshot| read_database(snapshot, path, browser))
}

fn read_database(
    snapshot: &Path,
    path: &Path,
    browser: &'static str,
) -> rusqlite::Result<Vec<Download>> {
    let db = Connection::open_with_flags(
        snapshot,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_millis(100))?;
    let mut query = db.prepare("SELECT id, current_path, target_path, received_bytes, total_bytes, state FROM downloads ORDER BY start_time DESC LIMIT 100")?;
    let rows = query.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let current = decode_path(row.get_ref(1)?);
        let target = decode_path(row.get_ref(2)?);
        let received: i64 = row.get(3)?;
        let total: i64 = row.get(4)?;
        let state: i64 = row.get(5)?;
        let state = match state {
            0 => DownloadState::Active,
            1 => DownloadState::Complete,
            _ => DownloadState::Failed,
        };
        let received = if state == DownloadState::Active {
            std::fs::metadata(&current)
                .map(|m| m.len())
                .unwrap_or(received.max(0) as u64)
        } else {
            received.max(0) as u64
        };
        Ok(Download {
            id: format!("{}:{id}", path.display()),
            browser,
            path: if target.as_os_str().is_empty() {
                current
            } else {
                target
            },
            received,
            total: (total > 0).then_some(total as u64),
            bytes_per_second: 0.0,
            state,
        })
    })?;
    rows.collect()
}

fn decode_path(value: ValueRef<'_>) -> PathBuf {
    match value {
        ValueRef::Text(bytes) => PathBuf::from(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Blob(bytes) => {
            #[cfg(target_os = "windows")]
            {
                let words: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                PathBuf::from(String::from_utf16_lossy(&words))
            }
            #[cfg(not(target_os = "windows"))]
            {
                PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
            }
        }
        _ => PathBuf::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_chromium_download_states_without_modifying_the_database() {
        let path =
            std::env::temp_dir().join(format!("peninsula-chromium-{}.sqlite", std::process::id()));
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE downloads(id INTEGER,current_path TEXT,target_path TEXT,received_bytes INTEGER,total_bytes INTEGER,state INTEGER,start_time INTEGER); INSERT INTO downloads VALUES(1,'/tmp/a.part','/tmp/a',40,100,0,1); INSERT INTO downloads VALUES(2,'/tmp/b','/tmp/b',100,100,1,2);").unwrap();
        let result = read(&path, "Chrome").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].state, DownloadState::Complete);
        assert_eq!(result[1].progress(), Some(0.4));
        db.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let locked_result = read(&path, "Chrome").unwrap();
        assert_eq!(locked_result, result);
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}
