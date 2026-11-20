use crate::{extension::ExtensionContext, sdk::storage::ExtensionStorage};
#[cfg(not(target_os = "macos"))]
use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fmt, fs,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretBackend {
    Keyring,
    PlaintextFallback,
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct SecretStore(Arc<Mutex<SecretStoreState>>);

#[derive(Debug)]
struct SecretStoreState {
    service: String,
    path: PathBuf,
    last_backend: HashMap<String, SecretBackend>,
}

#[derive(Default, Deserialize, Serialize)]
struct SecretFile {
    schema_version: u32,
    entries: BTreeMap<String, String>,
}

impl SecretStore {
    pub fn for_context(context: &ExtensionContext) -> Self {
        let id = context
            .extension_id()
            .map(ToString::to_string)
            .unwrap_or_else(|| "core.host".into());
        let storage = ExtensionStorage::for_context(context);
        Self::new(
            format!("Peninsula::{id}"),
            storage
                .config()
                .path("secrets.json")
                .expect("static secret filename is valid"),
        )
    }

    pub fn new(service: impl Into<String>, fallback_path: PathBuf) -> Self {
        Self(Arc::new(Mutex::new(SecretStoreState {
            service: service.into(),
            path: fallback_path,
            last_backend: HashMap::new(),
        })))
    }

    pub fn set(&self, key: &str, secret: &str) -> Result<SecretBackend, SecretsError> {
        validate_key(key)?;
        let mut state = self.0.lock().map_err(|_| SecretsError::Poisoned)?;
        match platform_set(&state.service, key, secret) {
            Ok(()) => {
                remove_fallback(&state.path, key)?;
                state
                    .last_backend
                    .insert(key.into(), SecretBackend::Keyring);
                Ok(SecretBackend::Keyring)
            }
            Err(PlatformError::Unavailable(_)) => {
                let mut file = read_fallback(&state.path)?;
                file.schema_version = 1;
                file.entries.insert(key.into(), secret.into());
                write_fallback(&state.path, &file)?;
                state
                    .last_backend
                    .insert(key.into(), SecretBackend::PlaintextFallback);
                Ok(SecretBackend::PlaintextFallback)
            }
            Err(PlatformError::Other(message)) => Err(SecretsError::Keyring(message)),
        }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, SecretsError> {
        validate_key(key)?;
        let mut state = self.0.lock().map_err(|_| SecretsError::Poisoned)?;
        match platform_get(&state.service, key) {
            Ok(Some(secret)) => {
                state
                    .last_backend
                    .insert(key.into(), SecretBackend::Keyring);
                // A successful keychain read supersedes an older plaintext copy.
                remove_fallback(&state.path, key)?;
                Ok(Some(secret))
            }
            Ok(None) => {
                if let Some(secret) = self.migrate_fallback(&mut state, key)? {
                    return Ok(Some(secret));
                }
                state.last_backend.remove(key);
                Ok(None)
            }
            Err(PlatformError::Unavailable(_)) => {
                if let Some(secret) = self.read_fallback(&state, key)? {
                    state
                        .last_backend
                        .insert(key.into(), SecretBackend::PlaintextFallback);
                    Ok(Some(secret))
                } else {
                    state
                        .last_backend
                        .insert(key.into(), SecretBackend::Unavailable);
                    Ok(None)
                }
            }
            Err(PlatformError::Other(message)) => Err(SecretsError::Keyring(message)),
        }
    }

    fn migrate_fallback(
        &self,
        state: &mut SecretStoreState,
        key: &str,
    ) -> Result<Option<String>, SecretsError> {
        let Some(secret) = self.read_fallback(state, key)? else {
            return Ok(None);
        };
        match platform_set(&state.service, key, &secret) {
            Ok(()) => {
                remove_fallback(&state.path, key)?;
                state
                    .last_backend
                    .insert(key.into(), SecretBackend::Keyring);
            }
            Err(PlatformError::Unavailable(_)) => {
                state
                    .last_backend
                    .insert(key.into(), SecretBackend::PlaintextFallback);
            }
            Err(PlatformError::Other(message)) => return Err(SecretsError::Keyring(message)),
        }
        Ok(Some(secret))
    }

    fn read_fallback(
        &self,
        state: &SecretStoreState,
        key: &str,
    ) -> Result<Option<String>, SecretsError> {
        Ok(read_fallback(&state.path)?.entries.remove(key))
    }

    pub fn delete(&self, key: &str) -> Result<(), SecretsError> {
        validate_key(key)?;
        let mut state = self.0.lock().map_err(|_| SecretsError::Poisoned)?;
        let had_fallback = read_fallback(&state.path)?.entries.contains_key(key);
        match platform_delete(&state.service, key) {
            Ok(()) => {
                if had_fallback {
                    remove_fallback(&state.path, key)?;
                }
                state.last_backend.remove(key);
                Ok(())
            }
            Err(PlatformError::Unavailable(_)) if had_fallback => {
                remove_fallback(&state.path, key)?;
                state.last_backend.remove(key);
                Ok(())
            }
            Err(PlatformError::Unavailable(message)) | Err(PlatformError::Other(message)) => {
                Err(SecretsError::Keyring(message))
            }
        }
    }

    pub fn backend(&self, key: &str) -> Result<Option<SecretBackend>, SecretsError> {
        validate_key(key)?;
        let state = self.0.lock().map_err(|_| SecretsError::Poisoned)?;
        if let Some(backend) = state.last_backend.get(key).copied() {
            return Ok(Some(backend));
        }
        if read_fallback(&state.path)?.entries.contains_key(key) {
            return Ok(Some(SecretBackend::PlaintextFallback));
        }
        match platform_get(&state.service, key) {
            Ok(Some(_)) => Ok(Some(SecretBackend::Keyring)),
            Ok(None) => Ok(None),
            Err(PlatformError::Unavailable(_)) => Ok(Some(SecretBackend::Unavailable)),
            Err(PlatformError::Other(message)) => Err(SecretsError::Keyring(message)),
        }
    }
}

enum PlatformError {
    #[allow(dead_code)]
    Unavailable(String),
    Other(String),
}

#[cfg(target_os = "macos")]
const MAC_ITEM_NOT_FOUND: i32 = -25300;

#[cfg(target_os = "macos")]
fn platform_set(service: &str, key: &str, secret: &str) -> Result<(), PlatformError> {
    security_framework::passwords::set_generic_password(service, key, secret.as_bytes())
        .map_err(|error| PlatformError::Other(error.to_string()))
}

#[cfg(target_os = "macos")]
fn platform_get(service: &str, key: &str) -> Result<Option<String>, PlatformError> {
    match security_framework::passwords::get_generic_password(service, key) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| PlatformError::Other("Secret 不是 UTF-8".into())),
        Err(error) if error.code() == MAC_ITEM_NOT_FOUND => Ok(None),
        Err(error) => Err(PlatformError::Other(error.to_string())),
    }
}

