use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Phase {
    #[default]
    Focus,
    Break,
}

#[derive(Clone, Debug)]
pub struct Pomodoro {
    pub phase: Phase,
    pub running: bool,
    remaining_seconds: f32,
    deadline: Option<Instant>,
    focus_minutes: u16,
    break_minutes: u16,
}

impl Default for Pomodoro {
    fn default() -> Self {
        Self {
            phase: Phase::Focus,
            remaining_seconds: 25.0 * 60.0,
            running: false,
            deadline: None,
            focus_minutes: 25,
            break_minutes: 5,
        }
    }
}

impl Pomodoro {
    pub fn set_durations(&mut self, focus_minutes: u16, break_minutes: u16) -> bool {
        let focus_minutes = focus_minutes.clamp(1, 180);
        let break_minutes = break_minutes.clamp(1, 180);
        if (self.focus_minutes, self.break_minutes) == (focus_minutes, break_minutes) {
            return false;
        }
        self.focus_minutes = focus_minutes;
        self.break_minutes = break_minutes;
        self.reset();
        true
    }

    pub fn update(&mut self, now: Instant) -> bool {
        let Some(deadline) = self.deadline else {
            return false;
        };
        if now < deadline {
            return false;
        }
        self.phase = match self.phase {
            Phase::Focus => Phase::Break,
            Phase::Break => Phase::Focus,
        };
        self.remaining_seconds = self.phase_seconds();
        self.running = false;
        self.deadline = None;
        true
    }

    pub fn remaining_at(&self, now: Instant) -> f32 {
        if self.running {
            self.deadline
                .map(|deadline| deadline.saturating_duration_since(now).as_secs_f32())
                .unwrap_or(self.remaining_seconds)
        } else {
            self.remaining_seconds
        }
    }

    pub fn progress_at(&self, now: Instant) -> f32 {
        (self.remaining_at(now) / self.phase_seconds()).clamp(0.0, 1.0)
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn toggle_at(&mut self, now: Instant) {
        if self.running {
            self.remaining_seconds = self.remaining_at(now);
            self.deadline = None;
            self.running = false;
        } else {
            self.deadline = Some(now + Duration::from_secs_f32(self.remaining_seconds.max(0.0)));
            self.running = true;
        }
    }

    pub fn reset(&mut self) {
        self.remaining_seconds = self.phase_seconds();
        self.running = false;
        self.deadline = None;
    }

    pub fn select_phase(&mut self, phase: Phase) {
        if self.phase == phase {
            return;
        }
        self.phase = phase;
        self.reset();
    }

    pub fn phase_seconds(&self) -> f32 {
        match self.phase {
            Phase::Focus => f32::from(self.focus_minutes) * 60.0,
            Phase::Break => f32::from(self.break_minutes) * 60.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Phase, Pomodoro};
    use std::time::{Duration, Instant};

    #[test]
    fn timer_starts_paused_with_focus_defaults() {
        let timer = Pomodoro::default();
        assert_eq!(timer.phase, Phase::Focus);
        assert_eq!(timer.remaining_at(Instant::now()), 25.0 * 60.0);
        assert!(!timer.running);
    }

    #[test]
    fn countdown_uses_deadline_and_pause_captures_remaining_time() {
        let start = Instant::now();
        let mut timer = Pomodoro::default();
        timer.toggle_at(start);
        assert_eq!(
            timer.remaining_at(start + Duration::from_secs(12)),
            25.0 * 60.0 - 12.0
        );
        timer.toggle_at(start + Duration::from_secs(12));
        assert_eq!(
            timer.remaining_at(start + Duration::from_secs(30)),
            25.0 * 60.0 - 12.0
        );
        assert!(!timer.running);
    }

    #[test]
    fn timer_deadline_switches_to_a_paused_break() {
        let start = Instant::now();
        let mut timer = Pomodoro {
            remaining_seconds: 0.1,
            ..Pomodoro::default()
        };
        timer.toggle_at(start);
        assert!(!timer.update(start + Duration::from_millis(99)));
        assert!(timer.update(start + Duration::from_millis(101)));
        assert_eq!(timer.phase, Phase::Break);
        assert_eq!(
            timer.remaining_at(start + Duration::from_millis(101)),
            5.0 * 60.0
        );
        assert!(!timer.running);
        assert!(!timer.update(start + Duration::from_secs(10)));
    }

    #[test]
    fn reset_cancels_deadline_and_selecting_a_phase_resets_that_phase() {
        let start = Instant::now();
        let mut timer = Pomodoro::default();
        timer.toggle_at(start);
        timer.reset();
        assert!(!timer.update(start + Duration::from_secs(100)));
        timer.toggle_at(start + Duration::from_secs(100));
        timer.select_phase(Phase::Break);
        assert_eq!(timer.phase, Phase::Break);
        assert_eq!(timer.remaining_at(start), 5.0 * 60.0);
        assert!(!timer.running);
    }

    #[test]
    fn configured_durations_apply_to_both_phases_and_reset_the_active_timer() {
        let mut timer = Pomodoro::default();
        timer.set_durations(50, 10);
        assert_eq!(timer.remaining_at(Instant::now()), 50.0 * 60.0);
        timer.select_phase(Phase::Break);
        assert_eq!(timer.remaining_at(Instant::now()), 10.0 * 60.0);
        assert!(!timer.set_durations(50, 10));
    }
}
