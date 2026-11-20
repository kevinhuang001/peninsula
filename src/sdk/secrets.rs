use crate::extension::ExtensionContext;
use argon2::Argon2;
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

const VAULT_FILE: &str = "secrets-vault.json";
const CIPHER_AAD: &[u8] = b"peninsula-secret-vault-v1";
const VERIFY_TEXT: &[u8] = b"Peninsula secret vault is unlocked";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretBackend {
    Plaintext,
    Encrypted,
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct SecretStore {
    path: PathBuf,
    vault: Arc<Mutex<VaultState>>,
    root: PathBuf,
}

#[derive(Debug)]
struct VaultState {
    encrypted: bool,
    key: Option<[u8; 32]>,
}

#[derive(Default, Deserialize, Serialize)]
struct SecretFile {
    schema_version: u32,
    entries: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize)]
struct VaultMeta {
    schema_version: u32,
    salt: String,
    verifier: CipherFile,
}

#[derive(Deserialize, Serialize)]
struct CipherFile {
    nonce: String,
    ciphertext: String,
}

static VAULTS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<VaultState>>>>> = OnceLock::new();

impl SecretStore {
    pub fn for_context(context: &ExtensionContext) -> Self {
        let path = context.extension_id().map_or_else(
            || context.data_dir().join("secrets.json"),
            |id| context.extension_data_dir(id).join("secrets.json"),
        );
        Self::new(path, context.data_dir().to_owned())
    }

    pub fn new(path: PathBuf, root: PathBuf) -> Self {
        let vaults = VAULTS.get_or_init(|| Mutex::new(HashMap::new()));
        let vault = vaults
            .lock()
            .expect("secret vault registry lock poisoned")
            .entry(root.clone())
            .or_insert_with(|| {
                let encrypted = root.join(VAULT_FILE).exists();
                Arc::new(Mutex::new(VaultState {
                    encrypted,
                    key: None,
                }))
            })
            .clone();
        Self { path, vault, root }
    }

    pub fn is_encrypted(&self) -> bool {
        self.vault
            .lock()
            .map(|state| state.encrypted)
            .unwrap_or(false)
    }

    pub fn is_locked(&self) -> bool {
        self.vault
            .lock()
            .map(|state| state.encrypted && state.key.is_none())
            .unwrap_or(true)
    }

    pub fn unlock(&self, password: &str) -> Result<(), SecretsError> {
        let meta = read_meta(&self.root)?;
        let salt = decode(&meta.salt)?;
        let key = derive_key(password, &salt)?;
        if decrypt(&key, &meta.verifier)? != VERIFY_TEXT {
            return Err(SecretsError::InvalidPassword);
        }
        let mut state = self.vault.lock().map_err(|_| SecretsError::Poisoned)?;
        state.encrypted = true;
        state.key = Some(key);
        Ok(())
    }

