use super::{
    alerts::{self, WeatherAlert},
    client,
    model::{
        time_hm, uv_level, weather_description, wind_direction, wind_level, Location, Snapshot,
    },
    preferences::WeatherPreferences,
};
use crate::{
    extension::{
        Action, CompactView, ExtensionContext, ExtensionDescriptor, ExtensionId, ExtensionKind,
        ExtensionOutput, Notification, NotificationAlertView, NotificationCenterItemView,
        NotificationPriority, RuntimeExtension,
    },
    ui::declarative::*,
};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant, SystemTime},
};

pub const WIDGET_ID: &str = "builtin.weather/forecast";
pub const COMPACT_ID: &str = "builtin.weather/current";
pub const ALERTS_ID: &str = "builtin.weather/alerts";

const RETRY_INTERVAL: Duration = Duration::from_secs(60);
const WIDGET_SIZE: [f32; 2] = [506.0, 456.0];
const COMPACT_WIDTH: f32 = 68.0;
const HOURLY_COUNT: usize = 5;

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(
            WIDGET_ID,
            ExtensionKind::Widget,
            "天气",
            "cloud-sun",
            true,
            4,
        )
        .with_size_hint(WIDGET_SIZE[0], WIDGET_SIZE[1])
        .with_widget_bounds(WidgetBounds {
            fit_height: true,
            overflow: Overflow::Scroll(ScrollAxes::Vertical),
            ..WidgetBounds::default()
        })
        .with_capabilities(vec![
            crate::extension::Capability::View,
            crate::extension::Capability::Action,
            crate::extension::Capability::Event,
        ]),
        ExtensionDescriptor::builtin(
            COMPACT_ID,
            ExtensionKind::Compact,
            "天气",
            "cloud-sun",
            true,
            8,
        )
        .with_size_hint(COMPACT_WIDTH, 28.0),
        ExtensionDescriptor::builtin(
            ALERTS_ID,
            ExtensionKind::Notification,
            "天气风险提示",
            "triangle-alert",
            true,
            2,
        )
        .with_capabilities(vec![
            crate::extension::Capability::Event,
            crate::extension::Capability::Action,
        ]),
    ]
}

pub struct WeatherExtension {
    preferences: WeatherPreferences,
    snapshot: Option<Snapshot>,
    alerts: Vec<WeatherAlert>,
    error: Option<String>,
    pending: Option<Receiver<Result<Snapshot, String>>>,
    auto_location: Option<Location>,
    published_alerts: HashMap<String, alerts::Severity>,
    alert_history_path: Option<PathBuf>,
    next_refresh: Instant,
    last_success: Option<Instant>,
    forecast_hour: Option<i64>,
}

impl WeatherExtension {
    pub fn new(preferences: WeatherPreferences) -> Self {
        Self {
            preferences,
            snapshot: None,
            alerts: Vec::new(),
            error: None,
            pending: None,
            auto_location: None,
            published_alerts: HashMap::new(),
            alert_history_path: None,
            next_refresh: Instant::now(),
            last_success: None,
            forecast_hour: None,
        }
    }

    pub fn set_preferences(&mut self, preferences: &WeatherPreferences) -> bool {
        if self.preferences == *preferences {
            return false;
        }
        let location_changed = self.preferences.auto_locate != preferences.auto_locate
            || self.preferences.location != preferences.location;
        let alerts_enabled = !self.preferences.alerts_enabled && preferences.alerts_enabled;
        let interval_changed = self.preferences.refresh_minutes != preferences.refresh_minutes;
        self.preferences = preferences.clone();
        if location_changed {
            self.snapshot = None;
            self.alerts.clear();
            self.error = None;
            self.auto_location = None;
            self.pending = None;
            self.last_success = None;
            self.next_refresh = Instant::now();
        } else if alerts_enabled && self.snapshot.is_some() {
            self.next_refresh = Instant::now();
        } else if interval_changed && self.error.is_none() {
            if let Some(last_success) = self.last_success {
                self.next_refresh = last_success + self.preferences.refresh_interval();
            }
        }
        true
    }

