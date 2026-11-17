use super::model::{HourlyForecast, Snapshot};

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum Severity {
    Yellow,
    Orange,
    Red,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeatherAlert {
    pub id: String,
    pub kind: &'static str,
    pub severity: Severity,
    pub title: String,
    pub body: String,
}

impl WeatherAlert {
    fn new(
        date: &str,
        kind: &'static str,
        severity: Severity,
        location: &str,
        body: String,
    ) -> Self {
        Self {
            id: format!("{date}:{kind}"),
            kind,
            severity,
            title: format!("{}风险提示", kind_title(kind)),
            body: format!("{location}：{body}"),
        }
    }

    pub fn short_title(&self) -> String {
        format!("{}风险", kind_title(self.kind))
    }

    pub fn is_urgent(&self) -> bool {
        self.severity >= Severity::Orange
    }
}

fn kind_title(kind: &str) -> &'static str {
    match kind {
        "rain" => "暴雨",
        "thunder" => "雷电",
        "wind" => "大风",
        "heat" => "高温",
        "cold" => "低温",
        "snow" => "暴雪",
        "fog" => "大雾",
        "uv" => "紫外线",
        _ => "天气",
    }
}

/// Derives locally actionable severe-weather notices from the forecast.
/// This keeps alerts available worldwide without relying on a regional
/// warning feed.
pub fn derive(snapshot: &Snapshot) -> Vec<WeatherAlert> {
    let location = if snapshot.location.name.is_empty() {
        "当前地区"
    } else {
        snapshot.location.name.as_str()
    };
    let Some(today) = snapshot.daily.first() else {
        return Vec::new();
    };
    let date = today.date.as_str();
    let next_24: Vec<&HourlyForecast> = snapshot.hourly_from_now(24);
    let next_12: Vec<&HourlyForecast> = next_24.iter().take(12).copied().collect();
    let rain_12: f64 = next_12.iter().map(|hour| hour.rainfall).sum();
    let rain_24: f64 = next_24.iter().map(|hour| hour.rainfall).sum();
    let gust_24 = next_24
        .iter()
        .map(|hour| hour.wind_gusts)
        .fold(snapshot.current.wind_gusts, f64::max);
    // Sustained wind and cold thresholds follow the defaults from the
    // cited Open-Meteo MCP tool; gust and rain thresholds use separate windows.
    let sustained_wind = next_24
        .iter()
        .map(|hour| hour.wind_speed)
        .fold(snapshot.current.wind_speed, f64::max);
    let visibility = next_12
        .iter()
        .map(|hour| hour.visibility)
        .filter(|value| *value > 0.0)
        .fold(
            if snapshot.current.visibility > 0.0 {
                snapshot.current.visibility
            } else {
                f64::INFINITY
            },
            f64::min,
        );
    let thunder_code = next_12
        .iter()
        .map(|hour| hour.weather_code)
        .chain(std::iter::once(snapshot.current.weather_code))
        .max()
        .unwrap_or_default();

    let mut alerts = Vec::new();

    if rain_12 >= 100.0 {
        alerts.push(WeatherAlert::new(
            date,
            "rain",
            Severity::Red,
            location,
            format!("未来 12 小时累计降雨约 {rain_12:.0} 毫米，请停止户外活动。"),
        ));
    } else if rain_12 >= 50.0 || rain_24 >= 80.0 {
        alerts.push(WeatherAlert::new(
            date,
            "rain",
            Severity::Orange,
            location,
            format!("未来 24 小时累计降雨约 {rain_24:.0} 毫米，低洼路段注意积水。"),
        ));
    } else if rain_12 >= 25.0 {
        alerts.push(WeatherAlert::new(
            date,
            "rain",
            Severity::Yellow,
            location,
            format!("未来 12 小时累计降雨约 {rain_12:.0} 毫米，出门请带伞。"),
        ));
    }

    if matches!(thunder_code, 96 | 99) {
        alerts.push(WeatherAlert::new(
            date,
            "thunder",
            Severity::Orange,
            location,
            "可能出现雷暴并伴有冰雹，请留在室内并远离窗户。".into(),
        ));
    } else if thunder_code == 95 {
        alerts.push(WeatherAlert::new(
            date,
            "thunder",
            Severity::Yellow,
            location,
            "未来 12 小时可能出现雷暴，避免在空旷处停留。".into(),
        ));
    }

    if gust_24 >= 100.0 {
        alerts.push(WeatherAlert::new(
            date,
            "wind",
            Severity::Red,
            location,
            format!("阵风可达 {gust_24:.0} 公里/小时，请尽量避免外出。"),
        ));
    } else if gust_24 >= 75.0 {
        alerts.push(WeatherAlert::new(
            date,
            "wind",
            Severity::Orange,
            location,
            format!("阵风可达 {gust_24:.0} 公里/小时，请远离临时搭建物。"),
        ));
    } else if gust_24 >= 62.0 || sustained_wind >= 50.0 {
        alerts.push(WeatherAlert::new(
            date,
            "wind",
            Severity::Yellow,
            location,
            format!("未来 24 小时风速最高约 {sustained_wind:.0} 公里/小时，阵风约 {gust_24:.0} 公里/小时。"),
        ));
    }

    if let Some((day, high)) = hottest_day(snapshot) {
        if high >= 35.0 {
            let severity = if high >= 40.0 {
                Severity::Red
            } else if high >= 37.0 {
                Severity::Orange
            } else {
                Severity::Yellow
            };
            alerts.push(WeatherAlert::new(
                &day.date,
                "heat",
                severity,
                location,
                format!("{} 最高气温可能达 {high:.0}°C。", day.date),
            ));
        }
    }

    if let Some((day, low)) = coldest_day(snapshot) {
        if low <= -5.0 {
            let severity = if low <= -20.0 {
                Severity::Red
            } else if low <= -10.0 {
                Severity::Orange
            } else {
                Severity::Yellow
            };
            alerts.push(WeatherAlert::new(
                &day.date,
                "cold",
                severity,
                location,
                format!("{} 最低气温可能降至 {low:.0}°C。", day.date),
            ));
        }
    }

    if let Some((day, snow)) = snowiest_day(snapshot) {
        if snow >= 5.0 {
            alerts.push(WeatherAlert::new(
                &day.date,
                "snow",
                if snow >= 10.0 {
                    Severity::Orange
                } else {
                    Severity::Yellow
                },
                location,
                format!("{} 预计降雪约 {snow:.0} 厘米。", day.date),
            ));
        }
    }

    if visibility < 1000.0 {
        let severity = if visibility < 200.0 {
            Severity::Red
        } else if visibility < 500.0 {
            Severity::Orange
        } else {
            Severity::Yellow
        };
        alerts.push(WeatherAlert::new(
            date,
            "fog",
            severity,
            location,
            format!("能见度约 {visibility:.0} 米，驾车请减速慢行。"),
        ));
    }

    if let Some((day, uv)) = highest_uv(snapshot) {
        if uv >= 8.0 {
            alerts.push(WeatherAlert::new(
                &day.date,
                "uv",
                if uv >= 11.0 {
                    Severity::Orange
                } else {
                    Severity::Yellow
                },
                location,
                format!("紫外线指数可达 {uv:.0}，外出请做好防晒。"),
            ));
        }
    }

    alerts.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.title.cmp(&right.title))
    });
    alerts
}

