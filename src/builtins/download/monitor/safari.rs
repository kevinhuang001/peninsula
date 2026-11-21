use super::model::{Download, DownloadState};
use std::path::Path;
#[cfg(target_os = "macos")]
use std::{fs, path::PathBuf};

#[cfg(target_os = "macos")]
pub fn read(path: &Path) -> Result<Vec<Download>, Box<dyn std::error::Error>> {
    let root = plist::Value::from_file(path)?;
    let entries = root
        .as_dictionary()
        .and_then(|dict| dict.get("DownloadHistory"))
        .and_then(plist::Value::as_array)
        .or_else(|| root.as_array());
    Ok(entries
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let dict = entry.as_dictionary()?;
            let target = PathBuf::from(dict.get("DownloadEntryPath")?.as_string()?);
            let id = dict
                .get("DownloadEntryIdentifier")
                .and_then(plist::Value::as_string)
                .map(str::to_owned)
                .unwrap_or_else(|| target.to_string_lossy().into_owned());
            let saved = dict
                .get("DownloadEntryProgressBytesSoFar")
                .and_then(plist::Value::as_unsigned_integer)
                .unwrap_or(0);
            let received = partial_size(&target).unwrap_or(saved);
            let state = if dict.get("DownloadEntryDateFinishedKey").is_some() {
                DownloadState::Complete
            } else if partial_size(&target).is_some() {
                DownloadState::Active
            } else {
                DownloadState::Failed
            };
            Some(Download {
                id: format!("{}:{id}", path.display()),
                browser: "Safari",
                path: target,
                received,
                bytes_per_second: 0.0,
                state,
            })
        })
        .collect())
}

#[cfg(not(target_os = "macos"))]
pub fn read(_path: &Path) -> Result<Vec<Download>, Box<dyn std::error::Error>> {
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
fn partial_size(target: &Path) -> Option<u64> {
    let mut name = target.as_os_str().to_os_string();
    name.push(".download");
    let path = PathBuf::from(name);
    if path.is_file() {
        return fs::metadata(path).ok().map(|m| m.len());
    }
    fs::read_dir(path)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .max()
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn parses_safari_downloads_and_partial_size() {
        let folder = tempfile::tempdir().unwrap();
        let target = folder.path().join("report.zip");
        fs::write(folder.path().join("report.zip.download"), vec![0; 40]).unwrap();
        let mut item = plist::Dictionary::new();
        item.insert(
            "DownloadEntryPath".into(),
            plist::Value::String(target.to_string_lossy().into()),
        );
        item.insert(
            "DownloadEntryIdentifier".into(),
            plist::Value::String("abc".into()),
        );
        let mut root = plist::Dictionary::new();
        root.insert(
            "DownloadHistory".into(),
            plist::Value::Array(vec![plist::Value::Dictionary(item)]),
        );
        let path = folder.path().join("Downloads.plist");
        plist::Value::Dictionary(root).to_file_xml(&path).unwrap();
        let items = read(&path).unwrap();
        assert_eq!(items[0].state, DownloadState::Active);
        assert_eq!(items[0].received, 40);
    }
}
