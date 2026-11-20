mod lyrics;
mod platform;
mod view;

use crate::{
    extension::{
        Action, Capability, CompactView, CompactVisibility, ContributionDescriptor,
        ContributionKind, ExtensionContext, ExtensionId, ExtensionOutput,
        NotificationPresentationProvider, RuntimeExtension, WidgetVisibility,
    },
    ui::declarative::{Overflow, ScrollAxes, WidgetBounds, WidgetView, MAX_CONTENT_WIDTH},
};
use lyrics::Lyrics;
use platform::{Backend, Command, Playback, Track};
use std::{
    collections::VecDeque,
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};

const WIDGET_ID: &str = "builtin.music/now-playing";
const COMPACT_ID: &str = "builtin.music/lyrics";
const WIDGET_SIZE: [f32; 2] = [280.0, 188.0];
const COMPACT_WIDTH: f32 = 176.0;
const COMPACT_LYRIC_MIN_WIDTH: f32 = 360.0;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const IDLE_INTERVAL: Duration = Duration::from_millis(500);
const DISCONNECTED_AFTER: Duration = Duration::from_secs(4);

pub(super) fn descriptors() -> Vec<ContributionDescriptor> {
    vec![
        ContributionDescriptor::builtin(
            WIDGET_ID,
            ContributionKind::Widget,
            "音乐",
            "music",
            true,
            5,
        )
        .with_size_hint(WIDGET_SIZE[0], WIDGET_SIZE[1])
        .with_widget_bounds(widget_bounds())
        .with_capabilities(vec![Capability::View, Capability::Action]),
        ContributionDescriptor::builtin(
            COMPACT_ID,
            ContributionKind::Compact,
            "歌词",
            "music",
            true,
            9,
        )
        .with_size_hint(COMPACT_WIDTH, 28.0),
    ]
}

fn widget_bounds() -> WidgetBounds {
    WidgetBounds {
        // Let Core apply the same resolution-dependent minimum as every Widget.
        min: [0.0, 0.0],
        max: [Some(MAX_CONTENT_WIDTH), None],
        fit_width: true,
        fit_height: true,
        overflow: Overflow::Scroll(ScrollAxes::Vertical),
        ..Default::default()
    }
}

enum MediaUpdate {
    Snapshot(Option<Playback>),
    ControlFailed(String),
}

type ControlRequest = (String, Command);
type LyricsResult = Result<Option<Arc<Lyrics>>, ()>;

#[derive(Default)]
pub(super) struct MusicExtension {
    workers: Vec<std::thread::JoinHandle<()>>,
    playback: Option<Playback>,
    controller: Option<mpsc::Sender<ControlRequest>>,
    media_updates: Option<mpsc::Receiver<MediaUpdate>>,
    lyric_requests: Option<mpsc::Sender<Track>>,
    lyric_updates: Option<mpsc::Receiver<(Track, LyricsResult)>>,
    lyric_result: Option<LyricsResult>,
    lyric_cache: VecDeque<(Track, Option<Arc<Lyrics>>)>,
    active_line: Option<usize>,
    plain_lyrics_open: bool,
    control_failed: bool,
}

impl NotificationPresentationProvider for MusicExtension {}

impl RuntimeExtension for MusicExtension {
    fn descriptor(&self) -> crate::extension::ExtensionDescriptor {
        crate::extension::ExtensionDescriptor::builtin(
            "builtin.music",
            "音乐",
            "播放控制与同步歌词",
            descriptors(),
        )
    }

