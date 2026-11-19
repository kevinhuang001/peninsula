use super::parcels::Parcel;
use crate::sdk::process::{ProcessExecutor, ProcessSpec};
use serde_json::Value;
use std::{collections::HashSet, path::Path, time::Duration};

pub(super) const PLUGIN_INSTALL: &str =
    "opencli plugin install github:kevinhuang001/opencli-kuaidi";
const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(super) enum Platform {
    Taobao = 0,
    Jingdong = 1,
}

impl Platform {
    pub const ALL: [Self; 2] = [Self::Taobao, Self::Jingdong];

    pub fn name(self) -> &'static str {
        match self {
            Self::Taobao => "淘宝",
            Self::Jingdong => "京东",
        }
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Taobao => "taobao",
            Self::Jingdong => "jingdong",
        }
    }

    pub fn collect(self, program: &Path) -> Result<Vec<Parcel>, SyncError> {
        let result = ProcessExecutor
            .run(
                ProcessSpec::new(program)
                    .args([
                        "kuaidi",
                        self.command(),
                        "--max-pages",
                        "0",
                        "--window",
                        "background",
                        "--trace",
                        "off",
                        "-f",
                        "json",
                    ])
                    .timeout(Duration::from_secs(240))
                    .max_output_bytes(OUTPUT_LIMIT),
            )
            .map_err(|_| SyncError::Query)?;
        let value: Option<Value> = serde_json::from_slice(&result.stdout).ok();
        if !result.success()
            || value.as_ref().and_then(|v: &Value| v["ok"].as_bool()) == Some(false)
        {
            let code = value.as_ref().and_then(|v| v["error"]["code"].as_str());
            return Err(match code {
                Some("AUTH_REQUIRED") => SyncError::Login,
                Some(
                    "UNKNOWN_COMMAND" | "UNKNOWN_SITE" | "COMMAND_NOT_FOUND" | "NO_SUCH_COMMAND",
                ) => SyncError::Plugin,
                Some("DAEMON_UNAVAILABLE" | "EXTENSION_UNAVAILABLE" | "BROWSER_CONNECT") => {
                    SyncError::Browser
                }
                _ => SyncError::Query,
            });
        }
        parse_rows(value.ok_or(SyncError::Query)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SyncError {
    OpenCli,
    Plugin,
    Login,
    Browser,
    Query,
    Save,
}

fn parse_rows(mut value: Value) -> Result<Vec<Parcel>, SyncError> {
    for _ in 0..2 {
        if !value.is_object() {
            break;
        }
        if value["ok"].as_bool() == Some(false) {
            return Err(SyncError::Query);
        }
        value = value["data"].take();
    }
    let mut parcels: Vec<Parcel> = serde_json::from_value(value).map_err(|_| SyncError::Query)?;
    if parcels.iter().any(|row| row.order_id.trim().is_empty()) {
        return Err(SyncError::Query);
    }
    let mut seen = HashSet::new();
    parcels.retain(|row| seen.insert((row.order_id.clone(), row.tracking_no.clone())));
    Ok(parcels)
}

pub(super) fn check_plugin(program: &Path) -> Result<(), SyncError> {
    let output = ProcessExecutor
        .run(
            ProcessSpec::new(program)
                .args(["plugin", "list"])
                .timeout(Duration::from_secs(20)),
        )
        .map_err(|_| SyncError::OpenCli)?;
    if !output.success() {
        return Err(SyncError::OpenCli);
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| SyncError::OpenCli)?;
    let plugins = value
        .as_array()
        .or_else(|| value["data"].as_array())
        .ok_or(SyncError::OpenCli)?;
    if plugins.iter().any(|item| item["name"] == "kuaidi") {
        Ok(())
    } else {
        Err(SyncError::Plugin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_current_opencli_envelope_and_preserves_string_order_ids() {
        let row = json!({"order_id": "1234567890123456789", "current_status": "运输中"});
        let parsed = parse_rows(json!({"ok": true, "data": [row.clone(), row]})).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].order_id, "1234567890123456789");
        assert!(parse_rows(json!({"ok": true, "data": []}))
            .unwrap()
            .is_empty());
        assert!(parse_rows(json!({"ok": false, "data": []})).is_err());
        assert!(parse_rows(json!({"data": [{}]})).is_err());
    }
}
