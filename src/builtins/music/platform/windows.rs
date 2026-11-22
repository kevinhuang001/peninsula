use super::{artwork, duration, Command, Controls, Playback, Track};
use std::time::Instant;
use windows::{
    core::{AgileReference, Result},
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    },
    Storage::Streams::{DataReader, IRandomAccessStreamReference},
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};

#[derive(Default)]
pub(in crate::builtins::music) struct Backend {
    manager: Option<Manager>,
    active: Option<(String, Session)>,
    initialized: bool,
    artwork: artwork::Cache,
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.active = None;
        self.manager = None;
        if self.initialized {
            // The backend lives on a single worker thread.
            unsafe { RoUninitialize() }
        }
    }
}

impl Backend {
    fn manager(&mut self) -> Result<&Manager> {
        if !self.initialized {
            unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
            self.initialized = true;
        }
        if self.manager.is_none() {
            self.manager = Some(Manager::RequestAsync()?.get()?);
        }
        Ok(self.manager.as_ref().expect("initialized"))
    }

    pub fn observe(
        &mut self,
        previous: Option<&str>,
    ) -> std::result::Result<Option<Playback>, String> {
        let active = self.active.take();
        match self.manager().and_then(|manager| {
            observe(
                manager,
                previous,
                active.as_ref().map(|(_, session)| session),
            )
        }) {
            Ok(Some((mut playback, session, thumbnail))) => {
                let key = format!(
                    "{}\0{}\0{}\0{}",
                    playback.session,
                    playback.track.title,
                    playback.track.artist,
                    playback.track.album
                );
                let thumbnail = thumbnail
                    .as_ref()
                    .and_then(|image| AgileReference::new(image).ok());
                playback.artwork = self
                    .artwork
                    .get(&key, move || read_agile_thumbnail(thumbnail));
                self.active = Some((playback.session.clone(), session));
                Ok(Some(playback))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                self.manager = None;
                Err(error.to_string())
            }
        }
    }

    pub fn control(&mut self, session: &str, command: Command) -> std::result::Result<(), String> {
        let (_, player) = self
            .active
            .as_ref()
            .filter(|(source, _)| source == session)
            .ok_or("Media session ended")?;
        let result = match command {
            Command::Previous => player.TrySkipPreviousAsync(),
            Command::Toggle => {
                let info = player.GetPlaybackInfo().map_err(|e| e.to_string())?;
                let capabilities = info.Controls().map_err(|e| e.to_string())?;
                if capabilities.IsPlayPauseToggleEnabled().unwrap_or(false) {
                    player.TryTogglePlayPauseAsync()
                } else if info.PlaybackStatus().map_err(|e| e.to_string())? == Status::Playing {
                    player.TryPauseAsync()
                } else {
                    player.TryPlayAsync()
                }
            }
            Command::Next => player.TrySkipNextAsync(),
            Command::Seek(milliseconds) => {
                let start = player
                    .GetTimelineProperties()
                    .and_then(|t| t.StartTime())
                    .map_or(0, |t| t.Duration);
                let ticks = milliseconds.saturating_mul(10_000).min(i64::MAX as u64) as i64;
                player.TryChangePlaybackPositionAsync(start.saturating_add(ticks))
            }
        };
        if result
            .and_then(|operation| operation.get())
            .map_err(|e| e.to_string())?
        {
            Ok(())
        } else {
            Err("Media action was declined".into())
        }
    }
}

fn sessions(manager: &Manager, active: Option<&Session>) -> Result<Vec<Session>> {
    let current = manager.GetCurrentSession().ok();
    let list = manager.GetSessions()?;
    let mut sessions = Vec::new();
    for index in 0..list.Size()? {
        let Ok(session) = list.GetAt(index) else {
            continue;
        };
        sessions.push(session);
    }
    sessions.sort_by_key(|session| (active != Some(session), current.as_ref() != Some(session)));
    Ok(sessions)
}