#[cfg(target_os = "macos")]
fn platform_delete(service: &str, key: &str) -> Result<(), PlatformError> {
    match security_framework::passwords::delete_generic_password(service, key) {
        Ok(()) => Ok(()),
        Err(error) if error.code() == MAC_ITEM_NOT_FOUND => Ok(()),
        Err(error) => Err(PlatformError::Other(error.to_string())),
    }
}

#[cfg(not(target_os = "macos"))]
fn platform_set(service: &str, key: &str, secret: &str) -> Result<(), PlatformError> {
    let entry =
        Entry::new(service, key).map_err(|error| PlatformError::Other(error.to_string()))?;
    match entry.set_password(secret) {
        Ok(()) => Ok(()),
        Err(error) if backend_unavailable(&error) => {
            Err(PlatformError::Unavailable(error.to_string()))
        }
        Err(error) => Err(PlatformError::Other(error.to_string())),
    }
}

#[cfg(not(target_os = "macos"))]
fn platform_get(service: &str, key: &str) -> Result<Option<String>, PlatformError> {
    let entry =
        Entry::new(service, key).map_err(|error| PlatformError::Other(error.to_string()))?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) if backend_unavailable(&error) => {
            Err(PlatformError::Unavailable(error.to_string()))
        }
        Err(error) => Err(PlatformError::Other(error.to_string())),
    }
}

#[cfg(not(target_os = "macos"))]
fn platform_delete(service: &str, key: &str) -> Result<(), PlatformError> {
    let entry =
        Entry::new(service, key).map_err(|error| PlatformError::Other(error.to_string()))?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) if backend_unavailable(&error) => {
            Err(PlatformError::Unavailable(error.to_string()))
        }
        Err(error) => Err(PlatformError::Other(error.to_string())),
    }
}

