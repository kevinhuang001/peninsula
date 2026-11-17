use super::model::{BilibiliEvent, EventKind};
use crate::core::external::opencli::{parse_json, OpenCli};
use serde_json::Value;
use std::{path::Path, time::Duration};

pub fn account(cli: &OpenCli) -> Result<String, String> {
    let output = cli.run(&["bilibili", "me", "-f", "json"], Duration::from_secs(20))?;
    let value = parse_json(&output)?;
    find_string(&value, &["mid", "uid", "id"])
        .ok_or_else(|| "OpenCLI did not return a Bilibili account ID".into())
}

pub fn collect(
    cli: &OpenCli,
    account: &str,
    dm_enabled: bool,
    likes_enabled: bool,
) -> Result<Vec<BilibiliEvent>, String> {
    let mut items = Vec::new();
    if dm_enabled {
        let dm = cli.run(
            &["peninsula-bilibili", "dm", "--limit", "200", "-f", "json"],
            Duration::from_secs(20),
        )?;
        items.extend(parse_items(&parse_json(&dm)?, account, EventKind::Message));
    }
    if likes_enabled {
        let likes = cli.run(
            &[
                "peninsula-bilibili",
                "likes",
                "--limit",
                "200",
                "-f",
                "json",
            ],
            Duration::from_secs(20),
        )?;
        items.extend(parse_items(&parse_json(&likes)?, account, EventKind::Like));
    }
    Ok(items)
}

fn parse_items(value: &Value, account: &str, kind: EventKind) -> Vec<BilibiliEvent> {
    let rows = value.as_array().cloned().unwrap_or_else(|| {
        value
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    });
    rows.into_iter()
        .filter_map(|row| {
            let get = |keys: &[&str]| find_string(&row, keys);
            let raw_id = get(&["id", "msg_key", "msg_seqno", "like_id"])?;
            let id = format!(
                "{}:{}:{}",
                account,
                if kind == EventKind::Message {
                    "dm"
                } else {
                    "like"
                },
                raw_id
            );
            let url = get(&["url"]).unwrap_or_else(|| "https://www.bilibili.com/".into());
            let (title, body) = match kind {
                EventKind::Message => {
                    let sender = get(&["sender_name", "sender", "uname"])
                        .unwrap_or_else(|| "Bilibili 用户".into());
                    (
                        "Bilibili 新私信".into(),
                        format!(
                            "{sender}：{}",
                            get(&["summary", "content", "message"]).unwrap_or_default()
                        ),
                    )
                }
                EventKind::Like => (
                    "Bilibili 收到新赞".into(),
                    format!(
                        "{} 赞了 {}",
                        get(&["actor", "uname"]).unwrap_or_else(|| "Bilibili 用户".into()),
                        get(&["content", "title"]).unwrap_or_else(|| "你的内容".into())
                    ),
                ),
            };
            Some(BilibiliEvent {
                id,
                account: account.into(),
                kind,
                title,
                body,
                url,
            })
        })
        .collect()
}

fn find_string(value: &Value, keys: &[&str]) -> Option<String> {
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(value) = object.get(*key) {
                if let Some(text) = value.as_str() {
                    return Some(text.to_owned());
                }
                if let Some(number) = value.as_i64() {
                    return Some(number.to_string());
                }
            }
        }
        for nested in object.values() {
            if let Some(found) = find_string(nested, keys) {
                return Some(found);
            }
        }
    } else if let Some(array) = value.as_array() {
        for nested in array {
            if let Some(found) = find_string(nested, keys) {
                return Some(found);
            }
        }
    }
    None
}

pub fn is_installed(cli: &OpenCli) -> Result<bool, String> {
    let output = cli.run(&["plugin", "list"], Duration::from_secs(20))?;
    if output.status != 0 {
        return Err(format!(
            "opencli plugin list failed: {}",
            output.stderr.trim()
        ));
    }
    Ok(output.stdout.contains("peninsula-bilibili"))
}

pub fn copy_plugin(target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|error| error.to_string())?;
    for (name, content) in [
        (
            "opencli-plugin.json",
            include_str!("../../../integrations/opencli/peninsula-bilibili/opencli-plugin.json"),
        ),
        (
            "package.json",
            include_str!("../../../integrations/opencli/peninsula-bilibili/package.json"),
        ),
        (
            "dm.ts",
            include_str!("../../../integrations/opencli/peninsula-bilibili/dm.ts"),
        ),
        (
            "likes.ts",
            include_str!("../../../integrations/opencli/peninsula-bilibili/likes.ts"),
        ),
        (
            "dm.js",
            include_str!("../../../integrations/opencli/peninsula-bilibili/dm.js"),
        ),
        (
            "likes.js",
            include_str!("../../../integrations/opencli/peninsula-bilibili/likes.js"),
        ),
        (
            "README.md",
            include_str!("../../../integrations/opencli/peninsula-bilibili/README.md"),
        ),
    ] {
        let content = if name.ends_with(".json") {
            content.replace("0.1.0", env!("CARGO_PKG_VERSION"))
        } else {
            content.to_owned()
        };
        std::fs::write(target.join(name), content)
            .map_err(|error| format!("unable to prepare {name}: {error}"))?;
    }
    Ok(())
}
