use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, TextureHandle, Vec2};

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Pin,
    Moon,
    Settings,
    Battery,
    BatteryCharging,
    Cpu,
    Gpu,
    ArrowDown,
    ArrowUp,
    Clock,
    Close,
    Sun,
    Cloud,
    CloudSun,
    CloudMoon,
    CloudRain,
    CloudSnow,
    CloudLightning,
    CloudFog,
    CloudDrizzle,
    Wind,
    Droplet,
    Thermometer,
    Sunrise,
    Sunset,
    Gauge,
    Eye,
    AlertTriangle,
    MapPin,
    Search,
    Locate,
    CloudOff,
}

impl Icon {
    pub(super) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "pin" => Icon::Pin,
            "moon" | "clear-night" => Icon::Moon,
            "settings" => Icon::Settings,
            "battery" => Icon::Battery,
            "battery-charging" => Icon::BatteryCharging,
            "cpu" => Icon::Cpu,
            "gpu" => Icon::Gpu,
            "arrow-down" => Icon::ArrowDown,
            "arrow-up" => Icon::ArrowUp,
            "clock" => Icon::Clock,
            "close" => Icon::Close,
            "sun" | "clear-day" => Icon::Sun,
            "cloud" => Icon::Cloud,
            "cloud-sun" | "partly-cloudy-day" => Icon::CloudSun,
            "cloud-moon" | "partly-cloudy-night" => Icon::CloudMoon,
            "cloud-rain" | "rain" => Icon::CloudRain,
            "cloud-snow" | "snow" => Icon::CloudSnow,
            "cloud-lightning" | "thunderstorm" => Icon::CloudLightning,
            "cloud-fog" | "fog" => Icon::CloudFog,
            "cloud-drizzle" | "drizzle" => Icon::CloudDrizzle,
            "wind" => Icon::Wind,
            "droplet" => Icon::Droplet,
            "thermometer" => Icon::Thermometer,
            "sunrise" => Icon::Sunrise,
            "sunset" => Icon::Sunset,
            "gauge" => Icon::Gauge,
            "eye" => Icon::Eye,
            "triangle-alert" | "alert" => Icon::AlertTriangle,
            "map-pin" => Icon::MapPin,
            "search" => Icon::Search,
            "locate-fixed" | "locate" => Icon::Locate,
            "cloud-off" => Icon::CloudOff,
            _ => return None,
        })
    }
}

pub(super) struct Icons {
    pin: TextureHandle,
    moon: TextureHandle,
    settings: TextureHandle,
    battery: TextureHandle,
    battery_charging: TextureHandle,
    arrow_down: TextureHandle,
    arrow_up: TextureHandle,
    clock: TextureHandle,
    close: TextureHandle,
    sun: TextureHandle,
    cloud: TextureHandle,
    cloud_sun: TextureHandle,
    cloud_moon: TextureHandle,
    cloud_rain: TextureHandle,
    cloud_snow: TextureHandle,
    cloud_lightning: TextureHandle,
    cloud_fog: TextureHandle,
    cloud_drizzle: TextureHandle,
    wind: TextureHandle,
    droplet: TextureHandle,
    thermometer: TextureHandle,
    sunrise: TextureHandle,
    sunset: TextureHandle,
    gauge: TextureHandle,
    eye: TextureHandle,
    alert_triangle: TextureHandle,
    map_pin: TextureHandle,
    search: TextureHandle,
    locate: TextureHandle,
    cloud_off: TextureHandle,
}

