// Lyrics lookup and timing adapted from Daozai (MIT). See DAOZAI-LICENSE.
use super::platform::Track;
use serde_json::Value;
use std::{fs::File, io::Read, time::Duration};

const MAX_RESPONSE: u64 = 1024 * 1024;
const MAX_LINE_BYTES: usize = 2_048;
const MAX_LINES: usize = 1_024;
const LRCLIB: &str = "https://lrclib.net";
const NETEASE: &str = "https://music.163.com";
const QQ: &str = "https://c.y.qq.com";

#[derive(Debug, PartialEq)]
pub(super) enum Lyrics {
    Synced(Vec<Line>),
    Plain(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub(super) struct Line {
    pub at: f64,
    pub text: String,
}

impl Lyrics {
    pub fn current_index(&self, position: f64) -> Option<usize> {
        match self {
            Self::Synced(lines) => lines
                .partition_point(|line| line.at <= position)
                .checked_sub(1),
            Self::Plain(_) => None,
        }
    }

    pub fn current_line(&self, index: Option<usize>) -> Option<&str> {
        let Self::Synced(lines) = self else {
            return None;
        };
        let line = &lines.get(index?)?.text;
        (!line.is_empty()).then_some(line.as_str())
    }
}

pub(super) fn lookup(track: &Track, agent: &ureq::Agent) -> Result<Option<Lyrics>, ()> {
    if let Some(local) = track.file.as_ref().and_then(|file| {
        let filename = file.with_extension("lrc");
        if !std::fs::metadata(&filename).ok()?.is_file() {
            return None;
        }
        let file = File::open(filename).ok()?;
        let mut text = String::new();
        file.take(MAX_RESPONSE).read_to_string(&mut text).ok()?;
        parse(&text)
    }) {
        return Ok(Some(local));
    }
    if normalized(&track.title).is_empty() || artists(&track.artist).is_empty() {
        return Ok(None);
    }

    let has_cjk = track.title.chars().chain(track.artist.chars()).any(|c| {
        matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}')
    });
    let providers = if has_cjk {
        [netease as Provider, qq as Provider, lrclib as Provider]
    } else {
        [lrclib as Provider, netease as Provider, qq as Provider]
    };
    let mut plain = None;
    let mut had_error = false;
    for provider in providers {
        match provider(track, agent) {
            Ok(Some(synced @ Lyrics::Synced(_))) => return Ok(Some(synced)),
            Ok(Some(lyrics)) => {
                plain.get_or_insert(lyrics);
            }
            Ok(None) => {}
            Err(()) => had_error = true,
        }
    }
    if plain.is_none() && had_error {
        Err(())
    } else {
        Ok(plain)
    }
}

type Provider = fn(&Track, &ureq::Agent) -> Result<Option<Lyrics>, ()>;

pub(super) fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(3))
        .user_agent(concat!(
            "Peninsula/",
            env!("CARGO_PKG_VERSION"),
            " (lyrics)"
        ))
        .build()
}

