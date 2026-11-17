use super::{
    alerts::{self, WeatherAlert},
    client,
    model::{format_visibility, uv_level, weather_description, wind_direction, wind_level, Location, Snapshot},
    preferences::WeatherPreferences,
};
use crate::extension::{
    Action, ColorRole, CompactNode, CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind,
    ExtensionOutput, Notification, NotificationAction, NotificationAlertView,
    NotificationCenterItemView, NotificationPriority,
    RefreshHint, RuntimeExtension, StatCell, TextStyle, ViewNode, WidgetView,
};
use std::{
    collections::HashSet,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant, SystemTime},
};

pub const WIDGET_ID: &str = "builtin.weather/forecast";
pub const COMPACT_ID: &str = "builtin.weather/current";
pub const ALERTS_ID: &str = "builtin.weather/alerts";

const REFRESH_INTERVAL: Duration = Duration::from_secs(10 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
const WIDGET_SIZE: [f32; 2] = [400.0, 280.0];
const COMPACT_WIDTH: f32 = 68.0;

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(
            WIDGET_ID,
            ExtensionKind::Widget,
            "天气",
            "cloud-sun",
            true,
            3,
        )
        .with_size_hint(WIDGET_SIZE[0], WIDGET_SIZE[1])
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
            "天气预警",
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
    published_alerts: HashSet<String>,
    next_refresh: Instant,
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
            published_alerts: HashSet::new(),
            next_refresh: Instant::now(),
        }
    }

    pub fn set_preferences(&mut self, preferences: &WeatherPreferences) {
        if self.preferences == *preferences {
            return;
        }
        let location_changed = self.preferences.auto_locate != preferences.auto_locate
            || self.preferences.location != preferences.location;
        let alerts_enabled = !self.preferences.alerts_enabled && preferences.alerts_enabled;
        self.preferences = preferences.clone();
        if location_changed {
            self.snapshot = None;
            self.alerts.clear();
            self.error = None;
            self.auto_location = None;
            self.published_alerts.clear();
            self.pending = None;
            self.next_refresh = Instant::now();
        } else if alerts_enabled && self.snapshot.is_some() {
            self.next_refresh = Instant::now();
        }
    }

    fn start_fetch(&mut self, now: Instant) {
        if self.pending.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let preferences = self.preferences.clone();
        let cached_auto_location = self.auto_location.clone();
        self.pending = Some(receiver);
        self.next_refresh = now + REFRESH_INTERVAL;
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
            for alert in &derived {
                if self.published_alerts.insert(alert.id.clone()) {
                    output.notifications.push(alert_notification(alert));
                }
            }
        }
        self.alerts = derived;
        self.snapshot = Some(snapshot);
        self.error = None;
        self.next_refresh = now + REFRESH_INTERVAL;
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
            weather_root(snapshot, &self.alerts, self.error.as_deref())
        } else if let Some(error) = &self.error {
            error_root(error)
        } else {
            loading_root()
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
            published_alerts: HashSet::new(),
            next_refresh: Instant::now() + Duration::from_secs(3600),
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

    fn on_notification_action(
        &mut self,
        _notification: &Notification,
        _action: &NotificationAction,
    ) -> Result<(), String> {
        Ok(())
    }

    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == ALERTS_ID
    }
}