fn hottest_day(snapshot: &Snapshot) -> Option<(&super::model::DailyForecast, f64)> {
    snapshot
        .daily
        .iter()
        .take(2)
        .map(|day| (day, day.high))
        .max_by(|left, right| left.1.total_cmp(&right.1))
}

fn coldest_day(snapshot: &Snapshot) -> Option<(&super::model::DailyForecast, f64)> {
    snapshot
        .daily
        .iter()
        .take(2)
        .map(|day| (day, day.low))
        .min_by(|left, right| left.1.total_cmp(&right.1))
}

fn snowiest_day(snapshot: &Snapshot) -> Option<(&super::model::DailyForecast, f64)> {
    snapshot
        .daily
        .iter()
        .take(2)
        .map(|day| (day, day.snowfall_sum))
        .max_by(|left, right| left.1.total_cmp(&right.1))
}

fn highest_uv(snapshot: &Snapshot) -> Option<(&super::model::DailyForecast, f64)> {
    snapshot
        .daily
        .iter()
        .take(2)
        .map(|day| (day, day.uv_max))
        .max_by(|left, right| left.1.total_cmp(&right.1))
}

#[cfg(test)]
mod tests {
    use super::{derive, Severity};
    use crate::builtins::weather::model::{
        CurrentWeather, DailyForecast, HourlyForecast, Location, Snapshot,
    };

