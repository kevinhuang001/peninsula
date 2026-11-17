use super::{lyrics::Lyrics, platform::Playback, LyricsResult, WIDGET_SIZE};
use crate::{extension::Action, ui::declarative::*};
use std::time::Instant;

pub(super) fn compact(value: &str, has_lyric: bool) -> ViewNode {
    let note = || ViewNode::icon("music-note", 10.0).color(ColorSpec::SecondaryText);
    let mut items = vec![ViewNode::icon("music", 13.0)];
    if has_lyric {
        items.push(note());
    }
    items.push(
        ViewNode::text(
            value,
            TextStyle {
                align: if has_lyric {
                    Align::Center
                } else {
                    Align::Start
                },
                font_size: FontSize::Custom(11.0),
                ..Default::default()
            },
        )
        .sized(Size::Fill(1.0), Size::Auto),
    );
    if has_lyric {
        items.push(note());
    }
    ViewNode::row(items, gap(4.0)).sized(Size::Fill(1.0), Size::Auto)
}

pub(super) fn widget(
    playback: &Playback,
    lyrics: Option<&LyricsResult>,
    active_line: Option<usize>,
    control_failed: bool,
    plain_lyrics_open: bool,
) -> WidgetView {
    let track = &playback.track;
    let artist = if track.artist.is_empty() {
        &track.album
    } else {
        &track.artist
    };
    let (lyric, plain) = lyric_row(lyrics, active_line);

    let metadata = ViewNode::column(
        vec![
            ViewNode::text(
                &track.title,
                TextStyle {
                    align: Align::Start,
                    font_size: FontSize::Custom(16.0),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                },
            )
            .sized(Size::Fill(1.0), Size::Auto),
            label(artist, 12.0, ColorSpec::SecondaryText),
            lyric.sized(Size::Fill(1.0), Size::Fixed(17.0)),
        ],
        Flow {
            align: Align::Start,
            ..gap(2.0)
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    let track_row = ViewNode::row(vec![artwork(playback), metadata], gap(15.0))
        .sized(Size::Fill(1.0), Size::Auto);

    let controls = playback.controls;
    let (toggle_label, toggle_icon) = if playback.playing {
        ("暂停", "media-pause")
    } else {
        ("播放", "media-play")
    };
    let mut sections = vec![
        track_row,
        progress(playback),
        ViewNode::row(
            vec![
                control(
                    "上一首",
                    "media-previous",
                    Action::Custom("previous".into()),
                    controls.previous,
                ),
                control(toggle_label, toggle_icon, Action::Toggle, controls.toggle),
                control("下一首", "media-next", Action::Advance, controls.next),
            ],
            Flow {
                justify: Justify::Center,
                ..gap(12.0)
            },
        )
        .sized(Size::Fill(1.0), Size::Auto),
    ];
    if control_failed {
        sections.push(label("播放器未响应", 11.0, ColorSpec::Warning));
    }
    if let Some(lines) = plain.filter(|_| plain_lyrics_open) {
        let text = ViewNode::text(
            lines.join("\n"),
            TextStyle {
                align: Align::Start,
                wrap: true,
                line_height: Some(18.0),
                ..Default::default()
            },
        )
        .sized(Size::Fill(1.0), Size::Auto);
        sections.push(
            ViewNode::new(Node::Scroll(ScrollAxes::Vertical, Box::new(text)))
                .sized(Size::Fill(1.0), Size::Fixed(96.0)),
        );
    }
    let node = ViewNode::column(
        sections,
        Flow {
            align: Align::Stretch,
            ..gap(10.0)
        },
    )
    .sized(Size::Fill(1.0), Size::Auto)
    .styled(Style {
        padding: [14.0, 7.0, 14.0, 7.0],
        ..Default::default()
    });
    let mut view = WidgetView::new(node, WIDGET_SIZE);
    view.refresh_hint = RefreshHint::EverySecond;
    view
}

fn artwork(playback: &Playback) -> ViewNode {
    let mut node = match &playback.artwork {
        Some(image) => ViewNode::new(Node::RasterImage(image.as_ref().clone())).styled(Style {
            normal: Paint {
                corner_radius: Some(8.0),
                ..Default::default()
            },
            ..Default::default()
        }),
        None => ViewNode::new(Node::Stack(
            [Align::Center; 2],
            vec![ViewNode::icon("music", 22.0).color(ColorSpec::SecondaryText)],
        ))
        .styled(Style {
            normal: Paint {
                background: Some(ColorSpec::SurfaceVariant),
                corner_radius: Some(8.0),
                ..Default::default()
            },
            ..Default::default()
        }),
    };
    node.size.min_width = Some(40.0);
    node.sized(Size::Fixed(46.0), Size::Fixed(68.0))
}

fn lyric_row(
    result: Option<&LyricsResult>,
    active_line: Option<usize>,
) -> (ViewNode, Option<&[String]>) {
    match result {
        Some(Ok(Some(lyrics))) => match lyrics.as_ref() {
            Lyrics::Synced(_) => (
                label(
                    lyrics.current_line(active_line).unwrap_or(""),
                    11.5,
                    ColorSpec::Info,
                ),
                None,
            ),
            Lyrics::Plain(lines) => (lyric_action("查看纯文本歌词", "lyrics-plain"), Some(lines)),
        },
        Some(Err(())) => (lyric_action("歌词获取失败 · 重试", "lyrics-retry"), None),
        Some(Ok(None)) => (label("暂无同步歌词", 11.0, ColorSpec::MutedText), None),
        None => (label("正在查找歌词…", 11.0, ColorSpec::MutedText), None),
    }
}

fn lyric_action(value: &str, action: &str) -> ViewNode {
    ViewNode::new(Node::ActionArea {
        label: value.to_owned(),
        action: Action::Custom(action.into()),
        content: Box::new(label(value, 11.0, ColorSpec::Info)),
    })
}

fn progress(playback: &Playback) -> ViewNode {
    let position = playback.position_at(Instant::now());
    let duration = playback.track.duration;
    let timeline = duration.map_or_else(
        || ViewNode::new(Node::Spacer),
        |duration| {
            ViewNode::new(Node::ProgressBar {
                fraction: (position / duration) as f32,
                label: String::new(),
                marker: true,
            })
            .styled(Style {
                normal: Paint {
                    foreground: Some(ColorSpec::PrimaryText),
                    corner_radius: Some(3.0),
                    ..Default::default()
                },
                ..Default::default()
            })
        },
    );
    let row = vec![
        clock(position, false),
        timeline.sized(Size::Fill(1.0), Size::Fixed(5.0)),
        duration.map_or_else(
            || ViewNode::new(Node::Spacer),
            |duration| clock(duration - position, true),
        ),
    ];
    ViewNode::row(row, gap(8.0)).sized(Size::Fill(1.0), Size::Auto)
}

fn clock(seconds: f64, remaining: bool) -> ViewNode {
    let seconds = if remaining {
        seconds.ceil()
    } else {
        seconds.floor()
    }
    .max(0.0) as u64;
    let prefix = if remaining { "−" } else { "" };
    ViewNode::text(
        format!("{prefix}{}:{:02}", seconds / 60, seconds % 60),
        TextStyle {
            monospace: true,
            font_size: FontSize::Small,
            ..Default::default()
        },
    )
    .color(ColorSpec::SecondaryText)
}

fn label(value: impl Into<String>, size: f32, color: ColorSpec) -> ViewNode {
    ViewNode::text(
        value,
        TextStyle {
            align: Align::Start,
            font_size: FontSize::Custom(size),
            ..Default::default()
        },
    )
    .color(color)
}

fn control(label: &str, icon: &str, action: Action, enabled: bool) -> ViewNode {
    ViewNode::new(Node::Control {
        kind: ControlKind::Button,
        label: label.into(),
        action,
        enabled,
        icon: Some(icon.into()),
    })
    .sized(Size::Fixed(49.0), Size::Fixed(40.0))
    .styled(Style {
        normal: Paint {
            foreground: Some(ColorSpec::PrimaryText),
            background: Some(ColorSpec::Transparent),
            corner_radius: Some(8.0),
            ..Default::default()
        },
        hovered: Paint {
            background: Some(ColorSpec::SurfaceVariant),
            ..Default::default()
        },
        pressed: Paint {
            background: Some(ColorSpec::AccentSubtle),
            ..Default::default()
        },
        disabled: Paint {
            opacity: Some(0.28),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn gap(value: f32) -> Flow {
    Flow {
        gap: Some(value),
        ..Default::default()
    }
}
