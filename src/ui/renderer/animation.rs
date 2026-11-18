use super::Renderer;
use crate::ui::declarative::*;
use eframe::egui::{self, Color32, Id, Rect, Ui, Vec2};
use std::{collections::HashMap, time::Duration};

const MAX_KEYFRAMES: usize = 64;
const MAX_SPRITE_FRAMES: usize = 256;
const MAX_STATES: usize = 1024;

#[derive(Clone, Copy)]
struct Clock {
    time: f64,
    pass: u64,
}

impl Clock {
    fn new(ui: &Ui) -> Self {
        Self {
            time: ui.input(|input| input.time),
            pass: ui.ctx().cumulative_pass_nr(),
        }
    }
}

#[derive(Clone, Default)]
struct State {
    trigger: Option<Trigger>,
    playback: Playback,
    started: bool,
    elapsed: f64,
    transition: Option<Transition>,
    last: Option<Clock>,
}

#[derive(Clone, Copy)]
struct Transition {
    property: MotionProperty,
    from: f32,
    target: f32,
    elapsed: f64,
}

#[derive(Clone, Default)]
struct States(HashMap<Id, State>);

#[derive(Clone, Copy)]
struct Timing {
    duration: f64,
    delay: f64,
    gap: f64,
    repeat: Repeat,
}

struct Sample {
    at: f64,
    /// Seconds until the next change, including any initial delay or loop gap.
    wake: Option<f64>,
    backward: bool,
}

impl Timing {
    fn sample(self, elapsed: f64) -> Sample {
        if elapsed < self.delay {
            return Sample {
                at: 0.0,
                wake: Some(self.delay - elapsed),
                backward: false,
            };
        }
        let (count, alternates) = match self.repeat {
            Repeat::Once => (Some(1), false),
            Repeat::Count(n) => (Some(n.max(1)), false),
            Repeat::Reverse(n) => (n.map(|n| n.max(1)), true),
            Repeat::Infinite => (None, false),
        };
        let elapsed = elapsed - self.delay;
        let cycle = self.duration + self.gap;
        let pass = (elapsed / cycle).floor() as u64;
        if let Some(count) = count {
            let count = u64::from(count);
            if pass >= count || (pass == count - 1 && elapsed % cycle >= self.duration) {
                let backward = alternates && count % 2 == 0;
                return Sample {
                    at: if backward { 0.0 } else { self.duration },
                    wake: None,
                    backward,
                };
            }
        }
        let backward = alternates && pass % 2 == 1;
        let within = elapsed % cycle;
        let at = within.min(self.duration);
        Sample {
            at: if backward { self.duration - at } else { at },
            wake: Some(if within < self.duration {
                0.0
            } else {
                cycle - within
            }),
            backward,
        }
    }
}

impl State {
    fn step(&mut self, clock: Clock, trigger: &Trigger, playback: Playback) -> (f64, Playback) {
        if self.trigger.as_ref().is_some_and(|previous| {
            std::mem::discriminant(previous) != std::mem::discriminant(trigger)
        }) {
            self.started = false;
            self.elapsed = 0.0;
        }
        let delta = self
            .last
            .filter(|last| last.pass.saturating_add(1) == clock.pass)
            .map_or(0.0, |last| (clock.time - last.time).max(0.0));
        self.last = Some(clock);
        if self.started && self.playback == Playback::Running {
            self.elapsed += delta;
        }
        let next = match trigger {
            Trigger::Always | Trigger::While(true) => Playback::Running,
            Trigger::While(false) => Playback::Stopped,
            Trigger::OnChange(value) => {
                if matches!(&self.trigger, Some(Trigger::OnChange(previous)) if previous != value) {
                    self.elapsed = 0.0;
                    self.started = true;
                }
                if self.started {
                    Playback::Running
                } else {
                    Playback::Stopped
                }
            }
            Trigger::State {
                current,
                active,
                paused,
            } => {
                if current == active {
                    Playback::Running
                } else if paused.as_ref() == Some(current) {
                    Playback::Paused
                } else {
                    Playback::Stopped
                }
            }
        };
        if self.trigger.as_ref() != Some(trigger) {
            self.trigger = Some(trigger.clone());
        }
        self.playback = match (next, playback) {
            (Playback::Stopped, _) | (_, Playback::Stopped) => Playback::Stopped,
            (Playback::Paused, _) | (_, Playback::Paused) => Playback::Paused,
            _ => Playback::Running,
        };
        match self.playback {
            Playback::Stopped => {
                self.started = false;
                self.elapsed = 0.0;
            }
            Playback::Running => self.started = true,
            Playback::Paused => {}
        }
        (delta, self.playback)
    }

