use eframe::egui::Vec2;

#[derive(Default)]
pub(super) struct TrackpadSwipe {
    distance: f32,
    last_event: Option<f64>,
    switched: bool,
    switched_direction: f32,
}

impl TrackpadSwipe {
    pub fn update(&mut self, delta: Vec2, time: f64, eligible: bool) -> bool {
        if !eligible {
            *self = Self::default();
            return false;
        }
        if delta == Vec2::ZERO {
            return false;
        }
        if self.last_event.is_none_or(|last| time - last > 0.3) {
            self.distance = 0.0;
            self.switched = false;
            self.switched_direction = 0.0;
        }
        self.last_event = Some(time);
        if delta.x.abs() <= delta.y.abs() {
            return false;
        }
        if self.switched {
            if delta.x.signum() == self.switched_direction {
                self.distance = 0.0;
                return false;
            }
            self.distance += delta.x;
            if self.distance.abs() >= 35.0 {
                self.switched_direction = delta.x.signum();
                self.distance = 0.0;
                return true;
            }
            return false;
        }
        self.distance += delta.x;
        if self.distance.abs() >= 35.0 {
            self.switched = true;
            self.switched_direction = self.distance.signum();
            self.distance = 0.0;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::TrackpadSwipe;
    use eframe::egui::vec2;

    #[test]
    fn horizontal_trackpad_samples_accumulate_and_switch_only_once_per_gesture() {
        let mut swipe = TrackpadSwipe::default();
        assert!(!swipe.update(vec2(12.0, 0.0), 1.0, true));
        assert!(!swipe.update(vec2(12.0, 1.0), 1.03, true));
        assert!(swipe.update(vec2(15.0, 0.0), 1.06, true));
        assert!(!swipe.update(vec2(100.0, 0.0), 1.09, true));
        assert!(swipe.update(vec2(-40.0, 0.0), 1.6, true));
    }

    #[test]
    fn vertical_or_outside_scroll_cannot_change_widgets() {
        let mut swipe = TrackpadSwipe::default();
        assert!(!swipe.update(vec2(12.0, 60.0), 0.0, true));
        assert!(!swipe.update(vec2(100.0, 0.0), 0.1, false));
        assert!(!swipe.update(vec2(5.0, 0.0), 0.2, true));
    }

    #[test]
    fn opposite_swipes_can_switch_back_even_while_scroll_events_continue() {
        let mut swipe = TrackpadSwipe::default();
        assert!(swipe.update(vec2(40.0, 0.0), 1.0, true));
        assert!(!swipe.update(vec2(8.0, 0.0), 1.04, true));
        assert!(!swipe.update(vec2(-15.0, 0.0), 1.08, true));
        assert!(swipe.update(vec2(-25.0, 0.0), 1.12, true));
        assert!(!swipe.update(vec2(-50.0, 0.0), 1.16, true));
        assert!(swipe.update(vec2(40.0, 0.0), 1.2, true));
    }
}