    fn start_fetch(&mut self) {
        if self.pending.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let preferences = self.preferences.clone();
        let cached_auto_location = self.auto_location.clone();
        self.pending = Some(receiver);
        std::thread::spawn(move || {
            let result = client::fetch(&preferences, cached_auto_location.as_ref());
            let _ = sender.send(result);
        });
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot, now: Instant, output: &mut ExtensionOutput) {
        self.auto_location = self
            .preferences
            .auto_locate
            .then(|| snapshot.location.clone());
        let derived = alerts::derive(&snapshot);
        if self.preferences.alerts_enabled {
            if let Some(today) = snapshot.today() {
                self.published_alerts.retain(|id, _| {
                    id.split_once(':')
                        .is_some_and(|(date, _)| date >= today.date.as_str())
                });
            }
            let mut published = false;
            for alert in &derived {
                let key = format!(
                    "{}:{:.2}:{:.2}",
                    alert.id, snapshot.location.latitude, snapshot.location.longitude
                );
                let should_publish = self
                    .published_alerts
                    .get(&key)
                    .is_none_or(|severity| alert.severity > *severity);
                if should_publish {
                    self.published_alerts.insert(key, alert.severity);
                    output
                        .notifications
                        .push(alert_notification(alert, &snapshot.location));
                    published = true;
                }
            }
            if published {
                if let Some(path) = &self.alert_history_path {
                    if let Err(error) = save_alert_history(path, &self.published_alerts) {
                        eprintln!("Unable to save weather alert history: {error}");
                    }
                }
            }
        }
        self.alerts = derived;
        self.snapshot = Some(snapshot);
        self.error = None;
        self.last_success = Some(now);
        self.next_refresh = now + self.preferences.refresh_interval();
    }

    fn poll_fetch(&mut self, now: Instant, output: &mut ExtensionOutput) -> bool {
        let Some(receiver) = &self.pending else {
            return false;
        };
        match receiver.try_recv() {
            Ok(Ok(snapshot)) => {
                self.pending = None;
                self.apply_snapshot(snapshot, now, output);
                true
            }
            Ok(Err(error)) => {
                self.pending = None;
                self.error = Some(error);
                self.next_refresh = now + RETRY_INTERVAL;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                self.error = Some("天气服务意外结束，请稍后重试".into());
                self.next_refresh = now + RETRY_INTERVAL;
                true
            }
        }
    }

    fn root(&self) -> ViewNode {
        if let Some(snapshot) = &self.snapshot {
            weather_root(
                snapshot,
                &self.alerts,
                self.error.as_deref(),
                time::OffsetDateTime::now_utc(),
            )
        } else if let Some(error) = &self.error {
            error_root(error)
        } else {
            loading_root(self.preferences.auto_locate)
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        snapshot: Option<Snapshot>,
        alerts: Vec<WeatherAlert>,
        error: Option<String>,
    ) -> Self {
        Self {
            preferences: WeatherPreferences::default(),
            snapshot,
            alerts,
            error,
            pending: None,
            auto_location: None,
            published_alerts: HashMap::new(),
            alert_history_path: None,
            next_refresh: Instant::now() + Duration::from_secs(3600),
            last_success: None,
            forecast_hour: None,
        }
    }
}

impl crate::extension::NotificationPresentationProvider for WeatherExtension {
    fn alert_view(&self, notification: &Notification) -> NotificationAlertView {
        NotificationAlertView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("triangle-alert".into()),
            actions: notification.actions.clone(),
        }
    }

    fn center_item_view(&self, notification: &Notification) -> NotificationCenterItemView {
        NotificationCenterItemView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("triangle-alert".into()),
        }
    }

    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == ALERTS_ID
    }
}