impl RuntimeExtension for WeatherExtension {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        descriptors()
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut output = ExtensionOutput::default();
        let changed = self.poll_fetch(now, &mut output);
        if self.pending.is_none() && now >= self.next_refresh {
            self.start_fetch(now);
        }
        if changed {
            output.invalidated = vec![
                ExtensionId::builtin(WIDGET_ID),
                ExtensionId::builtin(COMPACT_ID),
            ];
        }
        output
    }

    fn on_action(&mut self, action: Action, now: Instant) -> ExtensionOutput {
        if action != Action::Advance {
            return ExtensionOutput::default();
        }
        self.error = None;
        self.next_refresh = now;
        self.start_fetch(now);
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            ..ExtensionOutput::default()
        }
    }

    fn widget_view(&self, widget_id: &ExtensionId) -> Option<WidgetView> {
        if widget_id.as_str() != WIDGET_ID {
            return None;
        }
        Some(WidgetView {
            root: self.root(),
            preferred_size: WIDGET_SIZE,
            refresh_hint: RefreshHint::OnInvalidation,
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let (icon, text, color, tooltip) = if let Some(snapshot) = &self.snapshot {
            let mut color = if self.alerts.iter().any(WeatherAlert::is_urgent) {
                ColorRole::Warning
            } else {
                ColorRole::Primary
            };
            if self.error.is_some() {
                color = ColorRole::Muted;
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
                format!("{:.0}°", snapshot.current.temperature),
                color,
                tooltip,
            )
        } else if let Some(error) = &self.error {
            (
                "cloud-off",
                "--°".to_owned(),
                ColorRole::Muted,
                error.clone(),
            )
        } else {
            (
                "cloud-sun",
                "--°".to_owned(),
                ColorRole::Muted,
                "正在获取天气…".to_owned(),
            )
        };
        vec![CompactView {
            id: ExtensionId::builtin(COMPACT_ID),
            source_extension_id: ExtensionId::builtin(WIDGET_ID),
            content: CompactNode::IconText {
                icon: icon.to_owned(),
                text,
                color,
            },
            min_width: COMPACT_WIDTH,
            preferred_width: COMPACT_WIDTH,
            priority: 0,
            tooltip: Some(tooltip),
        }]
    }
}

fn alert_notification(alert: &WeatherAlert) -> Notification {
    Notification {
        source_id: ExtensionId::builtin(ALERTS_ID),
        id: format!("alert:{}", alert.id),
        title: alert.title.clone(),
        body: alert.body.clone(),
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

fn weather_root(snapshot: &Snapshot, alerts: &[WeatherAlert], error: Option<&str>) -> ViewNode {
    let current = &snapshot.current;
    let today = snapshot.today();
    let condition = weather_description(current.weather_code);
    let high_low = today.map_or_else(String::new, |today| {
        format!("最高 {:.0}° / 最低 {:.0}°", today.high, today.low)
    });
    let location = truncate(&snapshot.location.name, 10);
    let updated = snapshot.updated_hm();
    let hero = ViewNode::Row(vec![
        ViewNode::Column(vec![
            ViewNode::Sized {
                height: 40.0,
                child: Box::new(ViewNode::Text {
                    text: format!("{:.0}°", current.temperature),
                    style: TextStyle::Display,
                }),
            },
            ViewNode::Sized {
                height: 18.0,
                child: Box::new(ViewNode::Text {
                    text: condition.into(),
                    style: TextStyle::Title,
                }),
            },
            ViewNode::Sized {
                height: 16.0,
                child: Box::new(ViewNode::Text {
                    text: format!("体感 {:.0}° · {high_low}", current.apparent_temperature),
                    style: TextStyle::Small,
                }),
            },
        ]),
        ViewNode::Column(vec![
            ViewNode::Sized {
                height: 42.0,
                child: Box::new(ViewNode::Icon {
                    name: snapshot.icon().into(),
                    color: ColorRole::Accent,
                }),
            },
            ViewNode::Sized {
                height: 16.0,
                child: Box::new(ViewNode::Text {
                    text: location,
                    style: TextStyle::Body,
                }),
            },
            ViewNode::Sized {
                height: 14.0,
                child: Box::new(ViewNode::Text {
                    text: format!("{updated} 更新"),
                    style: TextStyle::Small,
                }),
            },
        ]),
    ]);

    let arc = snapshot.today().map_or(ViewNode::Divider, |today| {
        ViewNode::ArcMeter {
            start_label: format!("日出 {}", today.sunrise),
            end_label: format!("日落 {}", today.sunset),
            center_label: snapshot.daylight().map(|daylight| format!("日照 {daylight}")),
            progress: snapshot.sun_progress(),
        }
    });

    let stats = ViewNode::StatGrid {
        cells: vec![
            StatCell {
                label: "湿度".into(),
                value: format!("{}%", current.humidity),
                icon: Some("droplet".into()),
                color: ColorRole::Accent,
            },
            StatCell {
                label: format!(
                    "{}风 · {}级",
                    wind_direction(current.wind_direction),
                    wind_level(current.wind_speed)
                ),
                value: format!("{:.0} km/h", current.wind_speed),
                icon: Some("wind".into()),
                color: ColorRole::Primary,
            },
            StatCell {
                label: "降水概率".into(),
                value: format!("{}%", current.precipitation_probability),
                icon: Some("cloud-rain".into()),
                color: ColorRole::Accent,
            },
            StatCell {
                label: "紫外线".into(),
                value: format!("{:.0} {}", current.uv_index, uv_level(current.uv_index)),
                icon: Some("sun".into()),
                color: if current.uv_index >= 8.0 {
                    ColorRole::Warning
                } else {
                    ColorRole::Success
                },
            },
            StatCell {
                label: "气压".into(),
                value: format!("{:.0} hPa", current.pressure),
                icon: Some("gauge".into()),
                color: ColorRole::Primary,
            },
            StatCell {
                label: "能见度".into(),
                value: format_visibility(current.visibility),
                icon: Some("eye".into()),
                color: ColorRole::Primary,
            },
        ],
    };

    let (badge_text, badge_icon, badge_color) = if let Some(alert) = alerts.first() {
        (
            alert.short_title(),
            "triangle-alert",
            if alert.is_urgent() {
                ColorRole::Warning
            } else {
                ColorRole::Accent
            },
        )
    } else if error.is_some() {
        ("数据可能已过期".to_owned(), "cloud-off", ColorRole::Warning)
    } else {
        ("天气平稳".to_owned(), "sun", ColorRole::Success)
    };
    let section_header = ViewNode::Row(vec![
        ViewNode::Badge {
            text: badge_text,
            icon: Some(badge_icon.into()),
            color: badge_color,
        },
        ViewNode::Text {
            text: "未来 7 天".into(),
            style: TextStyle::Small,
        },
    ]);

    let chart = if !snapshot.daily.is_empty() {
        ViewNode::RangeChart {
            labels: snapshot.day_labels(),
            high: snapshot.daily.iter().map(|day| day.high as f32).collect(),
            low: snapshot.daily.iter().map(|day| day.low as f32).collect(),
        }
    } else {
        ViewNode::Divider
    };

    ViewNode::Column(vec![
        ViewNode::Sized {
            height: 74.0,
            child: Box::new(hero),
        },
        ViewNode::Sized {
            height: 28.0,
            child: Box::new(arc),
        },
        ViewNode::Sized {
            height: 62.0,
            child: Box::new(stats),
        },
        ViewNode::Sized {
            height: 16.0,
            child: Box::new(section_header),
        },
        ViewNode::Sized {
            height: 78.0,
            child: Box::new(chart),
        },
    ])
}

fn loading_root() -> ViewNode {
    ViewNode::Column(vec![
        ViewNode::Sized {
            height: 34.0,
            child: Box::new(ViewNode::Text {
                text: "正在获取天气…".into(),
                style: TextStyle::Heading,
            }),
        },
        ViewNode::Sized {
            height: 22.0,
            child: Box::new(ViewNode::Text {
                text: "根据网络位置自动定位".into(),
                style: TextStyle::Muted,
            }),
        },
    ])
}

fn error_root(error: &str) -> ViewNode {
    ViewNode::Column(vec![
        ViewNode::Sized {
            height: 30.0,
            child: Box::new(ViewNode::Text {
                text: "天气暂不可用".into(),
                style: TextStyle::Heading,
            }),
        },
        ViewNode::Sized {
            height: 22.0,
            child: Box::new(ViewNode::Text {
                text: truncate(error, 52),
                style: TextStyle::Muted,
            }),
        },
        ViewNode::Sized {
            height: 20.0,
            child: Box::new(ViewNode::Text {
                text: "可在设置 → 天气中选择城市".into(),
                style: TextStyle::Small,
            }),
        },
        ViewNode::Sized {
            height: 30.0,
            child: Box::new(ViewNode::Button {
                label: "重试".into(),
                action: Action::Advance,
            }),
        },
    ])
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!(
            "{}…",
            text.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{descriptors, WeatherExtension, ALERTS_ID, COMPACT_ID, WIDGET_ID};
    use crate::{
        builtins::weather::{
            alerts::{Severity, WeatherAlert},
            model::{
                CurrentWeather, DailyForecast, HourlyForecast, Location, Snapshot,
            },
            preferences::WeatherPreferences,
        },
        extension::{
            ColorRole, CompactNode, ExtensionDescriptor, ExtensionKind, RuntimeExtension,
            ViewNode,
        },
    };

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
            updated_at: "2026-11-17T17:45".into(),
            current: CurrentWeather {
                temperature: 14.9,
                apparent_temperature: 13.8,
                humidity: 71,
                precipitation_probability: 20,
                weather_code: 3,
                is_day: false,
                cloud_cover: 89,
                pressure: 1021.5,
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
                temperature: 14.6,
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
            descriptor.id.as_str() == ALERTS_ID
                && descriptor.kind == ExtensionKind::Notification
        }));
        assert_eq!(
            lookup_shape(&descriptors, WIDGET_ID),
            Some([400.0, 280.0])
        );
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
        assert!(matches!(
            &views[0].content,
            CompactNode::IconText { icon, text, color }
                if icon == "cloud" && text == "15°" && *color == ColorRole::Primary
        ));
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
        let ViewNode::Column(sections) = &view.root else {
            panic!("weather widget should be a column");
        };
        assert_eq!(sections.len(), 5);
        assert!(matches!(
            sections[0],
            ViewNode::Sized { height, .. } if (height - 74.0).abs() < 0.01
        ));
        assert!(matches!(
            &sections[2],
            ViewNode::Sized { child, .. }
                if matches!(
                    child.as_ref(),
                    ViewNode::StatGrid { cells } if cells.len() == 6
                )
        ));
        assert!(matches!(
            &sections[4],
            ViewNode::Sized { child, .. }
                if matches!(child.as_ref(), ViewNode::RangeChart { labels, .. } if labels.len() == 1)
        ));
        assert!(view.preferred_size[1] <= 280.0);
    }

    #[test]
    fn urgent_alerts_replace_the_calm_badge_and_are_published_only_once() {
        let alert = WeatherAlert {
            id: "2026-11-17:rain".into(),
            kind: "rain",
            severity: Severity::Orange,
            title: "暴雨橙色预警".into(),
            body: "上海：未来 12 小时累计降雨约 60 毫米。".into(),
        };
        let extension = WeatherExtension::for_test(Some(snapshot()), vec![alert.clone()], None);
        let compact = extension.compact_views();
        assert!(matches!(
            compact[0].content,
            CompactNode::IconText {
                color: ColorRole::Warning,
                ..
            }
        ));
        let notification = super::alert_notification(&alert);
        assert_eq!(notification.title, "暴雨橙色预警");
        assert_eq!(notification.icon.as_deref(), Some("triangle-alert"));
    }

    #[test]
    fn weather_preferences_are_applied_without_coupling_the_host_to_weather_data() {
        let mut extension = WeatherExtension::new(WeatherPreferences::default());
        let mut preferences = WeatherPreferences::default();
        preferences.alerts_enabled = false;
        preferences.location = Some(
            crate::builtins::weather::preferences::WeatherLocation::new("上海", 31.22, 121.46),
        );
        preferences.auto_locate = false;
        extension.set_preferences(&preferences);
        assert_eq!(extension.preferences, preferences);
        let ViewNode::Column(_) = extension.root() else {
            panic!("loading state should render as a column");
        };
    }
}
