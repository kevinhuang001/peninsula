use crate::ui::declarative::RasterImage;
use std::{path::PathBuf, sync::Arc, time::Instant};

mod artwork;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod implementation;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod implementation;

pub(super) use implementation::Backend;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<f64>,
    pub file: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Command {
    Previous,
    Toggle,
    Next,
    Seek(u64),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Controls {
    pub previous: bool,
    pub toggle: bool,
    pub next: bool,
    pub seek: bool,
}

impl Controls {
    pub fn allows(self, command: Command) -> bool {
        match command {
            Command::Previous => self.previous,
            Command::Toggle => self.toggle,
            Command::Next => self.next,
            Command::Seek(_) => self.seek,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Playback {
    pub session: String,
    pub track: Track,
    pub playing: bool,
    /// Optional normalized speaker amplitude supplied by a playback backend.
    pub audio_level: Option<f32>,
    pub controls: Controls,
    pub position: f64,
    pub rate: f64,
    pub observed: Instant,
    pub artwork: Option<Arc<RasterImage>>,
}

impl Playback {
    pub fn position_at(&self, now: Instant) -> f64 {
        let elapsed = if self.playing {
            now.saturating_duration_since(self.observed).as_secs_f64()
        } else {
            0.0
        };
        positive_seconds(self.position + elapsed * self.rate)
            .min(self.track.duration.unwrap_or(f64::INFINITY))
    }
}

pub(super) fn positive_seconds(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

pub(super) fn duration(seconds: f64) -> Option<f64> {
    (positive_seconds(seconds) > 0.0).then_some(seconds)
}
