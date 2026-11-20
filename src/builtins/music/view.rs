use super::{lyrics::Lyrics, platform::Playback, LyricsResult, WIDGET_SIZE};
use crate::{extension::Action, ui::declarative::*};
use std::time::Instant;

pub(super) fn compact(
    value: &str,
    has_lyric: bool,
    playing: bool,
    _audio_level: Option<f32>,
) -> ViewNode {
    // Keep the notes in the same text run: no layout gap can separate them
    // from the lyric, and the host measures the complete line with its font.
    let text = if has_lyric {
        format!("♪{value}♪")
    } else {
        value.to_owned()
    };
    let indicator = equalizer(playing);
    ViewNode::compact(
        indicator,
        super::super::compact_value(text, false),
        CompactLayout {
            leading_size: [32.0, 24.0],
            width: Size::Fill(1.0),
            content_size: SizeSpec::default(),
            content_align: Align::Center,
            gap: 4.0,
            ..Default::default()
        },
    )
}

fn equalizer(playing: bool) -> ViewNode {
    ViewNode::icon(
        if playing {
            "equalizer-playing"
        } else {
            "equalizer"
        },
        32.0,
    )
    .sized(Size::Fixed(32.0), Size::Fixed(24.0))
    .color(ColorSpec::Accent)
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
            lyric.sized(Size::Fill(1.0), Size::Auto),
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
                wrap: false,
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
    view.bounds = super::widget_bounds();
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
            vec![equalizer(playback.playing)],
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
                ViewNode::text(
                    lyrics.current_line(active_line).unwrap_or(""),
                    TextStyle {
                        align: Align::Start,
                        font_size: FontSize::Custom(11.5),
                        wrap: false,
                        ..Default::default()
                    },
                )
                .color(ColorSpec::Info),
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
        context_menu: None,
    })
}

fn progress(playback: &Playback) -> ViewNode {
    let position = playback.position_at(Instant::now());
    let duration = playback.track.duration;
    let timeline = duration.map_or_else(
        || ViewNode::new(Node::Spacer),
        |duration| {
            ViewNode::new(Node::Slider {
                fraction: (position / duration) as f32,
                label: "播放进度".into(),
                action: "seek".into(),
                enabled: playback.controls.seek,
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
        timeline.sized(Size::Fill(1.0), Size::Fixed(24.0)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_keeps_full_lyric_with_adjacent_notes_and_playback_icon() {
        let lyric = "这是一句很长的歌词，需要完整显示而不是省略".repeat(8);
        for playing in [true, false] {
            let node = compact(&lyric, true, playing, Some(0.7));
            let Node::Compact(compact) = node.kind else {
                panic!("expected compact")
            };
            assert_eq!(compact.layout.width, Size::Fill(1.0));
            assert_eq!(compact.layout.content_align, Align::Center);
            assert!(matches!(&compact.leading.kind, Node::Icon { name, .. }
                if name == if playing { "equalizer-playing" } else { "equalizer" }));
            assert_eq!(
                compact.leading.style.normal.foreground,
                Some(ColorSpec::Accent)
            );
            assert!(matches!(&compact.content.kind, Node::Text { text, .. }
                if text == &format!("♪{lyric}♪")));
        }
    }

    #[test]
    fn synced_widget_lyric_stays_on_a_single_line_with_a_large_width_limit() {
        let lyric = "A long lyric that needs several lines in a narrow widget. ".repeat(5);
        let result = Some(Ok(Some(std::sync::Arc::new(Lyrics::Synced(vec![
            super::super::lyrics::Line {
                at: 0.0,
                text: lyric.clone(),
            },
        ])))));
        let (node, _) = lyric_row(result.as_ref(), Some(0));
        let Node::Text { text, text_style } = node.kind else {
            panic!("expected lyric")
        };
        assert_eq!(text, lyric);
        assert!(!text_style.wrap);
        assert_eq!(super::super::widget_bounds().max[0], Some(4096.0));
        assert!(super::super::widget_bounds().fit_width);
        assert_eq!(node.size.height, Size::Auto);
    }
}

#[cfg(test)]
mod rendering_tests {
    use super::*;
    use crate::{app::icons::Icons, ui::renderer};
    use eframe::egui::{self, pos2, vec2, RawInput, Rect};

    #[test]
    fn expanded_long_lyric_paints_one_complete_unelided_line() {
        let lyric = "Keep the whole lyric visible all the way to the final note. ".repeat(4);
        let playback = Playback {
            session: "test".into(),
            track: super::super::Track {
                title: "Long lyric regression".into(),
                artist: "Test".into(),
                album: String::new(),
                duration: Some(180.0),
                file: None,
            },
            playing: true,
            controls: Default::default(),
            position: 1.0,
            rate: 1.0,
            observed: Instant::now(),
            audio_level: None,
            artwork: None,
        };
        let lyrics = Some(Ok(Some(std::sync::Arc::new(Lyrics::Synced(vec![
            super::super::lyrics::Line {
                at: 0.0,
                text: lyric.clone(),
            },
        ])))));
        let view = widget(&playback, lyrics.as_ref(), Some(0), false, false);
        let ctx = egui::Context::default();
        let icons = Icons::load(&ctx);
        let output = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(4096.0, 500.0))),
                ..Default::default()
            },
            |ctx| {
                let width =
                    renderer::natural_content_width(ctx, &view.root, 4076.0, &icons).ceil() + 1.0;
                assert!(width > 600.0 && width < 4076.0, "width={width}");
                egui::CentralPanel::default().show(ctx, |ui| {
                    renderer::show_widget(
                        ui,
                        Rect::from_min_size(pos2(10.0, 10.0), vec2(width, 200.0)),
                        &super::super::ExtensionId::builtin(super::super::WIDGET_ID),
                        &view,
                        &icons,
                        1.0,
                    );
                });
            },
        );
        let galley = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == lyric => Some(&text.galley),
                _ => None,
            })
            .expect("lyric must be painted");
        assert_eq!(galley.rows.len(), 1);
        assert!(!galley.elided, "the final note must remain visible");
    }
}
