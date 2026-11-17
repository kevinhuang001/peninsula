use super::widgets::Direction;
use eframe::egui::Vec2;

#[derive(Default)]
pub(super) struct TrackpadSwipe {
    distance: f32,
    last_event: Option<f64>,
    switched_direction: Option<Direction>,
    #[cfg(any(target_os = "macos", test))]
    gesture: Gesture,
}

impl TrackpadSwipe {
    pub fn update(&mut self, delta: Vec2, time: f64, eligible: bool) -> Option<Direction> {
        if !eligible {
            *self = Self::default();
            return None;
        }
        if delta == Vec2::ZERO {
            return None;
        }
        if self.last_event.is_none_or(|last| time - last > 0.3) {
            self.distance = 0.0;
            self.switched_direction = None;
        }
        self.last_event = Some(time);
        if delta.x.abs() <= delta.y.abs() {
            return None;
        }
        if self.switched_direction == Some(Direction::from_swipe(delta.x)) {
            self.distance = 0.0;
            return None;
        }
        self.distance += delta.x;
        if self.distance.abs() < 35.0 {
            return None;
        }
        let direction = Direction::from_swipe(self.distance);
        self.switched_direction = Some(direction);
        self.distance = 0.0;
        Some(direction)
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, TrackpadSwipe};
    use eframe::egui::vec2;

    #[test]
    fn consecutive_native_swipes_turn_pages_even_during_momentum() {
        use super::ScrollPhase::*;
        let mut swipe = TrackpadSwipe::default();
        assert_eq!(swipe.phased(Began, vec2(-20.0, 0.0), true), None);
        assert_eq!(
            swipe.phased(Changed, vec2(-20.0, 0.0), true),
            Some(Direction::Next)
        );
        assert_eq!(swipe.phased(Changed, vec2(-100.0, 0.0), true), None);
        assert_eq!(swipe.phased(Ended, vec2(0.0, 0.0), true), None);
        assert_eq!(swipe.phased(Momentum, vec2(-70.0, 0.0), true), None);
        assert_eq!(
            swipe.phased(Began, vec2(-40.0, 0.0), true),
            Some(Direction::Next)
        );
        assert_eq!(swipe.phased(Momentum, vec2(60.0, 0.0), true), None);
        assert_eq!(swipe.phased(Began, vec2(2.0, 40.0), true), None);
        assert_eq!(swipe.phased(Changed, vec2(100.0, 0.0), true), None);
    }

    #[test]
    fn horizontal_trackpad_samples_accumulate_and_switch_only_once_per_gesture() {
        let mut swipe = TrackpadSwipe::default();
        assert_eq!(swipe.update(vec2(12.0, 0.0), 1.0, true), None);
        assert_eq!(swipe.update(vec2(12.0, 1.0), 1.03, true), None);
        assert_eq!(
            swipe.update(vec2(15.0, 0.0), 1.06, true),
            Some(Direction::Previous)
        );
        assert_eq!(swipe.update(vec2(100.0, 0.0), 1.09, true), None);
        assert_eq!(
            swipe.update(vec2(-40.0, 0.0), 1.6, true),
            Some(Direction::Next)
        );
    }

    #[test]
    fn vertical_or_outside_scroll_cannot_change_widgets() {
        let mut swipe = TrackpadSwipe::default();
        assert_eq!(swipe.update(vec2(12.0, 60.0), 0.0, true), None);
        assert_eq!(swipe.update(vec2(100.0, 0.0), 0.1, false), None);
        assert_eq!(swipe.update(vec2(5.0, 0.0), 0.2, true), None);
    }

    #[test]
    fn opposite_swipes_can_switch_back_even_while_scroll_events_continue() {
        let mut swipe = TrackpadSwipe::default();
        assert_eq!(
            swipe.update(vec2(40.0, 0.0), 1.0, true),
            Some(Direction::Previous)
        );
        assert_eq!(swipe.update(vec2(8.0, 0.0), 1.04, true), None);
        assert_eq!(swipe.update(vec2(-15.0, 0.0), 1.08, true), None);
        assert_eq!(
            swipe.update(vec2(-25.0, 0.0), 1.12, true),
            Some(Direction::Next)
        );
        assert_eq!(swipe.update(vec2(-50.0, 0.0), 1.16, true), None);
        assert_eq!(
            swipe.update(vec2(40.0, 0.0), 1.2, true),
            Some(Direction::Previous)
        );
    }
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy)]
pub(super) enum ScrollPhase {
    Began,
    Changed,
    Ended,
    Momentum,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Default)]
struct Gesture {
    distance: Vec2,
    finished: bool,
}

#[cfg(any(target_os = "macos", test))]
impl TrackpadSwipe {
    pub fn phased(&mut self, phase: ScrollPhase, delta: Vec2, eligible: bool) -> Option<Direction> {
        self.distance = 0.0;
        self.last_event = None;
        self.switched_direction = None;
        if !eligible || matches!(phase, ScrollPhase::Ended) {
            self.gesture = Gesture::default();
            return None;
        }
        if matches!(phase, ScrollPhase::Momentum) {
            return None;
        }
        if matches!(phase, ScrollPhase::Began) {
            self.gesture = Gesture::default();
        }
        if self.gesture.finished {
            return None;
        }
        self.gesture.distance += delta;
        let distance = self.gesture.distance;
        if distance.y.abs() >= 35.0 && distance.y.abs() >= distance.x.abs() {
            self.gesture.finished = true;
        } else if distance.x.abs() >= 35.0 && distance.x.abs() > distance.y.abs() {
            self.gesture.finished = true;
            return Some(Direction::from_swipe(distance.x));
        }
        None
    }
}