    fn mascot_state(&self) -> std::collections::BTreeMap<String, String> {
        [(
            "playing".into(),
            self.playback
                .as_ref()
                .is_some_and(|p| p.playing)
                .to_string(),
        )]
        .into()
    }
    fn mascot_expressions(&self) -> Vec<crate::core::mascot::MascotExpression> {
        vec![crate::core::mascot::MascotExpression::state(
            "listening",
            "playing",
            "true",
            crate::core::mascot::MascotMotion::Listening,
        )]
    }
    fn on_start(&mut self, _context: &ExtensionContext) -> Result<(), String> {
        let (controller, controls) = mpsc::channel();
        let (media_output, media_updates) = mpsc::channel();
        let (lyric_requests, requests) = mpsc::channel();
        let (lyric_output, lyric_updates) = mpsc::channel();
        self.workers.push(
            thread::Builder::new()
                .name("music-session".into())
                .spawn(move || watch_media(controls, media_output))
                .map_err(|error| error.to_string())?,
        );
        self.workers.push(
            thread::Builder::new()
                .name("music-lyrics".into())
                .spawn(move || fetch_lyrics(requests, lyric_output))
                .map_err(|error| error.to_string())?,
        );
        self.controller = Some(controller);
        self.media_updates = Some(media_updates);
        self.lyric_requests = Some(lyric_requests);
        self.lyric_updates = Some(lyric_updates);
        Ok(())
    }

    fn on_stop(&mut self, _context: &ExtensionContext) -> Result<(), String> {
        self.controller = None;
        self.media_updates = None;
        self.lyric_requests = None;
        self.lyric_updates = None;
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        self.set_playback(None);
        Ok(())
    }

    fn widget_visibility(&self, id: &ExtensionId) -> WidgetVisibility {
        if id.as_str() == WIDGET_ID && self.playback.is_some() {
            WidgetVisibility::Visible
        } else {
            WidgetVisibility::Hidden
        }
    }

    fn widget_view(&self, id: &ExtensionId) -> Option<WidgetView> {
        if id.as_str() != WIDGET_ID {
            return None;
        }
        let playback = self.playback.as_ref()?;
        Some(view::widget(
            playback,
            self.lyric_result.as_ref(),
            self.active_line,
            self.control_failed,
            self.plain_lyrics_open,
        ))
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let Some(playback) = self.playback.as_ref() else {
            return Vec::new();
        };
        let lyric = self
            .lyric_result
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .and_then(Option::as_deref)
            .and_then(|lyrics| lyrics.current_line(self.active_line));
        let title = &playback.track.title;
        let mut tooltip = title.clone();
        if !playback.track.artist.is_empty() {
            tooltip.push_str(" · ");
            tooltip.push_str(&playback.track.artist);
        }
        if let Some(lyric) = lyric {
            tooltip = format!("{lyric}\n{tooltip}");
        }
        let min_width = if lyric.is_some() {
            COMPACT_LYRIC_MIN_WIDTH
        } else {
            84.0
        };
        vec![CompactView {
            id: ExtensionId::builtin(COMPACT_ID),
            source_extension_id: ExtensionId::builtin(WIDGET_ID),
            content: view::compact(
                lyric.unwrap_or(title),
                lyric.is_some(),
                playback.playing,
                playback.audio_level,
            ),
            visibility: CompactVisibility::Visible,
            min_width,
            preferred_width: COMPACT_WIDTH.max(min_width),
            fit_width: true,
            priority: 0,
            tooltip: Some(tooltip),
        }]
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut changed = false;
        while let Some(update) = self
            .media_updates
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            match update {
                MediaUpdate::Snapshot(playback) => changed |= self.set_playback(playback),
                MediaUpdate::ControlFailed(session) => {
                    if self
                        .playback
                        .as_ref()
                        .is_some_and(|playing| playing.session == session)
                    {
                        self.control_failed = true;
                        changed = true;
                    }
                }
            }
        }
        while let Some((track, result)) = self
            .lyric_updates
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            if let Ok(lyrics) = &result {
                self.lyric_cache
                    .retain(|(entry, _)| !same_song(entry, &track));
                self.lyric_cache.push_front((track.clone(), lyrics.clone()));
                self.lyric_cache.truncate(48);
            }
            if self
                .playback
                .as_ref()
                .is_some_and(|current| same_song(&current.track, &track))
            {
                self.lyric_result = Some(result);
                changed = true;
            }
        }

        let line = self.playback.as_ref().and_then(|playback| {
            self.lyric_result
                .as_ref()?
                .as_ref()
                .ok()?
                .as_ref()?
                .current_index(playback.position_at(now))
        });
        changed |= self.active_line != line;
        self.active_line = line;

        let mut output = if changed {
            invalidate()
        } else {
            ExtensionOutput::default()
        };
        let next = self
            .playback
            .as_ref()
            .filter(|p| p.playing && p.rate > 0.0)
            .and_then(|p| {
                let lyrics = self.lyric_result.as_ref()?.as_ref().ok()?.as_ref()?;
                let position = p.position_at(now);
                let at = lyrics.next_time(position)?;
                now.checked_add(Duration::try_from_secs_f64((at - position) / p.rate).ok()?)
            });
        if let Some(deadline) = next {
            output.schedule.push(crate::extension::ScheduledTask {
                task_id: "music-lyric-boundary".into(),
                deadline,
            });
        } else {
            output.cancel_tasks.push("music-lyric-boundary".into());
        }
        output
    }

    fn on_action(&mut self, action: Action, now: Instant) -> ExtensionOutput {
        let Some(playback) = &self.playback else {
            return ExtensionOutput::default();
        };
        let command = match action {
            Action::Toggle => Command::Toggle,
            Action::SetProgress {
                ref key,
                basis_points,
            } if key == "seek" => {
                let Some(duration) = playback.track.duration else {
                    return ExtensionOutput::default();
                };
                Command::Seek(
                    (duration * 1000.0 * f64::from(basis_points.min(10_000)) / 10_000.0).round()
                        as u64,
                )
            }
            Action::Custom(ref name) if name == "previous" => Command::Previous,
            Action::Advance => Command::Next,
            Action::Custom(ref name) if name == "lyrics-retry" => {
                self.lyric_result = None;
                self.request_lyrics();
                return invalidate();
            }
            Action::Custom(ref name) if name == "lyrics-plain" => {
                self.plain_lyrics_open = !self.plain_lyrics_open;
                return invalidate();
            }
            _ => return ExtensionOutput::default(),
        };
        if !playback.controls.allows(command) {
            return ExtensionOutput::default();
        }
        self.control_failed = self
            .controller
            .as_ref()
            .is_none_or(|sender| sender.send((playback.session.clone(), command)).is_err());
        if !self.control_failed {
            if let Command::Seek(milliseconds) = command {
                if let Some(playback) = self.playback.as_mut() {
                    playback.position = milliseconds as f64 / 1000.0;
                    playback.observed = now;
                }
                self.active_line = self
                    .lyric_result
                    .as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .and_then(Option::as_deref)
                    .and_then(|lyrics| lyrics.current_index(milliseconds as f64 / 1000.0));
            }
        }
        invalidate()
    }
}

