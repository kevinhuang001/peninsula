use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use url::Url;

pub const SCHEMA_ID: &str = "peninsula.datasource/v1";
pub const SCHEMA_ID_V2: &str = "peninsula.datasource/v2";
pub const MIN_REFRESH: std::time::Duration = std::time::Duration::from_secs(30);
pub const MAX_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub schema: String,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_template: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<IconSpec>,
    #[serde(default)]
    pub inputs: BTreeMap<String, InputDefinition>,
    pub request: RequestDefinition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<ValueDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<AlertDefinition>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, MetricDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IconSpec {
    pub mime: String,
    pub base64: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    String,
    Number,
    Integer,
    Boolean,
    Enum,
    Duration,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InputDefinition {
    #[serde(rename = "type")]
    pub kind: InputType,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub min: Option<Value>,
    #[serde(default)]
    pub max: Option<Value>,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RequestDefinition {
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<Value>,
    #[serde(default)]
    pub body_text: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ValueDefinition {
    pub path: String,
    #[serde(rename = "type", default = "number_type")]
    pub kind: String,
    #[serde(default = "one")]
    pub multiply: f64,
    #[serde(default = "one")]
    pub divide: f64,
    #[serde(default)]
    pub offset: f64,
    #[serde(default = "default_decimals")]
    pub decimals: u8,
    #[serde(default)]
    pub separator: bool,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MetricDefinition {
    pub name: String,
    pub value: ValueDefinition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<AlertDefinition>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertMode {
    #[default]
    Value,
    Change,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertOperator {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Ne,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AlertDefinition {
    #[serde(default)]
    pub mode: AlertMode,
    pub operator: AlertOperator,
    pub threshold: Value,
    pub title: String,
    pub message: String,
}

fn default_timeout() -> u64 {
    10_000
}
fn default_decimals() -> u8 {
    2
}
fn one() -> f64 {
    1.0
}
fn number_type() -> String {
    "number".into()
}

impl Definition {
    pub fn parse(source: &str) -> Result<Self, String> {
        let definition: Self = serde_json::from_str(source).map_err(|error| error.to_string())?;
        definition.validate()?;
        Ok(definition)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA_ID && self.schema != SCHEMA_ID_V2 {
            return Err(format!("不支持的数据源 Schema：{}", self.schema));
        }
        if self.schema == SCHEMA_ID && (self.value.is_none() || !self.values.is_empty()) {
            return Err("V1 必须提供一个 value，且不能声明 values".into());
        }
        if self.schema == SCHEMA_ID_V2 &&
            (self.value.is_some() || self.alert.is_some() || self.values.is_empty() || self.values.len() > 32)
        {
            return Err("V2 必须提供 1 至 32 个 values，不能声明根级 value/alert".into());
        }
        if self.id.is_empty()
            || self.id.len() > 96
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
        {
            return Err("数据源 id 只能包含字母、数字、点、下划线和连字符".into());
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            return Err("数据源名称不能为空且最多 80 个字符".into());
        }
        if let Some(template) = &self.name_template {
            if template.chars().count() > 80 || template.trim().is_empty() {
                return Err("数据源动态名称无效".into());
            }
            validate_placeholders(template, self, false)?;
        }
        if self.description.chars().count() > 1000 || self.inputs.len() > 64 {
            return Err("数据源描述或参数数量超过限制".into());
        }
        if let Some(icon) = &self.icon {
            if icon.base64.len() > 1_500_000 {
                return Err("图标 Base64 数据过大".into());
            }
        }
        for (key, input) in &self.inputs {
            if key.is_empty()
                || key.len() > 64
                || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || input.label.trim().is_empty()
                || input.label.chars().count() > 80
                || input.description.chars().count() > 500
                || numeric_limit(input.min.as_ref(), input.kind).is_err()
                || numeric_limit(input.max.as_ref(), input.kind).is_err()
                || numeric_limit(input.min.as_ref(), input.kind)
                    .ok()
                    .flatten()
                    .zip(numeric_limit(input.max.as_ref(), input.kind).ok().flatten())
                    .is_some_and(|(min, max)| min > max)
            {
                return Err(format!("参数 {key} 的声明无效"));
            }
            if input.kind == InputType::Enum && input.options.is_empty() {
                return Err(format!("枚举参数 {key} 必须提供 options"));
            }
            if (input.min.is_some() || input.max.is_some())
                && !matches!(
                    input.kind,
                    InputType::Number | InputType::Integer | InputType::Duration
                )
            {
                return Err(format!(
                    "参数 {key} 的范围只支持 number、integer 或 duration"
                ));
            }
            if input.kind != InputType::Enum && !input.options.is_empty() {
                return Err(format!("非枚举参数 {key} 不能声明 options"));
            }
            if let Some(default) = &input.default {
                validate_input_value(key, input, default)?;
            } else if input.required {
                // Required values are supplied by the generated form.
            }
        }
        if self.request.url.len() > 4096
            || self.request.headers.len() > 64
            || self.request.query.len() > 128
            || self.request.timeout_ms == 0
            || std::time::Duration::from_millis(self.request.timeout_ms) > MAX_TIMEOUT
            || (self.request.body.is_some() && self.request.body_text.is_some())
        {
            return Err("HTTP 请求超出 V1 限制".into());
        }
        let url_template = replace_placeholders_for_check(&self.request.url)?;
        let url = Url::parse(&url_template).map_err(|_| "请求 URL 无效".to_owned())?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("请求 URL 必须是没有用户信息或片段的 HTTP(S) URL".into());
        }
        let authority_start = self.request.url.find("://").map(|i| i + 3).unwrap_or(0);
        let authority_end = self.request.url[authority_start..]
            .find(['/', '?', '#'])
            .map(|i| authority_start + i)
            .unwrap_or(self.request.url.len());
        if self.request.url[authority_start..authority_end].contains("${") {
            return Err("请求目标 Origin 不能使用占位符".into());
        }
        for (key, value) in &self.request.headers {
            if key.is_empty() || key.len() > 256 || value.len() > 16_384 {
                return Err("HTTP Header 超出限制".into());
            }
            validate_placeholders(value, self, false)
                .map_err(|error| format!("请求头 {key}：{error}"))?;
        }
        for (key, value) in &self.request.query {
            validate_placeholders(value, self, false)
                .map_err(|error| format!("查询参数 {key}：{error}"))?;
        }
        validate_placeholders(&self.request.url, self, false)
            .map_err(|error| format!("请求 URL：{error}"))?;
        if let Some(body) = &self.request.body {
            validate_json_placeholders(body, self)?;
        }
        if let Some(body) = &self.request.body_text {
            validate_placeholders(body, self, false).map_err(|error| format!("请求体：{error}"))?;
        }
        if let Some(spec) = &self.value {
            validate_value_spec(spec)?;
        }
        if let Some(alert) = &self.alert {
            validate_alert_spec(alert, self)?;
        }
        for (key, metric) in &self.values {
            if key.is_empty() || key.len() > 48
                || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                || metric.name.trim().is_empty() || metric.name.chars().count() > 80
            {
                return Err(format!("指标 {key} 的 id 或名称无效"));
            }
            validate_value_spec(&metric.value)?;
            if let Some(alert) = &metric.alert {
                validate_alert_spec(alert, self)?;
            }
        }
        Ok(())
    }

    pub fn origin(&self) -> Result<String, String> {
        let url = Url::parse(&replace_placeholders_for_check(&self.request.url)?)
            .map_err(|_| "请求 URL 无效".to_owned())?;
        Ok(url.origin().ascii_serialization())
    }
}

impl Definition {
    pub fn is_multi(&self) -> bool {
        !self.values.is_empty()
    }

    pub fn metric_value(&self, id: &str) -> Option<&ValueDefinition> {
        if self.is_multi() {
            self.values.get(id).map(|metric| &metric.value)
        } else if id == "value" {
            self.value.as_ref()
        } else {
            None
        }
    }

    pub fn metric_alert(&self, id: &str) -> Option<&AlertDefinition> {
        if self.is_multi() {
            self.values.get(id)?.alert.as_ref()
        } else if id == "value" {
            self.alert.as_ref()
        } else {
            None
        }
    }

    pub fn display_name(&self, inputs: &BTreeMap<String, Value>) -> String {
        let Some(template) = self.name_template.as_deref() else {
            return self.name.clone();
        };
        let mut name = template.to_owned();
        for (key, value) in inputs {
            let label = value.as_str().map_or_else(|| value.to_string(), str::to_owned);
            // Do not leak the value of an unresolved secret reference into names.
            let label = if label.starts_with('$') && label.ends_with('$') { "****".to_owned() } else { label };
            name = name.replace(&format!("${{input.{key}}}"), &label);
        }
        if name.trim().is_empty() { self.name.clone() } else { name }
    }
}

fn validate_value_spec(spec: &ValueDefinition) -> Result<(), String> {
    if spec.kind != "number" || spec.path.is_empty() || spec.path.len() > 512
        || !spec.multiply.is_finite() || !spec.divide.is_finite() || spec.divide == 0.0
        || !spec.offset.is_finite() || spec.decimals > 12
    {
        return Err("数值提取或转换配置无效".into());
    }
    jsonpath_lib::Selector::new().str_path(&spec.path)
        .map_err(|_| "JSONPath 无效".to_owned())?;
    Ok(())
}

fn validate_alert_spec(alert: &AlertDefinition, definition: &Definition) -> Result<(), String> {
    if alert.title.trim().is_empty() || alert.title.chars().count() > 120
        || alert.message.chars().count() > 1000
    {
        return Err("提醒标题或正文无效".into());
    }
    if let Some(value) = alert.threshold.as_f64() {
        if !value.is_finite() { return Err("提醒阈值必须是有限数值".into()); }
    } else if let Some(template) = alert.threshold.as_str() {
        validate_placeholders(template, definition, false)?;
        let name = template.strip_prefix("${input.")
            .and_then(|value| value.strip_suffix('}'))
            .ok_or_else(|| "提醒阈值只能引用一个数值参数".to_owned())?;
        if definition.inputs.get(name).is_none_or(|input| {
            !matches!(input.kind, InputType::Number | InputType::Integer)
        }) {
            return Err(format!("提醒阈值参数 {name} 必须是 number 或 integer"));
        }
    } else {
        return Err("提醒阈值必须是数值或参数占位符".into());
    }
    validate_placeholders(&alert.title, definition, true)?;
    validate_placeholders(&alert.message, definition, true)?;
    Ok(())
}

fn validate_input_value(
    key: &str,
    definition: &InputDefinition,
    value: &Value,
) -> Result<(), String> {
    let invalid = || format!("参数 {key} 的默认值类型不匹配");
    match definition.kind {
        InputType::String if !value.is_string() => return Err(invalid()),
        InputType::Number if !value.as_f64().is_some_and(f64::is_finite) => return Err(invalid()),
        InputType::Integer if value.as_i64().is_none() => return Err(invalid()),
        InputType::Boolean if !value.is_boolean() => return Err(invalid()),
        InputType::Enum
            if !value
                .as_str()
                .is_some_and(|value| definition.options.contains(&value.to_owned())) =>
        {
            return Err(invalid());
        }
        InputType::Duration
            if value
                .as_str()
                .is_none_or(|value| parse_duration(value).is_err()) =>
        {
            return Err(invalid());
        }
        _ => {}
    }
    if let Some(number) = input_numeric_value(value, definition.kind) {
        let min = numeric_limit(definition.min.as_ref(), definition.kind)?;
        let max = numeric_limit(definition.max.as_ref(), definition.kind)?;
        if min.is_some_and(|min| number < min) || max.is_some_and(|max| number > max) {
            return Err(format!("参数 {key} 的默认值超出范围"));
        }
    }
    Ok(())
}

pub fn input_numeric_value(value: &Value, kind: InputType) -> Option<f64> {
    match kind {
        InputType::Duration => value
            .as_str()
            .and_then(|value| parse_duration(value).ok())
            .map(|duration| duration.as_millis() as f64),
        _ => value.as_f64(),
    }
}

fn numeric_limit(value: Option<&Value>, kind: InputType) -> Result<Option<f64>, String> {
    let Some(value) = value else { return Ok(None) };
    if kind == InputType::Duration {
        return value
            .as_str()
            .ok_or_else(|| "时间间隔范围必须使用字符串".to_owned())
            .and_then(parse_duration)
            .map(|duration| Some(duration.as_millis() as f64));
    }
    let number = value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| "参数范围必须是有限数值".to_owned())?;
    Ok(Some(number))
}

pub fn validate_placeholders(
    text: &str,
    definition: &Definition,
    allow_value: bool,
) -> Result<(), String> {
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        let value = &rest[start + 2..];
        let end = value.find('}').ok_or_else(|| "占位符未闭合".to_owned())?;
        let token = &value[..end];
        if matches!(token, "value" | "previous" | "delta") {
            if !allow_value {
                return Err("数值占位符只能用于提醒文本".into());
            }
        } else if let Some(name) = token.strip_prefix("input.") {
            if !definition.inputs.contains_key(name) {
                return Err(format!("未声明的参数占位符：{name}"));
            }
        } else {
            return Err(format!("不支持的占位符：${{{token}}}"));
        }
        rest = &value[end + 1..];
    }
    if rest.contains('}') {
        return Err("占位符格式无效".into());
    }
    Ok(())
}

fn validate_json_placeholders(value: &Value, definition: &Definition) -> Result<(), String> {
    validate_json_placeholders_at(value, definition, "请求体")
}

fn validate_json_placeholders_at(
    value: &Value,
    definition: &Definition,
    path: &str,
) -> Result<(), String> {
    match value {
        Value::String(text) => validate_placeholders(text, definition, false)
            .map_err(|error| format!("{path}：{error}")),
        Value::Array(values) => values.iter().enumerate().try_for_each(|(index, value)| {
            validate_json_placeholders_at(value, definition, &format!("{path}[{index}]"))
        }),
        Value::Object(values) => values.iter().try_for_each(|(key, value)| {
            validate_json_placeholders_at(value, definition, &format!("{path}.{key}"))
        }),
        _ => Ok(()),
    }
}

fn replace_placeholders_for_check(value: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let value = &rest[start + 2..];
        let end = value.find('}').ok_or_else(|| "占位符未闭合".to_owned())?;
        output.push('x');
        rest = &value[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

pub fn parse_duration(value: &str) -> Result<std::time::Duration, String> {
    let value = value.trim();
    let unit_start = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(unit_start);
    let number: u64 = number.parse().map_err(|_| "时间间隔无效".to_owned())?;
    let millis = match unit {
        "ms" => number,
        "s" => number.saturating_mul(1_000),
        "m" => number.saturating_mul(60_000),
        "h" => number.saturating_mul(3_600_000),
        _ => return Err("时间间隔必须使用 ms、s、m 或 h".into()),
    };
    if millis == 0 || millis > 31_536_000_000 {
        return Err("时间间隔超出限制".into());
    }
    Ok(std::time::Duration::from_millis(millis))
}

#[cfg(test)]
mod tests {
    use super::{parse_duration, Definition, InputType};
    use serde_json::Value;
    use std::time::Duration;

    fn sample() -> Value {
        serde_json::from_str(include_str!(
            "../../../examples/datasources/shanghai-temperature.pds.json"
        ))
        .unwrap()
    }

    #[test]
    fn current_sample_has_no_runtime_refresh_setting() {
        let definition = Definition::parse(include_str!(
            "../../../examples/datasources/shanghai-temperature.pds.json"
        ))
        .unwrap();
        assert!(!serde_json::to_value(definition).unwrap()["request"]
            .as_object()
            .unwrap()
            .contains_key("interval"));
    }

    #[test]
    fn removed_request_interval_is_rejected() {
        let mut document = sample();
        document["request"]["interval"] = Value::String("10m".into());
        assert!(Definition::parse(&document.to_string()).is_err());
    }

    #[test]
    fn removed_alert_fields_are_rejected() {
        for field in ["repeat_interval", "notify_on_initial"] {
            let mut document = sample();
            document["alert"][field] = if field == "notify_on_initial" {
                Value::Bool(true)
            } else {
                Value::String("1h".into())
            };
            assert!(Definition::parse(&document.to_string()).is_err(), "{field}");
        }
    }

    #[test]
    fn ordinary_duration_inputs_still_parse_and_validate() {
        let mut document = sample();
        document["inputs"]["window"] = serde_json::json!({
            "type": "duration",
            "label": "统计窗口",
            "default": "2m",
            "required": true
        });
        let definition = Definition::parse(&document.to_string()).unwrap();
        assert_eq!(definition.inputs["window"].kind, InputType::Duration);
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
    }

    #[test]
    fn placeholder_errors_name_the_request_field() {
        let mut document = sample();
        document["request"]["headers"]["Authorization"] =
            Value::String("Bearer ${input.api_key}".into());

        let error = Definition::parse(&document.to_string()).unwrap_err();
        assert!(error.contains("请求头 Authorization"), "{error}");
        assert!(error.contains("未声明的参数占位符：api_key"), "{error}");

        document["inputs"]["api_key"] = serde_json::json!({
            "type": "string",
            "label": "API Key",
            "required": true
        });
        assert!(Definition::parse(&document.to_string()).is_ok());
    }

    #[test]
    fn json_body_placeholder_errors_name_the_body_path() {
        let mut document = sample();
        document["request"]["body"] = serde_json::json!({"auth": {"token": "${input.api_key}"}});

        let error = Definition::parse(&document.to_string()).unwrap_err();
        assert!(error.contains("请求体.auth.token"), "{error}");
    }

    #[test]
    fn secret_inputs_and_secret_placeholders_are_rejected() {
        let mut document = sample();
        document["inputs"]["api_key"] = serde_json::json!({
            "type": "secret",
            "label": "API Key",
            "required": true
        });
        assert!(Definition::parse(&document.to_string()).is_err());

        document["inputs"]["api_key"] = serde_json::json!({
            "type": "string",
            "label": "API Key",
            "required": true
        });
        document["request"]["headers"]["Authorization"] =
            Value::String("Bearer ${secret.api_key}".into());
        let error = Definition::parse(&document.to_string()).unwrap_err();
        assert!(error.contains("不支持的占位符"), "{error}");
    }
}
