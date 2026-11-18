use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, TextureHandle, Vec2};

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Pin,
    Dashboard,
    Music,
    MusicNote,
    Speaker,
    SpeakerPlaying,
    MediaPlay,
    MediaPause,
    MediaPrevious,
    MediaNext,
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
}

impl Icons {
    pub fn load(context: &egui::Context) -> Self {
        use crate::ui::renderer::register_svg;
        for (asset, source) in [
            (
                "lucide/clock",
                include_str!("../../assets/icons/lucide/clock.svg"),
            ),
            (
                "lucide/arrow-down",
                include_str!("../../assets/icons/lucide/arrow-down.svg"),
            ),
            (
                "compact/battery",
                include_str!("../../assets/icons/compact/battery.svg"),
            ),
            (
                "compact/check",
                include_str!("../../assets/icons/compact/check.svg"),
            ),
            (
                "compact/network",
                include_str!("../../assets/icons/compact/network.svg"),
            ),
            (
                "compact/speaker-body",
                include_str!("../../assets/icons/compact/speaker-body.svg"),
            ),
            (
                "compact/speaker-cone",
                include_str!("../../assets/icons/compact/speaker-cone.svg"),
            ),
            (
                "compact/speaker-waves",
                include_str!("../../assets/icons/compact/speaker-waves.svg"),
            ),
        ] {
            register_svg(context, asset, source);
        }
        crate::builtins::register_mascot_assets(context);
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
        for (name, svg) in [
            (
                "cloud-rain",
                include_bytes!("../../assets/icons/lucide/cloud-rain.svg").as_slice(),
            ),
            (
                "droplet",
                include_bytes!("../../assets/icons/lucide/droplet.svg").as_slice(),
            ),
            (
                "eye",
                include_bytes!("../../assets/icons/lucide/eye.svg").as_slice(),
            ),
            (
                "gauge",
                include_bytes!("../../assets/icons/lucide/gauge.svg").as_slice(),
            ),
            (
                "locate-fixed",
                include_bytes!("../../assets/icons/lucide/locate-fixed.svg").as_slice(),
            ),
            (
                "map-pin",
                include_bytes!("../../assets/icons/lucide/map-pin.svg").as_slice(),
            ),
            (
                "search",
                include_bytes!("../../assets/icons/lucide/search.svg").as_slice(),
            ),
            (
                "sun",
                include_bytes!("../../assets/icons/lucide/sun.svg").as_slice(),
            ),
            (
                "sunrise",
                include_bytes!("../../assets/icons/lucide/sunrise.svg").as_slice(),
            ),
            (
                "sunset",
                include_bytes!("../../assets/icons/lucide/sunset.svg").as_slice(),
            ),
            (
                "thermometer",
                include_bytes!("../../assets/icons/lucide/thermometer.svg").as_slice(),
            ),
            (
                "triangle-alert",
                include_bytes!("../../assets/icons/lucide/triangle-alert.svg").as_slice(),
            ),
            (
                "wind",
                include_bytes!("../../assets/icons/lucide/wind.svg").as_slice(),
            ),
        ] {
            crate::ui::renderer::register_image(context, name, load(context, name, svg));
        }
        register_weather_icons(context);
        icons
    }