impl RuntimeExtension for WeatherExtension {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        descriptors()
    }

    fn on_start(&mut self, context: &ExtensionContext) -> Result<(), String> {
        let path = context
            .extension_data_dir(&ExtensionId::builtin(ALERTS_ID))
            .join("published.json");
        if let Ok(file) = fs::File::open(&path) {
            self.published_alerts =
                serde_json::from_reader(io::BufReader::new(file)).unwrap_or_default();
        }
        self.alert_history_path = Some(path);
        Ok(())
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut output = ExtensionOutput::default();
        let changed = self.poll_fetch(now, &mut output);
        if self.pending.is_none() && now >= self.next_refresh {
            self.start_fetch();
        }
        if changed {
            output.invalidated = vec![
                ExtensionId::builtin(WIDGET_ID),
                ExtensionId::builtin(COMPACT_ID),
            ];
        }
        let forecast_hour = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.utc_offset)
            .map(|offset| {
                (time::OffsetDateTime::now_utc().unix_timestamp()
                    + i64::from(offset.whole_seconds()))
                .div_euclid(3600)
            });
        if std::mem::replace(&mut self.forecast_hour, forecast_hour) != forecast_hour && !changed {
            output.invalidated.push(ExtensionId::builtin(WIDGET_ID));
        }
        output
    }

    fn on_action(&mut self, action: Action, now: Instant) -> ExtensionOutput {
        if action != Action::Advance {
            return ExtensionOutput::default();
        }
        self.error = None;
        self.next_refresh = now;
        self.start_fetch();
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            ..ExtensionOutput::default()
        }
    }

    fn widget_view(&self, widget_id: &ExtensionId) -> Option<WidgetView> {
        if widget_id.as_str() != WIDGET_ID {
            return None;
        }
        Some(WidgetView::new(self.root(), WIDGET_SIZE))
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let (icon, label, color, tooltip) = if let Some(snapshot) = &self.snapshot {
            let mut color = if self.alerts.iter().any(WeatherAlert::is_urgent) {
                ColorSpec::Warning
            } else {
                ColorSpec::PrimaryText
            };
            if self.error.is_some() {
                color = ColorSpec::MutedText;
            }
            let today = snapshot.today();
            let tooltip = today.map_or_else(
                || format!("{} {}", snapshot.location.name, snapshot.condition()),
                |today| {
                    format!(
                        "{} {} · 体感 {:.0}° · 最高 {:.0}° / 最低 {:.0}°",
                        snapshot.location.label(),
                        snapshot.condition(),
                        snapshot.current.apparent_temperature,
                        today.high,
                        today.low
                    )
                },
            );
            (
                snapshot.icon(),
                temperature_label(snapshot.current.temperature),
                color,
                tooltip,
            )
        } else if let Some(error) = &self.error {
            (
                "meteo/not-available",
                "--°".to_owned(),
                ColorSpec::MutedText,
                error.clone(),
            )
        } else {
            (
                "meteo/partly-cloudy-day",
                "--°".to_owned(),
                ColorSpec::MutedText,
                "正在获取天气…".to_owned(),
            )
        };
        vec![CompactView {
            id: ExtensionId::builtin(COMPACT_ID),
            source_extension_id: ExtensionId::builtin(WIDGET_ID),
            content: ViewNode::row(
                vec![weather_icon(icon, 25.0), text(label, 11.0, color)],
                Flow {
                    gap: Some(5.0),
                    ..Default::default()
                },
            )
            .color(color),
            visibility: Default::default(),
            min_width: COMPACT_WIDTH,
            preferred_width: COMPACT_WIDTH,
            priority: 0,
            tooltip: Some(tooltip),
        }]
    }
}

fn save_alert_history(path: &Path, alerts: &HashMap<String, alerts::Severity>) -> io::Result<()> {
    let directory = path
        .parent()
        .expect("weather history is in the extension data directory");
    fs::create_dir_all(directory)?;
    let file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(file.as_file(), alerts)?;
    file.persist(path)?;
    Ok(())
}

fn alert_notification(alert: &WeatherAlert, location: &Location) -> Notification {
    Notification {
        source_id: ExtensionId::builtin(ALERTS_ID),
        id: format!(
            "forecast:{:.2}:{:.2}:{}:{}",
            location.latitude, location.longitude, alert.id, alert.severity as u8
        ),
        title: alert.title.clone(),
        body: format!("{}\n来源：Open-Meteo 预报推算", alert.body),
        icon: Some("triangle-alert".into()),
        priority: if alert.is_urgent() {
            NotificationPriority::High
        } else {
            NotificationPriority::Normal
        },
        actions: Vec::new(),
        created_at: SystemTime::now(),
    }
}

