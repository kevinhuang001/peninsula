use super::model::{Download, DownloadState};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn read(path: &Path) -> rusqlite::Result<Vec<Download>> {
    super::database::read_with_fallback(path, |snapshot| read_database(snapshot, path))
}

fn read_database(snapshot: &Path, path: &Path) -> rusqlite::Result<Vec<Download>> {
    let db = Connection::open_with_flags(
        snapshot,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_millis(100))?;
    let sql = "SELECT dest.content, dest.lastModified, meta.content FROM moz_annos dest JOIN moz_anno_attributes a ON a.id=dest.anno_attribute_id LEFT JOIN moz_annos meta ON meta.place_id=dest.place_id AND meta.anno_attribute_id=(SELECT id FROM moz_anno_attributes WHERE name='downloads/metaData') WHERE a.name='downloads/destinationFileURI' ORDER BY dest.lastModified DESC LIMIT 100";
    let mut query = db.prepare(sql)?;
    let rows = query.query_map([], |row| {
        let uri: String = row.get(0)?;
        let modified: i64 = row.get(1)?;
        let metadata: Option<String> = row.get(2)?;
        Ok((uri, modified, metadata))
    })?;
    let mut downloads = Vec::new();
    for row in rows {
        let (uri, modified, metadata) = row?;
        let Some(target) = url::Url::parse(&uri)
            .ok()
            .and_then(|url| url.to_file_path().ok())
        else {
            continue;
        };
        let meta: Value = metadata
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null);
        let state = match meta.get("state").and_then(Value::as_i64) {
            Some(1) => DownloadState::Complete,
            Some(0) => DownloadState::Active,
            Some(_) => DownloadState::Failed,
            None if part_size(&target).is_some() => DownloadState::Active,
            None => DownloadState::Failed,
        };
        let total = meta
            .get("fileSize")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0);
        let received = if state == DownloadState::Complete {
            total.unwrap_or(0)
        } else {
            part_size(&target).unwrap_or(0)
        };
        downloads.push(Download {
            id: format!("{}:{}:{modified}", path.display(), target.display()),
            browser: "Firefox",
            path: target,
            received,
            total,
            bytes_per_second: 0.0,
            state,
        });
    }
    if let Some(profile) = path.parent() {
        downloads.extend(read_store(&profile.join("downloads.json")));
    }
    Ok(downloads)
}

fn read_store(path: &Path) -> Vec<Download> {
    let Some(json) = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return Vec::new();
    };
    let Some(items) = json.get("list").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let target = item.get("target")?;
            let target = target
                .as_str()
                .or_else(|| target.get("path").and_then(Value::as_str))?;
            let target = PathBuf::from(target);
            let state = if item.get("succeeded").and_then(Value::as_bool) == Some(true) {
                DownloadState::Complete
            } else if item.get("canceled").and_then(Value::as_bool) == Some(true)
                || item.get("errorObj").is_some()
            {
                DownloadState::Failed
            } else {
                DownloadState::Active
            };
            let part = item
                .pointer("/target/partFilePath")
                .and_then(Value::as_str)
                .and_then(|p| fs::metadata(p).ok())
                .map(|m| m.len())
                .or_else(|| part_size(&target));
            let received = part
                .or_else(|| item.get("currentBytes").and_then(Value::as_u64))
                .unwrap_or(0);
            let total = item
                .get("totalBytes")
                .and_then(Value::as_u64)
                .filter(|n| *n > 0);
            let started = item
                .get("startTime")
                .map(Value::to_string)
                .unwrap_or_default();
            Some(Download {
                id: format!("{}:{}:{started}", path.display(), target.display()),
                browser: "Firefox",
                path: target,
                received,
                total,
                bytes_per_second: 0.0,
                state,
            })
        })
        .collect()
}

fn part_size(target: &Path) -> Option<u64> {
    let mut name = target.as_os_str().to_os_string();
    name.push(".part");
    fs::metadata(PathBuf::from(name)).ok().map(|m| m.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_firefox_in_progress_store_without_guessing_unknown_total() {
        let path =
            std::env::temp_dir().join(format!("peninsula-firefox-{}.json", std::process::id()));
        fs::write(&path, r#"{"list":[{"target":{"path":"/tmp/report.pdf"},"currentBytes":50,"totalBytes":100,"startTime":"2026-11-16T00:00:00Z"}]}"#).unwrap();
        let list = read_store(&path);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].progress(), Some(0.5));
        assert_eq!(list[0].state, DownloadState::Active);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_firefox_download_annotations_as_completed_files() {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("places.sqlite");
        let target = folder.path().join("文件.pdf");
        let uri = url::Url::from_file_path(&target).unwrap().to_string();
        let db = Connection::open(&database).unwrap();
        db.execute_batch("CREATE TABLE moz_anno_attributes(id INTEGER,name TEXT); CREATE TABLE moz_annos(place_id INTEGER,anno_attribute_id INTEGER,content TEXT,lastModified INTEGER); INSERT INTO moz_anno_attributes VALUES(1,'downloads/destinationFileURI'),(2,'downloads/metaData');").unwrap();
        db.execute("INSERT INTO moz_annos VALUES(10,1,?1,42)", [&uri])
            .unwrap();
        db.execute(
            "INSERT INTO moz_annos VALUES(10,2,?1,42)",
            [r#"{"state":1,"fileSize":100}"#],
        )
        .unwrap();
        let items = read(&database).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, target);
        assert_eq!(items[0].state, DownloadState::Complete);
        assert_eq!(items[0].total, Some(100));
    }
}