    /// Encrypts every secret file under the application's data directory.
    pub fn enable_encryption(&self, password: &str) -> Result<(), SecretsError> {
        if password.is_empty() {
            return Err(SecretsError::WeakPassword);
        }
        let mut state = self.vault.lock().map_err(|_| SecretsError::Poisoned)?;
        if state.encrypted {
            return Err(SecretsError::AlreadyEncrypted);
        }
        let salt: [u8; 16] = rand::random();
        let key = derive_key(password, &salt)?;
        let files = secret_files(&self.root)?;
        let originals = files
            .iter()
            .map(|path| fs::read(path).map(|bytes| (path.clone(), bytes)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SecretsError::Io(self.root.clone(), error))?;
        let mut replacements = Vec::with_capacity(originals.len());
        for (path, bytes) in &originals {
            let entries: SecretFile = serde_json::from_slice(bytes)
                .map_err(|_| SecretsError::InvalidSecretFile(path.clone()))?;
            let encrypted = serde_json::to_vec(&encrypt(&key, &serde_json::to_vec(&entries)?)?)?;
            replacements.push((path.clone(), encrypted));
        }
        let meta = VaultMeta {
            schema_version: 1,
            salt: encode(&salt),
            verifier: encrypt(&key, VERIFY_TEXT)?,
        };
        let meta_bytes = serde_json::to_vec(&meta)?;
        for (path, encrypted) in &replacements {
            if let Err(error) = write_atomic(path, &encrypted) {
                restore_files(&originals);
                return Err(error);
            }
        }
        if let Err(error) = write_atomic(&self.root.join(VAULT_FILE), &meta_bytes) {
            restore_files(&originals);
            return Err(error);
        }
        state.encrypted = true;
        state.key = Some(key);
        Ok(())
    }

    /// Decrypts all secret files and removes the vault configuration.
    pub fn disable_encryption(&self, password: &str) -> Result<(), SecretsError> {
        self.unlock(password)?;
        let mut state = self.vault.lock().map_err(|_| SecretsError::Poisoned)?;
        let key = state.key.ok_or(SecretsError::Locked)?;
        let files = secret_files(&self.root)?;
        let originals = files
            .iter()
            .map(|path| fs::read(path).map(|bytes| (path.clone(), bytes)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SecretsError::Io(self.root.clone(), error))?;
        let mut replacements = Vec::with_capacity(originals.len());
        for (path, bytes) in &originals {
            let cipher: CipherFile = serde_json::from_slice(bytes)
                .map_err(|_| SecretsError::InvalidSecretFile(path.clone()))?;
            let plain = decrypt(&key, &cipher)?;
            let entries: SecretFile = serde_json::from_slice(&plain)
                .map_err(|_| SecretsError::InvalidSecretFile(path.clone()))?;
            replacements.push((path.clone(), serde_json::to_vec_pretty(&entries)?));
        }
        for (path, plain) in &replacements {
            if let Err(error) = write_atomic(path, plain) {
                restore_files(&originals);
                return Err(error);
            }
        }
        let meta_path = self.root.join(VAULT_FILE);
        if let Err(error) = fs::remove_file(&meta_path) {
            restore_files(&originals);
            return Err(SecretsError::Io(meta_path, error));
        }
        state.encrypted = false;
        state.key = None;
        Ok(())
    }

    pub fn set(&self, key: &str, secret: &str) -> Result<SecretBackend, SecretsError> {
        validate_key(key)?;
        let state = self.vault.lock().map_err(|_| SecretsError::Poisoned)?;
        let mut file = read_secret_file(&self.path, &state)?;
        file.schema_version = 1;
        file.entries.insert(key.into(), secret.into());
        write_secret_file(&self.path, &file, &state)?;
        Ok(if state.encrypted {
            SecretBackend::Encrypted
        } else {
            SecretBackend::Plaintext
        })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, SecretsError> {
        validate_key(key)?;
        let state = self.vault.lock().map_err(|_| SecretsError::Poisoned)?;
        Ok(read_secret_file(&self.path, &state)?.entries.remove(key))
    }

    pub fn delete(&self, key: &str) -> Result<(), SecretsError> {
        validate_key(key)?;
        let state = self.vault.lock().map_err(|_| SecretsError::Poisoned)?;
        let mut file = read_secret_file(&self.path, &state)?;
        if file.entries.remove(key).is_some() {
            if file.entries.is_empty() {
                match fs::remove_file(&self.path) {
                    Ok(()) => return Ok(()),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                    Err(error) => return Err(SecretsError::Io(self.path.clone(), error)),
                }
            }
            write_secret_file(&self.path, &file, &state)?;
        }
        Ok(())
    }

    pub fn backend(&self, key: &str) -> Result<Option<SecretBackend>, SecretsError> {
        validate_key(key)?;
        let state = self.vault.lock().map_err(|_| SecretsError::Poisoned)?;
        if read_secret_file(&self.path, &state)?
            .entries
            .contains_key(key)
        {
            Ok(Some(if state.encrypted {
                SecretBackend::Encrypted
            } else {
                SecretBackend::Plaintext
            }))
        } else {
            Ok(None)
        }
    }
}

fn read_secret_file(path: &Path, state: &VaultState) -> Result<SecretFile, SecretsError> {
    match fs::read(path) {
        Ok(bytes) => {
            if state.encrypted {
                let key = state.key.as_ref().ok_or(SecretsError::Locked)?;
                let cipher: CipherFile = serde_json::from_slice(&bytes)
                    .map_err(|_| SecretsError::InvalidSecretFile(path.to_owned()))?;
                let plain = decrypt(key, &cipher)?;
                serde_json::from_slice(&plain)
                    .map_err(|_| SecretsError::InvalidSecretFile(path.to_owned()))
            } else {
                serde_json::from_slice(&bytes)
                    .map_err(|_| SecretsError::InvalidSecretFile(path.to_owned()))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SecretFile::default()),
        Err(error) => Err(SecretsError::Io(path.to_owned(), error)),
    }
}

fn write_secret_file(
    path: &Path,
    file: &SecretFile,
    state: &VaultState,
) -> Result<(), SecretsError> {
    let bytes = if state.encrypted {
        let key = state.key.as_ref().ok_or(SecretsError::Locked)?;
        serde_json::to_vec(&encrypt(key, &serde_json::to_vec(file)?)?)?
    } else {
        serde_json::to_vec_pretty(file)?
    };
    write_atomic(path, &bytes)
}

fn read_meta(root: &Path) -> Result<VaultMeta, SecretsError> {
    let path = root.join(VAULT_FILE);
    let bytes = fs::read(&path).map_err(|error| SecretsError::Io(path.clone(), error))?;
    serde_json::from_slice(&bytes).map_err(SecretsError::Json)
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], SecretsError> {
    let mut key = [0; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|error| SecretsError::Crypto(error.to_string()))?;
    Ok(key)
}

fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<CipherFile, SecretsError> {
    let nonce: [u8; 24] = rand::random();
    let cipher = XChaCha20Poly1305::new(key.into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: CIPHER_AAD,
            },
        )
        .map_err(|_| SecretsError::Crypto("加密失败".into()))?;
    Ok(CipherFile {
        nonce: encode(&nonce),
        ciphertext: encode(&ciphertext),
    })
}

fn decrypt(key: &[u8; 32], file: &CipherFile) -> Result<Vec<u8>, SecretsError> {
    let nonce = decode(&file.nonce)?;
    if nonce.len() != 24 {
        return Err(SecretsError::Crypto("无效的加密文件".into()));
    }
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &decode(&file.ciphertext)?,
                aad: CIPHER_AAD,
            },
        )
        .map_err(|_| SecretsError::InvalidPassword)
}

fn encode(bytes: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
}
fn decode(value: &str) -> Result<Vec<u8>, SecretsError> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, value)
        .map_err(|_| SecretsError::Crypto("无效的加密数据".into()))
}

