use crate::extension::ExtensionId;
use rand::seq::IndexedRandom;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};
mod renderer;
pub use renderer::register_assets;
pub const MASCOT_SLOT_ID: &str = "core.mascot/slot";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MotionPolicy {
    #[default]
    Full,
    Reduced,
    Off,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    While(Vec<(String, String)>),
    OnEvent(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MascotExpression {
    pub id: String,
    pub name: String,
    /// Index into the host's bundled sprite resource catalog.
    pub asset: usize,
    /// An optional declaration can use SVG, raster, sprite sequences or Motion nodes.
    pub view: Option<crate::extension::ViewNode>,
    pub trigger: Trigger,
    pub priority: i32,
    pub duration: Duration,
}

impl MascotExpression {
    pub fn event(id: &str, event: &str, asset: usize) -> Self {
        Self {
            id: id.into(),
            name: id.into(),
            asset,
            view: None,
            trigger: Trigger::OnEvent(event.into()),
            priority: 10,
            duration: Duration::from_secs(5),
        }
    }

    pub fn state(id: &str, key: &str, value: &str, asset: usize) -> Self {
        Self {
            id: id.into(),
            name: id.into(),
            asset,
            view: None,
            trigger: Trigger::While(vec![(key.into(), value.into())]),
            priority: 0,
            duration: Duration::from_secs(7),
        }
    }
}

#[derive(Default)]
pub struct MascotHost {
    renderer: renderer::Mascot,
    current: Option<(ExtensionId, String)>,
    candidates: BTreeSet<(ExtensionId, String)>,
    event_deadlines: BTreeMap<(ExtensionId, String), Instant>,
    deadline: Option<Instant>,
    asset: usize,
    view: Option<crate::extension::ViewNode>,
}

pub type MascotSource = (ExtensionId, Vec<MascotExpression>, BTreeMap<String, String>);

impl MascotHost {
    pub fn compact_slot(
        &mut self,
        size: f32,
        motion: MotionPolicy,
    ) -> crate::extension::CompactView {
        self.renderer.select(self.asset);
        let mut view = self.renderer.compact_views().remove(0);
        view.id = ExtensionId::builtin(MASCOT_SLOT_ID);
        view.source_extension_id = view.id.clone();
        if let Some(declaration) = &self.view {
            view.content = declaration.clone();
        }
        if let crate::ui::declarative::Node::Compact(compact) = &mut view.content.kind {
            compact.layout.leading_size[0] = (size - 6.0).max(1.0);
            compact.leading.size.width = crate::ui::declarative::Size::Fixed((size - 6.0).max(1.0));
        }
        view.min_width = size;
        view.preferred_width = size;
        if motion != MotionPolicy::Full {
            renderer::freeze(&mut view.content);
        }
        view
    }
    pub fn cycle_idle(&mut self) {
        if self.current.is_none() {
            let choices: Vec<_> = (0..12).filter(|asset| *asset != self.asset).collect();
            self.asset = *choices.choose(&mut rand::rng()).unwrap_or(&0);
            self.deadline = Some(Instant::now() + Duration::from_secs(7));
        }
    }
    pub fn next_wake(&self) -> Option<Instant> {
        self.deadline
            .into_iter()
            .chain(self.event_deadlines.values().copied())
            .min()
    }
    pub fn event(&mut self, event: &str, sources: &[MascotSource], now: Instant) {
        for (owner, expressions, _) in sources {
            for e in expressions {
                if matches!(&e.trigger, Trigger::OnEvent(name) if name == event) {
                    self.event_deadlines
                        .insert((owner.clone(), e.id.clone()), now + e.duration);
                }
            }
        }
    }
    pub fn update(&mut self, sources: &[MascotSource], enabled: bool, now: Instant) -> usize {
        if !enabled {
            self.current = None;
            self.candidates.clear();
            self.event_deadlines.clear();
            self.deadline = None;
            self.asset = 0;
            self.view = None;
            return 0;
        }
        self.event_deadlines.retain(|(owner, id), deadline| {
            *deadline > now
                && sources
                    .iter()
                    .any(|(o, es, _)| o == owner && es.iter().any(|e| &e.id == id))
        });
        let mut valid = Vec::new();
        for (owner, expressions, state) in sources {
            for e in expressions {
                let key = (owner.clone(), e.id.clone());
                let active = match &e.trigger {
                    Trigger::While(conditions) => {
                        !conditions.is_empty()
                            && conditions.iter().all(|(k, v)| state.get(k) == Some(v))
                    }
                    Trigger::OnEvent(_) => self.event_deadlines.contains_key(&key),
                };
                if active {
                    valid.push((key, e));
                }
            }
        }
        if let Some(priority) = valid.iter().map(|(_, e)| e.priority).max() {
            valid.retain(|(_, e)| e.priority == priority);
        }
        let candidates: BTreeSet<_> = valid.iter().map(|(key, _)| key.clone()).collect();
        if candidates != self.candidates || self.deadline.is_none_or(|deadline| now >= deadline) {
            let previous = self.current.clone();
            let mut choices: Vec<_> = valid.iter().collect();
            if choices.len() > 1 {
                choices.retain(|(key, _)| Some(key) != previous.as_ref());
            }
            if let Some((key, e)) = choices.choose(&mut rand::rng()) {
                self.current = Some(key.clone());
                self.asset = e.asset;
                self.view = e.view.clone();
                self.deadline = Some(now + e.duration.max(Duration::from_millis(100)));
            } else {
                self.current = None;
                self.asset = 0;
                self.view = None;
                self.deadline = None;
            }
            self.candidates = candidates;
        }
        self.asset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expressions_do_not_flicker_and_stopped_sources_expire_immediately() {
        let now = Instant::now();
        let sources = vec![(
            ExtensionId::builtin("music"),
            vec![MascotExpression::state("listening", "playing", "true", 3)],
            [("playing".into(), "true".into())].into(),
        )];
        let mut host = MascotHost::default();
        assert_eq!(host.update(&sources, true, now), 3);
        assert_eq!(
            host.update(&sources, true, now + Duration::from_millis(16)),
            3
        );
        assert_eq!(host.update(&[], true, now), 0);
        assert_eq!(host.update(&sources, false, now), 0);
    }
    #[test]
    fn highest_priority_event_expires_and_equal_candidates_alternate() {
        let now = Instant::now();
        let mut a = MascotExpression::state("a", "active", "true", 1);
        a.duration = Duration::from_secs(1);
        let mut b = a.clone();
        b.id = "b".into();
        b.asset = 2;
        let mut event = a.clone();
        event.id = "event".into();
        event.asset = 3;
        event.priority = 10;
        event.trigger = Trigger::OnEvent("done".into());
        let sources = vec![(
            ExtensionId::builtin("test"),
            vec![a, b, event],
            [("active".into(), "true".into())].into(),
        )];
        let mut host = MascotHost::default();
        let first = host.update(&sources, true, now);
        assert_ne!(
            first,
            host.update(&sources, true, now + Duration::from_secs(1))
        );
        host.event("done", &sources, now);
        assert_eq!(host.update(&sources, true, now), 3);
        assert_ne!(host.update(&sources, true, now + Duration::from_secs(2)), 3);
    }
}