fn weather_root(
    snapshot: &Snapshot,
    alerts: &[WeatherAlert],
    error: Option<&str>,
    now: time::OffsetDateTime,
) -> ViewNode {
    let current = &snapshot.current;
    let today = snapshot.today();
    let current_row = ViewNode::row(
        vec![
            column(
                vec![
                    ViewNode::row(
                        vec![
                            ViewNode::icon("map-pin", 13.0).color(ColorSpec::Info),
                            strong(&snapshot.location.name, 13.0),
                        ],
                        flow(4.0, Align::Center),
                    ),
                    ViewNode::row(
                        vec![
                            strong(temperature_label(current.temperature), 40.0),
                            text(snapshot.condition(), 14.0, ColorSpec::SecondaryText),
                        ],
                        flow(9.0, Align::Center),
                    ),
                ],
                3.0,
            )
            .sized(Size::Fill(1.0), Size::Auto),
            ViewNode::column(
                vec![
                    weather_icon(snapshot.icon(), 78.0),
                    text(
                        format!("{} 更新", snapshot.updated_hm()),
                        9.5,
                        ColorSpec::SecondaryText,
                    ),
                ],
                flow(0.0, Align::Center),
            ),
        ],
        flow(10.0, Align::Center),
    )
    .sized(Size::Fill(1.0), Size::Auto);
    let mut temperatures = vec![temperature_stat("体感温度", current.apparent_temperature)];
    if let Some(day) = today {
        temperatures.push(temperature_stat("今日最高", day.high));
        temperatures.push(temperature_stat("今日最低", day.low));
    }
    let hero = panel(
        column(
            vec![
                current_row,
                ViewNode::new(Node::Divider { vertical: false })
                    .sized(Size::Fill(1.0), Size::Fixed(1.0))
                    .color(ColorSpec::Border),
                ViewNode::row(temperatures, flow(8.0, Align::Center))
                    .sized(Size::Fill(1.0), Size::Auto),
            ],
            8.0,
        ),
        ColorSpec::AccentSubtle,
        [13.0, 8.0, 13.0, 11.0],
    );

    let mut sections = vec![hero];
    if error.is_some() {
        sections.push(text("暂未更新 · 显示上次预报", 11.0, ColorSpec::Warning));
    } else if let Some(alert) = alerts.first() {
        let extra = if alerts.len() > 1 {
            format!("等 {} 项", alerts.len())
        } else {
            String::new()
        };
        sections.push(panel(
            ViewNode::row(
                vec![
                    ViewNode::icon("triangle-alert", 14.0).color(ColorSpec::Warning),
                    text(
                        format!("{}{}", alert.short_title(), extra),
                        11.0,
                        ColorSpec::Warning,
                    ),
                ],
                flow(7.0, Align::Center),
            ),
            ColorSpec::SurfaceVariant,
            [9.0, 6.0, 9.0, 6.0],
        ));
    }
    sections.push(hourly_chart(snapshot, now));
    // Six cards, one three-column grid: the sun block fills the first row and
    // humidity/wind/rain the second, so the section stays exactly two rows.
    let mut metrics = Vec::with_capacity(6);
    if let Some(day) = today {
        metrics.push(metric(
            "sunrise",
            "日出",
            day.sunrise.clone(),
            ColorSpec::Warning,
        ));
        metrics.push(metric(
            "sunset",
            "日落",
            day.sunset.clone(),
            ColorSpec::Warning,
        ));
    }
    metrics.push(metric(
        "sun",
        "紫外线",
        format!("{:.0} · {}", current.uv_index, uv_level(current.uv_index)),
        ColorSpec::Info,
    ));
    metrics.push(metric(
        "droplet",
        "湿度",
        format!("{}%", current.humidity),
        ColorSpec::Info,
    ));
    metrics.push(metric(
        "wind",
        &format!(
            "{}风 · {}级",
            wind_direction(current.wind_direction),
            wind_level(current.wind_speed)
        ),
        format!("{:.0} km/h", current.wind_speed),
        ColorSpec::Info,
    ));
    metrics.push(metric(
        "cloud-rain",
        "降水概率",
        format!("{}%", current.precipitation_probability),
        ColorSpec::Info,
    ));
    sections.push(grid(metrics));

    let heading = ViewNode::row(
        vec![
            strong(format!("未来 {} 天", snapshot.daily.len()), 12.0),
            text("最高温  /  最低温", 10.0, ColorSpec::SecondaryText),
        ],
        Flow {
            justify: Justify::SpaceBetween,
            ..flow(5.0, Align::Center)
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    let chart = ViewNode::new(Node::RangeChart {
        labels: snapshot.day_labels(),
        high: snapshot.daily.iter().map(|day| day.high as f32).collect(),
        low: snapshot.daily.iter().map(|day| day.low as f32).collect(),
        unit: "°".into(),
    })
    .sized(Size::Fill(1.0), Size::Fixed(92.0));
    sections.push(panel(
        column(vec![heading, chart], 3.0).sized(Size::Fill(1.0), Size::Auto),
        ColorSpec::SurfaceVariant,
        [11.0, 9.0, 11.0, 3.0],
    ));
    sections.push(text(
        "Open-Meteo 预报 · 风险提示由模型数据推算",
        9.0,
        ColorSpec::MutedText,
    ));
    column(sections, 7.0).sized(Size::Fill(1.0), Size::Auto)
}

fn hourly_chart(snapshot: &Snapshot, now: time::OffsetDateTime) -> ViewNode {
    let points: Vec<_> = snapshot
        .upcoming_hours(now, HOURLY_COUNT)
        .into_iter()
        .map(|hour| {
            let label = time_hm(&hour.time);
            let asset = snapshot.hourly_icon(hour);
            let condition = hour
                .weather_code
                .filter(|_| asset != "meteo/not-available")
                .map_or("暂无天气信息", weather_description);
            LineChartPoint {
                tooltip: Some(format!("{label} · {condition}")),
                label,
                value: hour.temperature.map(|value| value as f32),
                image: Some(asset.into()),
            }
        })
        .collect();
    let chart = if points.is_empty() {
        text("暂无逐小时预报", 11.0, ColorSpec::SecondaryText)
    } else {
        ViewNode::new(Node::LineChart {
            points,
            unit: "°".into(),
        })
        .sized(Size::Fill(1.0), Size::Fixed(104.0))
        .color(ColorSpec::Info)
    };
    panel(
        column(
            vec![strong(format!("未来 {HOURLY_COUNT} 小时"), 12.0), chart],
            3.0,
        ),
        ColorSpec::SurfaceVariant,
        [11.0, 9.0, 11.0, 5.0],
    )
}

fn weather_icon(asset: &str, size: f32) -> ViewNode {
    ViewNode::new(Node::Image {
        asset: asset.into(),
    })
    .sized(Size::Fixed(size), Size::Fixed(size))
}

fn temperature_label(value: f64) -> String {
    let rounded = value.round();
    if rounded < 0.0 {
        format!("−{:.0}°", rounded.abs())
    } else {
        format!("{:.0}°", rounded.abs())
    }
}

fn temperature_stat(label: &str, value: f64) -> ViewNode {
    column(
        vec![
            text(label, 10.0, ColorSpec::SecondaryText),
            strong(temperature_label(value), 15.0),
        ],
        2.0,
    )
    .sized(Size::Fill(1.0), Size::Auto)
}

fn flow(gap: f32, align: Align) -> Flow {
    Flow {
        gap: Some(gap),
        align,
        ..Default::default()
    }
}

fn column(children: Vec<ViewNode>, gap: f32) -> ViewNode {
    ViewNode::column(children, flow(gap, Align::Start))
}

fn text(value: impl Into<String>, size: f32, color: ColorSpec) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Custom(size),
            align: Align::Start,
            ..Default::default()
        },
    )
    .color(color)
}

