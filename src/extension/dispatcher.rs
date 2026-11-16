use super::{Event, ExtensionId, HostScheduler};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

#[derive(Clone, Debug, Default)]
pub struct Dispatcher {
    invalidated: HashSet<ExtensionId>,
    events: VecDeque<Event>,
    deadlines: HashMap<String, Instant>,
}

impl Dispatcher {
    pub fn take_invalidated(&mut self) -> HashSet<ExtensionId> {
        std::mem::take(&mut self.invalidated)
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = Event> + '_ {
        self.events.drain(..)
    }

    pub fn due_tasks(&mut self, now: Instant) -> Vec<String> {
        let mut due: Vec<_> = self
            .deadlines
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .map(|(task_id, _)| task_id.clone())
            .collect();
        due.sort();
        for task_id in &due {
            self.deadlines.remove(task_id);
            self.events.push_back(Event::Custom(task_id.clone()));
        }
        due
    }
}

impl HostScheduler for Dispatcher {
    fn invalidate(&mut self, extension_id: &ExtensionId) {
        self.invalidated.insert(extension_id.clone());
    }

    fn schedule_at(&mut self, task_id: &str, deadline: Instant) {
        self.deadlines.insert(task_id.to_owned(), deadline);
    }

    fn cancel(&mut self, task_id: &str) {
        self.deadlines.remove(task_id);
    }

    fn on_event(&mut self, event: Event) {
        self.events.push_back(event);
    }
}

#[cfg(test)]
mod tests {
    use super::Dispatcher;
    use crate::extension::{Event, ExtensionId, HostScheduler};
    use std::time::{Duration, Instant};

    #[test]
    fn invalidation_events_and_deadlines_are_dispatched_once() {
        let now = Instant::now();
        let mut dispatcher = Dispatcher::default();
        let extension = ExtensionId::builtin("builtin.pomodoro/timer");
        dispatcher.invalidate(&extension);
        dispatcher.on_event(Event::SnapshotUpdated);
        dispatcher.schedule_at("pomodoro", now + Duration::from_secs(1));
        dispatcher.schedule_at("cancelled", now);
        dispatcher.cancel("cancelled");
        assert_eq!(dispatcher.take_invalidated(), [extension].into());
        assert_eq!(dispatcher.due_tasks(now), Vec::<String>::new());
        assert_eq!(
            dispatcher.due_tasks(now + Duration::from_secs(1)),
            ["pomodoro"]
        );
        assert_eq!(
            dispatcher.drain_events().collect::<Vec<_>>(),
            [Event::SnapshotUpdated, Event::Custom("pomodoro".into())]
        );
        assert!(dispatcher
            .due_tasks(now + Duration::from_secs(2))
            .is_empty());
    }
}
