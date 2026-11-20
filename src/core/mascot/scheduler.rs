use super::{MascotExpression, MascotMotion, MascotSource, Trigger};
use crate::extension::ExtensionId;
use rand::Rng;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const CAPACITY: usize = 16;
const MERGE_WINDOW: Duration = Duration::from_millis(220);
const IDLE_ACTIONS: &[(MascotMotion, u8)] = &[
    (MascotMotion::LookAround, 30),
    (MascotMotion::Happy, 20),
    (MascotMotion::Thinking, 15),
    (MascotMotion::Sleepy, 15),
    (MascotMotion::Surprised, 10),
    (MascotMotion::Love, 7),
    (MascotMotion::Excited, 3),
];

pub(super) fn first_pause() -> Duration {
    Duration::from_millis(rand::rng().random_range(3000..=5000))
}

pub(super) fn idle_pause() -> Duration {
    Duration::from_millis(rand::rng().random_range(8000..=18000))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Registration(ExtensionId, String);

#[derive(Clone, Debug)]
pub(super) struct Action {
    pub motion: MascotMotion,
    pub duration: Duration,
    pub registration: Option<Registration>,
    pub event: bool,
}

impl Action {
    fn registered(owner: &ExtensionId, expression: &MascotExpression, event: bool) -> Self {
        Self {
            motion: expression.motion,
            duration: expression
                .duration
                .clamp(Duration::from_millis(180), Duration::from_secs(10)),
            registration: Some(Registration(owner.clone(), expression.id.clone())),
            event,
        }
    }

    pub fn valid(&self, sources: &[MascotSource]) -> bool {
        self.registration
            .as_ref()
            .is_none_or(|Registration(owner, id)| {
                sources.iter().any(|(source, expressions, state)| {
                    source == owner
                        && expressions.iter().any(|e| {
                            e.id == *id
                                && match &e.trigger {
                                    Trigger::While(conditions) => {
                                        !self.event && active(conditions, state)
                                    }
                                    Trigger::OnEvent { .. } => self.event,
                                }
                        })
                })
            })
    }
}

struct Pending {
    action: Action,
    received: Instant,
    expires: Instant,
}

#[derive(Default)]
pub(super) struct Scheduler {
    events: VecDeque<Pending>,
    idle_ticket: Option<u16>,
    last_played_event: Option<(Action, Instant)>,
}

fn active(
    conditions: &[(String, String)],
    state: &std::collections::BTreeMap<String, String>,
) -> bool {
    !conditions.is_empty()
        && conditions
            .iter()
            .all(|(key, value)| state.get(key) == Some(value))
}

impl Scheduler {
    pub fn prune(&mut self, sources: &[MascotSource], now: Instant) {
        self.events
            .retain(|event| event.expires > now && event.action.valid(sources));
        if self
            .last_played_event
            .as_ref()
            .is_some_and(|(action, received)| {
                now.saturating_duration_since(*received) >= MERGE_WINDOW || !action.valid(sources)
            })
        {
            self.last_played_event = None;
        }
    }

    pub fn publish(&mut self, name: &str, sources: &[MascotSource], now: Instant) {
        self.prune(sources, now);
        for (owner, expressions, _) in sources {
            for expression in expressions {
                let Trigger::OnEvent { name: trigger, ttl } = &expression.trigger else {
                    continue;
                };
                if trigger != name {
                    continue;
                }
                let action = Action::registered(owner, expression, true);
                let is_duplicate = |other: &Action, received: Instant| {
                    other.registration == action.registration
                        && now.saturating_duration_since(received) < MERGE_WINDOW
                };
                if self
                    .events
                    .iter()
                    .any(|event| is_duplicate(&event.action, event.received))
                    || self
                        .last_played_event
                        .as_ref()
                        .is_some_and(|(last, received)| is_duplicate(last, *received))
                {
                    continue;
                }
                if self.events.len() < CAPACITY {
                    self.events.push_back(Pending {
                        action,
                        received: now,
                        expires: now + (*ttl).min(Duration::from_secs(120)),
                    });
                }
            }
        }
    }

    pub fn pop_event(&mut self) -> Option<Action> {
        let event = self.events.pop_front()?;
        self.last_played_event = Some((event.action.clone(), event.received));
        Some(event.action)
    }

    pub fn choose(&mut self, sources: &[MascotSource], rng: &mut impl Rng) -> Action {
        let extensions = sources
            .iter()
            .flat_map(|(owner, expressions, state)| {
                expressions.iter().filter_map(move |e| match &e.trigger {
                    Trigger::While(conditions) if active(conditions, state) => {
                        Some((Action::registered(owner, e, false), e.weight))
                    }
                    _ => None,
                })
            })
            .collect::<Vec<_>>();
        if extensions.is_empty() || !rng.random_bool(0.75) {
            // A uniform rotation preserves the weighted ticket distribution.
            // A shift of at least the largest weight in either direction cannot repeat a motion.
            let largest = IDLE_ACTIONS
                .iter()
                .map(|&(_, weight)| u16::from(weight))
                .max()
                .unwrap();
            let total: u16 = IDLE_ACTIONS
                .iter()
                .map(|&(_, weight)| u16::from(weight))
                .sum();
            let ticket = match self.idle_ticket {
                Some(previous) => (previous + rng.random_range(largest..=total - largest)) % total,
                None => rng.random_range(0..total),
            };
            self.idle_ticket = Some(ticket);
            let mut remaining = ticket;
            for &(motion, weight) in IDLE_ACTIONS {
                let weight = u16::from(weight);
                if remaining < weight {
                    return Action {
                        motion,
                        duration: motion.duration(),
                        registration: None,
                        event: false,
                    };
                }
                remaining -= weight;
            }
            unreachable!("ticket within the idle pool");
        }
        let weight = |weight: u8| u32::from(weight.clamp(1, 10));
        let mut choice =
            rng.random_range(0..extensions.iter().map(|(_, w)| weight(*w)).sum::<u32>());
        for (action, w) in extensions {
            if choice < weight(w) {
                return action;
            }
            choice -= weight(w);
        }
        unreachable!("nonempty weighted extension pool")
    }
}
