use super::MascotPose;
use eframe::egui::{vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

pub(crate) fn paint(painter: &Painter, rect: Rect, pose: MascotPose, dark: bool) {
    if !rect.is_positive() {
        return;
    }

    // The mascot is just its face now. Scale the features to fill the compact slot
    // without drawing a shell or visor around them.
    let unit = rect.width().min(rect.height()) / 1.75;
    let center = rect.center() + vec2(0.0, -0.16 * unit * pose.lift);
    let (sin, cos) = pose.tilt.sin_cos();
    let point = |x: f32, y: f32| {
        let x = x * (1.0 - pose.squash * 0.35);
        let y = y * (1.0 + pose.squash * 0.55);
        center + unit * vec2(x * cos - y * sin, x * sin + y * cos)
    };

    let feature = if dark {
        Color32::from_rgb(232, 250, 255)
    } else {
        Color32::from_rgb(24, 48, 63)
    };

    for (index, side) in [-1.0, 1.0].into_iter().enumerate() {
        let x = side * (0.39 + 0.06 * pose.spacing) + 0.10 * pose.gaze[0];
        let y = -0.13 + 0.08 * pose.gaze[1];
        let eye_height = (0.34 * pose.eyes[index]).max(0.015);
        painter.line_segment(
            [
                point(x, y - eye_height * 0.5),
                point(x, y + eye_height * 0.5),
            ],
            Stroke::new(unit * 0.20, feature),
        );
    }

    let open = super::animation::ease(pose.mouth / 0.24);
    if open > 0.0 {
        painter.add(Shape::ellipse_filled(
            point(0.0, 0.34),
            vec2(0.34, 0.12 + 0.18 * pose.mouth) * unit,
            feature.linear_multiply(open),
        ));
    }
    if open < 1.0 {
        let mouth: Vec<Pos2> = (0..=8)
            .map(|i| {
                let x = i as f32 / 4.0 - 1.0;
                point(x * 0.42, 0.28 + (1.0 - x * x) * 0.22 * pose.smile)
            })
            .collect();
        painter.add(Shape::line(
            mouth,
            Stroke::new(unit * 0.09, feature.linear_multiply(1.0 - open)),
        ));
    }
}