    pub fn paint(&self, painter: &Painter, icon: Icon, rect: Rect, color: Color32, opacity: f32) {
        if matches!(icon, Icon::Dashboard) {
            let center = rect.center();
            let cell = rect.width().min(rect.height()) * 0.36;
            let offset = cell * 0.68;
            let stroke = Stroke::new(1.25, color.linear_multiply(opacity));
            for y in [-offset, offset] {
                for x in [-offset, offset] {
                    painter.rect_stroke(
                        Rect::from_center_size(center + Vec2::new(x, y), Vec2::splat(cell)),
                        1.0,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }
            }
            return;
        }
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
        if matches!(icon, Icon::Speaker | Icon::SpeakerPlaying) {
            let playing = matches!(icon, Icon::SpeakerPlaying);
            let time = painter.ctx().input(|input| input.time) as f32;
            paint_speaker(
                painter,
                rect,
                color.linear_multiply(opacity),
                if playing { Some(time) } else { None },
            );
            if playing {
                painter
                    .ctx()
                    .request_repaint_after(std::time::Duration::from_millis(16));
            }
            return;
        }
        if matches!(icon, Icon::Music) {
            let origin = rect.center();
            let unit = rect.width().min(rect.height()) / 24.0;
            let point = |x, y| origin + Vec2::new(x, y) * unit;
            let color = color.linear_multiply(opacity);
            let stroke = Stroke::new(1.8 * unit, color);
            painter.line_segment([point(-3.0, 5.0), point(-3.0, -7.0)], stroke);
            painter.line_segment([point(8.0, 2.0), point(8.0, -10.0)], stroke);
            painter.line_segment([point(-3.0, -7.0), point(8.0, -10.0)], stroke);
            painter.circle_filled(point(-5.4, 5.7), 2.8 * unit, color);
            painter.circle_filled(point(5.6, 2.7), 2.8 * unit, color);
            return;
        }
        if matches!(
            icon,
            Icon::MusicNote
                | Icon::MediaPlay
                | Icon::MediaPause
                | Icon::MediaPrevious
                | Icon::MediaNext
        ) {
            paint_media_icon(painter, icon, rect, color.linear_multiply(opacity));
            return;
        }
        let texture = match icon {
            Icon::Pin => &self.pin,
            Icon::Moon => &self.moon,
            Icon::Settings => &self.settings,
            Icon::Battery => &self.battery,
            Icon::BatteryCharging => &self.battery_charging,
            Icon::Cpu
            | Icon::Gpu
            | Icon::Bell
            | Icon::Dashboard
            | Icon::Music
            | Icon::MusicNote
            | Icon::Speaker
            | Icon::SpeakerPlaying
            | Icon::MediaPlay
            | Icon::MediaPause
            | Icon::MediaPrevious
            | Icon::MediaNext => unreachable!(),
            Icon::ArrowDown => &self.arrow_down,
            Icon::ArrowUp => &self.arrow_up,
            Icon::Clock => &self.clock,
            Icon::Close => &self.close,
        };
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            color.linear_multiply(opacity),
        );
    }
}

/// A speaker cabinet with a pulsing woofer and a small rhythmic shake.
fn paint_speaker(painter: &Painter, rect: Rect, color: Color32, time: Option<f32>) {
    let unit = rect.width().min(rect.height()) / 24.0;
    let beat = time.map_or(0.0, |t| (t * 10.0).sin());
    let shake = time.map_or(0.0, |t| (t * 28.0).sin() * 0.65);
    let center = rect.center() + Vec2::new(shake, -beat.abs() * 0.55) * unit;
    let at = |x, y| center + Vec2::new(x, y) * unit;
    let stroke = Stroke::new((1.5 * unit).max(0.8), color);
    painter.rect_stroke(
        Rect::from_min_max(at(-6.0, -9.0), at(6.0, 9.0)),
        2.0 * unit,
        stroke,
        egui::StrokeKind::Inside,
    );
    painter.circle_stroke(at(0.0, -4.5), 1.6 * unit, stroke);
    painter.circle_stroke(at(0.0, 3.0), (3.4 + beat * 0.45) * unit, stroke);
    painter.circle_filled(at(0.0, 3.0), (1.15 + beat * 0.20) * unit, color);
    if time.is_some() {
        for side in [-1.0, 1.0] {
            painter.add(egui::Shape::line(
                vec![
                    at(side * 8.5, -3.0 - beat * 0.5),
                    at(side * (9.5 + beat.abs() * 0.7), 0.0),
                    at(side * 8.5, 3.0 + beat * 0.5),
                ],
                stroke,
            ));
        }
    }
}

