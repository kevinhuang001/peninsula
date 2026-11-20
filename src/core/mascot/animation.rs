use super::{MascotMotion, MotionPolicy};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MascotPose {
    pub(crate) lift: f32,
    pub(crate) tilt: f32,
    pub(crate) squash: f32,
    pub(crate) eyes: [f32; 2],
    pub(crate) gaze: [f32; 2],
    pub(crate) spacing: f32,
    pub(crate) smile: f32,
    pub(crate) mouth: f32,
    pub(crate) brow: f32,
    pub(crate) blush: f32,
    pub(crate) sparks: f32,
    pub(crate) heart: f32,
    pub(crate) bolt: f32,
}

impl Default for MascotPose {
    fn default() -> Self {
        Self {
            lift: 0.0,
            tilt: 0.0,
            squash: 0.0,
            eyes: [1.0; 2],
            gaze: [0.0; 2],
            spacing: 0.0,
            smile: 0.2,
            mouth: 0.0,
            brow: 0.0,
            blush: 0.0,
            sparks: 0.0,
            heart: 0.0,
            bolt: 0.0,
        }
    }
}

pub(super) fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl MascotPose {
    pub(super) fn blend(self, other: Self, t: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self {
            lift: mix(self.lift, other.lift),
            tilt: mix(self.tilt, other.tilt),
            squash: mix(self.squash, other.squash),
            eyes: [
                mix(self.eyes[0], other.eyes[0]),
                mix(self.eyes[1], other.eyes[1]),
            ],
            gaze: [
                mix(self.gaze[0], other.gaze[0]),
                mix(self.gaze[1], other.gaze[1]),
            ],
            spacing: mix(self.spacing, other.spacing),
            smile: mix(self.smile, other.smile),
            mouth: mix(self.mouth, other.mouth),
            brow: mix(self.brow, other.brow),
            blush: mix(self.blush, other.blush),
            sparks: mix(self.sparks, other.sparks),
            heart: mix(self.heart, other.heart),
            bolt: mix(self.bolt, other.bolt),
        }
    }
}

pub(super) fn sample(
    motion: MascotMotion,
    t: f32,
    time: f32,
    blink: f32,
    policy: MotionPolicy,
) -> MascotPose {
    use MascotMotion::*;
    let still = policy == MotionPolicy::Off;
    let phase = if still { 0.24 } else { t };
    let sway = (phase * TAU).sin();
    let bounce = (phase * TAU * 2.0).sin().abs() * (-2.5 * phase).exp();
    let mut pose = MascotPose::default();
    match motion {
        Idle => pose.tilt = 0.025 * (time * 0.95).sin(),
        Blink => pose.eyes = [1.0 - (phase * std::f32::consts::PI).sin(); 2],
        LookAround => pose.gaze = [keyframes(phase, &[0.0, -0.85, -0.85, 0.9, 0.9, 0.0]), -0.15],
        Sleepy => {
            pose.eyes = [0.33; 2];
            pose.mouth = 0.25 + bounce;
            pose.tilt = -0.13;
        }
        Happy => {
            pose.smile = 0.95;
            pose.lift = 0.55 * bounce;
        }
        Excited => {
            pose.smile = 1.0;
            pose.lift = bounce;
            pose.mouth = 0.55;
        }
        Thinking => {
            pose.tilt = -0.22;
            pose.gaze = [0.4 + 0.4 * sway, -0.65];
            pose.smile = -0.15;
        }
        Surprised => {
            pose.eyes = [1.3; 2];
            pose.spacing = 0.25;
            pose.mouth = 0.8;
            pose.tilt = -0.17;
        }
        Love => {
            pose.blush = 1.0;
            pose.heart = 0.8 + 0.2 * sway;
            pose.smile = 0.95;
        }
        Celebrate => {
            pose.lift = bounce;
            pose.tilt = 0.36 * sway;
            pose.sparks = 0.9;
            pose.smile = 1.0;
        }
        Listening => {
            pose.tilt = 0.16 * (phase * TAU * 3.0).sin();
            pose.eyes = [0.75; 2];
        }
        Focused => {
            pose.eyes = [0.8; 2];
            pose.brow = 0.35;
            pose.smile = 0.0;
        }
        Charging => {
            pose.bolt = 0.7 + 0.3 * sway;
            pose.squash = 0.07 * sway;
        }
        Typing => {
            pose.gaze = [0.75, 0.4];
            pose.tilt = 0.05 + 0.04 * sway;
        }
        Verifying => {
            pose.gaze = [0.85 * sway, 0.1];
            pose.brow = 0.35;
            pose.tilt = 0.09;
            pose.eyes = [0.83; 2];
        }
        Error => {
            let recoil = ease((phase - 0.13) / 0.22);
            pose.eyes = [1.24 - 0.55 * recoil; 2];
            pose.brow = 0.85 * recoil;
            pose.smile = 0.1 - 0.83 * recoil;
            pose.mouth = 0.8 * (1.0 - recoil);
            pose.squash = -0.17 * recoil;
            pose.tilt = -0.15 * recoil;
        }
        Success => {
            pose.smile = 1.15;
            pose.eyes = [1.0 - 0.85 * bounce, 0.85];
            pose.lift = bounce;
            pose.sparks = 0.75;
        }
    }
    let movement = match policy {
        MotionPolicy::Full => 1.0,
        MotionPolicy::Reduced => 0.28,
        MotionPolicy::Off => 0.0,
    };
    pose.tilt *= movement;
    pose.lift *= movement;
    pose.squash = movement * (pose.squash + 0.032 * (time * 1.8).sin());
    if !still && !matches!(motion, Blink | Verifying | Error) {
        pose.eyes = pose.eyes.map(|eye| eye * (1.0 - blink * 0.94));
    }
    if policy != MotionPolicy::Full {
        pose.sparks = 0.0;
    }
    if still {
        if motion == Verifying {
            pose.gaze = [0.55, 0.05];
        }
        if motion == Error {
            pose.brow = 0.85;
            pose.smile = -0.73;
            pose.mouth = 0.0;
        }
    }
    pose
}

/// Interpolate uniformly spaced keyframes with zero velocity at each hold.
fn keyframes(phase: f32, values: &[f32]) -> f32 {
    let position = phase.clamp(0.0, 1.0) * (values.len() - 1) as f32;
    let segment = (position as usize).min(values.len() - 2);
    values[segment] + (values[segment + 1] - values[segment]) * ease(position - segment as f32)
}
