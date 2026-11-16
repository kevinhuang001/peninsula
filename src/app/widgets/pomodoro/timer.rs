#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Phase {
    #[default]
    Focus,
    Break,
}

#[derive(Clone, Debug)]
pub struct Pomodoro {
    pub phase: Phase,
    pub remaining_seconds: f32,
    pub running: bool,
    focus_minutes: u16,
    break_minutes: u16,
}

impl Default for Pomodoro {
    fn default() -> Self {
        Self {
            phase: Phase::Focus,
            remaining_seconds: 25.0 * 60.0,
            running: false,
            focus_minutes: 25,
            break_minutes: 5,
        }
    }
}

impl Pomodoro {
    pub fn advance(&mut self, dt: f32) -> bool {
        if !self.running {
            return false;
        }
        self.remaining_seconds -= dt.max(0.0);
        if self.remaining_seconds > 0.0 {
            return false;
        }
        self.phase = match self.phase {
            Phase::Focus => Phase::Break,
            Phase::Break => Phase::Focus,
        };
        self.remaining_seconds = self.phase_seconds();
        self.running = false;
        true
    }

    pub fn toggle(&mut self) {
        if self.remaining_seconds <= 0.0 {
            self.remaining_seconds = self.phase_seconds();
        }
        self.running = !self.running;
    }

    pub fn reset(&mut self) {
        self.remaining_seconds = self.phase_seconds();
        self.running = false;
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

    pub fn progress(&self) -> f32 {
        (self.remaining_seconds / self.phase_seconds()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Phase, Pomodoro};

    #[test]
    fn timer_starts_paused_with_daozai_focus_defaults() {
        let timer = Pomodoro::default();
        assert_eq!(timer.phase, Phase::Focus);
        assert_eq!(timer.remaining_seconds, 25.0 * 60.0);
        assert!(!timer.running);
    }

    #[test]
    fn countdown_advances_only_while_running() {
        let mut timer = Pomodoro::default();
        timer.toggle();
        timer.advance(12.5);
        assert_eq!(timer.remaining_seconds, 25.0 * 60.0 - 12.5);
        timer.toggle();
        timer.advance(10.0);
        assert_eq!(timer.remaining_seconds, 25.0 * 60.0 - 12.5);
    }

    #[test]
    fn completed_focus_switches_to_paused_break_timer() {
        let mut timer = Pomodoro {
            remaining_seconds: 0.1,
            ..Pomodoro::default()
        };
        timer.toggle();
        assert!(timer.advance(0.2));
        assert_eq!(timer.phase, Phase::Break);
        assert_eq!(timer.remaining_seconds, 5.0 * 60.0);
        assert!(!timer.running);
    }

    #[test]
    fn selecting_a_phase_resets_and_pauses_that_phase() {
        let mut timer = Pomodoro::default();
        timer.toggle();
        timer.advance(120.0);
        timer.select_phase(Phase::Break);
        assert_eq!(timer.phase, Phase::Break);
        assert_eq!(timer.remaining_seconds, 5.0 * 60.0);
        assert!(!timer.running);
    }
}