fn strong(value: impl Into<String>, size: f32) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            font_size: FontSize::Custom(size),
            font_weight: FontWeight::Bold,
            align: Align::Start,
            ..Default::default()
        },
    )
}

fn panel(mut node: ViewNode, fill: ColorSpec, padding: [f32; 4]) -> ViewNode {
    node.style = Style {
        normal: Paint {
            background: Some(fill),
            corner_radius: Some(10.0),
            ..Default::default()
        },
        padding,
        ..Default::default()
    };
    node.sized(Size::Fill(1.0), Size::Auto)
}

fn metric(icon: &str, label: &str, value: String, accent: ColorSpec) -> ViewNode {
    panel(
        ViewNode::row(
            vec![
                ViewNode::icon(icon, 15.0).color(accent),
                column(
                    vec![
                        text(label, 9.5, ColorSpec::SecondaryText),
                        strong(value, 12.0),
                    ],
                    2.0,
                )
                .sized(Size::Fill(1.0), Size::Auto),
            ],
            flow(7.0, Align::Center),
        ),
        ColorSpec::SurfaceVariant,
        [9.0, 6.0, 9.0, 6.0],
    )
}

fn grid(children: Vec<ViewNode>) -> ViewNode {
    let mut grid = Grid::new(Columns::AutoFit {
        min_item_width: 102.0,
        max_columns: 3,
    });
    grid.column_gap = Some(6.0);
    grid.row_gap = Some(6.0);
    ViewNode::new(Node::Grid(grid, children)).sized(Size::Fill(1.0), Size::Auto)
}

