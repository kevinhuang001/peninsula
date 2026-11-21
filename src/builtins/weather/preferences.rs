use std::time::Duration;

const DEFAULT_REFRESH_MINUTES: u16 = 10;
pub(super) const CITY_SEARCH_PROVIDER: &str = "weather.open-meteo-city";

#[derive(Clone, Debug, PartialEq)]
pub struct WeatherLocation {
    pub name: String,
    pub detail: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl WeatherLocation {
    pub fn new(name: impl Into<String>, latitude: f64, longitude: f64) -> Self {
        Self {
            name: name.into(),
            detail: String::new(),
            latitude,
            longitude,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeatherPreferences {
    pub auto_locate: bool,
    pub refresh_minutes: u16,
    pub location: Option<WeatherLocation>,
}

impl Default for WeatherPreferences {
    fn default() -> Self {
        Self {
            auto_locate: true,
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
            S::new("refresh_minutes", V::Integer(10), "刷新间隔（分钟）").range(1.0, 180.0),
            S::search(
                "location",
                "手动天气城市（关闭自动定位后使用）",
                CITY_SEARCH_PROVIDER,
            )
            .search_placeholder("搜索城市名称", 2),
        ]
    }
    pub fn from_settings(settings: &crate::extension::Settings) -> Self {
        use crate::extension::SettingValue as V;
        let mut prefs = Self::default();
        if let Some(V::Boolean(v)) = settings.get("auto_locate") {
            prefs.auto_locate = *v;
        }
        if let Some(V::Integer(v)) = settings.get("refresh_minutes") {
            prefs.refresh_minutes = *v as u16;
        }
        if let Some(V::Search(Some(selection))) = settings.get("location") {
            if let (Some(lat), Some(lon)) = (
                selection
                    .data
                    .get("latitude")
                    .and_then(|value| value.parse().ok()),
                selection
                    .data
                    .get("longitude")
                    .and_then(|value| value.parse().ok()),
            ) {
                if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
                    prefs.location = Some(
                        WeatherLocation::new(&selection.label, lat, lon)
                            .with_detail(selection.detail.clone()),
                    );
                }
            }
        }
        prefs
    }
}
