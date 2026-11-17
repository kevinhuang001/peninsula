use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, TextureHandle, Vec2};

#[derive(Clone, Copy)]
pub(crate) enum Icon {
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
    Bell,
}

pub(crate) struct Icons {
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
        let icons = Self {
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
        };
        for (name, texture) in [
            ("lucide/clock", &icons.clock),
            ("lucide/moon", &icons.moon),
            ("lucide/arrow-down", &icons.arrow_down),
            ("lucide/arrow-up", &icons.arrow_up),
        ] {
            crate::ui::renderer::register_image(context, name, texture.clone());
        }
        icons
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
        if matches!(icon, Icon::Bell) {
            let c = rect.center();
            let r = rect.width().min(rect.height()) * 0.30;
            let stroke = Stroke::new(1.2, color.linear_multiply(opacity));
            painter.add(eframe::egui::Shape::line(
                vec![
                    c + Vec2::new(-r * 1.25, r * 0.65),
                    c + Vec2::new(-r, 0.0),
                    c + Vec2::new(-r * 0.8, -r * 0.75),
                    c + Vec2::new(0.0, -r),
                    c + Vec2::new(r * 0.8, -r * 0.75),
                    c + Vec2::new(r, 0.0),
                    c + Vec2::new(r * 1.25, r * 0.65),
                    c + Vec2::new(-r * 1.25, r * 0.65),
                ],
                stroke,
            ));
            painter.circle_filled(c + Vec2::new(0.0, r), 0.9, stroke.color);
            return;
        }
        let texture = match icon {
            Icon::Pin => &self.pin,
            Icon::Moon => &self.moon,
            Icon::Settings => &self.settings,
            Icon::Battery => &self.battery,
            Icon::BatteryCharging => &self.battery_charging,
            Icon::Cpu | Icon::Gpu | Icon::Bell => unreachable!(),
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
    let image = egui_extras::image::load_svg_bytes_with_size(
        svg.as_bytes(),
        Some(egui::SizeHint::Size(96, 96)),
    )
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