impl MusicExtension {
    fn set_playback(&mut self, next: Option<Playback>) -> bool {
        let next = next.filter(|next| {
            next.playing
                || self
                    .playback
                    .as_ref()
                    .is_some_and(|old| old.session == next.session)
        });
        let song_changed = match (&self.playback, &next) {
            (Some(previous), Some(next)) => !same_song(&previous.track, &next.track),
            (None, None) => false,
            _ => true,
        };
        let amplitude_changed = match (&self.playback, &next) {
            (Some(previous), Some(next)) => previous.audio_level != next.audio_level,
            _ => false,
        };
        let presentation_changed = match (&self.playback, &next) {
            (Some(previous), Some(next)) => {
                previous.session != next.session
                    || previous.playing != next.playing
                    || previous.rate != next.rate
                    || previous.controls != next.controls
                    || previous.artwork != next.artwork
                    || (previous.position_at(next.observed) - next.position_at(next.observed)).abs()
                        > 1.25
            }
            _ => song_changed,
        };
        self.playback = next;
        if song_changed {
            self.active_line = None;
            self.plain_lyrics_open = false;
            self.lyric_result = self.playback.as_ref().and_then(|playback| {
                self.lyric_cache
                    .iter()
                    .find(|(track, _)| same_song(track, &playback.track))
                    .map(|(_, lyrics)| Ok(lyrics.clone()))
            });
            if self.lyric_result.is_none() {
                self.request_lyrics();
            }
        }
        if song_changed || presentation_changed {
            self.control_failed = false;
        }
        song_changed || presentation_changed || amplitude_changed
    }

    fn request_lyrics(&mut self) {
        if let Some(playback) = &self.playback {
            if self
                .lyric_requests
                .as_ref()
                .is_none_or(|sender| sender.send(playback.track.clone()).is_err())
            {
                self.lyric_result = Some(Err(()));
            }
        }
    }
}

