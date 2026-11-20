use super::MascotPose;
use eframe::egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

pub(crate) fn paint(painter: &Painter, rect: Rect, pose: MascotPose, dark: bool) {
    if !rect.is_positive() {
        return;
    }
    let unit = rect.width().min(rect.height()) / 2.85;
    let center = rect.center() + vec2(0.0, -0.29 * unit * pose.lift);
    let (sin, cos) = pose.tilt.sin_cos();
    let point = |x: f32, y: f32| {
        let x = x * (1.0 - pose.squash * 0.55);
        let y = y * (1.0 + pose.squash);
        center + unit * vec2(x * cos - y * sin, x * sin + y * cos)
    };
    let shell = if dark {
        Color32::from_rgb(152, 229, 219)
    } else {
        Color32::from_rgb(62, 156, 151)
    };
    let visor = Color32::from_rgb(21, 39, 58);
    let eye = Color32::from_rgb(192, 255, 237);
    let coral = Color32::from_rgb(255, 144, 139);
    let gold = Color32::from_rgb(255, 212, 108);
    let rounded = |x: f32, y: f32, r: f32, color| {
        let corners = [
            (x - r, y - r),
            (-x + r, y - r),
            (-x + r, -y + r),
            (x - r, -y + r),
        ];
        let vertices: Vec<Pos2> = corners
            .into_iter()
            .enumerate()
            .flat_map(|(i, (cx, cy))| {
                (0..=5).map(move |j| {
                    let angle = (i as f32 + j as f32 / 5.0) * std::f32::consts::FRAC_PI_2;
                    point(cx + r * angle.cos(), cy + r * angle.sin())
                })
            })
            .collect();
        painter.add(Shape::convex_polygon(vertices, color, Stroke::NONE));
    };

    rounded(0.96, 0.72, 0.39, shell);
    rounded(0.77, 0.50, 0.28, visor);
    for (index, side) in [-1.0, 1.0].into_iter().enumerate() {
        let x = side * (0.31 + 0.15 * pose.spacing) + 0.13 * pose.gaze[0];
        let y = -0.10 + 0.14 * pose.gaze[1];
        let h = (0.17 * pose.eyes[index]).max(0.018);
        painter.line_segment(
            [point(x, y - h * 0.62), point(x, y + h * 0.62)],
            Stroke::new(unit * 0.15, eye),
        );
        if pose.brow > 0.04 {
            painter.line_segment(
                [
                    point(x - 0.12, y - 0.28 - side * 0.10 * pose.brow),
                    point(x + 0.12, y - 0.28 + side * 0.10 * pose.brow),
                ],
                Stroke::new(unit * 0.07, eye),
            );
        }
        if pose.blush > 0.02 {
            painter.circle_filled(
                point(side * 0.59, 0.24),
                unit * 0.09,
                coral.linear_multiply(pose.blush),
            );
        }
    }
    let open = super::animation::ease(pose.mouth / 0.24);
    if open > 0.0 {
        painter.add(Shape::ellipse_filled(
            point(0.0, 0.25),
            vec2(0.13, 0.06 + 0.11 * pose.mouth) * unit,
            eye.linear_multiply(open),
        ));
    }
    if open < 1.0 {
        let mouth = (0..=8)
            .map(|i| {
                let x = i as f32 / 4.0 - 1.0;
                point(x * 0.21, 0.22 + (1.0 - x * x) * 0.17 * pose.smile)
            })
            .collect();
        painter.add(Shape::line(
            mouth,
            Stroke::new(unit * 0.064, eye.linear_multiply(1.0 - open)),
        ));
    }
    if pose.heart > 0.02 {
        let color = coral.linear_multiply(pose.heart);
        for x in [-1.02, -0.82] {
            painter.circle_filled(point(x, -0.83), 0.13 * unit, color);
        }
        painter.add(Shape::convex_polygon(
            vec![
                point(-1.14, -0.80),
                point(-0.70, -0.80),
                point(-0.92, -0.53),
            ],
            color,
            Stroke::NONE,
        ));
    }
    if pose.bolt > 0.02 {
        painter.add(Shape::line(
            vec![
                point(-0.95, -0.91),
                point(-1.13, -0.68),
                point(-0.95, -0.68),
                point(-1.13, -0.45),
            ],
            Stroke::new(unit * 0.09, gold.linear_multiply(pose.bolt)),
        ));
    }
    if pose.sparks > 0.02 {
        for (x, y) in [(-1.12, -0.65), (1.13, -0.24), (0.94, 0.75)] {
            let c = point(x, y);
            let r = unit * (0.08 + pose.lift * 0.05);
            let color = gold.linear_multiply(pose.sparks);
            painter.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - r, c.y),
                    pos2(c.x, c.y - r),
                    pos2(c.x + r, c.y),
                    pos2(c.x, c.y + r),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}