fn paint_media_icon(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let center = rect.center();
    let unit = rect.width().min(rect.height()) / 24.0;
    let at = |x, y| center + Vec2::new(x, y) * unit;
    let triangle = |middle: f32, direction: f32| {
        painter.add(egui::Shape::convex_polygon(
            vec![
                at(middle - 4.8 * direction, -7.0),
                at(middle + 5.6 * direction, 0.0),
                at(middle - 4.8 * direction, 7.0),
            ],
            color,
            Stroke::NONE,
        ));
    };
    match icon {
        Icon::MediaPlay => triangle(0.0, 1.0),
        Icon::MediaPrevious | Icon::MediaNext => {
            let direction = if matches!(icon, Icon::MediaPrevious) {
                -1.0
            } else {
                1.0
            };
            triangle(-5.4, direction);
            triangle(5.4, direction);
        }
        Icon::MediaPause => {
            for x in [-4.7, 4.7] {
                painter.rect_filled(
                    Rect::from_center_size(at(x, 0.0), Vec2::new(4.8, 19.0) * unit),
                    1.3 * unit,
                    color,
                );
            }
        }
        Icon::MusicNote => {
            let stroke = Stroke::new(1.8 * unit, color);
            painter.line_segment([at(1.0, 5.0), at(1.0, -8.0)], stroke);
            painter.line_segment([at(1.0, -8.0), at(6.5, -4.5)], stroke);
            painter.circle_filled(at(-2.1, 5.9), 3.1 * unit, color);
        }
        _ => unreachable!(),
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

const WEATHER_ICON_PIXELS: u32 = 192;

fn register_weather_icons(context: &egui::Context) {
    for (name, svg) in [
        (
            "meteo/clear-day",
            include_bytes!("../../assets/icons/meteocons/clear-day.svg").as_slice(),
        ),
        (
            "meteo/clear-night",
            include_bytes!("../../assets/icons/meteocons/clear-night.svg").as_slice(),
        ),
        (
            "meteo/partly-cloudy-day",
            include_bytes!("../../assets/icons/meteocons/partly-cloudy-day.svg").as_slice(),
        ),
        (
            "meteo/partly-cloudy-night",
            include_bytes!("../../assets/icons/meteocons/partly-cloudy-night.svg").as_slice(),
        ),
        (
            "meteo/overcast",
            include_bytes!("../../assets/icons/meteocons/overcast.svg").as_slice(),
        ),
        (
            "meteo/fog",
            include_bytes!("../../assets/icons/meteocons/fog.svg").as_slice(),
        ),
        (
            "meteo/drizzle",
            include_bytes!("../../assets/icons/meteocons/drizzle.svg").as_slice(),
        ),
        (
            "meteo/rain",
            include_bytes!("../../assets/icons/meteocons/rain.svg").as_slice(),
        ),
        (
            "meteo/sleet",
            include_bytes!("../../assets/icons/meteocons/sleet.svg").as_slice(),
        ),
        (
            "meteo/snow",
            include_bytes!("../../assets/icons/meteocons/snow.svg").as_slice(),
        ),
        (
            "meteo/thunderstorms",
            include_bytes!("../../assets/icons/meteocons/thunderstorms.svg").as_slice(),
        ),
        (
            "meteo/not-available",
            include_bytes!("../../assets/icons/meteocons/not-available.svg").as_slice(),
        ),
    ] {
        let texture = rasterize_svg(context, name, svg, WEATHER_ICON_PIXELS);
        crate::ui::renderer::register_image(context, name, texture);
    }
}

fn load(context: &egui::Context, name: &str, svg: &[u8]) -> TextureHandle {
    let svg = String::from_utf8_lossy(svg).replace("currentColor", "#FFFFFF");
    rasterize_svg(context, name, svg.as_bytes(), 96)
}

fn rasterize_svg(context: &egui::Context, name: &str, svg: &[u8], pixels: u32) -> TextureHandle {
    let image = svg_image(svg, pixels)
        .unwrap_or_else(|error| panic!("invalid bundled icon {name}: {error}"));
    context.load_texture(
        name,
        image,
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    )
}

fn svg_image(svg: &[u8], pixels: u32) -> Result<egui::ColorImage, String> {
    egui_extras::image::load_svg_bytes_with_size(svg, Some(egui::SizeHint::Size(pixels, pixels)))
}

#[cfg(test)]
mod tests {
    use super::Icons;

    #[test]
    fn bundled_lucide_icons_load_and_are_attributed_under_the_open_license() {
        let context = eframe::egui::Context::default();
        let icons = Icons::load(&context);
        assert!(icons.pin.size()[0] >= 96);
        assert!(icons.battery_charging.size()[0] > 0);
        let license = include_str!("../../assets/icons/lucide/LICENSE");
        assert!(license.contains("ISC License"));
    }
    #[test]
    fn weather_svg_keeps_its_color_and_rasters_above_retina_hero_size() {
        let svg = include_bytes!("../../assets/icons/meteocons/partly-cloudy-night.svg");
        let pixels = super::WEATHER_ICON_PIXELS as usize;
        let image = super::svg_image(svg, super::WEATHER_ICON_PIXELS).unwrap();
        assert_eq!(image.size, [pixels, pixels]);
        let foreground = image.pixels.iter().filter(|pixel| pixel.a() > 200).count();
        assert!(foreground > pixels * pixels / 20);
        assert!(image
            .pixels
            .iter()
            .any(|pixel| pixel.a() > 200 && pixel.r() != pixel.b()));
    }
}
