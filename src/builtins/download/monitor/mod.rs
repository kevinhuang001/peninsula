mod chromium;
mod completion;
mod database;
mod firefox;
mod model;
mod partial;
mod paths;
mod rate;
#[cfg(target_os = "macos")]
mod safari;
mod stamp;
mod tracker;

pub use model::{Download, DownloadState, Snapshot};
use paths::{Browser, Source};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use tracker::Tracker;

pub fn start(
    tasks: &crate::sdk::tasks::TaskScope,
) -> std::io::Result<crate::core::worker::Worker<Snapshot>> {
    let mut tracker = Tracker::default();
    let mut partial_tracker = partial::CompletionTracker::default();
    let mut rates = rate::RateTracker::default();
    let mut sources = Vec::new();
    let mut last_scan: Option<Instant> = None;
    let mut active_by_source: HashMap<String, Vec<Download>> = HashMap::new();
    let mut last_read: HashMap<String, Instant> = HashMap::new();
    let mut stamps = HashMap::new();
    let mut completed_files = completion::CompletedFiles::default();
    let mut directories = HashSet::new();
    if let Some(path) = partial::default_directory() {
        directories.insert(path);
    }
    crate::core::worker::Worker::spawn_scoped(
        tasks,
        "downloads",
        Duration::from_secs(1),
        move || {
            if last_scan.is_none_or(|time| time.elapsed() >= Duration::from_secs(5)) {
                sources = paths::discover();
                active_by_source.retain(|key, _| {
                    sources
                        .iter()
                        .any(|source| source.path.to_string_lossy() == key.as_str())
                });
                last_scan = Some(Instant::now());
            }
            let mut completed = Vec::new();
            for source in &sources {
                let key = source.path.to_string_lossy().into_owned();
                if last_read
                    .get(&key)
                    .is_some_and(|time| time.elapsed() < Duration::from_secs(3))
                {
                    continue;
                }
                last_read.insert(key.clone(), Instant::now());
                let stamp = stamp::capture(&source.path);
                let has_active = active_by_source
                    .get(&key)
                    .is_some_and(|items| !items.is_empty());
                if !has_active && stamps.get(&key) == Some(&stamp) {
                    continue;
                }
                if let Ok(items) = read_source(source) {
                    stamps.insert(key.clone(), stamp);
                    for item in &items {
                        if let Some(parent) = item.path.parent() {
                            directories.insert(parent.to_path_buf());
                        }
                    }
                    completed.extend(tracker.update(&key, &items));
                    active_by_source.insert(
                        key,
                        items
                            .into_iter()
                            .filter(|item| item.state == DownloadState::Active)
                            .collect(),
                    );
                }
            }
            let mut active: Vec<_> = active_by_source.values().flatten().cloned().collect();
            let known_paths: HashSet<_> = active.iter().map(|item| item.path.clone()).collect();
            let partials = partial::scan(&directories);
            completed.extend(partial_tracker.update(&partials));
            active.extend(
                partials
                    .into_iter()
                    .filter(|item| !known_paths.contains(&item.path)),
            );
            active.sort_by(|a, b| a.id.cmp(&b.id));
            active.dedup_by(|a, b| a.path == b.path && a.browser == b.browser);
            rates.sample(&mut active, Instant::now());
            completed.retain(|item| completed_files.first_completion(&item.path));
            Snapshot { active, completed }
        },
    )
}

fn read_source(source: &Source) -> Result<Vec<Download>, Box<dyn std::error::Error>> {
    match source.browser {
        Browser::Chromium => Ok(chromium::read(&source.path, source.label)?),
        Browser::Firefox => Ok(firefox::read(&source.path)?),
        #[cfg(target_os = "macos")]
        Browser::Safari => safari::read(&source.path),
    }
}
