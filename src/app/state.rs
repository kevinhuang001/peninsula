use super::animation::Spring;

#[cfg(test)]
pub(super) fn window_size(progress: f32) -> [f32; 2] {
    let progress = progress.clamp(0.0, 1.0);
    [190.0 + 230.0 * progress, 64.0 + 126.0 * progress]
}

#[cfg(any(not(target_os = "macos"), test))]
pub(super) fn window_left_for_center(center_x: f32, width: f32) -> f32 {
    center_x - width * 0.5
}

pub(super) struct IslandState {
    pub expanded: bool,
    pub width: Spring,
    pub height: Spring,
    expanded_width: f32,
    compact_width: f32,
    hover_elapsed: f32,
    leave_elapsed: f32,
}

impl Default for IslandState {
    fn default() -> Self {
        Self {
            expanded: false,
            width: Spring::new(148.0),
            height: Spring::new(0.0),
            expanded_width: 244.0,
            compact_width: 148.0,
            hover_elapsed: 0.0,
            leave_elapsed: 0.0,
        }
    }
}

impl IslandState {
    pub fn set_expanded_width(&mut self, width: f32) {
        // Metrics are sampled continuously and their text width can change each
        // frame. Freeze the target for the whole transition to avoid retargeting
        // the spring while the native window is also resizing.
        if !self.expanded && !self.animating() {
            self.expanded_width = width.clamp(244.0, 360.0);
        }
    }

    pub fn set_compact_width(&mut self, width: f32) {
        self.compact_width = width.max(148.0);
        if !self.expanded && !self.animating() {
            self.width.target(self.compact_width);
        }
    }

    pub fn observe_hover(&mut self, hovered: bool, dt: f32) -> bool {
        if hovered {
            self.hover_elapsed = (self.hover_elapsed + dt).min(0.18);
            self.leave_elapsed = 0.0;
            if !self.expanded && self.hover_elapsed >= 0.18 {
                self.expanded = true;
                self.width.tune(330.0, 0.70);
                self.height.tune(290.0, 0.74);
                self.width.target(self.expanded_width);
                self.height.target(1.0);
                return true;
            }
        } else {
            self.hover_elapsed = 0.0;
            if self.expanded {
                self.leave_elapsed = (self.leave_elapsed + dt).min(0.28);
                if self.leave_elapsed >= 0.28 {
                    self.expanded = false;
                    self.leave_elapsed = 0.0;
                    self.width.tune(620.0, 1.0);
                    self.height.tune(620.0, 1.0);
                    self.width.target(self.compact_width);
                    self.height.target(0.0);
                    return true;
                }
            } else {
                self.leave_elapsed = 0.0;
            }
        }
        false
    }

    pub fn advance(&mut self, dt: f32) -> (f32, f32) {
        self.width.step(dt);
        self.height.step(dt);
        (self.width.value.max(0.0), self.height.value.clamp(0.0, 1.0))
    }

    pub fn animating(&self) -> bool {
        !self.width.settled() || !self.height.settled()
    }
}

#[cfg(test)]
mod tests {
    use super::{window_left_for_center, window_size, IslandState};

    #[test]
    fn island_starts_collapsed_and_expands_after_hover_intent() {
        let mut state = IslandState::default();
        assert!(!state.expanded);
        assert!(!state.observe_hover(false, 1.0));
        assert!(!state.observe_hover(true, 0.1));
        assert!(!state.expanded);
        assert!(state.observe_hover(true, 0.1));
        assert!(state.expanded);
        assert_eq!(state.width.target_value(), 244.0);
        assert_eq!(state.height.target_value(), 1.0);
    }

    #[test]
    fn moving_from_capsule_to_panel_keeps_it_open_during_leave_grace() {
        let mut state = IslandState::default();
        state.observe_hover(true, 0.2);
        assert!(state.expanded);
        assert!(!state.observe_hover(false, 0.15));
        assert!(state.expanded);
        assert!(!state.observe_hover(true, 0.01));
        assert!(state.expanded);
        assert!(!state.observe_hover(false, 0.15));
        assert!(state.observe_hover(false, 0.15));
        assert!(!state.expanded);
        assert_eq!(state.width.target_value(), 148.0);
        assert_eq!(state.height.target_value(), 0.0);
    }

    #[test]
    fn opening_state_advances_smoothly_between_endpoints() {
        let mut state = IslandState::default();
        state.observe_hover(true, 0.2);
        let mut previous = 0.0;
        let mut middle = 0.0;
        for frame in 0..120 {
            let (_, progress) = state.advance(1.0 / 60.0);
            assert!(
                progress >= previous - 0.02,
                "spring should move continuously"
            );
            if frame == 8 {
                middle = progress;
            }
            previous = progress;
        }
        assert!((0.2..0.95).contains(&middle));
        assert!(!state.animating());
    }

    #[test]
    fn collapsed_window_is_narrower_than_the_previous_compact_layout() {
        let size = window_size(0.0);
        assert_eq!(size, [190.0, 64.0]);
        assert!(size[0] < 274.0);
        assert_eq!(window_size(1.0), [420.0, 190.0]);
    }

    #[test]
    fn window_expands_equally_on_both_sides_of_its_original_center() {
        let center = 715.0;
        let closed = window_left_for_center(center, window_size(0.0)[0]);
        let open = window_left_for_center(center, window_size(1.0)[0]);
        assert_eq!(closed, 620.0);
        assert_eq!(open, 505.0);
        assert_eq!(closed + window_size(0.0)[0] * 0.5, center);
        assert_eq!(open + window_size(1.0)[0] * 0.5, center);
    }

    #[test]
    fn changing_metric_width_during_expansion_does_not_retarget_the_spring() {
        let mut state = IslandState::default();
        state.set_expanded_width(300.0);
        state.observe_hover(true, 0.2);
        state.advance(1.0 / 60.0);
        let target = state.width.target_value();
        state.set_expanded_width(350.0);
        assert_eq!(state.width.target_value(), target);
        assert_eq!(target, 300.0);
    }

    #[test]
    fn compact_content_width_is_used_again_after_closing() {
        let mut state = IslandState::default();
        state.set_compact_width(240.0);
        state.observe_hover(true, 0.2);
        state.observe_hover(false, 0.3);
        assert_eq!(state.width.target_value(), 240.0);
    }

    #[test]
    fn a_settled_close_returns_capsule_and_window_to_the_compact_bounds() {
        let mut state = IslandState::default();
        state.observe_hover(true, 0.2);
        for _ in 0..120 {
            state.advance(1.0 / 60.0);
        }
        state.observe_hover(false, 0.3);
        let (width, progress) = (0..120).map(|_| state.advance(1.0 / 60.0)).last().unwrap();
        assert!(!state.animating());
        assert!(!state.expanded);
        assert!(progress <= 0.002);
        assert!((width - 148.0).abs() < 0.1);
        assert!((window_size(progress)[0] - 190.0).abs() < 0.5);
        assert!(4.0 + 52.0 + 126.0 * progress < 64.0);
    }
}
