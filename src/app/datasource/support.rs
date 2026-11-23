use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DurationUnit {
    Seconds,
    #[default]
    Minutes,
    Hours,
    Days,
}

impl DurationUnit {
    fn label(self) -> &'static str {
        match self {
            Self::Seconds => "秒",
            Self::Minutes => "分钟",
            Self::Hours => "小时",
            Self::Days => "天",
        }
    }

    fn multiplier(self) -> u64 {
        match self {
            Self::Seconds => 1_000,
            Self::Minutes => 60_000,
            Self::Hours => 3_600_000,
            Self::Days => 86_400_000,
        }
    }
}


pub(super) fn required_inputs_ready(source: &SourceSnapshot) -> bool {
    source.input_schema.iter().all(|(key, schema)| {
        if !schema.required {
            return true;
        }
        source
            .inputs
            .get(key)
            .or(schema.default.as_ref())
            .is_some_and(|value| match schema.kind {
                InputType::String | InputType::Enum | InputType::Duration => value
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty()),
                _ => !value.is_null(),
            })
    })
}

pub(super) fn doctor_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).weak());
    ui.label(value);
    ui.end_row();
}

pub(super) fn method_text(method: datasource::HttpMethod) -> &'static str {
    match method {
        datasource::HttpMethod::Get => "GET",
        datasource::HttpMethod::Post => "POST",
        datasource::HttpMethod::Put => "PUT",
        datasource::HttpMethod::Patch => "PATCH",
    }
}

pub(super) fn timestamp_text(milliseconds: u64) -> String {
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
        .map(|value| {
            let value = value.to_offset(time::UtcOffset::UTC);
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02} UTC",
                value.year(),
                u8::from(value.month()),
                value.day(),
                value.hour(),
                value.minute()
            )
        })
        .unwrap_or_else(|_| milliseconds.to_string())
}

pub(super) fn updated_time_text(milliseconds: u64) -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let Ok(value) =
        time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
    else {
        return timestamp_text(milliseconds);
    };
    let value = value.to_offset(offset);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    if value.date() == now.date() {
        format!("今天 {:02}:{:02}", value.hour(), value.minute())
    } else if value.year() == now.year() {
        format!(
            "{}月{}日 {:02}:{:02}",
            u8::from(value.month()),
            value.day(),
            value.hour(),
            value.minute()
        )
    } else {
        format!(
            "{}年{}月{}日 {:02}:{:02}",
            value.year(),
            u8::from(value.month()),
            value.day(),
            value.hour(),
            value.minute()
        )
    }
}

pub(super) fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

pub(super) fn duration_parts(milliseconds: u64) -> (DurationUnit, u64) {
    for unit in [
        DurationUnit::Days,
        DurationUnit::Hours,
        DurationUnit::Minutes,
        DurationUnit::Seconds,
    ] {
        if milliseconds >= unit.multiplier() && milliseconds.is_multiple_of(unit.multiplier()) {
            return (unit, milliseconds / unit.multiplier());
        }
    }
    (DurationUnit::Seconds, milliseconds.div_ceil(1000))
}

pub(super) fn display_millis(milliseconds: u64) -> String {
    let (unit, value) = duration_parts(milliseconds);
    format!("{value} {}", unit.label())
}

pub(super) fn classify_error(error: &str) -> String {
    if error.contains("缺少必填参数") || error.contains("请填写必填参数") {
        format!("尚未配置完成：{error}")
    } else if error.contains("Secret") {
        format!("Secret 不可用：{error}")
    } else if error.contains("授权") || error.contains("单独授权") {
        format!("权限不足：{error}")
    } else if error.contains("429") || error.contains("限流") {
        format!("API 限流：{error}")
    } else if error.contains("JSONPath") || error.contains("数值") {
        format!("路径或类型转换失败：{error}")
    } else if error.contains("JSON") || error.contains("响应") {
        format!("响应解析失败：{error}")
    } else if error.contains("超时") {
        format!("请求超时：{error}")
    } else if error.contains("HTTP 请求") || error.contains("解析请求目标") {
        format!("网络失败：{error}")
    } else {
        format!("运行时错误：{error}")
    }
}