    fn transition(&mut self, spec: ValueTransition, delta: f64, playback: Playback) -> (f32, bool) {
        let target = finite(spec.value, spec.property.rest());
        let duration = seconds(spec.duration_ms);
        let mut current = self
            .transition
            .filter(|previous| previous.property == spec.property)
            .unwrap_or(Transition {
                property: spec.property,
                from: target,
                target,
                elapsed: duration,
            });
        current.elapsed = match playback {
            Playback::Running => current.elapsed + delta,
            Playback::Paused => current.elapsed,
            Playback::Stopped => duration,
        };
        let value_at = |state: Transition| {
            let progress = if duration > 0.0 {
                (state.elapsed / duration).min(1.0) as f32
            } else {
                1.0
            };
            mix(state.from, state.target, ease(spec.easing, progress))
        };
        if current.target != target {
            current = Transition {
                property: spec.property,
                from: value_at(current),
                target,
                elapsed: if playback == Playback::Stopped {
                    duration
                } else {
                    0.0
                },
            };
        }
        self.transition = Some(current);
        (
            value_at(current),
            playback == Playback::Running && current.elapsed < duration && current.from != target,
        )
    }
}

fn with_state<R>(ui: &Ui, id: Id, clock: Clock, run: impl FnOnce(&mut State) -> R) -> Option<R> {
    let id = id.with(ui.ctx().viewport_id());
    ui.data_mut(|data| {
        let states = &mut data
            .get_temp_mut_or_default::<States>(Id::new("declarative-animation-states"))
            .0;
        if states.len() >= MAX_STATES && !states.contains_key(&id) {
            states.retain(|_, state| state.last.is_some_and(|last| clock.time - last.time < 30.0));
            if states.len() >= MAX_STATES {
                return None;
            }
        }
        Some(run(states.entry(id).or_default()))
    })
}

pub(crate) fn set_reduced_motion(ctx: &egui::Context, reduced: bool) {
    ctx.data_mut(|data| data.insert_temp(Id::new("declarative-reduced-motion"), reduced));
}

pub(crate) fn reduced_motion(ctx: &egui::Context) -> bool {
    ctx.data(|data| {
        data.get_temp(Id::new("declarative-reduced-motion"))
            .unwrap_or(false)
    })
}

impl MotionProperty {
    fn rest(self) -> f32 {
        match self {
            Self::Opacity | Self::Scale => 1.0,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Effects {
    pub translation: Vec2,
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub color: Option<Color32>,
    pub progress: Option<f32>,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            scale: 1.0,
            rotation: 0.0,
            opacity: 1.0,
            color: None,
            progress: None,
        }
    }
}

impl Effects {
    fn set(&mut self, property: MotionProperty, value: f32) {
        use MotionProperty::*;
        let value = finite(value, property.rest());
        match property {
            TranslateX => self.translation.x = value.clamp(-256.0, 256.0),
            TranslateY => self.translation.y = value.clamp(-256.0, 256.0),
            Scale => self.scale = value.clamp(0.01, 8.0),
            Rotation => self.rotation = value.rem_euclid(std::f32::consts::TAU),
            Opacity => self.opacity = value.clamp(0.0, 1.0),
            Progress => self.progress = Some(value.clamp(0.0, 1.0)),
        }
    }
}

impl Renderer<'_> {
    pub(super) fn effects(
        &mut self,
        ui: &Ui,
        id: Id,
        spec: &Motion,
        supports_progress: bool,
    ) -> Effects {
        let mut effects = Effects::default();
        let reduced = reduced_motion(ui.ctx());
        let transition = spec.transition.filter(|transition| {
            supports_progress || transition.property != MotionProperty::Progress
        });
        let snap = |mut effects: Effects| {
            if let Some(transition) = transition
                .filter(|transition| !reduced || transition.property == MotionProperty::Progress)
            {
                effects.set(transition.property, transition.value);
            }
            effects
        };
        if self.animations_remaining == 0 || reduced {
            return snap(effects);
        }
        let tracks = || {
            spec.tracks.iter().take(16).filter(|track| {
                !track.frames.is_empty()
                    && (supports_progress || track.property != MotionProperty::Progress)
            })
        };
        let has_timeline = tracks().next().is_some() || !spec.color.is_empty();
        if !has_timeline && transition.is_none() {
            return effects;
        }
        self.animations_remaining -= 1;
        let clock = Clock::new(ui);
        let Some((timing, transition, animate)) =
            with_state(ui, id.with("motion"), clock, |state| {
                let (delta, playback) = state.step(clock, &spec.trigger, spec.playback);
                // A data transition can run independently of an OnChange timeline.
                let data_playback = if matches!(spec.trigger, Trigger::OnChange(_)) {
                    spec.playback
                } else {
                    playback
                };
                let transition = transition.map(|transition| {
                    let (value, active) = state.transition(transition, delta, data_playback);
                    (transition.property, value, active)
                });
                let duration = seconds(spec.duration_ms).max(0.001);
                let sample = (state.started && has_timeline).then(|| {
                    let repeat = if matches!(spec.trigger, Trigger::OnChange(_)) {
                        Repeat::Once
                    } else {
                        spec.repeat
                    };
                    Timing {
                        duration,
                        delay: seconds(spec.delay_ms),
                        gap: seconds(spec.gap_ms),
                        repeat,
                    }
                    .sample(state.elapsed)
                });
                (sample, transition, playback == Playback::Running)
            })
        else {
            return snap(effects);
        };
        let interval = 1.0 / f64::from(spec.fps.clamp(1, 60));
        if let Some(sample) = timing {
            let at = sample.at * 1000.0;
            for track in tracks() {
                if let Some((a, b, t)) = keyframes(&track.frames, at, spec.easing) {
                    let value = mix(a, b, t);
                    let rest = track.property.rest();
                    effects.set(
                        track.property,
                        rest + (value - rest) * track.amplitude.map_or(1.0, mapping),
                    );
                }
            }
            if let Some((a, b, t)) = keyframes(&spec.color, at, spec.easing) {
                let from = egui::Rgba::from(self.theme.color(ui, a));
                let to = egui::Rgba::from(self.theme.color(ui, b));
                effects.color = Some(Color32::from(from * (1.0 - t) + to * t));
            }
            if animate {
                if let Some(wake) = sample.wake {
                    repaint(ui, if wake > 0.0 { wake } else { interval });
                }
            }
        }
        if let Some((property, value, active)) = transition {
            effects.set(property, value);
            if active {
                repaint(ui, interval);
            }
        }
        effects
    }