fn request(request: ureq::Request, params: &[(&str, &str)]) -> Result<Value, ()> {
    let mut request = request.set("Accept", "application/json");
    for (key, value) in params {
        request = request.query(key, value);
    }
    let mut bytes = Vec::new();
    request
        .call()
        .map_err(|_| ())?
        .into_reader()
        .take(MAX_RESPONSE)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

fn lrclib(track: &Track, agent: &ureq::Agent) -> Result<Option<Lyrics>, ()> {
    let response = request(
        agent.get(&format!("{LRCLIB}/api/search")),
        &[("track_name", &track.title), ("artist_name", &track.artist)],
    )?;
    let items = response.as_array().ok_or(())?;
    let best = items
        .iter()
        .filter_map(|item| {
            let distance = match_score(
                track,
                item["trackName"].as_str()?,
                item["artistName"].as_str()?,
                item["duration"].as_f64(),
            )?;
            let lyrics = item["syncedLyrics"]
                .as_str()
                .and_then(parse)
                .or_else(|| item["plainLyrics"].as_str().and_then(parse))?;
            Some((distance, lyrics))
        })
        .min_by(|(da, a), (db, b)| {
            (matches!(a, Lyrics::Plain(_)), *da)
                .partial_cmp(&(matches!(b, Lyrics::Plain(_)), *db))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    Ok(best.map(|(_, lyrics)| lyrics))
}

fn netease(track: &Track, agent: &ureq::Agent) -> Result<Option<Lyrics>, ()> {
    let query = format!("{} {}", track.title, track.artist);
    let response = request(
        agent.get(&format!("{NETEASE}/api/search/get/web")),
        &[("s", &query), ("type", "1"), ("limit", "16")],
    )?;
    if response["code"].as_i64() != Some(200) {
        return Err(());
    }
    let Some(items) = response.pointer("/result/songs").and_then(Value::as_array) else {
        return Ok(None);
    };
    let best_song = items
        .iter()
        .filter_map(|item| {
            let artist = candidate_artists(&item["artists"])?;
            let duration = item["duration"].as_f64().map(|ms| ms / 1000.0);
            let score = match_score(track, item["name"].as_str()?, &artist, duration)?;
            Some((score, item["id"].as_u64()?))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, song_id)) = best_song else {
        return Ok(None);
    };
    let response = request(
        agent.get(&format!("{NETEASE}/api/song/lyric")),
        &[("id", &song_id.to_string()), ("lv", "-1")],
    )?;
    if response["code"].as_i64() != Some(200) {
        return Err(());
    }
    Ok(response
        .pointer("/lrc/lyric")
        .and_then(Value::as_str)
        .and_then(parse))
}

fn qq(track: &Track, agent: &ureq::Agent) -> Result<Option<Lyrics>, ()> {
    let query = format!("{} {}", track.title, track.artist);
    let get = |path| {
        agent
            .get(&format!("{QQ}{path}"))
            .set("Referer", "https://y.qq.com/")
    };
    let response = request(
        get("/soso/fcgi-bin/client_search_cp"),
        &[("w", &query), ("format", "json"), ("p", "1"), ("n", "12")],
    )?;
    if response["code"].as_i64() != Some(0) {
        return Err(());
    }
    let Some(items) = response
        .pointer("/data/song/list")
        .and_then(Value::as_array)
    else {
        return Ok(None);
    };
    let candidate = items
        .iter()
        .filter_map(|item| {
            let artist = candidate_artists(&item["singer"])?;
            let score = match_score(
                track,
                item["songname"].as_str()?,
                &artist,
                item["interval"].as_f64(),
            )?;
            Some((score, item["songmid"].as_str()?))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, song_id)) = candidate else {
        return Ok(None);
    };
    let response = request(
        get("/lyric/fcgi-bin/fcg_query_lyric_new.fcg"),
        &[("songmid", song_id), ("format", "json"), ("nobase64", "1")],
    )?;
    if response["code"].as_i64() != Some(0) {
        return Err(());
    }
    Ok(response["lyric"].as_str().and_then(|raw| {
        parse(
            &raw.replace("&#10;", "\n")
                .replace("&#13;", "")
                .replace("&#58;", ":")
                .replace("&apos;", "'")
                .replace("&quot;", "\"")
                .replace("&amp;", "&"),
        )
    }))
}

fn candidate_artists(artists: &Value) -> Option<String> {
    Some(
        artists
            .as_array()?
            .iter()
            .filter_map(|artist| artist["name"].as_str())
            .collect::<Vec<_>>()
            .join("、"),
    )
}

fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn artists(value: &str) -> Vec<String> {
    let mut names = value
        .to_lowercase()
        .replace(" & ", "、")
        .replace(" feat. ", "、")
        .replace(" featuring ", "、")
        .split(['、', '/', ',', '，', ';', '；'])
        .map(normalized)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

fn match_score(track: &Track, title: &str, artist: &str, duration: Option<f64>) -> Option<f64> {
    if normalized(&track.title) != normalized(title) || artists(&track.artist) != artists(artist) {
        return None;
    }
    match (
        track.duration,
        duration.filter(|v| v.is_finite() && *v > 0.0),
    ) {
        (Some(expected), Some(actual)) => {
            let difference = (expected - actual).abs();
            (difference <= (expected * 0.03).clamp(5.0, 10.0)).then_some(difference)
        }
        (Some(_), None) => None,
        (None, _) => Some(0.0),
    }
}

fn timestamp(value: &str) -> Option<f64> {
    let (minutes, seconds) = value.split_once(':')?;
    let minutes = minutes.parse::<u32>().ok()?;
    let seconds = seconds.replace([',', ':'], ".").parse::<f64>().ok()?;
    (0.0..60.0)
        .contains(&seconds)
        .then_some(f64::from(minutes) * 60.0 + seconds)
}

fn parse(raw: &str) -> Option<Lyrics> {
    let mut offset = 0.0;
    let mut lines = Vec::new();
    let mut plain = Vec::new();
    for raw_line in raw.trim_start_matches('\u{feff}').lines() {
        let mut remaining = raw_line.trim();
        if let Some(tag) = remaining
            .strip_prefix("[offset:")
            .and_then(|line| line.strip_suffix(']'))
        {
            if let Ok(milliseconds) = tag.trim().parse::<i32>() {
                offset = f64::from(milliseconds) / 1000.0;
            }
            continue;
        }
        let mut starts = Vec::new();
        while let Some((tag, text)) = remaining
            .strip_prefix('[')
            .and_then(|line| line.split_once(']'))
        {
            let Some(at) = timestamp(tag) else {
                break;
            };
            if lines.len() + starts.len() >= MAX_LINES {
                return None;
            }
            starts.push(at);
            remaining = text;
        }
        let text = remaining.trim();
        if starts.is_empty() && looks_like_metadata(text) {
            continue;
        }
        if text.len() > MAX_LINE_BYTES || plain.len() >= MAX_LINES {
            return None;
        }
        if !text.is_empty() {
            plain.push(text.to_owned());
        }
        if !starts.is_empty() {
            let text = text.to_owned();
            lines.extend(starts.into_iter().map(|at| Line {
                at,
                text: text.clone(),
            }));
        }
    }

    if lines.iter().any(|line| !line.text.is_empty()) {
        for line in &mut lines {
            line.at = (line.at + offset).max(0.0);
        }
        lines.sort_by(|a, b| a.at.total_cmp(&b.at));
        Some(Lyrics::Synced(lines))
    } else if plain.is_empty() {
        None
    } else {
        Some(Lyrics::Plain(plain))
    }
}

fn looks_like_metadata(line: &str) -> bool {
    line.strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
        .and_then(|tag| tag.split_once(':'))
        .is_some_and(|(key, _)| !key.is_empty() && key.bytes().all(|ch| ch.is_ascii_alphabetic()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_honors_offsets_repeated_tags_and_explicit_gaps() {
        let lyrics =
            parse("\u{feff}[ar:Artist]\n[00:01.0][00:04.05]你好\n[00:02,50]\n[offset:+250]")
                .unwrap();
        assert_eq!(lyrics.current_index(1.24), None);
        assert_eq!(
            lyrics.current_line(lyrics.current_index(1.25)),
            Some("你好")
        );
        assert_eq!(lyrics.current_line(lyrics.current_index(2.75)), None);
        assert_eq!(
            lyrics.current_line(lyrics.current_index(4.30)),
            Some("你好")
        );
    }

    #[test]
    fn static_lyrics_never_claim_a_current_line() {
        let lyrics = parse("[ti:Song Title]\nFirst line\n\n第二句").unwrap();
        assert_eq!(
            lyrics,
            Lyrics::Plain(vec!["First line".into(), "第二句".into()])
        );
        assert_eq!(lyrics.current_index(30.0), None);
        assert_eq!(parse("[ar:Artist]\n[00:12.34]"), None);
    }

    #[test]
    fn rejects_other_artists_and_song_versions() {
        let track = Track {
            title: "月光".into(),
            artist: "甲、乙".into(),
            album: String::new(),
            duration: Some(195.0),
            file: None,
        };
        assert_eq!(
            match_score(&track, "月光", "乙、甲", Some(196.0)),
            Some(1.0)
        );
        assert_eq!(match_score(&track, "月光", "丙", Some(195.0)), None);
        assert_eq!(
            match_score(&track, "月光 (Live)", "甲、乙", Some(195.0)),
            None
        );
        assert_eq!(match_score(&track, "月光", "甲、乙", Some(212.0)), None);
    }
}
