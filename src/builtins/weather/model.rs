use serde_json::Value;
use time::{Date, Month};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Location {
    pub name: String,
    pub admin: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl Location {
    pub fn label(&self) -> String {
        if self.admin.is_empty() || self.admin == self.name {
            self.name.clone()
        } else {
            format!("{} · {}", self.name, self.admin)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurrentWeather {
    pub temperature: f64,
    pub apparent_temperature: f64,
    pub humidity: u8,
    pub precipitation: f64,
    pub precipitation_probability: u8,
    pub weather_code: u8,
    pub is_day: bool,
    pub cloud_cover: u8,
    pub pressure: f64,
    pub wind_speed: f64,
    pub wind_gusts: f64,
    pub wind_direction: u16,
    pub uv_index: f64,
    pub visibility: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DailyForecast {
    pub date: String,
    pub weather_code: u8,
    pub high: f64,
    pub low: f64,
    pub apparent_high: f64,
    pub apparent_low: f64,
    pub sunrise: String,
    pub sunset: String,
    pub uv_max: f64,
    pub precipitation_sum: f64,
    pub precipitation_probability: u8,
    pub wind_max: f64,
    pub wind_gusts_max: f64,
    pub wind_direction: u16,
    pub snowfall_sum: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HourlyForecast {
    pub time: String,
    pub temperature: f64,
    pub apparent_temperature: f64,
    pub precipitation_probability: u8,
    pub rainfall: f64,
    pub weather_code: u8,
    pub wind_speed: f64,
    pub wind_gusts: f64,
    pub uv_index: f64,
    pub visibility: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub location: Location,
    pub timezone: String,
    pub updated_at: String,
    pub current: CurrentWeather,
    pub daily: Vec<DailyForecast>,
    pub hourly: Vec<HourlyForecast>,
}

impl Snapshot {
    pub fn today(&self) -> Option<&DailyForecast> {
        self.daily.first()
    }

    pub fn condition(&self) -> &'static str {
        weather_description(self.current.weather_code)
    }

    pub fn icon(&self) -> &'static str {
        weather_icon(self.current.weather_code, self.current.is_day)
    }

    pub fn updated_hm(&self) -> String {
        time_hm(&self.updated_at)
    }

    pub fn day_labels(&self) -> Vec<String> {
        self.daily
            .iter()
            .enumerate()
            .map(|(index, day)| {
                if index == 0 {
                    "今天".into()
                } else if index == 1 {
                    "明天".into()
                } else {
                    weekday_label(&day.date)
                }
            })
            .collect()
    }

    pub fn daylight(&self) -> Option<String> {
        let today = self.today()?;
        let sunrise = clock_minutes(&today.sunrise)?;
        let sunset = clock_minutes(&today.sunset)?;
        let minutes = sunset.saturating_sub(sunrise);
        Some(format!("{}小时{}分", minutes / 60, minutes % 60))
    }

    pub fn hourly_from_now(&self, count: usize) -> Vec<&HourlyForecast> {
        let Some(start) = self
            .hourly
            .iter()
            .position(|hour| hour.time.as_str() >= self.updated_at.as_str())
            .or_else(|| (!self.hourly.is_empty()).then_some(0))
        else {
            return Vec::new();
        };
        self.hourly.iter().skip(start).take(count).collect()
    }
}

pub fn parse_snapshot(json: &str, location: Location) -> Result<Snapshot, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("天气数据解析失败：{error}"))?;
    let current = root
        .get("current")
        .ok_or_else(|| "天气数据缺少 current".to_owned())?;
    let daily = root
        .get("daily")
        .ok_or_else(|| "天气数据缺少 daily".to_owned())?;
    let hourly = root
        .get("hourly")
        .ok_or_else(|| "天气数据缺少 hourly".to_owned())?;
    let current_number = |key: &str| {
        current
            .get(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("天气数据缺少 {key}"))
    };
    let temperature = current_number("temperature_2m")?;
    let apparent_temperature = current_number("apparent_temperature")?;

    let dates = array(daily, "time")?;
    let mut daily_forecasts = Vec::with_capacity(dates.len());
    for (index, date) in dates.iter().enumerate() {
        let Some(date) = date.as_str().filter(|date| parse_date(date).is_some()) else {
            continue;
        };
        let number = |key: &str| {
            daily[key]
                .as_array()
                .and_then(|data| data.get(index))
                .and_then(Value::as_f64)
        };
        let (Some(high), Some(low)) = (number("temperature_2m_max"), number("temperature_2m_min"))
        else {
            continue;
        };
        if high < low {
            continue;
        }
        daily_forecasts.push(DailyForecast {
            date: date.to_owned(),
            weather_code: u8_at(daily, "weather_code", index),
            high,
            low,
            apparent_high: number("apparent_temperature_max").unwrap_or(high),
            apparent_low: number("apparent_temperature_min").unwrap_or(low),
            sunrise: time_hm(&str_at(daily, "sunrise", index)),
            sunset: time_hm(&str_at(daily, "sunset", index)),
            uv_max: f64_at(daily, "uv_index_max", index),
            precipitation_sum: f64_at(daily, "precipitation_sum", index),
            precipitation_probability: u8_at(daily, "precipitation_probability_max", index),
            wind_max: f64_at(daily, "wind_speed_10m_max", index),
            wind_gusts_max: f64_at(daily, "wind_gusts_10m_max", index),
            wind_direction: u16_at(daily, "wind_direction_10m_dominant", index),
            snowfall_sum: f64_at(daily, "snowfall_sum", index),
        });
    }

    let times = array(hourly, "time")?;
    let mut hourly_forecasts = Vec::with_capacity(times.len());
    for (index, time) in times.iter().enumerate() {
        let time = time.as_str().unwrap_or_default().to_owned();
        hourly_forecasts.push(HourlyForecast {
            time,
            temperature: f64_at(hourly, "temperature_2m", index),
            apparent_temperature: f64_at(hourly, "apparent_temperature", index),
            precipitation_probability: u8_at(hourly, "precipitation_probability", index),
            rainfall: f64_at(hourly, "rain", index) + f64_at(hourly, "showers", index),
            weather_code: u8_at(hourly, "weather_code", index),
            wind_speed: f64_at(hourly, "wind_speed_10m", index),
            wind_gusts: f64_at(hourly, "wind_gusts_10m", index),
            uv_index: f64_at(hourly, "uv_index", index),
            visibility: f64_at(hourly, "visibility", index),
        });
    }

    let updated_at = current
        .get("time")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let precipitation_probability = hourly_forecasts
        .iter()
        .find(|hour| hour.time.as_str() >= updated_at.as_str())
        .map_or(0, |hour| hour.precipitation_probability);

    let snapshot = Snapshot {
        location,
        timezone: root
            .get("timezone")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        updated_at,
        current: CurrentWeather {
            temperature,
            apparent_temperature,
            humidity: u8_field(current, "relative_humidity_2m"),
            precipitation: f64_field(current, "precipitation"),
            precipitation_probability,
            weather_code: u8_field(current, "weather_code"),
            is_day: current.get("is_day").and_then(Value::as_u64).unwrap_or(1) != 0,
            cloud_cover: u8_field(current, "cloud_cover"),
            pressure: f64_field(current, "pressure_msl"),
            wind_speed: f64_field(current, "wind_speed_10m"),
            wind_gusts: f64_field(current, "wind_gusts_10m"),
            wind_direction: u16_field(current, "wind_direction_10m"),
            uv_index: f64_field(current, "uv_index"),
            visibility: f64_field(current, "visibility"),
        },
        daily: daily_forecasts,
        hourly: hourly_forecasts,
    };
    if snapshot.daily.is_empty() {
        return Err("天气数据没有可用的逐日预报".into());
    }
    Ok(snapshot)
}

pub fn weather_description(code: u8) -> &'static str {
    match code {
        0 => "晴",
        1 => "大部晴朗",
        2 => "局部多云",
        3 => "阴",
        45 | 48 => "有雾",
        51 | 53 | 55 => "毛毛雨",
        56 | 57 => "冻毛毛雨",
        61 => "小雨",
        63 => "中雨",
        65 => "大雨",
        66 | 67 => "冻雨",
        71 => "小雪",
        73 => "中雪",
        75 => "大雪",
        77 => "米雪",
        80 => "小阵雨",
        81 => "阵雨",
        82 => "强阵雨",
        85 | 86 => "阵雪",
        95 => "雷暴",
        96 | 99 => "雷暴伴冰雹",
        _ => "多云",
    }
}

pub fn weather_icon(code: u8, is_day: bool) -> &'static str {
    match code {
        0 | 1 => {
            if is_day {
                "sun"
            } else {
                "moon"
            }
        }
        2 => {
            if is_day {
                "cloud-sun"
            } else {
                "cloud-moon"
            }
        }
        3 => "cloud",
        45 | 48 => "cloud-fog",
        51 | 53 | 55 | 56 | 57 => "cloud-drizzle",
        61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 => "cloud-rain",
        71 | 73 | 75 | 77 | 85 | 86 => "cloud-snow",
        95 | 96 | 99 => "cloud-lightning",
        _ => "cloud",
    }
}