fn observe(
    manager: &Manager,
    previous: Option<&str>,
    active: Option<&Session>,
) -> Result<Option<(Playback, Session, Option<IRandomAccessStreamReference>)>> {
    let mut paused = None;
    for player in sessions(manager, active)? {
        let Ok(source) = player.SourceAppUserModelId() else {
            continue;
        };
        let source = source.to_string_lossy();
        let Ok(info) = player.GetPlaybackInfo() else {
            continue;
        };
        let Ok(state) = info.PlaybackStatus() else {
            continue;
        };
        let playing = state == Status::Playing;
        if !playing
            && (state != Status::Paused
                || previous != Some(source.as_str())
                || active.is_some_and(|known| known != &player))
        {
            continue;
        }
        let hardware = info.Controls().ok();
        let controls = hardware.map_or_else(Controls::default, |hardware| Controls {
            previous: hardware.IsPreviousEnabled().unwrap_or(false),
            toggle: hardware.IsPlayPauseToggleEnabled().unwrap_or(false)
                || if playing {
                    hardware.IsPauseEnabled()
                } else {
                    hardware.IsPlayEnabled()
                }
                .unwrap_or(false),
            next: hardware.IsNextEnabled().unwrap_or(false),
            seek: hardware.IsPlaybackPositionEnabled().unwrap_or(false),
        });
        let rate = info
            .PlaybackRate()
            .and_then(|value| value.Value())
            .ok()
            .and_then(duration)
            .unwrap_or(1.0);
        if let Ok(Some((snapshot, thumbnail))) = snapshot(&player, source, playing, controls, rate)
        {
            if playing {
                return Ok(Some((snapshot, player, thumbnail)));
            }
            paused = Some((snapshot, player, thumbnail));
        }
    }
    Ok(paused)
}

fn snapshot(
    player: &Session,
    session: String,
    playing: bool,
    controls: Controls,
    rate: f64,
) -> Result<Option<(Playback, Option<IRandomAccessStreamReference>)>> {
    let properties = player.TryGetMediaPropertiesAsync()?.get()?;
    let title = properties.Title()?.to_string_lossy();
    if title.trim().is_empty() {
        return Ok(None);
    }
    let seconds = |ticks: i64| ticks as f64 / 10_000_000.0;
    let (position, end) = player
        .GetTimelineProperties()
        .map_or((0.0, 0.0), |timeline| {
            let start = timeline.StartTime().map_or(0.0, |ts| seconds(ts.Duration));
            let end = timeline.EndTime().map_or(0.0, |ts| seconds(ts.Duration));
            let position = timeline.Position().map_or(0.0, |ts| seconds(ts.Duration));
            // GSMTC reports its last published position, possibly well before this poll.
            let elapsed = if playing {
                timeline
                    .LastUpdatedTime()
                    .ok()
                    .and_then(|date| {
                        // Windows DateTime counts 100 ns intervals since 1601-01-01.
                        const UNIX_EPOCH: f64 = 11_644_473_600.0;
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()?;
                        (date.UniversalTime > 0).then(|| {
                            (now.as_secs_f64() + UNIX_EPOCH - seconds(date.UniversalTime)).max(0.0)
                        })
                    })
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            (position - start + elapsed * rate, end - start)
        });
    Ok(Some((
        Playback {
            session,
            track: Track {
                title,
                artist: properties.Artist()?.to_string_lossy(),
                album: properties.AlbumTitle()?.to_string_lossy(),
                duration: duration(end),
                file: None,
            },
            position,
            rate,
            playing,
            controls,
            observed: Instant::now(),
            audio_level: None,
            artwork: None,
        },
        properties.Thumbnail().ok(),
    )))
}

fn read_thumbnail(thumbnail: &IRandomAccessStreamReference) -> Option<Vec<u8>> {
    let stream = thumbnail.OpenReadAsync().ok()?.get().ok()?;
    let size = u32::try_from(stream.Size().ok()?).ok()?;
    if size == 0 || u64::from(size) > artwork::MAX_BYTES {
        return None;
    }
    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    if reader.LoadAsync(size).ok()?.get().ok()? != size {
        return None;
    }
    let mut data = vec![0; size as usize];
    reader.ReadBytes(&mut data).ok()?;
    Some(data)
}

// Marshal WinRT references and initialize the artwork worker's own apartment.
fn read_agile_thumbnail(
    thumbnail: Option<AgileReference<IRandomAccessStreamReference>>,
) -> Option<Vec<u8>> {
    let thumbnail = thumbnail?;
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.ok()?;
    let result = thumbnail
        .resolve()
        .ok()
        .and_then(|image| read_thumbnail(&image));
    unsafe {
        RoUninitialize();
    }
    result
}
