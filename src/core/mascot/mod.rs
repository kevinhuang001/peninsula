mod animation;
mod expression;
mod renderer;
mod scheduler;
#[cfg(test)]
mod tests;

use crate::{
    extension::{CompactView, ExtensionId},
    ui::declarative::{Node, Size, ViewNode},
};
pub use animation::MascotPose;
pub use expression::{MascotExpression, MascotMotion, Trigger};
use rand::Rng;
pub(crate) use renderer::paint;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const SLOT_WIDTH: f32 = 38.0;
pub const MASCOT_SLOT_ID: &str = "core.mascot/slot";
pub type MascotSource = (ExtensionId, Vec<MascotExpression>, BTreeMap<String, String>);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MotionPolicy {
    #[default]
    Full,
    Reduced,
    Off,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnlockMascotState {
    Idle,
    Typing,
    Attentive,
    Verifying,
    Failed,
    Succeeded,
}

impl UnlockMascotState {
    fn motion(self) -> MascotMotion {
        match self {
            Self::Idle => MascotMotion::Idle,
            Self::Typing | Self::Attentive => MascotMotion::Typing,
            Self::Verifying => MascotMotion::Verifying,
            Self::Failed => MascotMotion::Error,
            Self::Succeeded => MascotMotion::Success,
        }
    }
}

struct Playback {
    action: scheduler::Action,
    start: Instant,
    from: MascotPose,
}

#[derive(Default)]
pub struct MascotHost {
    scheduler: scheduler::Scheduler,
    playback: Option<Playback>,
    idle_at: Option<Instant>,
    settling: Option<(Instant, MascotPose)>,
    idle_clock: BreathClock,
    unlock_clock: BreathClock,
    paused_at: Option<Instant>,
    unlock: Option<(UnlockMascotState, Instant, MascotPose)>,
    pose: MascotPose,
}

impl MascotHost {
    pub fn node(&self) -> ViewNode {
        ViewNode::new(Node::CoreMascot(self.pose)).sized(Size::Fill(1.0), Size::Fill(1.0))
    }

    pub fn compact_slot(&self) -> CompactView {
        let id = ExtensionId::builtin(MASCOT_SLOT_ID);
        CompactView {
            id: id.clone(),
            source_extension_id: id,
            content: self.node(),
            visibility: Default::default(),
            min_width: SLOT_WIDTH,
            preferred_width: SLOT_WIDTH,
            fit_width: false,
            priority: 0,
            tooltip: None,
        }
    }

    pub fn event(&mut self, event: &str, sources: &[MascotSource], now: Instant) {
        self.scheduler.publish(event, sources, now);
    }

    fn pause(&mut self, now: Instant) {
        self.paused_at.get_or_insert(now);
    }

    fn resume(&mut self, now: Instant) {
        if let Some(paused) = self.paused_at.take() {
            let gap = now.saturating_duration_since(paused);
            if let Some(playback) = &mut self.playback {
                playback.start += gap;
            }
            if let Some((start, _)) = &mut self.settling {
                *start += gap;
            }
            if let Some(at) = &mut self.idle_at {
                *at += gap;
            }
            for at in [&mut self.idle_clock.blink_at, &mut self.idle_clock.epoch]
                .into_iter()
                .flatten()
            {
                *at += gap;
            }
        }
    }

    fn frame(policy: MotionPolicy, active: bool) -> Option<Duration> {
        match policy {
            MotionPolicy::Full => Some(Duration::from_millis(if active { 32 } else { 72 })),
            MotionPolicy::Reduced => Some(Duration::from_millis(if active { 64 } else { 100 })),
            MotionPolicy::Off => None,
        }
    }

    pub fn update(
        &mut self,
        sources: &[MascotSource],
        visible: bool,
        now: Instant,
        policy: MotionPolicy,
    ) -> Option<Duration> {
        self.scheduler.prune(sources, now);
        if self
            .playback
            .as_ref()
            .is_some_and(|p| !p.action.valid(sources))
        {
            self.playback = None;
            self.settling = Some((self.paused_at.unwrap_or(now), self.pose));
            self.idle_at = Some(self.paused_at.unwrap_or(now) + scheduler::idle_pause());
        }
        if !visible {
            self.pause(now);
            return None;
        }
        if self.unlock.take().is_some() {
            self.unlock_clock = BreathClock::default();
        }
        self.resume(now);
        let (time, blink) = self.idle_clock.sample(now);
        let idle = animation::sample(MascotMotion::Idle, 0.0, time, blink, policy);
        let complete = self
            .playback
            .as_ref()
            .is_some_and(|p| now >= p.start + p.action.duration);
        if complete {
            self.playback = None;
            self.pose = idle;
        }
        let idle_at = *self
            .idle_at
            .get_or_insert_with(|| now + scheduler::first_pause());
        let event = if self.playback.as_ref().is_none_or(|p| !p.action.event) {
            self.scheduler.pop_event()
        } else {
            None
        };
        let next = event.or_else(|| {
            (self.playback.is_none() && now >= idle_at && policy != MotionPolicy::Off)
                .then(|| self.scheduler.choose(sources, &mut rand::rng()))
        });
        if let Some(action) = next {
            self.settling = None;
            self.idle_at = Some(now + action.duration + scheduler::idle_pause());
            self.playback = Some(Playback {
                action,
                start: now,
                from: self.pose,
            });
        }
        let deadline = if let Some(p) = &self.playback {
            let elapsed = now.saturating_duration_since(p.start).as_secs_f32();
            let duration = p.action.duration.as_secs_f32();
            let expression =
                animation::sample(p.action.motion, elapsed / duration, time, blink, policy);
            self.pose = if policy == MotionPolicy::Off {
                expression
            } else {
                p.from
                    .blend(expression, animation::ease(elapsed / 0.12))
                    .blend(idle, animation::ease(1.0 - (duration - elapsed) / 0.20))
            };
            Some(p.start + p.action.duration)
        } else {
            self.pose = idle;
            if let Some((start, from)) = self.settling {
                let elapsed = now.saturating_duration_since(start).as_secs_f32();
                if policy != MotionPolicy::Off && elapsed < 0.20 {
                    self.pose = from.blend(idle, animation::ease(elapsed / 0.20));
                } else {
                    self.settling = None;
                }
            }
            (policy != MotionPolicy::Off).then_some(idle_at)
        };
        deadline
            .map(|at| at.saturating_duration_since(now))
            .into_iter()
            .chain(Self::frame(
                policy,
                self.playback.is_some() || self.settling.is_some() || self.idle_clock.blinking(now),
            ))
            .chain(self.idle_clock.next_blink(now, policy))
            .min()
    }

    pub fn update_unlock(
        &mut self,
        state: UnlockMascotState,
        now: Instant,
        policy: MotionPolicy,
    ) -> Option<Duration> {
        self.pause(now);
        if self.unlock.is_none_or(|(previous, _, _)| previous != state) {
            self.unlock = Some((state, now, self.pose));
        }
        let (_, start, from) = self.unlock.expect("unlock pose");
        let (time, blink) = self.unlock_clock.sample(now);
        let elapsed = now.saturating_duration_since(start).as_secs_f32();
        let motion = state.motion();
        let ratio = elapsed / motion.duration().as_secs_f32();
        let phase = match state {
            UnlockMascotState::Failed | UnlockMascotState::Succeeded => ratio.min(1.0),
            UnlockMascotState::Attentive => 0.0,
            _ => ratio.fract(),
        };
        let mut pose = animation::sample(motion, phase, time, blink, policy);
        if state == UnlockMascotState::Succeeded && policy != MotionPolicy::Off {
            let idle = animation::sample(MascotMotion::Idle, 0.0, time, blink, policy);
            pose = pose.blend(idle, animation::ease((ratio - 0.76) / 0.24));
        }
        self.pose = if policy == MotionPolicy::Off {
            pose
        } else {
            from.blend(pose, animation::ease(elapsed / 0.12))
        };
        Self::frame(
            policy,
            self.unlock_clock.blinking(now)
                || !matches!(
                    state,
                    UnlockMascotState::Idle | UnlockMascotState::Attentive
                ),
        )
        .into_iter()
        .chain(self.unlock_clock.next_blink(now, policy))
        .min()
    }
}

#[derive(Default)]
struct BreathClock {
    epoch: Option<Instant>,
    blink_at: Option<Instant>,
}

impl BreathClock {
    const BLINK_DURATION: Duration = Duration::from_millis(190);

    fn blinking(&self, now: Instant) -> bool {
        self.blink_at
            .is_some_and(|start| now >= start && now - start < Self::BLINK_DURATION)
    }

    fn next_blink(&self, now: Instant, policy: MotionPolicy) -> Option<Duration> {
        self.blink_at
            .filter(|at| *at > now && policy != MotionPolicy::Off)
            .map(|at| at - now)
    }

    fn sample(&mut self, now: Instant) -> (f32, f32) {
        let time = now
            .saturating_duration_since(*self.epoch.get_or_insert(now))
            .as_secs_f32();
        let mut rng = rand::rng();
        let blink = *self
            .blink_at
            .get_or_insert_with(|| now + Duration::from_millis(rng.random_range(3000..=7000)));
        let elapsed = now.saturating_duration_since(blink);
        if elapsed >= Self::BLINK_DURATION {
            let interval = Duration::from_millis(rng.random_range(3000..=7000));
            let next = blink + interval;
            self.blink_at = Some(if next > now { next } else { now + interval });
        }
        let amount = if self.blinking(now) {
            animation::blink_amount(elapsed.as_secs_f32() / Self::BLINK_DURATION.as_secs_f32())
        } else {
            0.0
        };
        (time, amount)
    }
}
