use super::{MascotMotion, MotionPolicy};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MascotPose {
    pub(crate) lift: f32,
    pub(crate) drift: f32,
    pub(crate) tilt: f32,
    pub(crate) squash: f32,
    pub(crate) eyes: [f32; 2],
    pub(crate) gaze: [f32; 2],
    pub(crate) spacing: f32,
    pub(crate) smile: f32,
    pub(crate) mouth: f32,
}

impl Default for MascotPose {
    fn default() -> Self {
        Self {
            lift: 0.0,
            drift: 0.0,
            tilt: 0.0,
            squash: 0.0,
            eyes: [1.0; 2],
            gaze: [0.0; 2],
            spacing: 0.0,
            smile: 0.64,
            mouth: 0.0,
        }
    }
}

pub(super) fn blink_amount(phase: f32) -> f32 {
    keyframes(phase, &[0.0, 0.8, 1.0, 0.42, 0.0])
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
            drift: mix(self.drift, other.drift),
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
    let still_phase = match motion {
        Blink => 0.0,
        Error => 1.0,
        Success => 0.4,
        _ => 0.36,
    };
    let mut pose = match policy {
        MotionPolicy::Full => motion_pose(motion, t, time),
        MotionPolicy::Reduced => {
            motion_pose(motion, still_phase, 0.0).blend(motion_pose(motion, t, time), 0.55)
        }
        MotionPolicy::Off => motion_pose(motion, still_phase, 0.0),
    };
    let movement = match policy {
        MotionPolicy::Full => 0.68,
        MotionPolicy::Reduced => 0.20,
        MotionPolicy::Off => 0.0,
    };
    pose.tilt *= movement;
    pose.lift *= movement;
    pose.drift *= movement;
    pose.squash *= movement;
    if policy != MotionPolicy::Off && !matches!(motion, Blink | Verifying | Error) {
        pose.eyes = pose.eyes.map(|eye| eye * (1.0 - blink * 0.96));
    }
    pose
}

