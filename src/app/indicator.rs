use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Indicator {
    shown_at: Option<Instant>,
}

impl Indicator {
    pub fn show(&mut self, now: Instant) {
        self.shown_at = Some(now);
    }
    pub fn opacity(&self, now: Instant) -> f32 {
        let Some(shown_at) = self.shown_at else {
            return 0.0;
        };
        let elapsed = now.saturating_duration_since(shown_at);
        if elapsed <= Duration::from_millis(900) {
            1.0
        } else {
            (1.0 - (elapsed - Duration::from_millis(900)).as_secs_f32() / 0.3).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Indicator;
    use std::time::{Duration, Instant};

    #[test]
    fn indicator_is_hidden_until_a_switch_then_fades_out() {
        let now = Instant::now();
        let mut indicator = Indicator::default();
        assert_eq!(indicator.opacity(now), 0.0);
        indicator.show(now);
        assert_eq!(indicator.opacity(now + Duration::from_millis(800)), 1.0);
        assert!((indicator.opacity(now + Duration::from_millis(1050)) - 0.5).abs() < 0.01);
        assert_eq!(indicator.opacity(now + Duration::from_millis(1200)), 0.0);
        indicator.show(now + Duration::from_millis(1100));
        assert_eq!(indicator.opacity(now + Duration::from_millis(1200)), 1.0);
    }
}
