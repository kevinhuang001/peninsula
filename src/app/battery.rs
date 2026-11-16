use crate::metrics::Snapshot;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

const GREEN: Color32 = Color32::from_rgb(110, 231, 183);
const BLUE: Color32 = Color32::from_rgb(112, 175, 255);
const LIGHTNING: Color32 = Color32::from_rgb(255, 214, 76);

pub(super) fn paint_chip(
    painter: &egui::Painter,
    origin: Pos2,
    metrics: &Snapshot,
    opacity: f32,
) -> f32 {
    let percent = metrics
        .battery_percent
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "--".into());
    let label_color = battery_text_color(metrics);
    let icon_color = if metrics.battery_percent.is_some_and(|value| value < 20) && !metrics.charging
    {
        label_color
    } else {
        BLUE
    };
    paint_icon(
        painter,
        Rect::from_min_size(origin, Vec2::new(15.0, 13.0)),
        metrics.battery_percent,
        icon_color,
        metrics.charging,
        opacity,
    );
    let label = painter.layout_no_wrap(percent, FontId::proportional(12.0), label_color);
    painter.galley(
        origin + Vec2::new(20.0, -2.0),
        label.clone(),
        label_color.linear_multiply(opacity),
    );
    20.0 + label.size().x
}

pub(super) fn paint_icon(
    painter: &egui::Painter,
    rect: Rect,
    percent: Option<u8>,
    color: Color32,
    charging: bool,
    opacity: f32,
) {
    let body = Rect::from_min_max(rect.min, Pos2::new(rect.right() - 2.0, rect.bottom()));
    let color = color.linear_multiply(opacity);
    painter.rect_stroke(body, 2, Stroke::new(1.2, color), egui::StrokeKind::Inside);
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(body.right(), rect.center().y - 2.0),
            Pos2::new(rect.right(), rect.center().y + 2.0),
        ),
        1,
        color,
    );
    let fill = body.shrink(2.5);
    if let Some(level) = percent.filter(|level| *level > 0) {
        painter.rect_filled(
            Rect::from_min_max(
                fill.min,
                Pos2::new(
                    fill.left() + fill.width() * f32::from(level) / 100.0,
                    fill.bottom(),
                ),
            ),
            1,
            color,
        );
    }
    if charging {
        painter.add(egui::Shape::line(
            lightning_path(body),
            Stroke::new(1.8, LIGHTNING.linear_multiply(opacity)),
        ));
    }
}

pub(super) fn battery_text_color(metrics: &Snapshot) -> Color32 {
    if metrics.battery_percent.is_some_and(|value| value < 20) {
        Color32::from_rgb(255, 117, 117)
    } else if metrics.charging {
        GREEN
    } else {
        Color32::from_rgb(230, 234, 241)
    }
}

fn lightning_path(rect: Rect) -> Vec<Pos2> {
    let center = rect.center();
    vec![
        Pos2::new(center.x + 2.4, rect.top() + 1.0),
        Pos2::new(center.x - 2.2, center.y + 0.3),
        Pos2::new(center.x, center.y + 0.3),
        Pos2::new(center.x - 1.8, rect.bottom() - 1.0),
        Pos2::new(center.x + 2.6, center.y - 1.0),
        Pos2::new(center.x + 0.5, center.y - 1.0),
        Pos2::new(center.x + 2.4, rect.top() + 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::lightning_path;
    use eframe::egui::{Pos2, Rect, Vec2};

    #[test]
    fn charging_bolt_stays_inside_the_battery_outline() {
        let outline = Rect::from_min_size(Pos2::ZERO, Vec2::new(16.0, 14.0));
        assert!(lightning_path(outline)
            .into_iter()
            .all(|point| outline.contains(point)));
    }
}