impl Icons {
    pub fn load(context: &egui::Context) -> Self {
        Self {
            pin: load(
                context,
                "lucide-pin",
                include_bytes!("../../assets/icons/lucide/pin.svg"),
            ),
            moon: load(
                context,
                "lucide-moon",
                include_bytes!("../../assets/icons/lucide/moon.svg"),
            ),
            settings: load(
                context,
                "lucide-settings",
                include_bytes!("../../assets/icons/lucide/settings-2.svg"),
            ),
            battery: load(
                context,
                "lucide-battery",
                include_bytes!("../../assets/icons/lucide/battery.svg"),
            ),
            battery_charging: load(
                context,
                "lucide-battery-charging",
                include_bytes!("../../assets/icons/lucide/battery-charging.svg"),
            ),
            arrow_down: load(
                context,
                "lucide-arrow-down",
                include_bytes!("../../assets/icons/lucide/arrow-down.svg"),
            ),
            arrow_up: load(
                context,
                "lucide-arrow-up",
                include_bytes!("../../assets/icons/lucide/arrow-up.svg"),
            ),
            clock: load(
                context,
                "lucide-clock",
                include_bytes!("../../assets/icons/lucide/clock.svg"),
            ),
            close: load(
                context,
                "lucide-close",
                include_bytes!("../../assets/icons/lucide/x.svg"),
            ),
            sun: load(
                context,
                "lucide-sun",
                include_bytes!("../../assets/icons/lucide/sun.svg"),
            ),
            cloud: load(
                context,
                "lucide-cloud",
                include_bytes!("../../assets/icons/lucide/cloud.svg"),
            ),
            cloud_sun: load(
                context,
                "lucide-cloud-sun",
                include_bytes!("../../assets/icons/lucide/cloud-sun.svg"),
            ),
            cloud_moon: load(
                context,
                "lucide-cloud-moon",
                include_bytes!("../../assets/icons/lucide/cloud-moon.svg"),
            ),
            cloud_rain: load(
                context,
                "lucide-cloud-rain",
                include_bytes!("../../assets/icons/lucide/cloud-rain.svg"),
            ),
            cloud_snow: load(
                context,
                "lucide-cloud-snow",
                include_bytes!("../../assets/icons/lucide/cloud-snow.svg"),
            ),
            cloud_lightning: load(
                context,
                "lucide-cloud-lightning",
                include_bytes!("../../assets/icons/lucide/cloud-lightning.svg"),
            ),
            cloud_fog: load(
                context,
                "lucide-cloud-fog",
                include_bytes!("../../assets/icons/lucide/cloud-fog.svg"),
            ),
            cloud_drizzle: load(
                context,
                "lucide-cloud-drizzle",
                include_bytes!("../../assets/icons/lucide/cloud-drizzle.svg"),
            ),
            wind: load(
                context,
                "lucide-wind",
                include_bytes!("../../assets/icons/lucide/wind.svg"),
            ),
            droplet: load(
                context,
                "lucide-droplet",
                include_bytes!("../../assets/icons/lucide/droplet.svg"),
            ),
            thermometer: load(
                context,
                "lucide-thermometer",
                include_bytes!("../../assets/icons/lucide/thermometer.svg"),
            ),
            sunrise: load(
                context,
                "lucide-sunrise",
                include_bytes!("../../assets/icons/lucide/sunrise.svg"),
            ),
            sunset: load(
                context,
                "lucide-sunset",
                include_bytes!("../../assets/icons/lucide/sunset.svg"),
            ),
            gauge: load(
                context,
                "lucide-gauge",
                include_bytes!("../../assets/icons/lucide/gauge.svg"),
            ),
            eye: load(
                context,
                "lucide-eye",
                include_bytes!("../../assets/icons/lucide/eye.svg"),
            ),
            alert_triangle: load(
                context,
                "lucide-triangle-alert",
                include_bytes!("../../assets/icons/lucide/triangle-alert.svg"),
            ),
            map_pin: load(
                context,
                "lucide-map-pin",
                include_bytes!("../../assets/icons/lucide/map-pin.svg"),
            ),
            search: load(
                context,
                "lucide-search",
                include_bytes!("../../assets/icons/lucide/search.svg"),
            ),
            locate: load(
                context,
                "lucide-locate-fixed",
                include_bytes!("../../assets/icons/lucide/locate-fixed.svg"),
            ),
            cloud_off: load(
                context,
                "lucide-cloud-off",
                include_bytes!("../../assets/icons/lucide/cloud-off.svg"),
            ),
        }
    }

    pub fn paint(&self, painter: &Painter, icon: Icon, rect: Rect, color: Color32, opacity: f32) {
        if matches!(icon, Icon::Cpu) {
            paint_cpu(painter, rect, color.linear_multiply(opacity));
            return;
        }
        if matches!(icon, Icon::Gpu) {
            paint_gpu(painter, rect, color.linear_multiply(opacity));
            return;
        }
        let texture = match icon {
            Icon::Pin => &self.pin,
            Icon::Moon => &self.moon,
            Icon::Settings => &self.settings,
            Icon::Battery => &self.battery,
            Icon::BatteryCharging => &self.battery_charging,
            Icon::Cpu | Icon::Gpu => unreachable!(),
            Icon::ArrowDown => &self.arrow_down,
            Icon::ArrowUp => &self.arrow_up,
            Icon::Clock => &self.clock,
            Icon::Close => &self.close,
            Icon::Sun => &self.sun,
            Icon::Cloud => &self.cloud,
            Icon::CloudSun => &self.cloud_sun,
            Icon::CloudMoon => &self.cloud_moon,
            Icon::CloudRain => &self.cloud_rain,
            Icon::CloudSnow => &self.cloud_snow,
            Icon::CloudLightning => &self.cloud_lightning,
            Icon::CloudFog => &self.cloud_fog,
            Icon::CloudDrizzle => &self.cloud_drizzle,
            Icon::Wind => &self.wind,
            Icon::Droplet => &self.droplet,
            Icon::Thermometer => &self.thermometer,
            Icon::Sunrise => &self.sunrise,
            Icon::Sunset => &self.sunset,
            Icon::Gauge => &self.gauge,
            Icon::Eye => &self.eye,
            Icon::AlertTriangle => &self.alert_triangle,
            Icon::MapPin => &self.map_pin,
            Icon::Search => &self.search,
            Icon::Locate => &self.locate,
            Icon::CloudOff => &self.cloud_off,
        };
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            color.linear_multiply(opacity),
        );
    }
}

