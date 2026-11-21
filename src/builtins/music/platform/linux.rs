use super::{artwork, duration, Command, Controls, Playback, Track};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use zbus::{
    blocking::{fdo::DBusProxy, Connection, Proxy},
    zvariant::OwnedValue,
};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Default)]
pub(in crate::builtins::music) struct Backend {
    connection: Option<Connection>,
    artwork: artwork::Cache,
}

impl Backend {
    pub fn observe(
        &mut self,
        previous: Option<&str>,
        artwork_scope: Option<&crate::sdk::tasks::TaskScope>,
    ) -> Result<Option<Playback>, String> {
        if artwork_scope.is_none() {
            self.artwork = artwork::Cache::default();
        }
        if self.connection.is_none() {
            self.connection = Some(
                zbus::blocking::connection::Builder::session()
                    .and_then(|builder| builder.method_timeout(Duration::from_millis(500)).build())
                    .map_err(|e| e.to_string())?,
            );
        }
        let connection = self.connection.as_ref().expect("connected");
        let names = DBusProxy::new(connection).and_then(|bus| Ok(bus.list_names()?));
        let mut names = match names {
            Ok(names) => names,
            Err(error) => {
                self.connection = None;
                return Err(error.to_string());
            }
        };
        names.sort_by_key(|name| previous != Some(name.as_str()));
        let mut paused = None;
        for name in names
            .iter()
            .map(|name| name.as_str())
            .filter(|name| name.starts_with(PREFIX))
        {
            let Ok(player) = player_proxy(connection, name) else {
                continue;
            };
            let Ok(status) = player.get_property::<String>("PlaybackStatus") else {
                continue;
            };
            let playing = status == "Playing";
            if !playing && (status != "Paused" || previous != Some(name)) {
                continue;
            }
            if let Some(snapshot) = snapshot(&player, name, playing) {
                if playing {
                    return Ok(Some(apply_artwork(
                        &mut self.artwork,
                        snapshot,
                        artwork_scope,
                    )));
                }
                paused = Some(snapshot);
            }
        }
        Ok(paused.map(|snapshot| apply_artwork(&mut self.artwork, snapshot, artwork_scope)))
    }

    pub fn control(&mut self, session: &str, command: Command) -> Result<(), String> {
        let connection = self.connection.as_ref().ok_or("D-Bus is disconnected")?;
        let player = player_proxy(connection, session).map_err(|e| e.to_string())?;
        if let Command::Seek(milliseconds) = command {
            let metadata = player
                .get_property::<HashMap<String, OwnedValue>>("Metadata")
                .map_err(|e| e.to_string())?;
            let track = metadata
                .get("mpris:trackid")
                .and_then(|v| v.try_clone().ok())
                .and_then(|v| zbus::zvariant::OwnedObjectPath::try_from(v).ok())
                .ok_or("Player has no seekable track")?;
            let microseconds = milliseconds.saturating_mul(1000).min(i64::MAX as u64) as i64;
            return player
                .call::<_, _, ()>("SetPosition", &(track, microseconds))
                .map_err(|e| e.to_string());
        }
        let method = match command {
            Command::Previous => "Previous",
            Command::Toggle => "PlayPause",
            Command::Next => "Next",
            Command::Seek(_) => unreachable!(),
        };
        player
            .call::<_, _, ()>(method, &())
            .map_err(|e| e.to_string())
    }
}

fn player_proxy<'a>(connection: &Connection, name: &'a str) -> zbus::Result<Proxy<'a>> {
    // Snapshots should reflect the player state now, including Position which
    // MPRIS may update without emitting the PropertiesChanged signal.
    zbus::blocking::proxy::Builder::new(connection)
        .destination(name)?
        .path(PATH)?
        .interface(PLAYER)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
}

fn snapshot(
    player: &Proxy<'_>,
    session: &str,
    playing: bool,
) -> Option<(Playback, Option<String>)> {
    let metadata = player
        .get_property::<HashMap<String, OwnedValue>>("Metadata")
        .ok()?;
    let text = |key: &str| {
        metadata
            .get(key)
            .and_then(|value| <&str>::try_from(value).ok())
    };
    let title = text("xesam:title")?.trim().to_owned();
    if title.is_empty() {
        return None;
    }
    let artist = metadata
        .get("xesam:artist")
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| Vec::<String>::try_from(value).ok())
        .unwrap_or_default()
        .join("、");
    let length = metadata
        .get("mpris:length")
        .and_then(|value| i64::try_from(value).ok());
    let file = text("xesam:url")
        .and_then(|raw| url::Url::parse(raw).ok())
        .and_then(|url| url.to_file_path().ok());
    // A missing position is not zero: resetting it would rewind lyrics.
    let position = player.get_property::<i64>("Position").ok()?;
    let observed = Instant::now();
    let rate = player
        .get_property::<f64>("Rate")
        .ok()
        .and_then(duration)
        .unwrap_or(1.0);
    let can = |property| player.get_property::<bool>(property).unwrap_or(false);
    let controls = if can("CanControl") {
        Controls {
            previous: can("CanGoPrevious"),
            seek: can("CanSeek"),
            next: can("CanGoNext"),
            toggle: can(if playing { "CanPause" } else { "CanPlay" }),
        }
    } else {
        Controls::default()
    };
    Some((
        Playback {
            session: session.to_owned(),
            track: Track {
                title,
                artist,
                album: text("xesam:album").unwrap_or_default().to_owned(),
                duration: length.and_then(|us| duration(us as f64 / 1_000_000.0)),
                file,
            },
            position: position as f64 / 1_000_000.0,
            rate,
            playing,
            controls,
            observed,
            audio_level: None,
            artwork: None,
        },
        text("mpris:artUrl").map(str::to_owned),
    ))
}

fn apply_artwork(
    cache: &mut artwork::Cache,
    (mut playback, url): (Playback, Option<String>),
    tasks: Option<&crate::sdk::tasks::TaskScope>,
) -> Playback {
    playback.artwork = url.as_deref().and_then(|url| {
        let source = url.to_owned();
        cache.get(tasks?, url, move |cancel| {
            artwork::read_url(&source, cancel)
        })
    });
    playback
}