fn loading_root(auto_locate: bool) -> ViewNode {
    column(
        vec![
            strong("正在获取天气…", 15.0),
            text(
                if auto_locate {
                    "正在根据网络位置定位"
                } else {
                    "正在查询所选城市"
                },
                11.0,
                ColorSpec::SecondaryText,
            ),
        ],
        6.0,
    )
}

fn error_root(error: &str) -> ViewNode {
    column(
        vec![
            strong("天气暂不可用", 15.0),
            ViewNode::text(
                error,
                TextStyle {
                    wrap: true,
                    align: Align::Start,
                    ..Default::default()
                },
            )
            .color(ColorSpec::SecondaryText)
            .sized(Size::Fill(1.0), Size::Auto),
            ViewNode::subtle_button("重试", Action::Advance),
        ],
        7.0,
    )
    .sized(Size::Fill(1.0), Size::Auto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::weather::model::{CurrentWeather, DailyForecast, HourlyForecast};

    fn snapshot() -> Snapshot {
        Snapshot {
            location: Location {
                name: "上海".into(),
                admin: "上海市".into(),
                country: "中国".into(),
                latitude: 31.22,
                longitude: 121.46,
            },
            timezone: "Asia/Shanghai".into(),
            utc_offset: None,
            updated_at: "2026-11-17T17:45".into(),
            current: CurrentWeather {
                temperature: 14.9,
                apparent_temperature: 13.8,
                humidity: 71,
                precipitation_probability: 20,
                weather_code: 3,
                is_day: false,
                cloud_cover: 89,
                wind_speed: 6.8,
                wind_gusts: 14.8,
                wind_direction: 358,
                uv_index: 0.0,
                visibility: 22_320.0,
                ..CurrentWeather::default()
            },
            daily: vec![DailyForecast {
                date: "2026-11-17".into(),
                weather_code: 3,
                high: 18.2,
                low: 11.1,
                sunrise: "06:18".into(),
                sunset: "16:57".into(),
                uv_max: 2.4,
                ..DailyForecast::default()
            }],
            hourly: vec![HourlyForecast {
                time: "2026-11-17T18:00".into(),
                temperature: Some(14.6),
                weather_code: Some(2),
                is_day: Some(false),
                ..HourlyForecast::default()
            }],
        }
    }

    #[test]
    fn descriptors_register_a_widget_a_compact_item_and_an_alert_source() {
        let descriptors = descriptors();
        assert_eq!(descriptors.len(), 3);
        assert!(descriptors.iter().any(|descriptor| {
            descriptor.id.as_str() == WIDGET_ID && descriptor.kind == ExtensionKind::Widget
        }));
        assert!(descriptors.iter().any(|descriptor| {
            descriptor.id.as_str() == COMPACT_ID && descriptor.kind == ExtensionKind::Compact
        }));
        assert!(descriptors.iter().any(|descriptor| {
            descriptor.id.as_str() == ALERTS_ID && descriptor.kind == ExtensionKind::Notification
        }));
        assert_eq!(lookup_shape(&descriptors, WIDGET_ID), Some(WIDGET_SIZE));
    }

    fn lookup_shape(descriptors: &[ExtensionDescriptor], id: &str) -> Option<[f32; 2]> {
        descriptors
            .iter()
            .find(|descriptor| descriptor.id.as_str() == id)
            .map(|descriptor| descriptor.size_hint)
    }

    #[test]
    fn compact_view_uses_a_weather_icon_and_the_temperature_text() {
        let extension = WeatherExtension::for_test(Some(snapshot()), Vec::new(), None);
        let views = extension.compact_views();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id.as_str(), COMPACT_ID);
        let Node::Row(_, children) = &views[0].content.kind else {
            panic!("weather compact row");
        };
        assert!(matches!(&children[0].kind, Node::Image { asset } if asset == "meteo/overcast"));
        assert!(matches!(&children[1].kind, Node::Text { text, .. } if text == "15°"));
        assert!(views[0]
            .tooltip
            .as_deref()
            .is_some_and(|tooltip| tooltip.contains("体感 14°")));
    }

    #[test]
    fn widget_contains_current_feels_like_sun_times_stats_and_seven_day_chart() {
        let extension = WeatherExtension::for_test(Some(snapshot()), Vec::new(), None);
        let view = extension
            .widget_view(&crate::extension::ExtensionId::builtin(WIDGET_ID))
            .unwrap();
        let tree = serde_json::to_string(&view.root).unwrap();
        for visible in [
            "上海",
            "体感温度",
            "14°",
            "今日最高",
            "今日最低",
            "日出",
            "06:18",
            "日落",
            "16:57",
            "未来 5 小时",
            "LineChart",
            "18:00",
            "meteo/partly-cloudy-night",
            "RangeChart",
        ] {
            assert!(tree.contains(visible), "missing {visible}");
        }
        for removed in ["日照时长", "气压", "能见度"] {
            assert!(!tree.contains(removed), "unexpected {removed}");
        }
    }

    #[test]
    fn risk_notices_only_repeat_when_the_forecast_worsens() {
        let mut extension = WeatherExtension::new(WeatherPreferences::default());
        let mut data = snapshot();
        let now = Instant::now();
        let mut output = ExtensionOutput::default();
        extension.apply_snapshot(data.clone(), now, &mut output);
        assert!(output.notifications.is_empty());
        data.current.wind_gusts = 90.0;
        extension.apply_snapshot(data.clone(), now, &mut output);
        assert_eq!(output.notifications.len(), 1);
        assert!(output.notifications[0].body.contains("Open-Meteo"));
        extension.apply_snapshot(data.clone(), now, &mut output);
        assert_eq!(output.notifications.len(), 1);
        data.current.wind_gusts = 130.0;
        extension.apply_snapshot(data, now, &mut output);
        assert_eq!(output.notifications.len(), 2);
        assert_ne!(output.notifications[0].id, output.notifications[1].id);
        assert_eq!(
            extension.compact_views()[0].content.style.normal.foreground,
            Some(ColorSpec::Warning)
        );
    }

    #[test]
    fn editing_refresh_minutes_reschedules_the_next_forecast() {
        let mut extension = WeatherExtension::new(WeatherPreferences::default());
        let fetched = Instant::now();
        extension.apply_snapshot(snapshot(), fetched, &mut ExtensionOutput::default());
        assert_eq!(extension.next_refresh, fetched + Duration::from_secs(600));
        let preferences = WeatherPreferences {
            refresh_minutes: 45,
            ..extension.preferences.clone()
        };
        extension.set_preferences(&preferences);
        assert_eq!(extension.next_refresh, fetched + Duration::from_secs(2700));
    }

    #[test]
    fn weather_preferences_are_applied_without_coupling_the_host_to_weather_data() {
        let mut extension = WeatherExtension::new(WeatherPreferences::default());
        let mut preferences = WeatherPreferences::default();
        preferences.alerts_enabled = false;
        preferences.location = Some(crate::builtins::weather::preferences::WeatherLocation::new(
            "上海", 31.22, 121.46,
        ));
        preferences.auto_locate = false;
        extension.set_preferences(&preferences);
        assert_eq!(extension.preferences, preferences);
        let Node::Column(..) = extension.root().kind else {
            panic!("loading state should render as a column");
        };
    }
}