pub fn wind_direction(degrees: u16) -> &'static str {
    let labels = ["北", "东北", "东", "东南", "南", "西南", "西", "西北"];
    labels[((degrees as f32 + 22.5) / 45.0).floor() as usize % labels.len()]
}

pub fn wind_level(kmh: f64) -> u8 {
    match kmh {
        value if value < 1.0 => 0,
        value if value < 6.0 => 1,
        value if value < 12.0 => 2,
        value if value < 20.0 => 3,
        value if value < 29.0 => 4,
        value if value < 39.0 => 5,
        value if value < 50.0 => 6,
        value if value < 62.0 => 7,
        value if value < 75.0 => 8,
        value if value < 89.0 => 9,
        value if value < 103.0 => 10,
        value if value < 118.0 => 11,
        _ => 12,
    }
}

pub fn uv_level(uv: f64) -> &'static str {
    match uv {
        value if value < 3.0 => "低",
        value if value < 6.0 => "中等",
        value if value < 8.0 => "高",
        value if value < 11.0 => "很高",
        _ => "极高",
    }
}

pub fn format_visibility(meters: f64) -> String {
    if meters >= 1000.0 {
        format!("{:.0} km", meters / 1000.0)
    } else {
        format!("{:.0} m", meters.max(0.0))
    }
}

