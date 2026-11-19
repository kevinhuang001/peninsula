use std::time::Duration;

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

    pub fn selected_location(&self) -> Option<&WeatherLocation> {
        self.location.as_ref()
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
