use super::{animation::ease, MascotPose};
use eframe::egui::{vec2, Painter, Rect, Shape, Stroke};
use std::f32::consts::TAU;

pub(crate) fn paint(painter: &Painter, rect: Rect, pose: MascotPose) {
    if !rect.is_positive() {
        return;
    }

    let unit = rect.width().min(rect.height()) / 1.85;
    let center = rect.center() + unit * vec2(0.20 * pose.drift, -0.26 * pose.lift);
    let (sin, cos) = pose.tilt.sin_cos();
    let point = |x: f32, y: f32| {
        let x = x * (1.0 - pose.squash * 0.55);
        let y = y * (1.0 + pose.squash * 0.75);
        center + unit * vec2(x * cos - y * sin, x * sin + y * cos)
    };

    let ink = crate::ui::palette::TextPalette::DEFAULT.primary;

    for (index, side) in [-1.0, 1.0].into_iter().enumerate() {
        let x = side * (0.38 + 0.08 * pose.spacing) + 0.19 * pose.gaze[0];
        let y = -0.18 + 0.13 * pose.gaze[1];
        let openness = pose.eyes[index].clamp(0.0, 1.3);
        let closed = 1.0 - ease(openness / 0.54);
        let eye = (0..20)
            .map(|i| {
                let (ey, ex) = (i as f32 * TAU / 20.0).sin_cos();
                point(
                    x + ex * (0.133 + 0.05 * closed),
                    y + ey * (0.049 + 0.145 * openness) + 0.018 * closed * ex * ex,
                )
            })
            .collect();
        painter.add(Shape::convex_polygon(eye, ink, Stroke::NONE));
    }

    let openness = ease(pose.mouth / 0.82);
    let half_width = 0.30 - 0.13 * openness;
    let y = 0.23 + 0.07 * openness;
    let expression = pose.smile * (1.0 - openness);
    let upper_depth = 0.04 + 0.13 * (-expression).max(0.0) + 0.15 * openness;
    let lower_depth = 0.04 + 0.17 * expression.max(0.0) + 0.17 * openness;

    // The two halves share their corners as the smile opens into an oval.
    let mouth = (0..32)
        .map(|i| {
            let (sine, cosine) = (i as f32 * TAU / 32.0).sin_cos();
            let depth = if sine < 0.0 { upper_depth } else { lower_depth };
            point(cosine * half_width, y + sine * depth)
        })
        .collect();
    painter.add(Shape::convex_polygon(mouth, ink, Stroke::NONE));
}