fn same_song(a: &Track, b: &Track) -> bool {
    a.title == b.title
        && a.artist == b.artist
        && a.album == b.album
        && a.file == b.file
        && match (a.duration, b.duration) {
            (Some(a), Some(b)) => (a - b).abs() < 2.0,
            _ => true,
        }
}

fn invalidate() -> ExtensionOutput {
    ExtensionOutput {
        invalidated: vec![
            ExtensionId::builtin(WIDGET_ID),
            ExtensionId::builtin(COMPACT_ID),
        ],
        ..Default::default()
    }
}

fn watch_media(controls: mpsc::Receiver<ControlRequest>, output: mpsc::Sender<MediaUpdate>) {
    let mut backend = Backend::default();
    let mut session = None::<String>;
    let mut last_success = Instant::now();
    loop {
        let snapshot = match backend.observe(session.as_deref()) {
            Ok(snapshot) => {
                last_success = Instant::now();
                Some(snapshot)
            }
            Err(_) if last_success.elapsed() >= DISCONNECTED_AFTER => Some(None),
            Err(_) => None,
        };
        if let Some(snapshot) = snapshot {
            session = snapshot.as_ref().map(|playback| playback.session.clone());
            if output.send(MediaUpdate::Snapshot(snapshot)).is_err() {
                return;
            }
        }
        let interval = if session.is_some() {
            POLL_INTERVAL
        } else {
            IDLE_INTERVAL
        };
        match controls.recv_timeout(interval) {
            Ok((target, command)) => {
                let failed = session.as_deref() != Some(&target)
                    || backend.control(&target, command).is_err();
                if failed && output.send(MediaUpdate::ControlFailed(target)).is_err() {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn fetch_lyrics(requests: mpsc::Receiver<Track>, output: mpsc::Sender<(Track, LyricsResult)>) {
    while let Ok(first) = requests.recv() {
        let track = requests.try_iter().last().unwrap_or(first);
        let result = lyrics::lookup(&track).map(|lyrics| lyrics.map(Arc::new));
        if output.send((track, result)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn sample(playing: bool, seconds: f64, now: Instant) -> Playback {
        Playback {
            session: "player".into(),
            track: Track {
                title: "示例歌曲".into(),
                artist: "示例艺人".into(),
                album: String::new(),
                file: None,
                duration: Some(100.0),
            },
            playing,
            controls: platform::Controls::default(),
            position: seconds,
            rate: 1.0,
            observed: now,
            audio_level: None,
            artwork: None,
        }
    }

    #[test]
    fn player_must_be_detected_playing_before_its_widget_or_compact_appears() {
        let mut music = MusicExtension::default();
        let id = ExtensionId::builtin(WIDGET_ID);
        let now = Instant::now();
        assert!(!music.set_playback(Some(sample(false, 0.0, now))));
        assert_eq!(music.widget_visibility(&id), WidgetVisibility::Hidden);
        assert!(music.widget_view(&id).is_none());
        assert!(music.compact_views().is_empty());

        assert!(music.set_playback(Some(sample(true, 1.0, now))));
        assert_eq!(music.widget_visibility(&id), WidgetVisibility::Visible);
        assert!(music.widget_view(&id).is_some());
        assert_eq!(
            music.compact_views()[0].visibility,
            CompactVisibility::Visible
        );

        assert!(music.set_playback(Some(sample(false, 2.0, now))));
        assert_eq!(music.widget_visibility(&id), WidgetVisibility::Visible);
        assert!(music.set_playback(None));
        assert_eq!(music.widget_visibility(&id), WidgetVisibility::Hidden);
        assert!(music.compact_views().is_empty());
    }

    #[test]
    fn compact_lyric_follows_playback_seek_and_pause() {
        let mut music = MusicExtension::default();
        let now = Instant::now();
        music.set_playback(Some(sample(true, 0.0, now)));
        music.lyric_result = Some(Ok(Some(Arc::new(Lyrics::Synced(vec![
            lyrics::Line {
                at: 0.5,
                text: "第一句".into(),
            },
            lyrics::Line {
                at: 1.5,
                text: "第二句".into(),
            },
        ])))));
        assert_eq!(
            music.update(now + Duration::from_secs(1)).invalidated.len(),
            2
        );
        assert_eq!(music.compact_views()[0].min_width, COMPACT_LYRIC_MIN_WIDTH);
        assert!(music.compact_views()[0].fit_width);
        assert!(music.compact_views()[0]
            .tooltip
            .as_deref()
            .unwrap()
            .starts_with("第一句"));
        assert_eq!(
            music.update(now + Duration::from_secs(2)).invalidated.len(),
            2
        );
        assert!(music.compact_views()[0]
            .tooltip
            .as_deref()
            .unwrap()
            .starts_with("第二句"));

        music.set_playback(Some(sample(false, 1.0, now + Duration::from_secs(2))));
        music.update(now + Duration::from_secs(10));
        assert_eq!(music.active_line, Some(0));
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    fn music(now: Instant) -> MusicExtension {
        let mut music = MusicExtension::default();
        music.set_playback(Some(super::tests::sample(true, 0.0, now)));
        music.lyric_result = Some(Ok(Some(Arc::new(Lyrics::Synced(vec![
            lyrics::Line {
                at: 0.125,
                text: "第一句".into(),
            },
            lyrics::Line {
                at: 0.275,
                text: "第二句".into(),
            },
            lyrics::Line {
                at: 2.0,
                text: "第三句".into(),
            },
        ])))));
        music
    }
    #[test]
    fn deadlines_follow_subsecond_boundaries_rate_pause_and_backward_seek() {
        let now = Instant::now();
        let mut music = music(now);
        assert_eq!(
            music.update(now).schedule[0].deadline,
            now + Duration::from_millis(125)
        );
        music.update(now + Duration::from_millis(124));
        assert_eq!(music.active_line, None);
        let output = music.update(now + Duration::from_millis(125));
        assert_eq!(music.active_line, Some(0));
        assert_eq!(
            output.schedule[0].deadline,
            now + Duration::from_millis(275)
        );
        let mut playback = super::tests::sample(true, 0.125, now + Duration::from_millis(125));
        playback.rate = 2.0;
        assert!(music.set_playback(Some(playback)));
        assert_eq!(
            music.update(now + Duration::from_millis(125)).schedule[0].deadline,
            now + Duration::from_millis(200)
        );
        music.set_playback(Some(super::tests::sample(false, 0.2, now)));
        assert!(music
            .update(now + Duration::from_secs(5))
            .schedule
            .is_empty());
        assert_eq!(music.active_line, Some(0));
        music.set_playback(Some(super::tests::sample(
            true,
            0.0,
            now + Duration::from_secs(5),
        )));
        music.update(now + Duration::from_secs(5));
        assert_eq!(music.active_line, None);
    }
}

#[cfg(test)]
mod seek_tests {
    use super::*;
    #[test]
    fn seek_sends_position_to_backend_and_updates_paused_playback_and_lyrics() {
        let now = Instant::now();
        let (sender, receiver) = mpsc::channel();
        let mut music = MusicExtension {
            controller: Some(sender),
            ..Default::default()
        };
        let mut playback = super::tests::sample(false, 10.0, now);
        playback.controls.seek = true;
        music.playback = Some(playback);
        music.lyric_result = Some(Ok(Some(Arc::new(Lyrics::Synced(vec![
            lyrics::Line {
                at: 0.0,
                text: "first".into(),
            },
            lyrics::Line {
                at: 50.0,
                text: "second".into(),
            },
        ])))));
        music.on_action(
            Action::SetProgress {
                key: "seek".into(),
                basis_points: 7500,
            },
            now,
        );
        assert_eq!(receiver.try_recv().unwrap().1, Command::Seek(75_000));
        assert_eq!(music.playback.as_ref().unwrap().position_at(now), 75.0);
        assert!(!music.playback.as_ref().unwrap().playing);
        assert_eq!(music.active_line, Some(1));
        music.playback.as_mut().unwrap().controls.seek = false;
        music.on_action(
            Action::SetProgress {
                key: "seek".into(),
                basis_points: 10_000,
            },
            now,
        );
        assert!(receiver.try_recv().is_err());
    }
}