fn paint_cpu(painter: &Painter, rect: Rect, color: Color32) {
    let body = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.52));
    let stroke = Stroke::new(1.15_f32, color);
    painter.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Inside);
    painter.rect_stroke(body.shrink(2.0), 0.5, stroke, egui::StrokeKind::Inside);
    for offset in [-2.0_f32, 0.0, 2.0] {
        let x = rect.center().x + offset;
        let y = rect.center().y + offset;
        painter.line_segment(
            [Pos2::new(x, body.top() - 2.0), Pos2::new(x, body.top())],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(x, body.bottom()),
                Pos2::new(x, body.bottom() + 2.0),
            ],
            stroke,
        );
        painter.line_segment(
            [Pos2::new(body.left() - 2.0, y), Pos2::new(body.left(), y)],
            stroke,
        );
        painter.line_segment(
            [Pos2::new(body.right(), y), Pos2::new(body.right() + 2.0, y)],
            stroke,
        );
    }
}

fn paint_gpu(painter: &Painter, rect: Rect, color: Color32) {
    let body = Rect::from_center_size(
        rect.center(),
        Vec2::new(rect.width() * 0.9, rect.height() * 0.66),
    );
    let stroke = Stroke::new(1.15_f32, color);
    painter.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Inside);
    let fan = Pos2::new(body.left() + body.width() * 0.38, body.center().y);
    painter.circle_stroke(fan, rect.width() * 0.16, stroke);
    painter.circle_filled(fan, rect.width() * 0.045, color);
    for offset in [-2.0_f32, 0.0, 2.0] {
        let y = body.center().y + offset;
        painter.line_segment(
            [
                Pos2::new(body.right() - 3.0, y),
                Pos2::new(body.right() - 1.0, y),
            ],
            stroke,
        );
    }
    painter.line_segment(
        [
            Pos2::new(body.left() + 3.0, body.bottom()),
            Pos2::new(body.left() + 3.0, rect.bottom()),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(body.right() - 3.0, body.bottom()),
            Pos2::new(body.right() - 3.0, rect.bottom()),
        ],
        stroke,
    );
}

fn load(context: &egui::Context, name: &str, svg: &[u8]) -> TextureHandle {
    let svg = String::from_utf8_lossy(svg).replace("currentColor", "#FFFFFF");
    let image = egui_extras::image::load_svg_bytes(svg.as_bytes())
        .unwrap_or_else(|error| panic!("invalid bundled Lucide icon {name}: {error}"));
    context.load_texture(name, image, egui::TextureOptions::LINEAR)
}

#[cfg(test)]
mod tests {
    use super::{Icon, Icons};

    #[test]
    fn bundled_lucide_icons_load_and_are_attributed_under_the_open_license() {
        let context = eframe::egui::Context::default();
        let icons = Icons::load(&context);
        assert!(icons.pin.size()[0] > 0);
        assert!(icons.battery_charging.size()[0] > 0);
        assert!(icons.cloud_sun.size()[0] > 0);
        let license = include_str!("../../assets/icons/lucide/LICENSE");
        assert!(license.contains("ISC License"));
    }

    #[test]
    fn weather_icon_names_resolve_for_views_and_compact_items() {
        for name in [
            "sun",
            "cloud",
            "cloud-sun",
            "cloud-moon",
            "cloud-rain",
            "cloud-snow",
            "cloud-lightning",
            "cloud-fog",
            "cloud-drizzle",
            "wind",
            "droplet",
            "thermometer",
            "sunrise",
            "sunset",
            "gauge",
            "eye",
            "triangle-alert",
            "map-pin",
            "search",
            "locate-fixed",
            "cloud-off",
        ] {
            assert!(Icon::from_name(name).is_some(), "missing icon {name}");
        }
        assert!(Icon::from_name("not-an-icon").is_none());
    }
}
