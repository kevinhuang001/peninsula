#[derive(Clone, Copy, Debug)]
pub(super) struct Spring {
    pub value: f32,
    velocity: f32,
    target: f32,
    stiffness: f32,
    damping: f32,
}

impl Spring {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            stiffness: 330.0,
            damping: 2.0 * 0.70 * 330.0_f32.sqrt(),
        }
    }

    pub fn target(&mut self, value: f32) {
        self.target = value;
    }

    pub fn tune(&mut self, stiffness: f32, damping_ratio: f32) {
        self.stiffness = stiffness;
        self.damping = 2.0 * damping_ratio * stiffness.sqrt();
    }

    #[cfg(test)]
    pub fn target_value(&self) -> f32 {
        self.target
    }

    pub fn finish(&mut self) {
        self.value = self.target;
        self.velocity = 0.0;
    }
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let steps = (dt * 480.0).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            let acceleration =
                -self.stiffness * (self.value - self.target) - self.damping * self.velocity;
            self.velocity += acceleration * h;
            self.value += self.velocity * h;
        }
    }

    pub fn settled(&self) -> bool {
        (self.value - self.target).abs() < 0.002 && self.velocity.abs() < 0.02
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Spring;

    #[test]
    fn opening_spring_overshoots_gently_then_settles() {
        let mut spring = Spring::new(0.0);
        spring.target(1.0);
        let mut peak: f32 = 0.0;
        for _ in 0..120 {
            spring.step(1.0 / 60.0);
            peak = peak.max(spring.value);
        }
        assert!(peak > 1.0 && peak < 1.08, "peak={peak}");
        assert!(spring.settled());
    }

    #[test]
    fn reversing_mid_transition_keeps_position_and_converges() {
        let mut spring = Spring::new(0.0);
        spring.target(1.0);
        for _ in 0..10 {
            spring.step(1.0 / 60.0);
        }
        let before = spring.value;
        spring.target(0.0);
        assert_eq!(spring.value, before);
        for _ in 0..120 {
            spring.step(1.0 / 60.0);
        }
        assert!(spring.settled());
        assert!(spring.value.abs() < 0.002);
    }
}
