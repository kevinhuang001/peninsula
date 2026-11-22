use super::ColorSpec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Trigger {
    #[default]
    Always,
    While(bool),
    /// Play once on each change. The initial value is only a baseline.
    OnChange(String),
    State {
        current: String,
        active: String,
        paused: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Playback {
    #[default]
    Running,
    Paused,
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Once,
    /// Total number of passes, including the initial pass.
    Count(u32),
    /// Alternate direction. A missing count repeats indefinitely.
    Reverse(Option<u32>),
    Infinite,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Cubic Bézier control points, each component in 0–1.
    CubicBezier([f32; 4]),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MotionProperty {
    TranslateX,
    TranslateY,
    Scale,
    /// Rotation in radians.
    Rotation,
    Opacity,
    /// Normalized fraction on ProgressRing, ProgressBar, Gauge and SegmentedIndicator.
    Progress,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe<T> {
    pub at_ms: u32,
    pub value: T,
    /// Easing from this frame to the next, overriding the timeline easing.
    #[serde(default)]
    pub easing: Option<Easing>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionTrack {
    pub property: MotionProperty,
    pub frames: Vec<Keyframe<f32>>,
    /// Multiply deviations from the property's resting value by this mapping.
    #[serde(default)]
    pub amplitude: Option<ValueMapping>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValueMapping {
    pub value: f32,
    pub input: [f32; 2],
    pub output: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValueTransition {
    pub property: MotionProperty,
    pub value: f32,
    pub duration_ms: u32,
    #[serde(default)]
    pub easing: Easing,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Motion {
    pub trigger: Trigger,
    pub playback: Playback,
    pub duration_ms: u32,
    pub delay_ms: u32,
    pub gap_ms: u32,
    pub repeat: Repeat,
    pub easing: Easing,
    pub tracks: Vec<MotionTrack>,
    pub color: Vec<Keyframe<ColorSpec>>,
    pub transition: Option<ValueTransition>,
    pub fps: u8,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            trigger: Trigger::Always,
            playback: Playback::Running,
            duration_ms: 300,
            delay_ms: 0,
            gap_ms: 0,
            repeat: Repeat::Once,
            easing: Easing::Linear,
            tracks: vec![],
            color: vec![],
            transition: None,
            fps: 30,
        }
    }
}

impl Motion {
    pub fn progress(value: f32, duration_ms: u32) -> Self {
        Self {
            transition: Some(ValueTransition {
                property: MotionProperty::Progress,
                value,
                duration_ms,
                easing: Easing::EaseOut,
            }),
            ..Self::default()
        }
    }
}

impl MotionTrack {
    pub fn new(property: MotionProperty, frames: &[(u32, f32)]) -> Self {
        Self {
            property,
            frames: frames
                .iter()
                .map(|&(at_ms, value)| Keyframe {
                    at_ms,
                    value,
                    easing: None,
                })
                .collect(),
            amplitude: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteFrame {
    pub asset: String,
    pub duration_ms: u32,
    /// Normalized source rectangle, left/top/right/bottom.
    #[serde(default = "full_uv")]
    pub uv: [f32; 4],
}

pub(super) fn full_uv() -> [f32; 4] {
    [0.0, 0.0, 1.0, 1.0]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteSequence {
    pub frames: Vec<SpriteFrame>,
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default)]
    pub playback: Playback,
    #[serde(default)]
    pub repeat: Repeat,
    #[serde(default)]
    pub delay_ms: u32,
    #[serde(default)]
    pub gap_ms: u32,
    #[serde(default = "normal_speed")]
    pub speed: f32,
}

fn normal_speed() -> f32 {
    1.0
}
