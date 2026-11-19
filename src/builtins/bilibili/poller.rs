use crate::core::external::opencli::{parse_json, OpenCli};
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

const CATEGORIES: &[&str] = &[
    "我的消息",
    "回复我的",
    "@我的",
    "收到的赞",
    "系统通知",
    "群消息",
    "疑似不良消息",
];

pub fn collect(cli: &OpenCli) -> Result<BTreeMap<String, u64>, String> {
    // Explicitly keep the adapter in the background so polling never steals
    // focus or opens a foreground browser window.
    let output = cli.run(
        &["bilibili", "unread", "-f", "json", "--window", "background"],
        Duration::from_secs(45),
    )?;
    let value = parse_json(&output)?;
    parse_counts(&value)
}

pub fn parse_counts(value: &Value) -> Result<BTreeMap<String, u64>, String> {
    let rows = value
        .as_array()
        .or_else(|| value.get("data").and_then(Value::as_array))
        .or_else(|| {
            value
                .get("data")
                .and_then(|data| data.get("data"))
                .and_then(Value::as_array)
        })
        .ok_or_else(|| "OpenCLI unread result did not contain a category list".to_owned())?;

    let mut counts: BTreeMap<_, _> = CATEGORIES
        .iter()
        .map(|category| ((*category).to_owned(), 0))
        .collect();
    let mut found_category = false;
    for row in rows {
        let Some(category) = row.get("category").and_then(Value::as_str) else {
            continue;
        };
        found_category = true;
        let count = row
            .get("count")
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
            })
            .unwrap_or(0);
        counts.insert(category.to_owned(), count);
    }
    if !found_category {
        return Err("OpenCLI unread result contained no categories".into());
    }
    Ok(counts)
}

pub fn is_installed(cli: &OpenCli) -> Result<bool, String> {
    Ok(plugin_list(cli)?.contains("bilibili-unread"))
}

fn plugin_list(cli: &OpenCli) -> Result<String, String> {
    let output = cli.run(&["plugin", "list"], Duration::from_secs(20))?;
    if output.status != 0 {
        return Err(format!(
            "opencli plugin list failed: {}",
            output.stderr.trim()
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::parse_counts;
    use serde_json::json;

    #[test]
    fn parses_plugin_rows_and_defaults_missing_optional_categories_to_zero() {
        let counts = parse_counts(&json!({
            "ok": true,
            "data": [
                {"category": "回复我的", "count": 3, "source": "msgfeed.reply"},
                {"category": "我的消息", "count": "2", "source": "single_unread.follow_unread"}
            ]
        }))
        .unwrap();
        assert_eq!(counts["回复我的"], 3);
        assert_eq!(counts["我的消息"], 2);
        assert_eq!(counts["群消息"], 0);
        assert_eq!(counts["收到的赞"], 0);
    }

    #[test]
    fn parses_plugin_rows_inside_the_opencli_data_envelope() {
        let counts =
            parse_counts(&json!({"data": {"data": [{"category": "@我的", "count": 1}]}})).unwrap();
        assert_eq!(counts["@我的"], 1);
    }
}