pub fn time_hm(value: &str) -> String {
    let clock = value.rsplit('T').next().unwrap_or(value);
    let mut parts = clock.split(':');
    let (Some(hour), Some(minute)) = (parts.next(), parts.next()) else {
        return value.to_owned();
    };
    let (Ok(hour), Ok(minute)) = (hour.parse::<u32>(), minute.parse::<u32>()) else {
        return value.to_owned();
    };
    format!("{hour:02}:{minute:02}")
}

fn clock_minutes(value: &str) -> Option<u16> {
    let clock = value.rsplit('T').next().unwrap_or(value);
    let (hour, minute) = clock.split_once(':')?;
    let hour = hour.parse::<u16>().ok()?;
    let minute = minute.get(..2)?.parse::<u16>().ok()?;
    Some(hour * 60 + minute)
}

fn weekday_label(value: &str) -> String {
    let Some(date) = parse_date(value) else {
        return value.to_owned();
    };
    let labels = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
    labels[date.weekday().number_days_from_monday() as usize].to_owned()
}

fn parse_date(value: &str) -> Option<Date> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = Month::try_from(parts.next()?.parse::<u8>().ok()?).ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    Date::from_calendar_date(year, month, day).ok()
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("天气数据缺少 {key}"))
}

fn f64_field(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or_default()
}

fn u8_field(value: &Value, key: &str) -> u8 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map_or(0, |value| value.min(255) as u8)
}

fn u16_field(value: &Value, key: &str) -> u16 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map_or(0, |value| value.min(65535) as u16)
}

fn f64_at(value: &Value, key: &str, index: usize) -> f64 {
    value
        .get(key)
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
        .and_then(Value::as_f64)
        .unwrap_or_default()
}

fn u8_at(value: &Value, key: &str, index: usize) -> u8 {
    value
        .get(key)
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
        .and_then(Value::as_u64)
        .map_or(0, |value| value.min(255) as u8)
}

fn u16_at(value: &Value, key: &str, index: usize) -> u16 {
    value
        .get(key)
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
        .and_then(Value::as_u64)
        .map_or(0, |value| value.min(65535) as u16)
}

