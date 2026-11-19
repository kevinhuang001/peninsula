use crate::extension::{ExtensionContext, ExtensionId};
use rusqlite::Connection;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fmt, fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AreaKind {
    Config,
    Data,
    Cache,
    State,
}

#[derive(Clone, Debug)]
pub struct ExtensionStorage {
    config: StorageArea,
    data: StorageArea,
    cache: StorageArea,
    state: StorageArea,
}

impl ExtensionStorage {
    pub fn for_context(context: &ExtensionContext) -> Self {
        let id = context
            .extension_id()
            .cloned()
            .unwrap_or_else(|| ExtensionId::builtin("core.host"));
        Self::new(context, &id)
    }

    pub fn new(context: &ExtensionContext, id: &ExtensionId) -> Self {
        let data = context.extension_data_dir(id);
        let config = context
            .config_dir()
            .join("extensions")
            .join(storage_component(id));
        Self {
            config: StorageArea::new(AreaKind::Config, config),
            data: StorageArea::new(AreaKind::Data, data.clone()),
            cache: StorageArea::new(AreaKind::Cache, data.join("cache")),
            state: StorageArea::new(AreaKind::State, data.join("state")),
        }
    }

    pub fn config(&self) -> &StorageArea {
        &self.config
    }
    pub fn data(&self) -> &StorageArea {
        &self.data
    }
    pub fn cache(&self) -> &StorageArea {
        &self.cache
    }
    pub fn state(&self) -> &StorageArea {
        &self.state
    }

    pub fn area(&self, kind: AreaKind) -> &StorageArea {
        match kind {
            AreaKind::Config => &self.config,
            AreaKind::Data => &self.data,
            AreaKind::Cache => &self.cache,
            AreaKind::State => &self.state,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StorageArea {
    kind: AreaKind,
    root: PathBuf,
}

impl StorageArea {
    pub fn from_root(kind: AreaKind, root: impl Into<PathBuf>) -> Self {
        Self::new(kind, root.into())
    }

    pub fn new(kind: AreaKind, root: PathBuf) -> Self {
        Self { kind, root }
    }

    pub fn kind(&self) -> AreaKind {
        self.kind
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, relative: impl AsRef<Path>) -> Result<PathBuf, StorageError> {
        let relative = relative.as_ref();
        if relative.as_os_str().is_empty()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::Prefix(_) | Component::RootDir | Component::ParentDir
                )
            })
        {
            return Err(StorageError::InvalidPath(relative.to_owned()));
        }
        Ok(self.root.join(relative))
    }

    pub fn read_bytes(&self, relative: impl AsRef<Path>) -> Result<Option<Vec<u8>>, StorageError> {
        let path = self.path(relative)?;
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StorageError::Io(path, error)),
        }
    }

    pub fn write_bytes(
        &self,
        relative: impl AsRef<Path>,
        bytes: &[u8],
    ) -> Result<(), StorageError> {
        let path = self.path(relative)?;
        let parent = path
            .parent()
            .ok_or_else(|| StorageError::InvalidPath(path.clone()))?;
        fs::create_dir_all(parent).map_err(|error| StorageError::Io(parent.to_owned(), error))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| StorageError::Io(parent.to_owned(), error))?;
        temporary
            .write_all(bytes)
            .map_err(|error| StorageError::Io(path.clone(), error))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| StorageError::Io(path.clone(), error))?;
        temporary
            .persist(&path)
            .map_err(|error| StorageError::Io(path, error.error))?;
        Ok(())
    }

    pub fn read_json<T: DeserializeOwned>(
        &self,
        relative: impl AsRef<Path>,
    ) -> Result<Option<T>, StorageError> {
        let path = self.path(relative)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StorageError::Io(path, error)),
        };
        serde_json::from_slice(&bytes).map(Some).map_err(|error| {
            // Leave malformed files in place for diagnosis or manual recovery.
            StorageError::Json(path, error.to_string())
        })
    }

    pub fn write_json<T: Serialize>(
        &self,
        relative: impl AsRef<Path>,
        value: &T,
    ) -> Result<(), StorageError> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|error| StorageError::Json(self.root.clone(), error.to_string()))?;
        self.write_bytes(relative, &bytes)
    }

    pub fn sqlite(&self, relative: impl AsRef<Path>) -> Result<Connection, StorageError> {
        let path = self.path(relative)?;
        let parent = path
            .parent()
            .ok_or_else(|| StorageError::InvalidPath(path.clone()))?;
        fs::create_dir_all(parent).map_err(|error| StorageError::Io(parent.to_owned(), error))?;
        let connection = Connection::open(&path)
            .map_err(|error| StorageError::Database(path.clone(), error.to_string()))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| StorageError::Database(path.clone(), error.to_string()))?;
        connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
            .map_err(|error| StorageError::Database(path, error.to_string()))?;
        Ok(connection)
    }

    pub fn remove(&self, relative: impl AsRef<Path>) -> Result<(), StorageError> {
        let path = self.path(relative)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StorageError::Io(path, error)),
        }
    }
}

fn storage_component(id: &ExtensionId) -> String {
    // '%' cannot occur in an ExtensionId, so this remains collision-free.
    id.as_str().replace('/', "%2F")
}

#[derive(Debug)]
pub enum StorageError {
    InvalidPath(PathBuf),
    Io(PathBuf, io::Error),
    Json(PathBuf, String),
    Database(PathBuf, String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(path) => write!(
                f,
                "storage path must stay inside its area: {}",
                path.display()
            ),
            Self::Io(path, error) => write!(f, "storage I/O at {}: {error}", path.display()),
            Self::Json(path, error) => write!(f, "invalid JSON at {}: {error}", path.display()),
            Self::Database(path, error) => write!(f, "SQLite at {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for StorageError {}

#[cfg(test)]
mod tests {
    use super::{AreaKind, StorageArea};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct Value {
        count: u32,
    }

    #[test]
    fn json_round_trip_is_atomic_and_corrupt_files_are_retained() {
        let root = tempfile::tempdir().unwrap();
        let area = StorageArea::new(AreaKind::Data, root.path().into());
        area.write_json("nested/value.json", &Value { count: 7 })
            .unwrap();
        assert_eq!(
            area.read_json::<Value>("nested/value.json").unwrap(),
            Some(Value { count: 7 })
        );
        std::fs::write(root.path().join("broken.json"), b"{").unwrap();
        assert!(area.read_json::<Value>("broken.json").is_err());
        assert_eq!(
            std::fs::read(root.path().join("broken.json")).unwrap(),
            b"{"
        );
    }

    #[test]
    fn paths_cannot_escape_the_extension_area() {
        let area = StorageArea::new(AreaKind::Data, "unused".into());
        assert!(area.path("../outside").is_err());
        assert!(area.path("/outside").is_err());
        assert!(area.path("nested/value.json").is_ok());
    }
}
