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
    pub location: Option<WeatherLocation>,
}

impl Default for WeatherPreferences {
    fn default() -> Self {
        Self {
            auto_locate: true,
            alerts_enabled: true,
            location: None,
        }
    }
}

impl WeatherPreferences {
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
            "weather_location" => {
                if let Some(location) = parse_location(value) {
                    self.location = Some(location);
                }
                true
            }
            _ => false,
        }
    }

    pub fn encode_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("weather_auto={}", self.auto_locate),
            format!("weather_alerts={}", self.alerts_enabled),
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

#[cfg(test)]
mod tests {
    use super::{WeatherLocation, WeatherPreferences};

    #[test]
    fn weather_preferences_round_trip_and_reject_invalid_locations() {
        let mut preferences = WeatherPreferences::default();
        assert!(preferences.apply_line("weather_auto", "false"));
        assert!(preferences.apply_line(
            "weather_location",
            "上海|31.22222|121.45806"
        ));
        assert_eq!(
            preferences.location,
            Some(WeatherLocation::new("上海", 31.22222, 121.45806))
        );
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