#[cfg(not(target_os = "macos"))]
fn backend_unavailable(error: &KeyringError) -> bool {
    match error {
        KeyringError::NoStorageAccess(source) | KeyringError::PlatformFailure(source) => {
            let detail = format!("{} {source:?}", source).to_ascii_lowercase();
            [
                "dbus",
                "d-bus",
                "secret service",
                "serviceunknown",
                "service unknown",
                "connection refused",
                "failed to connect",
                "cannot autolaunch",
                "no such file or directory",
            ]
            .iter()
            .any(|needle| detail.contains(needle))
                && !["locked", "denied", "permission", "not authorized"]
                    .iter()
                    .any(|needle| detail.contains(needle))
        }
        _ => false,
    }
}

fn remove_fallback(path: &PathBuf, key: &str) -> Result<(), SecretsError> {
    let mut file = read_fallback(path)?;
    if file.entries.remove(key).is_some() {
        if file.entries.is_empty() {
            match fs::remove_file(path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(SecretsError::Io(path.clone(), error)),
            }
        }
        write_fallback(path, &file)?;
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), SecretsError> {
    if key.is_empty() || key.len() > 1024 || key.chars().any(char::is_control) {
        Err(SecretsError::InvalidKey)
    } else {
        Ok(())
    }
}

fn read_fallback(path: &PathBuf) -> Result<SecretFile, SecretsError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| SecretsError::InvalidFallbackFile(path.clone())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SecretFile {
            schema_version: 1,
            entries: BTreeMap::new(),
        }),
        Err(error) => Err(SecretsError::Io(path.clone(), error)),
    }
}

fn write_fallback(path: &PathBuf, file: &SecretFile) -> Result<(), SecretsError> {
    let parent = path
        .parent()
        .ok_or_else(|| SecretsError::InvalidFallbackFile(path.clone()))?;
    fs::create_dir_all(parent).map_err(|error| SecretsError::Io(parent.to_owned(), error))?;
    secure_directory(parent)?;
    let bytes = serde_json::to_vec_pretty(file).map_err(|_| SecretsError::InvalidKey)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| SecretsError::Io(parent.to_owned(), error))?;
    secure_file(temporary.path())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| SecretsError::Io(path.clone(), error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| SecretsError::Io(path.clone(), error))?;
    temporary
        .persist(path)
        .map_err(|error| SecretsError::Io(path.clone(), error.error))?;
    secure_file(path)?;
    Ok(())
}

#[cfg(unix)]
fn secure_directory(path: &std::path::Path) -> Result<(), SecretsError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| SecretsError::Io(path.to_owned(), error))
}

#[cfg(not(unix))]
fn secure_directory(_: &std::path::Path) -> Result<(), SecretsError> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &std::path::Path) -> Result<(), SecretsError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| SecretsError::Io(path.to_owned(), error))
}

#[cfg(not(unix))]
fn secure_file(_: &std::path::Path) -> Result<(), SecretsError> {
    Ok(())
}

#[derive(Debug)]
pub enum SecretsError {
    InvalidKey,
    Poisoned,
    Keyring(String),
    Io(PathBuf, io::Error),
    InvalidFallbackFile(PathBuf),
}

impl fmt::Display for SecretsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => f.write_str("invalid secret key or fallback data"),
            Self::Poisoned => f.write_str("secret store lock poisoned"),
            Self::Keyring(error) => write!(f, "system keyring error: {error}"),
            Self::Io(path, error) => write!(f, "secret storage at {}: {error}", path.display()),
            Self::InvalidFallbackFile(path) => {
                write!(f, "invalid plaintext fallback store at {}", path.display())
            }
        }
    }
}

impl std::error::Error for SecretsError {}

#[cfg(test)]
mod tests {
    use super::{SecretBackend, SecretStore};
    use std::fs;

    #[test]
    fn fallback_secret_file_is_private_and_round_trips() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private").join("secrets.json");
        let store = SecretStore::new("Peninsula SDK test", path.clone());
        // The keyring can be present on a developer workstation, so test the
        // fallback representation directly instead of relying on host state.
        let file = super::SecretFile {
            schema_version: 1,
            entries: [("account".to_owned(), "test-only-secret".to_owned())].into(),
        };
        super::write_fallback(&path, &file).unwrap();
        assert_eq!(
            super::read_fallback(&path).unwrap().entries.get("account"),
            Some(&"test-only-secret".to_owned())
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = store;
        let _ = SecretBackend::PlaintextFallback;
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn locked_or_denied_keyring_errors_do_not_fall_back() {
        assert!(!super::backend_unavailable(
            &keyring::Error::NoStorageAccess(Box::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "locked"
            )))
        ));
        assert!(!super::backend_unavailable(&keyring::Error::NoEntry));
        assert!(!super::backend_unavailable(
            &keyring::Error::PlatformFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "access denied"
            )))
        ));
    }
}
