use super::model::{parse_snapshot, Location, Snapshot};
use super::preferences::WeatherPreferences;
use crate::core::http;
use serde_json::Value;
use std::{io::Read, time::Duration};

const USER_AGENT: &str = "island-lab/0.1 weather extension";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const IP_LOCATION_URL: &str = "https://ipwho.is/";

#[derive(Clone, Debug, PartialEq)]
pub struct CityResult {
    pub name: String,
    pub admin: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl CityResult {
    pub fn detail(&self) -> String {
        match (self.admin.is_empty(), self.country.is_empty()) {
            (false, false) => format!("{}, {}", self.admin, self.country),
            (false, true) => self.admin.clone(),
            (true, false) => self.country.clone(),
            (true, true) => String::new(),
        }
    }
}

pub fn fetch(
    preferences: &WeatherPreferences,
    cached_auto_location: Option<&Location>,
) -> Result<Snapshot, String> {
    let location = if preferences.auto_locate {
        if let Some(location) = cached_auto_location {
            location.clone()
        } else {
            match locate_by_ip() {
                Ok(location) => location,
                Err(error) => preferences
                    .selected_location()
                    .map(location_from_saved)
                    .ok_or(error)?,
            }
        }
    } else {
        preferences
            .selected_location()
            .map(location_from_saved)
            .ok_or_else(|| "请先在设置中选择天气城市".to_owned())?
    };
    let location =
        if !preferences.auto_locate && location.latitude == 0.0 && location.longitude == 0.0 {
            let city = search_cities(&location.name)?
                .into_iter()
                .next()
                .ok_or("没有找到天气城市")?;
            Location {
                name: format!("{} {}", city.name, city.detail()).trim().into(),
                latitude: city.latitude,
                longitude: city.longitude,
                ..location
            }
        } else {
            location
        };
    let body = get(&forecast_url(&location))?;
    parse_snapshot(&body, location)
}

pub fn search_cities(query: &str) -> Result<Vec<CityResult>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("name", query)
        .append_pair("count", "5")
        .append_pair("language", "zh")
        .append_pair("format", "json")
        .finish();
    let body = get(&format!("{GEOCODING_URL}?{params}"))?;
    parse_cities(&body)
}

pub fn parse_cities(body: &str) -> Result<Vec<CityResult>, String> {
    let root: Value =
        serde_json::from_str(body).map_err(|error| format!("城市搜索结果解析失败：{error}"))?;
    let Some(results) = root.get("results").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    Ok(results
        .iter()
        .filter_map(|result| {
            let name = result.get("name")?.as_str()?.trim().to_owned();
            let latitude = result.get("latitude")?.as_f64()?;
            let longitude = result.get("longitude")?.as_f64()?;
            if name.is_empty()
                || !(-90.0..=90.0).contains(&latitude)
                || !(-180.0..=180.0).contains(&longitude)
            {
                return None;
            }
            Some(CityResult {
                name,
                admin: result
                    .get("admin1")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                country: result
                    .get("country")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                latitude,
                longitude,
            })
        })
        .collect())
}

pub fn parse_ip_location(body: &str) -> Result<Location, String> {
    let root: Value =
        serde_json::from_str(body).map_err(|error| format!("定位结果解析失败：{error}"))?;
    if !root
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("无法根据网络位置定位，请在设置中选择城市".into());
    }
    let latitude = root
        .get("latitude")
        .and_then(Value::as_f64)
        .ok_or_else(|| "定位结果缺少纬度".to_owned())?;
    let longitude = root
        .get("longitude")
        .and_then(Value::as_f64)
        .ok_or_else(|| "定位结果缺少经度".to_owned())?;
    let city = root
        .get("city")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let region = root
        .get("region")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let country = root
        .get("country")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let name = if !city.is_empty() {
        city
    } else if !region.is_empty() {
        region
    } else if !country.is_empty() {
        country
    } else {
        return Err("定位结果没有城市名称".into());
    };
    Ok(Location {
        name: name.to_owned(),
        admin: region.to_owned(),
        country: country.to_owned(),
        latitude,
        longitude,
    })
}

fn locate_by_ip() -> Result<Location, String> {
    let body = get(IP_LOCATION_URL)?;
    parse_ip_location(&body)
}