fn secret_files(root: &Path) -> Result<Vec<PathBuf>, SecretsError> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for item in
            fs::read_dir(&directory).map_err(|error| SecretsError::Io(directory.clone(), error))?
        {
            let item = item.map_err(|error| SecretsError::Io(directory.clone(), error))?;
            let kind = item
                .file_type()
                .map_err(|error| SecretsError::Io(item.path(), error))?;
            if kind.is_dir() {
                pending.push(item.path());
            } else if kind.is_file() && item.file_name() == "secrets.json" {
                files.push(item.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn restore_files(files: &[(PathBuf, Vec<u8>)]) {
    for (path, bytes) in files {
        let _ = write_atomic(path, bytes);
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), SecretsError> {
    let parent = path
        .parent()
        .ok_or_else(|| SecretsError::InvalidSecretFile(path.to_owned()))?;
    fs::create_dir_all(parent).map_err(|error| SecretsError::Io(parent.to_owned(), error))?;
    secure_directory(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| SecretsError::Io(parent.to_owned(), error))?;
    secure_file(temporary.path())?;
    temporary
        .write_all(bytes)
        .map_err(|error| SecretsError::Io(path.to_owned(), error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| SecretsError::Io(path.to_owned(), error))?;
    temporary
        .persist(path)
        .map_err(|error| SecretsError::Io(path.to_owned(), error.error))?;
    secure_file(path)
}

fn validate_key(key: &str) -> Result<(), SecretsError> {
    if key.is_empty() || key.len() > 1024 || key.chars().any(char::is_control) {
        Err(SecretsError::InvalidKey)
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn secure_directory(path: &Path) -> Result<(), SecretsError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| SecretsError::Io(path.to_owned(), error))
}
#[cfg(not(unix))]
fn secure_directory(_: &Path) -> Result<(), SecretsError> {
    Ok(())
}
#[cfg(unix)]
fn secure_file(path: &Path) -> Result<(), SecretsError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| SecretsError::Io(path.to_owned(), error))
}
#[cfg(not(unix))]
fn secure_file(_: &Path) -> Result<(), SecretsError> {
    Ok(())
}

#[derive(Debug)]
pub enum SecretsError {
    InvalidKey,
    Locked,
    WeakPassword,
    AlreadyEncrypted,
    InvalidPassword,
    Poisoned,
    InvalidSecretFile(PathBuf),
    Io(PathBuf, io::Error),
    Json(serde_json::Error),
    Crypto(String),
}

impl fmt::Display for SecretsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => f.write_str("无效的 Secret 名称"),
            Self::Locked => f.write_str("Secret 已锁定，请先输入密码"),
            Self::WeakPassword => f.write_str("密码不能为空"),
            Self::AlreadyEncrypted => f.write_str("Secret 已启用加密"),
            Self::InvalidPassword => f.write_str("密码错误或加密文件已损坏"),
            Self::Poisoned => f.write_str("Secret 存储锁定失败"),
            Self::InvalidSecretFile(path) => write!(f, "无效的 Secret 文件：{}", path.display()),
            Self::Io(path, error) => write!(f, "Secret 存储 {}：{error}", path.display()),
            Self::Json(error) => write!(f, "Secret 数据格式错误：{error}"),
            Self::Crypto(error) => write!(f, "Secret 加密失败：{error}"),
        }
    }
}
impl std::error::Error for SecretsError {}

impl From<serde_json::Error> for SecretsError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
