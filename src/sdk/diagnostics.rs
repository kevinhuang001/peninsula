use crate::extension::ExtensionId;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
}

pub fn emit(extension: Option<&ExtensionId>, level: Level, message: &str) {
    let owner = extension.map_or("core", ExtensionId::as_str);
    eprintln!("[{}] [{level:?}] [{owner}] {message}", timestamp());
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
