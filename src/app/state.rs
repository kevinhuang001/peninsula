use super::animation::Spring;

const HOVER_LEAVE_DELAY: f32 = 0.28;
const PAGE_SHRINK_GRACE: f32 = 1.2;

pub(super) struct IslandState {
    pub expanded: bool,
    pinned: bool,
    pub width: Spring,
    pub height: Spring,
    expanded_width: f32,
    compact_width: f32,
    hover_elapsed: f32,
    leave_elapsed: f32,
    shrink_grace_remaining: f32,
    notification_visible: bool,
    suppress_hover_until_exit: bool,
    hide_widget_until_reentry: bool,
}

impl Default for IslandState {
    fn default() -> Self {
        Self {
            expanded: false,
            pinned: false,
            width: Spring::new(114.0),
            height: Spring::new(0.0),
            expanded_width: 244.0,
            compact_width: 114.0,
            hover_elapsed: 0.0,
            leave_elapsed: 0.0,
            shrink_grace_remaining: 0.0,
            notification_visible: false,
            suppress_hover_until_exit: false,
            hide_widget_until_reentry: false,
        }
    }
}

impl IslandState {
    pub fn begin_unlock_transition(&mut self, compact_width: f32) {
        self.expanded = false;
        self.hover_elapsed = 0.0;
        self.suppress_hover_until_exit = true;
        self.compact_width = compact_width;
        self.width = Spring::new(compact_width);
        self.height = Spring::new(0.0);
    }

    pub fn show_notification(&mut self) {
        self.notification_visible = true;
        self.suppress_hover_until_exit = false;
        self.hide_widget_until_reentry = false;
        self.expanded = true;
        self.hover_elapsed = 0.18;
        self.leave_elapsed = 0.0;
        self.shrink_grace_remaining = 0.0;
        self.width.tune(330.0, 0.70);
        self.height.tune(290.0, 0.74);
        self.width.target(super::capsule::ALERT_WIDTH);
        self.height.target(1.0);
    }

    pub fn close_notification(&mut self) {
        if !self.notification_visible {
            return;
        }
        self.notification_visible = false;
        self.expanded = false;
        self.suppress_hover_until_exit = true;
        self.hide_widget_until_reentry = true;
        self.hover_elapsed = 0.0;
        self.leave_elapsed = 0.0;
        self.shrink_grace_remaining = 0.0;
        self.width.tune(620.0, 1.0);
        self.height.tune(620.0, 1.0);
        self.width.target(self.compact_width);
        self.height.target(0.0);
    }