    pub(super) fn sprite_index(&mut self, ui: &Ui, id: Id, spec: &SpriteSequence) -> usize {
        let frames = &spec.frames[..spec.frames.len().min(MAX_SPRITE_FRAMES)];
        if frames.len() < 2 || self.animations_remaining == 0 || reduced_motion(ui.ctx()) {
            return 0;
        }
        self.animations_remaining -= 1;
        let speed = f64::from(finite(spec.speed, 1.0).clamp(0.05, 8.0));
        let frame_duration = |frame: &SpriteFrame| seconds(frame.duration_ms).max(0.016) / speed;
        let duration = frames.iter().map(frame_duration).sum();
        let clock = Clock::new(ui);
        let Some((sample, running)) = with_state(ui, id.with("sprite"), clock, |state| {
            let (_, playback) = state.step(clock, &spec.trigger, spec.playback);
            let repeat = if matches!(spec.trigger, Trigger::OnChange(_)) {
                Repeat::Once
            } else {
                spec.repeat
            };
            let timing = Timing {
                duration,
                delay: seconds(spec.delay_ms),
                gap: seconds(spec.gap_ms),
                repeat,
            };
            (timing.sample(state.elapsed), playback == Playback::Running)
        }) else {
            return 0;
        };
        let mut end = 0.0;
        let mut start = 0.0;
        let index = frames
            .iter()
            .position(|frame| {
                start = end;
                end += frame_duration(frame);
                end > sample.at
            })
            .unwrap_or(frames.len() - 1);
        if running {
            if let Some(wake) = sample.wake {
                let boundary = if sample.backward {
                    sample.at - start
                } else {
                    end - sample.at
                };
                repaint(
                    ui,
                    if wake > 0.0 {
                        wake
                    } else {
                        boundary.max(1.0 / 60.0)
                    },
                );
            }
        }
        index
    }

    pub(super) fn uniform_sprite_index(
        &mut self,
        ui: &Ui,
        id: Id,
        count: usize,
        frame_ms: u32,
    ) -> usize {
        if count < 2 || self.animations_remaining == 0 || reduced_motion(ui.ctx()) {
            return 0;
        }
        self.animations_remaining -= 1;
        let period = seconds(frame_ms).max(0.016);
        let clock = Clock::new(ui);
        let Some(elapsed) = with_state(ui, id.with("sprite"), clock, |state| {
            state.step(clock, &Trigger::Always, Playback::Running);
            state.elapsed
        }) else {
            return 0;
        };
        repaint(ui, period - elapsed % period);
        (elapsed / period) as usize % count.min(MAX_SPRITE_FRAMES)
    }
}

