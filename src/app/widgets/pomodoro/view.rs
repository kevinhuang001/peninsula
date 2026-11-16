use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};

use super::{Phase, Pomodoro};
use std::time::Instant;

const FOCUS: Color32 = Color32::from_rgb(255, 117, 107);
const REST: Color32 = Color32::from_rgb(106, 224, 180);
const MUTED: Color32 = Color32::from_rgb(239, 242, 247);

pub fn show(ui: &mut egui::Ui, rect: Rect, timer: &mut Pomodoro) {
    let now = Instant::now();
    let (center, radius, control_top) = timer_layout(rect);
    let color = if timer.phase == Phase::Focus {
        FOCUS
    } else {
        REST
    };
    let painter = ui.painter();
    painter.circle_filled(center, radius + 8.0, color.linear_multiply(0.08));
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(3.0_f32, Color32::from_rgb(55, 59, 68)),
    );
    painter.add(egui::Shape::line(
        progress_points(center, radius, timer.progress_at(now)),
        Stroke::new(3.0_f32, color),
    ));
    painter.text(
        center + Vec2::new(0.0, -3.0),
        Align2::CENTER_CENTER,
        format_time(timer.remaining_at(now)),
        FontId::proportional(19.0),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(center.x, center.y + 18.0),
        Align2::CENTER_CENTER,
        if timer.phase == Phase::Focus {
            "专注中"
        } else {
            "休息中"
        },
        FontId::proportional(10.0),
        MUTED,
    );

    let controls = Rect::from_min_size(
        Pos2::new(rect.center().x - 124.0, control_top),
        Vec2::new(248.0, 22.0),
    );
    if ui
        .put(
            Rect::from_min_size(controls.min, Vec2::new(65.0, 22.0)),
            phase_button(timer.phase == Phase::Focus, "专注", FOCUS),
        )
        .clicked()
    {
        timer.select_phase(Phase::Focus);
    }
    if ui
        .put(
            Rect::from_min_size(
                Pos2::new(controls.left() + 70.0, control_top),
                Vec2::new(65.0, 22.0),
            ),
            phase_button(timer.phase == Phase::Break, "休息", REST),
        )
        .clicked()
    {
        timer.select_phase(Phase::Break);
    }
    if ui
        .put(
            Rect::from_min_size(
                Pos2::new(controls.right() - 68.0, control_top),
                Vec2::new(68.0, 22.0),
            ),
            egui::Button::new(if timer.running { "暂停" } else { "开始" })
                .fill(color.linear_multiply(0.8))
                .corner_radius(11.0),
        )
        .clicked()
    {
        timer.toggle_at(now);
    }
}

fn timer_layout(rect: Rect) -> (Pos2, f32, f32) {
    let radius = 29.0;
    let center = Pos2::new(rect.center().x, rect.top() + 38.0);
    let control_top = rect.bottom() - 22.0;
    (center, radius, control_top)
}

fn phase_button(active: bool, label: &str, color: Color32) -> egui::Button<'static> {
    egui::Button::new(label.to_owned())
        .fill(if active {
            color.linear_multiply(0.18)
        } else {
            Color32::TRANSPARENT
        })
        .stroke(Stroke::new(
            1.0_f32,
            if active {
                color
            } else {
                MUTED.linear_multiply(0.35)
            },
        ))
        .corner_radius(11.0)
}

fn progress_points(center: Pos2, radius: f32, progress: f32) -> Vec<Pos2> {
    let segments = 64;
    let sweep = std::f32::consts::TAU * progress.clamp(0.0, 1.0);
    (0..=segments)
        .map(|index| {
            let angle = -std::f32::consts::FRAC_PI_2 + sweep * index as f32 / segments as f32;
            Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0).ceil() as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::{format_time, progress_points, timer_layout};
    use eframe::egui::{Pos2, Rect, Vec2};

    #[test]
    fn timer_label_uses_stable_minute_second_format() {
        assert_eq!(format_time(1500.0), "25:00");
        assert_eq!(format_time(1.2), "00:02");
    }

    #[test]
    fn progress_arc_starts_at_twelve_and_is_bounded() {
        let points = progress_points(Pos2::ZERO, 10.0, 0.5);
        assert_eq!(points.len(), 65);
        assert!(points.first().unwrap().y < 0.0);
        assert!(points.last().unwrap().x < 0.0);
    }

    #[test]
    fn timer_circle_and_controls_have_separate_vertical_space() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 115.0));
        let (center, radius, controls_top) = timer_layout(rect);
        assert!(center.y + radius + 8.0 < controls_top);
        assert!(controls_top + 22.0 <= rect.bottom());
    }
}
