use crate::sdk::storage::{StorageArea, StorageError};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ExtensionCache(StorageArea);

impl ExtensionCache {
    pub fn new(area: StorageArea) -> Self {
        Self(area)
    }
    pub fn area(&self) -> &StorageArea {
        &self.0
    }
    pub fn clear_file(&self, relative: impl AsRef<Path>) -> Result<(), StorageError> {
        self.0.remove(relative)
    }
    pub fn clear(&self) -> Result<(), StorageError> {
        match std::fs::remove_dir_all(self.0.root()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StorageError::Io(self.0.root().to_owned(), error)),
        }
    }
}
