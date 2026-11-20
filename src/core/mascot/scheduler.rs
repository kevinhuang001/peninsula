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
    (MascotMotion::LookAround, 5),
    (MascotMotion::Blink, 2),
    (MascotMotion::Sleepy, 2),
    (MascotMotion::Happy, 4),
    (MascotMotion::Excited, 1),
    (MascotMotion::Thinking, 4),
    (MascotMotion::Surprised, 2),
    (MascotMotion::Love, 2),
];

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
    previous: Option<MascotMotion>,
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
        let select_extension = !extensions.is_empty() && rng.random_bool(0.75);
        let mut pool = if select_extension {
            extensions
        } else {
            IDLE_ACTIONS
                .iter()
                .map(|&(motion, weight)| {
                    (
                        Action {
                            motion,
                            duration: motion.duration(),
                            registration: None,
                            event: false,
                        },
                        weight,
                    )
                })
                .collect()
        };
        if !select_extension {
            pool.retain(|(action, _)| Some(action.motion) != self.previous);
        }
        let weight = |weight: u8| u32::from(weight.clamp(1, 10));
        let mut choice = rng.random_range(0..pool.iter().map(|(_, w)| weight(*w)).sum::<u32>());
        for (action, w) in pool {
            if choice < weight(w) {
                if !select_extension {
                    self.previous = Some(action.motion);
                }
                return action;
            }
            choice -= weight(w);
        }
        unreachable!("nonempty weighted action pool")
    }
}
