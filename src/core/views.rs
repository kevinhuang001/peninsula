//! Retain extension snapshots until the host observes an invalidation or the
//! declared refresh deadline. A missing widget is cached too.
use std::{
    collections::HashMap,
    hash::Hash,
    time::{Duration, Instant},
};

pub struct ViewCache<K, V>(HashMap<K, Entry<V>>);

struct Entry<V> {
    value: V,
    refresh_at: Option<Instant>,
}

impl<K, V> Default for ViewCache<K, V> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<K: Eq + Hash, V> ViewCache<K, V> {
    pub fn get(&self, id: &K) -> Option<&V> {
        self.0.get(id).map(|entry| &entry.value)
    }

    pub fn invalidate(&mut self, id: &K) {
        self.0.remove(id);
    }

    pub fn get_or_build(
        &mut self,
        id: K,
        now: Instant,
        build: impl FnOnce() -> (V, Option<Duration>),
    ) -> &V {
        use std::collections::hash_map::Entry as Slot;
        let create = || {
            let (value, interval) = build();
            Entry {
                value,
                refresh_at: interval.and_then(|period| now.checked_add(period)),
            }
        };
        let entry = match self.0.entry(id) {
            Slot::Vacant(slot) => slot.insert(create()),
            Slot::Occupied(mut slot) => {
                if slot
                    .get()
                    .refresh_at
                    .is_some_and(|deadline| now >= deadline)
                {
                    slot.insert(create());
                }
                slot.into_mut()
            }
        };
        &entry.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn rebuilding_is_limited_to_expiry_and_invalidated_sources() {
        let mut cache = ViewCache::default();
        let start = Instant::now();
        let calls = Cell::new(0);
        let build = || {
            calls.set(calls.get() + 1);
            (Some(calls.get()), Some(Duration::from_secs(1)))
        };
        assert_eq!(*cache.get_or_build("timer", start, build), Some(1));
        assert_eq!(
            *cache.get_or_build("timer", start + Duration::from_millis(999), build),
            Some(1)
        );
        assert_eq!(*cache.get_or_build("absent", start, || (None, None)), None);
        assert_eq!(
            *cache.get_or_build("timer", start + Duration::from_secs(1), build),
            Some(2)
        );
        assert_eq!(
            *cache.get_or_build("absent", start + Duration::from_secs(300), build),
            None
        );
        cache.invalidate(&"timer");
        assert_eq!(
            *cache.get_or_build("timer", start + Duration::from_secs(1), build),
            Some(3)
        );
        assert_eq!(calls.get(), 3);
    }
}