    pub fn hides_widget(&self) -> bool {
        self.hide_widget_until_reentry
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub fn toggle_pin(&mut self) {
        self.pinned = !self.pinned;
        self.leave_elapsed = 0.0;
        self.shrink_grace_remaining = 0.0;
    }

    pub fn set_expanded_width(&mut self, width: f32) {
        // Metrics are sampled continuously and their text width can change each
        // frame. Freeze the target for the whole transition to avoid retargeting
        // the spring while the native window is also resizing.
        if !self.expanded && !self.animating() {
            self.expanded_width = width.max(0.0);
        }
    }

    pub fn retarget_expanded_width(&mut self, width: f32) {
        self.expanded_width = width.max(0.0);
        if self.expanded {
            self.width.target(self.expanded_width);
        }
    }

    pub fn set_compact_width(&mut self, width: f32) {
        self.compact_width = width.max(64.0);
        if !self.expanded && !self.animating() {
            self.width.target(self.compact_width);
        }
    }

    pub fn defer_collapse_after_shrink(&mut self) {
        self.shrink_grace_remaining = PAGE_SHRINK_GRACE;
    }

    pub fn observe_interaction(&mut self, hovered: bool, keep_open: bool, dt: f32) -> bool {
        self.observe_hover(hovered || keep_open, dt)
    }
    pub fn observe_hover(&mut self, hovered: bool, dt: f32) -> bool {
        self.shrink_grace_remaining = (self.shrink_grace_remaining - dt).max(0.0);
        if self.suppress_hover_until_exit {
            if hovered {
                self.hover_elapsed = 0.0;
                self.leave_elapsed = 0.0;
                return false;
            }
            self.suppress_hover_until_exit = false;
        }
        if self.expanded && self.pinned {
            self.leave_elapsed = 0.0;
            return false;
        }
        if hovered {
            self.hover_elapsed = (self.hover_elapsed + dt).min(0.18);
            // Re-entering the smaller page restores the normal delay.
            if self.leave_elapsed > 0.0 {
                self.shrink_grace_remaining = 0.0;
            }
            self.leave_elapsed = 0.0;
            if !self.expanded && self.hover_elapsed >= 0.18 {
                self.expanded = true;
                self.hide_widget_until_reentry = false;
                self.width.tune(330.0, 0.70);
                self.height.tune(290.0, 0.74);
                self.width.target(self.expanded_width);
                self.height.target(1.0);
                return true;
            }
        } else {
            self.hover_elapsed = 0.0;
            if self.expanded {
                self.leave_elapsed = (self.leave_elapsed + dt).min(HOVER_LEAVE_DELAY);
                if self.leave_elapsed >= HOVER_LEAVE_DELAY && self.shrink_grace_remaining == 0.0 {
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

    pub fn finish_motion(&mut self) {
        self.width.finish();
        self.height.finish();
    }
    pub fn hover_pending(&self) -> bool {
        (!self.expanded && self.hover_elapsed > 0.0 && self.hover_elapsed < 0.18)
            || (self.expanded && !self.pinned && self.leave_elapsed > 0.0)
            || self.shrink_grace_remaining > 0.0
    }
    pub fn animating(&self) -> bool {
        !self.width.settled() || !self.height.settled()
    }
}

#[cfg(test)]
mod tests {
    use super::IslandState;

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
    fn download_completion_opens_the_island_without_hover() {
        let mut state = IslandState::default();
        state.show_notification();
        assert!(state.expanded);
        assert_eq!(state.width.target_value(), 320.0);
        assert_eq!(state.height.target_value(), 1.0);
        assert!(!state.observe_hover(true, 0.2));
        assert!(state.expanded);
    }

    #[test]
    fn pinned_island_stays_expanded_until_unpinned() {
        let mut state = IslandState::default();
        state.observe_hover(true, 0.2);
        state.toggle_pin();
        assert!(state.is_pinned());
        assert!(!state.observe_hover(false, 1.0));
        assert!(state.expanded);
        state.toggle_pin();
        assert!(!state.is_pinned());
        assert!(!state.observe_hover(false, 0.15));
        assert!(state.observe_hover(false, 0.15));
        assert!(!state.expanded);
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
        assert_eq!(state.width.target_value(), 114.0);
        assert_eq!(state.height.target_value(), 0.0);
    }

    #[test]
    fn shrinking_page_stays_open_longer_when_the_pointer_falls_outside() {
        let mut state = IslandState::default();
        state.observe_hover(true, 0.2);
        state.defer_collapse_after_shrink();
        assert!(!state.observe_hover(true, 0.15));
        assert!(!state.observe_hover(false, 0.8));
        assert!(state.expanded);
        assert!(state.observe_hover(false, 0.3));
        assert!(!state.expanded);
    }

    #[test]
    fn returning_to_the_smaller_page_restores_the_normal_leave_delay() {
        let mut state = IslandState::default();
        state.observe_hover(true, 0.2);
        state.defer_collapse_after_shrink();
        assert!(!state.observe_hover(false, 0.15));
        assert!(!state.observe_hover(true, 0.05));
        assert!(state.observe_hover(false, 0.3));
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
    fn switching_to_and_from_settings_retargets_the_expanded_capsule_width() {
        let mut state = IslandState::default();
        state.observe_hover(true, 0.2);
        state.retarget_expanded_width(1080.0);
        assert_eq!(state.width.target_value(), 1080.0);
        state.retarget_expanded_width(400.0);
        assert_eq!(state.width.target_value(), 400.0);
        state.retarget_expanded_width(300.0);
        assert_eq!(state.width.target_value(), 300.0);
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
    fn a_settled_close_returns_the_capsule_to_compact_bounds() {
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
        assert!((width - 114.0).abs() < 0.1);
    }

    #[test]
    fn closing_a_notification_collapses_without_reopening_the_widget_under_the_pointer() {
        let mut state = IslandState::default();
        state.show_notification();
        state.close_notification();
        assert!(!state.expanded);
        assert!(state.hides_widget());
        assert!(!state.observe_hover(true, 1.0));
        assert!(state.hides_widget());
        state.observe_hover(false, 0.01);
        assert!(state.observe_hover(true, 0.2));
        assert!(!state.hides_widget());
    }
}

#[cfg(test)]
mod persistent_surface_tests {
    use super::*;
    #[test]
    fn settings_stay_open_without_pinning_and_closing_restores_hover_collapse() {
        let mut state = IslandState::default();
        state.observe_interaction(true, true, 0.2);
        assert!(state.expanded && !state.is_pinned());
        for _ in 0..100 {
            state.observe_interaction(false, true, 0.05);
        }
        assert!(state.expanded && !state.is_pinned());
        for _ in 0..8 {
            state.observe_interaction(false, false, 0.05);
        }
        assert!(!state.expanded);
    }
}