fn location_from_saved(saved: &super::preferences::WeatherLocation) -> Location {
    Location {
        name: saved.name.clone(),
        admin: String::new(),
        country: String::new(),
        latitude: saved.latitude,
        longitude: saved.longitude,
    }
}

fn forecast_url(location: &Location) -> String {
    let current = [
        "temperature_2m",
        "relative_humidity_2m",
        "apparent_temperature",
        "is_day",
        "precipitation",
        "weather_code",
        "cloud_cover",
        "wind_speed_10m",
        "wind_direction_10m",
        "wind_gusts_10m",
        "uv_index",
        "visibility",
    ]
    .join(",");
    let daily = [
        "weather_code",
        "temperature_2m_max",
        "temperature_2m_min",
        "apparent_temperature_max",
        "apparent_temperature_min",
        "sunrise",
        "sunset",
        "uv_index_max",
        "precipitation_sum",
        "precipitation_probability_max",
        "wind_speed_10m_max",
        "wind_gusts_10m_max",
        "wind_direction_10m_dominant",
        "snowfall_sum",
    ]
    .join(",");
    let hourly = [
        "temperature_2m",
        "is_day",
        "apparent_temperature",
        "precipitation_probability",
        "rain",
        "showers",
        "weather_code",
        "wind_speed_10m",
        "wind_gusts_10m",
        "uv_index",
        "visibility",
    ]
    .join(",");
    format!(
        "{FORECAST_URL}?latitude={:.5}&longitude={:.5}&current={current}&daily={daily}&hourly={hourly}&timezone=auto&forecast_days=7",
        location.latitude, location.longitude
    )
}

fn get(url: &str) -> Result<String, String> {
    const MAX_RESPONSE: usize = 10 * 1024 * 1024;
    let mut body = Vec::new();
    http::client(Duration::from_secs(6), Duration::from_secs(15), USER_AGENT)
        .and_then(|client| client.get(url).send())
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("天气网络请求失败：{error}"))?
        .take((MAX_RESPONSE + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| format!("天气数据读取失败：{error}"))?;
    if body.len() > MAX_RESPONSE {
        return Err("天气服务返回的数据过大".into());
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{forecast_url, parse_cities, parse_ip_location};
    use crate::builtins::weather::model::Location;

    #[test]
    fn ip_location_is_parsed_and_requires_a_success_flag() {
        let body = r#"{
            "success": true,
            "city": "香港",
            "region": "香港",
            "country": "Hong Kong",
            "latitude": 22.2783,
            "longitude": 114.1747
        }"#;
        let location = parse_ip_location(body).unwrap();
        assert_eq!(location.name, "香港");
        assert_eq!(location.country, "Hong Kong");
        assert!((location.latitude - 22.2783).abs() < 0.0001);
        assert!(parse_ip_location(r#"{"success": false}"#).is_err());
    }

    #[test]
    fn geocoding_results_are_parsed_and_incomplete_entries_are_skipped() {
        let body = r#"{
            "results": [
                {"name": "上海", "admin1": "上海市", "country": "中国", "latitude": 31.22, "longitude": 121.46},
                {"name": "", "latitude": 0.0, "longitude": 0.0},
                {"name": "Invalid", "latitude": 200.0, "longitude": 0.0}
            ]
        }"#;
        let cities = parse_cities(body).unwrap();
        assert_eq!(cities.len(), 1);
        assert_eq!(cities[0].name, "上海");
        assert_eq!(cities[0].detail(), "上海市, 中国");
        assert!(parse_cities("{}").unwrap().is_empty());
    }

    #[test]
    fn forecast_url_requests_every_field_the_weather_views_need() {
        let location = Location {
            name: "上海".into(),
            latitude: 31.22,
            longitude: 121.46,
            ..Location::default()
        };
        let url = forecast_url(&location);
        let url = url::Url::parse(&url).unwrap();
        let hourly = url
            .query_pairs()
            .find(|(key, _)| key == "hourly")
            .unwrap()
            .1;
        assert!(hourly.split(',').any(|field| field == "is_day"));
        for field in [
            "apparent_temperature",
            "sunrise",
            "sunset",
            "uv_index_max",
            "wind_gusts_10m",
            "precipitation_probability_max",
            "forecast_days=7",
        ] {
            assert!(url.as_str().contains(field), "missing {field}");
        }
    }
}