    fn snapshot() -> Snapshot {
        Snapshot {
            location: Location {
                name: "上海".into(),
                ..Location::default()
            },
            timezone: "Asia/Shanghai".into(),
            updated_at: "2026-11-17T17:45".into(),
            current: CurrentWeather {
                weather_code: 3,
                visibility: 10_000.0,
                wind_gusts: 10.0,
                ..CurrentWeather::default()
            },
            daily: vec![DailyForecast {
                date: "2026-11-17".into(),
                high: 20.0,
                low: 10.0,
                sunrise: "06:18".into(),
                sunset: "16:57".into(),
                uv_max: 2.0,
                ..DailyForecast::default()
            }],
            hourly: (0..24)
                .map(|hour| HourlyForecast {
                    time: format!("2026-11-17T{:02}:00", hour.min(23)),
                    temperature: 15.0,
                    rainfall: 0.0,
                    wind_gusts: 10.0,
                    visibility: 10_000.0,
                    weather_code: 3,
                    ..HourlyForecast::default()
                })
                .collect(),
        }
    }

    #[test]
    fn calm_weather_produces_no_alerts() {
        assert!(derive(&snapshot()).is_empty());
    }

    #[test]
    fn heavy_rain_and_wind_upgrade_to_urgent_alerts() {
        let mut snapshot = snapshot();
        for hour in &mut snapshot.hourly {
            hour.rainfall = 9.0;
            hour.wind_gusts = 80.0;
        }
        let alerts = derive(&snapshot);
        let rain = alerts.iter().find(|alert| alert.kind == "rain").unwrap();
        assert_eq!(rain.severity, Severity::Orange);
        assert!(rain.title.contains("暴雨风险"));
        assert!(rain.body.contains("上海"));
        let wind = alerts.iter().find(|alert| alert.kind == "wind").unwrap();
        assert_eq!(wind.severity, Severity::Orange);
        assert_eq!(alerts[0].severity, Severity::Orange);
    }

    #[test]
    fn heat_cold_fog_and_uv_thresholds_become_notices() {
        let mut snapshot = snapshot();
        snapshot.daily[0].high = 41.0;
        snapshot.daily[0].low = -12.0;
        snapshot.daily[0].uv_max = 9.0;
        snapshot.current.visibility = 450.0;
        for hour in &mut snapshot.hourly {
            hour.visibility = 450.0;
        }
        let alerts = derive(&snapshot);
        assert_eq!(
            alerts
                .iter()
                .find(|alert| alert.kind == "heat")
                .unwrap()
                .severity,
            Severity::Red
        );
        assert_eq!(
            alerts
                .iter()
                .find(|alert| alert.kind == "cold")
                .unwrap()
                .severity,
            Severity::Orange
        );
        assert_eq!(
            alerts
                .iter()
                .find(|alert| alert.kind == "fog")
                .unwrap()
                .severity,
            Severity::Orange
        );
        assert_eq!(
            alerts
                .iter()
                .find(|alert| alert.kind == "uv")
                .unwrap()
                .severity,
            Severity::Yellow
        );
    }
}
