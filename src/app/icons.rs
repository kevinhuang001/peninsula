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
        };
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            color.linear_multiply(opacity),
        );
    }

    pub fn paint_battery(
        &self,
        painter: &Painter,
        rect: Rect,
        color: Color32,
        charging: bool,
        opacity: f32,
    ) {
        self.paint(
            painter,
            if charging {
                Icon::BatteryCharging
            } else {
                Icon::Battery
            },
            rect,
            if charging {
                Color32::from_rgb(255, 214, 76)
            } else {
                color
            },
            opacity,
        );
    }
}

fn paint_cpu(painter: &Painter, rect: Rect, color: Color32) {
    let body = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.52));
    let stroke = Stroke::new(1.15, color);
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
    let stroke = Stroke::new(1.15, color);
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
    use super::Icons;

    #[test]
    fn bundled_lucide_icons_load_and_are_attributed_under_the_open_license() {
        let context = eframe::egui::Context::default();
        let icons = Icons::load(&context);
        assert!(icons.pin.size()[0] > 0);
        assert!(icons.battery_charging.size()[0] > 0);
        let license = include_str!("../../assets/icons/lucide/LICENSE");
        assert!(license.contains("ISC License"));
    }
}