fn motion_pose(motion: MascotMotion, phase: f32, time: f32) -> MascotPose {
    use MascotMotion::*;
    let frames = |values: &[f32]| keyframes(phase, values);
    let wave = (phase * TAU).sin();
    let breath = (time * TAU / 4.4).sin();
    let mut pose = MascotPose {
        lift: 0.12 * breath,
        squash: 0.025 * breath,
        ..MascotPose::default()
    };
    match motion {
        Idle => {
            pose.tilt = 0.04 * (time * TAU / 6.2).sin();
            pose.gaze = [0.18 * (time * TAU / 8.4).sin(), 0.0];
        }
        Blink => {
            pose.eyes = [1.0 - blink_amount(phase); 2];
            pose.tilt = -0.08 * blink_amount(phase);
        }
        LookAround => {
            pose.gaze = [frames(&[0.0, -1.0, -1.0, 0.9, 0.9, 0.0]), -0.12];
            pose.tilt = 0.17 * pose.gaze[0];
            pose.drift = 0.32 * pose.gaze[0];
        }
        Sleepy => {
            let yawn = frames(&[0.0, 0.0, 0.85, 0.46, 0.0, 0.0]);
            pose.eyes = [frames(&[0.66, 0.26, 0.05, 0.1, 0.25, 0.34]); 2];
            pose.mouth = yawn;
            pose.smile = 0.18;
            pose.lift = frames(&[0.0, -0.23, -0.08, -0.32, -0.44, -0.32]);
            pose.tilt = frames(&[0.0, -0.08, -0.12, -0.18, -0.28, -0.22]);
            pose.squash = -0.09 + 0.23 * yawn;
        }
        Happy => {
            pose.smile = 1.14;
            pose.eyes = [frames(&[0.72, 0.18, 0.38, 0.65]); 2];
            pose.lift = frames(&[0.0, 0.54, 0.26, 0.0]);
            pose.tilt = -0.10 * wave;
        }
        Excited => {
            let hop = frames(&[0.0, -0.23, 0.95, -0.13, 0.76, -0.10, 0.43, 0.0]);
            pose.smile = 1.05;
            pose.lift = hop;
            pose.squash = 0.24 * hop;
            pose.mouth = 0.32 + 0.34 * hop.max(0.0);
            pose.eyes = [1.08 - 0.55 * hop.max(0.0); 2];
        }
        Thinking => {
            pose.gaze = [frames(&[0.0, 0.95, 0.95, 0.55, 0.55]), -0.95];
            pose.tilt = frames(&[0.0, -0.23, -0.19, -0.27, -0.22]);
            pose.eyes = [0.71, 0.95];
            pose.smile = -0.16;
        }
        Surprised => {
            let startle = frames(&[0.12, 1.0, 0.83, 0.66]);
            pose.eyes = [1.0 + startle * 0.30; 2];
            pose.spacing = 0.48 * startle;
            pose.mouth = 0.82 * startle;
            pose.lift = 0.72 * startle;
            pose.squash = 0.17 * startle;
            pose.tilt = -0.19 * startle;
        }
        Love => {
            pose.smile = 1.06;
            let wink = frames(&[0.8, 0.07, 0.10, 0.55, 0.8]);
            pose.eyes = [wink, 0.58];
            pose.tilt = 0.18 * wave;
        }
        Celebrate => {
            let jump = frames(&[-0.15, 0.95, 0.12, -0.10, 0.66, -0.06, 0.0]);
            pose.lift = jump;
            pose.squash = 0.19 * jump;
            pose.tilt = 0.27 * wave;
            pose.drift = 0.62 * wave;
            pose.eyes = [0.24; 2];
            pose.smile = 1.16;
            pose.mouth = 0.42 * jump.max(0.0);
        }
        Listening => {
            let beat = (phase * TAU * 2.0).cos();
            pose.tilt = 0.20 * wave;
            pose.eyes = [0.26 - 0.08 * beat; 2];
            pose.smile = 0.92;
            pose.lift = 0.13 - 0.23 * beat;
            pose.squash = -0.065 * beat;
        }
        Focused => {
            pose.eyes = [0.45 + 0.04 * wave; 2];
            pose.gaze = [0.0, 0.44];
            pose.smile = -0.12;
            pose.lift = -0.20;
        }
        Charging => {
            let pulse = frames(&[0.48, 1.0, 0.60, 1.0, 0.48]);
            pose.lift = 0.34 * pulse;
            pose.squash = 0.12 * (pulse - 0.60);
            pose.gaze = [0.0, -0.36];
            pose.eyes = [0.73 + 0.35 * pulse; 2];
            pose.smile = 0.28 + 0.58 * pulse;
        }
        Typing => {
            let taps = (phase * TAU * 3.0).sin();
            pose.gaze = [0.68 + 0.23 * wave, 0.74];
            pose.tilt = 0.11 + 0.055 * taps;
            pose.lift = -0.16 + 0.13 * taps;
            pose.squash = 0.045 * taps;
            pose.eyes = [0.88; 2];
        }
        Verifying => {
            pose.gaze = [frames(&[-0.92, -0.92, 0.92, 0.92, -0.92]), 0.06];
            pose.drift = 0.28 * pose.gaze[0];
            pose.tilt = 0.12 * pose.gaze[0];
            pose.eyes = [0.84; 2];
            pose.smile = 0.08;
        }
        Error => {
            let recoil = frames(&[0.0, 1.0, 0.63, 0.26, 0.0]);
            pose.drift = frames(&[0.0, -0.85, 0.68, -0.36, 0.12, 0.0]);
            pose.tilt = 0.24 * pose.drift;
            pose.eyes = [0.59 + 0.64 * recoil; 2];
            pose.smile = -0.73;
            pose.mouth = 0.50 * recoil;
            pose.lift = 0.36 * recoil - 0.33;
            pose.squash = -0.14 * recoil;
        }
        Success => {
            let bounce = frames(&[0.0, 0.71, 0.47, -0.14, 0.0]);
            pose.smile = 1.16;
            pose.eyes = [frames(&[0.86, 0.08, 0.08, 0.74, 0.95]), 0.76];
            pose.lift = bounce;
            pose.squash = 0.14 * bounce;
            pose.tilt = -0.18 * bounce;
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