fn repaint(ui: &Ui, seconds: f64) {
    ui.ctx()
        .request_repaint_after(Duration::from_secs_f64(seconds.clamp(0.001, 1_000_000.0)));
}

fn seconds(ms: u32) -> f64 {
    f64::from(ms) / 1000.0
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn mapping(range: ValueMapping) -> f32 {
    let [start, end] = range.input;
    let t = finite((range.value - start) / (end - start), 0.0).clamp(0.0, 1.0);
    finite(mix(range.output[0], range.output[1], t), 1.0)
}

fn keyframes<T: Copy>(frames: &[Keyframe<T>], at: f64, easing: Easing) -> Option<(T, T, f32)> {
    let mut before = None;
    let mut after = None;
    for frame in frames.iter().take(MAX_KEYFRAMES) {
        if f64::from(frame.at_ms) <= at {
            if before.is_none_or(|previous: &Keyframe<T>| previous.at_ms <= frame.at_ms) {
                before = Some(frame);
            }
        } else if after.is_none_or(|next: &Keyframe<T>| next.at_ms > frame.at_ms) {
            after = Some(frame);
        }
    }
    match (before, after) {
        (Some(a), Some(b)) => {
            let t = ((at - f64::from(a.at_ms)) / f64::from(b.at_ms - a.at_ms)) as f32;
            Some((a.value, b.value, ease(a.easing.unwrap_or(easing), t)))
        }
        (Some(frame), None) | (None, Some(frame)) => Some((frame.value, frame.value, 0.0)),
        _ => None,
    }
}

fn ease(easing: Easing, t: f32) -> f32 {
    match easing {
        Easing::Linear => t,
        Easing::EaseIn => t * t,
        Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        Easing::EaseInOut => t * t * (3.0 - 2.0 * t),
        Easing::CubicBezier(points) => {
            let [x1, y1, x2, y2] = points.map(|v| finite(v, 0.0).clamp(0.0, 1.0));
            let bezier = |p: f32, a: f32, b: f32| {
                let inv = 1.0 - p;
                3.0 * inv * inv * p * a + 3.0 * inv * p * p * b + p * p * p
            };
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..12 {
                let center = (low + high) * 0.5;
                if bezier(center, x1, x2) < t {
                    low = center;
                } else {
                    high = center;
                }
            }
            bezier((low + high) * 0.5, y1, y2)
        }
    }
}

impl Effects {
    pub(super) fn has_transform(self) -> bool {
        self.translation != Vec2::ZERO || self.scale != 1.0 || self.rotation != 0.0
    }

    pub(super) fn transform_shapes(
        self,
        ui: &Ui,
        start: egui::layers::ShapeIdx,
        center: egui::Pos2,
    ) {
        let layer = ui.layer_id();
        let clip = ui.clip_rect();
        let transform = egui::emath::TSTransform::new(
            self.translation + center.to_vec2() * (1.0 - self.scale),
            self.scale,
        );
        let rotation = egui::emath::Rot2::from_angle(self.rotation);
        let rotation_center = center + self.translation;
        let transform_clip = |rect: Rect| {
            if rect == clip {
                return clip;
            }
            let rect = transform.mul_rect(rect);
            let bounds = if self.rotation == 0.0 {
                rect
            } else {
                Rect::from_points(
                    &[
                        rect.left_top(),
                        rect.right_top(),
                        rect.left_bottom(),
                        rect.right_bottom(),
                    ]
                    .map(|point| rotation_center + rotation * (point - rotation_center)),
                )
            };
            bounds.intersect(clip)
        };
        if self.rotation == 0.0 {
            ui.ctx().graphics_mut(|graphics| {
                let list = graphics.entry(layer);
                for index in start.0..list.next_idx().0 {
                    list.mutate_shape(egui::layers::ShapeIdx(index), |entry| {
                        entry.shape.transform(transform);
                        entry.clip_rect = transform_clip(entry.clip_rect);
                    });
                }
            });
            return;
        }

        let shapes = ui.ctx().graphics(|graphics| {
            graphics.get(layer).map_or_else(Vec::new, |list| {
                list.all_entries().skip(start.0).cloned().collect()
            })
        });
        // Tessellate before rotation so text, circles and textures share the same transform.
        for (index, mut entry) in shapes.into_iter().enumerate() {
            let bounds = transform_clip(entry.clip_rect);
            entry.shape.transform(transform);
            entry.clip_rect = Rect::EVERYTHING;
            let meshes = ui
                .ctx()
                .tessellate(vec![entry], ui.ctx().pixels_per_point());
            let shapes = meshes
                .into_iter()
                .filter_map(|item| {
                    if let egui::epaint::Primitive::Mesh(mut mesh) = item.primitive {
                        mesh.rotate(rotation, rotation_center);
                        Some(egui::Shape::mesh(mesh))
                    } else {
                        None
                    }
                })
                .collect();
            ui.ctx().graphics_mut(|graphics| {
                graphics.entry(layer).set(
                    egui::layers::ShapeIdx(start.0 + index),
                    bounds,
                    egui::Shape::Vec(shapes),
                );
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(pass: u64, time: f64) -> Clock {
        Clock { pass, time }
    }

    #[test]
    fn reverse_timeline_holds_during_delay_and_gap_then_stops() {
        let timing = Timing {
            duration: 1.0,
            delay: 0.25,
            gap: 0.25,
            repeat: Repeat::Reverse(Some(2)),
        };
        let delayed = timing.sample(0.125);
        assert_eq!((delayed.at, delayed.wake), (0.0, Some(0.125)));
        assert_eq!(timing.sample(0.75).at, 0.5);
        let gap = timing.sample(1.375);
        assert_eq!((gap.at, gap.wake), (1.0, Some(0.125)));
        let reverse = timing.sample(1.75);
        assert_eq!((reverse.at, reverse.backward), (0.75, true));
        let finished = timing.sample(2.5);
        assert_eq!((finished.at, finished.wake), (0.0, None));
    }

    #[test]
    fn state_pauses_on_hidden_passes_and_resumes_from_the_same_position() {
        let mut state = State::default();
        let trigger = |current: &str| Trigger::State {
            current: current.into(),
            active: "playing".into(),
            paused: Some("paused".into()),
        };
        for (pass, time, current, elapsed) in [
            (1, 2.0, "playing", 0.0),
            (2, 2.25, "playing", 0.25),
            (8, 84.0, "playing", 0.25),
            (9, 84.25, "paused", 0.5),
            (10, 89.0, "paused", 0.5),
            (11, 92.0, "playing", 0.5),
            (12, 92.25, "playing", 0.75),
            (13, 92.5, "stopped", 0.0),
        ] {
            state.step(clock(pass, time), &trigger(current), Playback::Running);
            assert_eq!(state.elapsed, elapsed, "{current} at pass {pass}");
        }
    }

    #[test]
    fn on_change_plays_from_the_start_without_animating_the_initial_value() {
        let mut state = State::default();
        let first = Trigger::OnChange("downloading".into());
        let next = Trigger::OnChange("complete".into());
        assert_eq!(
            state.step(clock(1, 0.0), &first, Playback::Running).1,
            Playback::Stopped
        );
        assert_eq!(
            state.step(clock(2, 0.5), &next, Playback::Running).1,
            Playback::Running
        );
        assert_eq!(state.elapsed, 0.0);
        state.step(clock(3, 0.75), &next, Playback::Running);
        assert_eq!(state.elapsed, 0.25);
        state.step(clock(4, 1.0), &first, Playback::Running);
        assert_eq!(state.elapsed, 0.0);
    }

    #[test]
    fn progress_retargets_from_the_current_interpolated_value() {
        let mut state = State::default();
        let spec = |value| ValueTransition {
            property: MotionProperty::Progress,
            value,
            duration_ms: 1000,
            easing: Easing::Linear,
        };
        assert_eq!(
            state.transition(spec(0.25), 0.0, Playback::Running),
            (0.25, false)
        );
        assert_eq!(
            state.transition(spec(0.75), 0.0, Playback::Running),
            (0.25, true)
        );
        assert_eq!(
            state.transition(spec(1.0), 0.5, Playback::Running),
            (0.5, true)
        );
        assert_eq!(
            state.transition(spec(1.0), 0.5, Playback::Running),
            (0.75, true)
        );
        assert_eq!(
            state.transition(spec(1.0), 1.0, Playback::Running),
            (1.0, false)
        );
    }

    #[test]
    fn keyframes_allow_unsorted_times_and_per_segment_easing() {
        let frames = [
            Keyframe {
                at_ms: 100,
                value: 10.0,
                easing: None,
            },
            Keyframe {
                at_ms: 0,
                value: 0.0,
                easing: Some(Easing::EaseIn),
            },
        ];
        assert_eq!(
            keyframes(&frames, 50.0, Easing::Linear),
            Some((0.0, 10.0, 0.25))
        );
        assert!((ease(Easing::CubicBezier([0.5, 0.5, 0.75, 0.75]), 0.6) - 0.6).abs() < 0.001);
    }
}