fn str_at(value: &Value, key: &str, index: usize) -> String {
    value
        .get(key)
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        format_visibility, parse_snapshot, time_hm, uv_level, weather_description, weather_icon,
        wind_direction, wind_level, Location,
    };

    fn location() -> Location {
        Location {
            name: "上海".into(),
            admin: "上海市".into(),
            country: "中国".into(),
            latitude: 31.22,
            longitude: 121.46,
        }
    }

    #[test]
    fn forecast_json_is_parsed_into_rich_current_daily_and_hourly_data() {
        let json = r#"{
            "timezone": "Asia/Shanghai",
            "current": {
                "time": "2026-11-17T17:45",
                "temperature_2m": 14.9,
                "relative_humidity_2m": 71,
                "apparent_temperature": 13.8,
                "is_day": 0,
                "precipitation": 0.0,
                "weather_code": 3,
                "cloud_cover": 89,
                "pressure_msl": 1021.5,
                "wind_speed_10m": 6.8,
                "wind_direction_10m": 358,
                "wind_gusts_10m": 14.8,
                "uv_index": 0.0,
                "visibility": 22320.0
            },
            "daily": {
                "time": ["2026-11-17", "2026-11-18"],
                "weather_code": [3, 61],
                "temperature_2m_max": [18.2, 16.5],
                "temperature_2m_min": [11.1, 10.2],
                "apparent_temperature_max": [17.4, 15.5],
                "apparent_temperature_min": [10.0, 9.1],
                "sunrise": ["2026-11-17T06:18", "2026-11-18T06:19"],
                "sunset": ["2026-11-17T16:57", "2026-11-18T16:56"],
                "uv_index_max": [2.4, 3.0],
                "precipitation_sum": [0.0, 4.2],
                "precipitation_probability_max": [10, 75],
                "wind_speed_10m_max": [14.0, 22.0],
                "wind_gusts_10m_max": [26.0, 38.0],
                "wind_direction_10m_dominant": [350, 20],
                "snowfall_sum": [0.0, 0.0]
            },
            "hourly": {
                "time": ["2026-11-17T17:00", "2026-11-17T18:00", "2026-11-17T19:00"],
                "temperature_2m": [15.1, 14.6, 14.2],
                "apparent_temperature": [14.0, 13.5, 13.1],
                "precipitation_probability": [10, 20, 30],
                "precipitation": [0.0, 0.1, 0.2],
                "weather_code": [3, 3, 61],
                "wind_speed_10m": [7.0, 7.5, 8.0],
                "wind_gusts_10m": [15.0, 16.0, 17.0],
                "uv_index": [0.0, 0.0, 0.0],
                "visibility": [22000.0, 21000.0, 18000.0]
            }
        }"#;
        let snapshot = parse_snapshot(json, location()).unwrap();
        assert_eq!(snapshot.location.name, "上海");
        assert_eq!(snapshot.location.label(), "上海 · 上海市");
        assert_eq!(snapshot.timezone, "Asia/Shanghai");
        assert_eq!(snapshot.updated_hm(), "17:45");
        assert_eq!(snapshot.current.temperature, 14.9);
        assert_eq!(snapshot.current.apparent_temperature, 13.8);
        assert_eq!(snapshot.current.precipitation_probability, 20);
        assert_eq!(snapshot.current.humidity, 71);
        assert!(!snapshot.current.is_day);
        assert_eq!(snapshot.icon(), "cloud");
        assert_eq!(snapshot.daily[0].high, 18.2);
        assert_eq!(snapshot.daily[0].sunrise, "06:18");
        assert_eq!(snapshot.daily[0].sunset, "16:57");
        assert_eq!(snapshot.day_labels(), ["今天", "明天"]);
        assert_eq!(snapshot.daylight().as_deref(), Some("10小时39分"));
        assert_eq!(snapshot.hourly_from_now(1)[0].temperature, 14.6);
    }

    #[test]
    fn weather_codes_have_chinese_descriptions_and_day_night_icons() {
        assert_eq!(weather_description(0), "晴");
        assert_eq!(weather_description(95), "雷暴");
        assert_eq!(weather_description(65), "大雨");
        assert_eq!(weather_icon(0, true), "sun");
        assert_eq!(weather_icon(0, false), "moon");
        assert_eq!(weather_icon(2, true), "cloud-sun");
        assert_eq!(weather_icon(2, false), "cloud-moon");
        assert_eq!(weather_icon(61, true), "cloud-rain");
        assert_eq!(weather_icon(75, true), "cloud-snow");
        assert_eq!(weather_icon(95, true), "cloud-lightning");
    }

    #[test]
    fn units_and_clock_labels_are_human_readable() {
        assert_eq!(time_hm("2026-11-17T06:18"), "06:18");
        assert_eq!(time_hm("06:18"), "06:18");
        assert_eq!(format_visibility(22320.0), "22 km");
        assert_eq!(format_visibility(800.0), "800 m");
        assert_eq!(uv_level(0.0), "低");
        assert_eq!(uv_level(9.0), "很高");
        assert_eq!(wind_direction(0), "北");
        assert_eq!(wind_direction(135), "东南");
        assert_eq!(wind_direction(225), "西南");
        assert_eq!(wind_level(6.8), 2);
        assert_eq!(wind_level(70.0), 8);
    }
}
