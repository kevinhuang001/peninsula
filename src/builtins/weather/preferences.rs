use std::time::Duration;

pub const MAX_REFRESH_MINUTES: u16 = 180;
const DEFAULT_REFRESH_MINUTES: u16 = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct WeatherLocation {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl WeatherLocation {
    pub fn new(name: impl Into<String>, latitude: f64, longitude: f64) -> Self {
        Self {
            name: name.into(),
            latitude,
            longitude,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeatherPreferences {
    pub auto_locate: bool,
    pub alerts_enabled: bool,
    pub refresh_minutes: u16,
    pub location: Option<WeatherLocation>,
}

impl Default for WeatherPreferences {
    fn default() -> Self {
        Self {
            auto_locate: true,
            alerts_enabled: true,
            refresh_minutes: DEFAULT_REFRESH_MINUTES,
            location: None,
        }
    }
}

impl WeatherPreferences {
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs(u64::from(self.refresh_minutes) * 60)
    }

    pub fn apply_line(&mut self, key: &str, value: &str) -> bool {
        match key.trim() {
            "weather_auto" => {
                if let Some(value) = parse_bool(value) {
                    self.auto_locate = value;
                }
                true
            }
            "weather_alerts" => {
                if let Some(value) = parse_bool(value) {
                    self.alerts_enabled = value;
                }
                true
            }
            "weather_refresh_minutes" => {
                if let Ok(minutes) = value.trim().parse::<u16>() {
                    self.refresh_minutes = minutes.clamp(1, MAX_REFRESH_MINUTES);
                }
                true
            }
            "weather_location" => {
                if let Some(location) = parse_location(value) {
                    self.location = Some(location);
                }
                true
            }
            _ => false,
        }
    }

    #[cfg(test)]
    pub fn encode_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("weather_auto={}", self.auto_locate),
            format!("weather_alerts={}", self.alerts_enabled),
            format!("weather_refresh_minutes={}", self.refresh_minutes),
        ];
        if let Some(location) = &self.location {
            let name = location.name.replace(['|', '\n', '\r'], " ");
            lines.push(format!(
                "weather_location={}|{:.5}|{:.5}",
                name.trim(),
                location.latitude,
                location.longitude
            ));
        }
        lines
    }

    pub fn selected_location(&self) -> Option<&WeatherLocation> {
        self.location.as_ref()
    }
}

fn parse_location(value: &str) -> Option<WeatherLocation> {
    let mut fields = value.split('|');
    let name = fields.next()?.trim();
    let latitude = fields.next()?.trim().parse::<f64>().ok()?;
    let longitude = fields.next()?.trim().parse::<f64>().ok()?;
    if name.is_empty() || name.chars().count() > 64 {
        return None;
    }
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return None;
    }
    Some(WeatherLocation {
        name: name.to_owned(),
        latitude,
        longitude,
    })
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

impl WeatherPreferences {
    pub fn schema() -> Vec<crate::extension::SettingSchema> {
        use crate::extension::{SettingSchema as S, SettingValue as V};
        vec![
            S::new("auto_locate", V::Boolean(true), "自动定位"),
            S::new("alerts", V::Boolean(true), "天气预警"),
            S::new("refresh_minutes", V::Integer(10), "刷新间隔（分钟）").range(1.0, 180.0),
            S::new("location_name", V::String(String::new()), "城市名称"),
            S::new("latitude", V::Float(0.0), "纬度").range(-90.0, 90.0),
            S::new("longitude", V::Float(0.0), "经度").range(-180.0, 180.0),
        ]
    }
    pub fn settings(&self) -> crate::extension::Settings {
        use crate::extension::SettingValue as V;
        let location = self
            .location
            .clone()
            .unwrap_or_else(|| WeatherLocation::new("", 0.0, 0.0));
        [
            ("auto_locate".into(), V::Boolean(self.auto_locate)),
            ("alerts".into(), V::Boolean(self.alerts_enabled)),
            (
                "refresh_minutes".into(),
                V::Integer(self.refresh_minutes.into()),
            ),
            ("location_name".into(), V::String(location.name)),
            ("latitude".into(), V::Float(location.latitude)),
            ("longitude".into(), V::Float(location.longitude)),
        ]
        .into()
    }
    pub fn from_settings(settings: &crate::extension::Settings) -> Self {
        use crate::extension::SettingValue as V;
        let mut prefs = Self::default();
        if let Some(V::Boolean(v)) = settings.get("auto_locate") {
            prefs.auto_locate = *v;
        }
        if let Some(V::Boolean(v)) = settings.get("alerts") {
            prefs.alerts_enabled = *v;
        }
        if let Some(V::Integer(v)) = settings.get("refresh_minutes") {
            prefs.refresh_minutes = *v as u16;
        }
        if let (Some(V::String(name)), Some(V::Float(lat)), Some(V::Float(lon))) = (
            settings.get("location_name"),
            settings.get("latitude"),
            settings.get("longitude"),
        ) {
            if !name.is_empty() {
                prefs.location = Some(WeatherLocation::new(name, *lat, *lon));
            }
        }
        prefs
    }
}

#[cfg(test)]
mod tests {
    use super::{WeatherLocation, WeatherPreferences};

    #[test]
    fn weather_preferences_round_trip_and_reject_invalid_locations() {
        let mut preferences = WeatherPreferences::default();
        assert!(preferences.apply_line("weather_auto", "false"));
        assert!(preferences.apply_line("weather_location", "上海|31.22222|121.45806"));
        assert_eq!(
            preferences.location,
            Some(WeatherLocation::new("上海", 31.22222, 121.45806))
        );
        assert!(preferences.apply_line("weather_refresh_minutes", "24"));
        assert_eq!(preferences.refresh_interval().as_secs(), 24 * 60);
        let encoded = preferences.encode_lines();
        let mut parsed = WeatherPreferences::default();
        for line in &encoded {
            let (key, value) = line.split_once('=').unwrap();
            assert!(parsed.apply_line(key, value));
        }
        assert_eq!(parsed, preferences);
        assert!(!parsed.auto_locate);
        assert!(parsed.apply_line("weather_alerts", "false"));
        assert!(!parsed.alerts_enabled);
        assert!(parsed.apply_line("weather_location", "bad|200|300"));
        assert_eq!(
            parsed.location,
            Some(WeatherLocation::new("上海", 31.22222, 121.45806))
        );
        assert!(!parsed.apply_line("unrelated", "true"));
    }
}
