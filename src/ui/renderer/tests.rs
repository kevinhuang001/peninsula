use super::*;
use crate::extension::Action;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};
use std::cell::Cell;

fn with_ui(test: impl FnOnce(&mut Ui, &Icons)) {
    let context = egui::Context::default();
    let icons = Icons::load(&context);
    let mut test = Some(test);
    let _ = context.run(RawInput::default(), |context| {
        egui::CentralPanel::default().show(context, |ui| test.take().unwrap()(ui, &icons));
    });
}

fn spacer(width: Size, height: Size) -> ViewNode {
    ViewNode::new(Node::Spacer).sized(width, height)
}

#[test]
fn table_shares_column_widths_and_wraps_rows_inside_the_viewport() {
    with_ui(|ui, icons| {
        let details = "The parcel has arrived at the local sorting facility. ".repeat(5);
        let table = Table {
            columns: vec![
                TableColumn::new("Status", 1.0),
                TableColumn::new("Details", 3.0),
            ],
            rows: vec![
                vec![ViewNode::text("In transit", TextStyle::default())],
                vec![
                    ViewNode::text("In transit", TextStyle::default()),
                    ViewNode::text(
                        details,
                        TextStyle {
                            wrap: true,
                            ..Default::default()
                        },
                    ),
                ],
            ],
        };
        let mut renderer = Renderer::new(ui, icons);
        let wide = renderer.table(ui, &table, 400.0, 1);
        let narrow = renderer.table(ui, &table, 220.0, 1);
        assert_eq!(wide.columns, vec![92.5, 277.5]);
        assert_eq!(narrow.columns, vec![47.5, 142.5]);
        assert_eq!(wide.rows[0], narrow.rows[0]);
        assert!(narrow.rows[1] > wide.rows[1]);
        let node = ViewNode::new(Node::Table(table));
        assert_eq!(content_width(ui.ctx(), &node, 400.0, icons), 400.0);
        let short_table = ViewNode::new(Node::Table(Table {
            columns: vec![TableColumn::new("Status", 1.0)],
            rows: vec![vec![ViewNode::text("Ready", TextStyle::default())]],
        }));
        assert!((20.0..200.0).contains(&content_width(ui.ctx(), &short_table, 1080.0, icons)));
        let size = renderer.in_cell(
            ui,
            &node,
            vec2(220.0, 0.0),
            [Align::Stretch, Align::Start],
            0,
        );
        assert_eq!(size.x, 220.0);
        assert_eq!(
            size.y,
            layout::TABLE_HEADER_HEIGHT + narrow.rows.iter().sum::<f32>()
        );
    });
}

#[test]
fn grid_reserves_spans_and_uses_mixed_tracks() {
    with_ui(|ui, icons| {
        let mut span = spacer(Size::Auto, Size::Auto);
        span.placement = Placement {
            column: Some(1),
            row: Some(0),
            column_span: 2,
            row_span: 2,
        };
        let mut grid = Grid::new(Columns::Tracks(vec![
            Track::Fixed(44.0),
            Track::Fraction(1.0),
            Track::Fraction(2.0),
        ]));
        grid.rows = vec![Track::Fixed(24.0), Track::Fixed(30.0)];
        grid.column_gap = Some(4.0);
        grid.row_gap = Some(3.0);
        let node = ViewNode::new(Node::Grid(
            grid,
            vec![
                spacer(Size::Auto, Size::Auto),
                span,
                spacer(Size::Auto, Size::Auto),
            ],
        ));
        let layout = Renderer::new(ui, icons)
            .layout(ui, &node, vec2(172.0, 90.0), 1)
            .unwrap();
        let cells = layout.cells;
        assert_eq!(
            cells[0].1,
            Rect::from_min_size(Pos2::ZERO, vec2(44.0, 24.0))
        );
        assert_eq!(
            cells[1].1,
            Rect::from_min_size(pos2(48.0, 0.0), vec2(124.0, 57.0))
        );
        assert_eq!(
            cells[2].1,
            Rect::from_min_size(pos2(0.0, 27.0), vec2(44.0, 30.0))
        );
    });
}

#[test]
fn autofit_reflows_and_column_first_fills_down() {
    with_ui(|ui, icons| {
        let mut layout = Renderer::new(ui, icons);
        let mut grid = Grid::new(Columns::AutoFit {
            min_item_width: 50.0,
            max_columns: 3,
        });
        grid.column_gap = Some(4.0);
        grid.row_gap = Some(2.0);
        let tiles = vec![spacer(Size::Auto, Size::Fixed(20.0)); 3];
        let node = ViewNode::new(Node::Grid(grid.clone(), tiles.clone()));
        for (available, expected) in [(162.0, 20.0), (110.0, 42.0), (55.0, 64.0)] {
            let result = layout.layout(ui, &node, vec2(available, 100.0), 1).unwrap();
            assert_eq!(result.size.y, expected);
        }
        grid.auto_flow = AutoFlow::Column;
        grid.rows = vec![Track::Fixed(20.0); 2];
        let node = ViewNode::new(Node::Grid(grid, tiles));
        let result = layout.layout(ui, &node, vec2(104.0, 100.0), 1).unwrap();
        assert_eq!(result.cells[1].1.min, pos2(0.0, 22.0));
        assert_eq!(result.cells[2].1.min, pos2(54.0, 0.0));
    });
}

#[test]
fn fill_ratios_redistribute_at_max_and_wrap_applies_line_gap() {
    with_ui(|ui, icons| {
        let mut capped = spacer(Size::Fill(1.0), Size::Fixed(15.0));
        capped.size.max_width = Some(20.0);
        let node = ViewNode::row(
            vec![
                capped,
                spacer(Size::Fill(2.0), Size::Fixed(15.0)),
                spacer(Size::Percent(0.25), Size::Fixed(15.0)),
            ],
            Flow {
                gap: Some(4.0),
                ..Default::default()
            },
        );
        let mut renderer = Renderer::new(ui, icons);
        let result = renderer.layout(ui, &node, vec2(160.0, 30.0), 1).unwrap();
        assert_eq!(result.cells[0].1.width(), 20.0);
        assert_eq!(result.cells[1].1.width(), 92.0);
        assert_eq!(result.cells[2].1.width(), 40.0);

        let node = ViewNode::new(Node::Wrap(
            Flow {
                gap: Some(2.0),
                line_gap: Some(8.0),
                ..Default::default()
            },
            vec![spacer(Size::Fixed(46.0), Size::Fixed(12.0)); 3],
        ));
        let result = renderer.layout(ui, &node, vec2(100.0, 60.0), 1).unwrap();
        assert_eq!(result.cells[2].1.min, pos2(0.0, 20.0));
    });
}

#[test]
fn stack_anchors_size_constrained_children_and_insets_clip() {
    with_ui(|ui, icons| {
        let mut tile = spacer(
            Size::MinMax {
                min: 24.0,
                max: 64.0,
            },
            Size::Percent(0.5),
        );
        tile.size.aspect_ratio = Some(2.0);
        let node = ViewNode::new(Node::Stack([Align::End, Align::Center], vec![tile]));
        let result = Renderer::new(ui, icons)
            .layout(ui, &node, vec2(100.0, 48.0), 1)
            .unwrap();
        assert_eq!(
            result.cells[0].1,
            Rect::from_min_size(pos2(52.0, 12.0), vec2(48.0, 24.0))
        );
        let rect = layout::inset(Rect::from_min_size(Pos2::ZERO, vec2(10.0, 8.0)), [20.0; 4]);
        assert_eq!(rect.size(), Vec2::ZERO);
    });
}

#[test]
fn widget_transmits_action_id_and_never_activates_disabled_or_clipped_control() {
    let context = egui::Context::default();
    let icons = Icons::load(&context);
    let source = ExtensionId::builtin("test.widget");
    let view = |enabled, overflow| {
        WidgetView::new(
            ViewNode::column(
                vec![
                    spacer(Size::Fixed(20.0), Size::Fixed(overflow)),
                    ViewNode::new(Node::Control {
                        kind: ControlKind::Toggle { checked: true },
                        label: "Enabled".into(),
                        action: Action::Custom("switch".into()),
                        enabled,
                        icon: None,
                    })
                    .sized(Size::Fill(1.0), Size::Fixed(22.0)),
                ],
                Flow {
                    gap: Some(0.0),
                    ..Default::default()
                },
            ),
            [100.0, 40.0],
        )
    };
    let origin = pos2(30.0, 30.0);
    let rect = Rect::from_min_size(origin, vec2(100.0, 45.0));
    let result = Cell::new(None);
    let run = |events: Vec<Event>, view: &WidgetView| {
        let _ = context.run(
            RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    result.set(show_widget(ui, rect, &source, view, &icons, 1.0).map(|a| {
                        assert_eq!(a.source_extension_id, source);
                        a.action
                    }));
                });
            },
        );
        result.take()
    };
    let click = |view: &WidgetView, point| {
        let _ = run(vec![Event::PointerMoved(point)], view);
        let event = |pressed| Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(run(vec![event(true)], view), None);
        run(vec![event(false)], view)
    };
    assert_eq!(
        click(&view(true, 0.0), origin + vec2(15.0, 10.0)),
        Some(Action::Custom("switch".into()))
    );
    assert_eq!(click(&view(false, 0.0), origin + vec2(15.0, 10.0)), None);
    assert_eq!(click(&view(true, 50.0), origin + vec2(15.0, 60.0)), None);
}

#[test]
fn serializable_tree_renders_inherited_custom_color_with_ancestor_opacity() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let rgba = ColorSpec::Rgba {
        r: 28,
        g: 183,
        b: 235,
        a: 160,
    };
    let mut root = ViewNode::new(Node::Container(Box::new(ViewNode::text(
        "visible",
        TextStyle::default(),
    ))))
    .color(rgba);
    root.style.normal.opacity = Some(0.5);
    let view = WidgetView::new(root, [100.0, 60.0]);
    let decoded: WidgetView = serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
    assert_eq!(decoded, view);
    let output = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut renderer = Renderer::new(ui, &icons);
            let color = renderer.theme.color(ui, rgba);
            assert_eq!(color, Color32::from_rgba_unmultiplied(28, 183, 235, 160));
            renderer.paint_root(
                ui,
                Rect::from_min_size(pos2(30.0, 30.0), vec2(100.0, 60.0)),
                &decoded.root,
                1.0,
            );
        });
    });
    assert!(output.shapes.iter().any(|clipped| match &clipped.shape {
        egui::Shape::Text(shape) =>
            shape.fallback_color
                == Color32::from_rgba_unmultiplied(28, 183, 235, 160).linear_multiply(0.5),
        _ => false,
    }));
}

#[test]
fn vertical_scroll_reveals_items_outside_the_original_viewport() {
    let context = egui::Context::default();
    let icons = Icons::load(&context);
    let source = ExtensionId::builtin("test.scrolling");
    let root = ViewNode::column(
        (1..=6)
            .map(|i| {
                ViewNode::text(format!("entry {i}"), TextStyle::default())
                    .sized(Size::Fixed(80.0), Size::Fixed(25.0))
            })
            .collect(),
        Flow {
            gap: Some(0.0),
            ..Default::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Fill(1.0));
    let mut view = WidgetView::new(root, [100.0, 45.0]);
    view.bounds.overflow = Overflow::Scroll(ScrollAxes::Vertical);
    let area = Rect::from_min_size(pos2(25.0, 25.0), vec2(100.0, 45.0));
    let draw = |events| {
        let out = context.run(
            RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show_widget(ui, area, &source, &view, &icons, 1.0);
                });
            },
        );
        out.shapes
            .into_iter()
            .filter_map(|clipped| match clipped.shape {
                egui::Shape::Text(shape) => Some(shape.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let point = area.center();
    let initial = draw(vec![Event::PointerMoved(point)]);
    assert!(initial.iter().any(|text| text == "entry 1"));
    assert!(!initial.iter().any(|text| text == "entry 6"));
    let mut visible = Vec::new();
    for _ in 0..6 {
        visible = draw(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -60.0),
            modifiers: Modifiers::NONE,
        }]);
    }
    assert!(visible.iter().any(|text| text == "entry 6"), "{visible:?}");
    assert!(!visible.iter().any(|text| text == "entry 1"));
}

#[test]
fn semantic_colors_follow_theme_and_custom_colors_do_not() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let read = |theme| {
        let colors = Cell::new(None);
        ctx.set_theme(theme);
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let theme = Renderer::new(ui, &icons).theme;
                colors.set(Some([
                    theme.color(ui, ColorSpec::PrimaryText),
                    theme.color(ui, ColorSpec::rgb(24, 89, 146)),
                ]));
            });
        });
        colors.get().unwrap()
    };
    let dark = read(egui::Theme::Dark);
    let light = read(egui::Theme::Light);
    assert_ne!(dark[0], light[0]);
    assert_eq!(dark[1], light[1]);
}

#[test]
fn content_height_grows_for_wrapped_text_and_additional_rows() {
    with_ui(|ui, icons| {
        let label = ViewNode::text(
            "A longer notification or widget label that can wrap onto several lines",
            TextStyle {
                wrap: true,
                ..TextStyle::default()
            },
        )
        .sized(Size::Fill(1.0), Size::Auto);
        let flow = Flow {
            gap: Some(5.0),
            ..Default::default()
        };
        let single = ViewNode::column(vec![label.clone()], flow);
        let double = ViewNode::column(vec![label.clone(), label], flow);
        let wide = content_height(ui.ctx(), &single, 260.0, icons);
        let narrow = content_height(ui.ctx(), &single, 120.0, icons);
        assert!(narrow > wide);
        assert_eq!(
            content_height(ui.ctx(), &double, 120.0, icons),
            2.0 * narrow + 5.0
        );
    });
}

#[test]
fn semantic_text_uses_the_shared_palette_in_both_themes() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    for dark in [true, false] {
        ctx.set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let theme = Renderer::new(ui, &icons).theme;
                let palette = TextPalette::for_mode(dark);
                for (role, expected) in [
                    (ColorSpec::SecondaryText, palette.secondary),
                    (ColorSpec::MutedText, palette.muted),
                    (ColorSpec::Success, palette.success),
                    (ColorSpec::Warning, palette.warning),
                    (ColorSpec::Danger, palette.danger),
                ] {
                    assert_eq!(theme.color(ui, role), expected);
                }
            });
        });
    }
}

#[test]
fn compact_preserves_independent_regions_and_draws_a_center_overlay() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(92.0, 32.0));
    let ring = ViewNode::ring(0.5, "", 24.0, 2.0, 8.0).color(ColorSpec::Info);
    let heading = "A heading that wraps and should be ellipsized".repeat(3);
    let node = ViewNode::compact(
        ViewNode::new(Node::Stack(
            [Align::Center; 2],
            vec![ring, ViewNode::text("Go", TextStyle::default())],
        ))
        .sized(Size::Fixed(24.0), Size::Fixed(24.0)),
        ViewNode::text(
            &heading,
            TextStyle {
                wrap: true,
                max_lines: Some(1),
                align: Align::Start,
                ..Default::default()
            },
        ),
        CompactLayout {
            leading_size: [45.0, 26.0],
            leading_max_size: [Some(26.0), None],
            padding: [2.0, 2.0, 4.0, 2.0],
            gap: 5.0,
            height: Size::Fixed(32.0),
            ..Default::default()
        },
    );
    let decoded = serde_json::from_str::<ViewNode>(&serde_json::to_string(&node).unwrap()).unwrap();
    assert_eq!(decoded, node);
    let output = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            show_compact(
                ui,
                rect,
                &ExtensionId::builtin("test.compact"),
                &node,
                &icons,
                1.0,
            );
        });
    });
    let find_text = |value: &str| {
        output
            .shapes
            .iter()
            .enumerate()
            .find_map(|(index, clipped)| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.job.text == value => {
                    Some((index, text, clipped.clip_rect))
                }
                _ => None,
            })
            .unwrap()
    };
    let (label_index, label, leading_clip) = find_text("Go");
    let (_, title, content_clip) = find_text(&heading);
    let center = label.pos + label.galley.rect.center().to_vec2();
    let ring_center = output.shapes[..label_index]
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) => Some(circle.center),
            _ => None,
        })
        .unwrap();
    assert!((center - ring_center).length() < 1.0);
    assert_eq!(ring_center.y, rect.center().y);
    assert!(leading_clip.right() + 5.0 <= content_clip.left());
    assert!(content_clip.right() <= rect.right() - 4.0);
    assert!(title.galley.elided);
    assert_eq!(title.galley.rows.len(), 1);
}

#[test]
fn compact_centers_its_leading_and_content_together_in_wide_chips() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let bounds = Rect::from_min_size(pos2(20.0, 20.0), vec2(360.0, 28.0));
    let marker = Color32::from_rgb(35, 178, 220);
    let mut leading = spacer(Size::Fixed(24.0), Size::Fixed(24.0));
    leading.style.normal.background = Some(ColorSpec::rgb(35, 178, 220));
    let label = "Short lyric";
    let node = ViewNode::compact(
        leading,
        ViewNode::text(
            label,
            TextStyle {
                align: Align::Start,
                ..Default::default()
            },
        ),
        CompactLayout {
            gap: 5.0,
            ..Default::default()
        },
    );
    let output = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            show_compact(
                ui,
                bounds,
                &ExtensionId::builtin("test.centered-compact"),
                &node,
                &icons,
                1.0,
            );
        });
    });

    let mut icon = None;
    let mut text = None;
    for clipped in &output.shapes {
        match &clipped.shape {
            egui::Shape::Rect(rect) if rect.fill == marker => icon = Some(rect.rect),
            egui::Shape::Text(shape) if shape.galley.job.text == label => {
                text = Some(shape.galley.rect.translate(shape.pos.to_vec2()));
                assert!(!shape.galley.elided);
            }
            _ => {}
        }
    }
    let (icon, text) = (icon.unwrap(), text.unwrap());
    assert_eq!(icon.width(), 24.0);
    assert!((text.left() - icon.right() - 5.0).abs() < 0.5);
    assert!(((icon.left() + text.right()) * 0.5 - bounds.center().x).abs() < 0.5);
}

#[test]
fn motion_transforms_paint_without_moving_compact_regions_and_reduced_motion_is_static() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(70.0, 28.0));
    let color = ColorSpec::rgb(47, 176, 200);
    let mut tile = spacer(Size::Fixed(10.0), Size::Fixed(10.0));
    tile.style.normal.background = Some(color);
    let motion = tile.motion(Motion {
        duration_ms: 100,
        tracks: vec![
            MotionTrack::new(MotionProperty::TranslateX, &[(0, 0.0), (100, 12.0)]),
            MotionTrack::new(MotionProperty::Rotation, &[(0, 0.0), (100, 1.0)]),
        ],
        ..Default::default()
    });
    let node = ViewNode::compact(
        motion,
        ViewNode::text("status", TextStyle::default()),
        CompactLayout {
            leading_size: [24.0, 24.0],
            ..Default::default()
        },
    );
    let render = |time| {
        ctx.run(
            RawInput {
                time: Some(time),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show_compact(
                        ui,
                        rect,
                        &ExtensionId::builtin("test.motion"),
                        &node,
                        &icons,
                        1.0,
                    );
                });
            },
        )
    };
    let resting = render(0.0);
    let moving = render(0.1);
    let label = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.job.text == "status" => Some(text.pos),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(label(&resting), label(&moving));
    let tile_rect = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .find_map(|entry| match &entry.shape {
                egui::Shape::Rect(shape) if shape.fill == Color32::from_rgb(47, 176, 200) => {
                    Some(shape.rect)
                }
                _ => None,
            })
            .unwrap()
    };
    let leading_right = tile_rect(&resting).center().x + 12.0;
    assert!(moving.shapes.iter().any(|entry| {
        matches!(&entry.shape, egui::Shape::Vec(shapes) if shapes.iter().any(|shape| matches!(shape, egui::Shape::Mesh(_))))
            && entry.clip_rect.right() <= leading_right
    }));
    set_reduced_motion(&ctx, true);
    let reduced = render(0.2);
    assert_eq!(tile_rect(&resting), tile_rect(&reduced));
}

#[test]
fn sprite_uses_atlas_frames_and_respects_the_reduced_motion_preference() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let node = ViewNode::new(Node::SpriteSequence(SpriteSequence {
        frames: vec![
            SpriteFrame {
                asset: "mascot/wink".into(),
                duration_ms: 100,
                uv: [0.0, 0.0, 1.0, 0.0625],
            },
            SpriteFrame {
                asset: "mascot/wink".into(),
                duration_ms: 100,
                uv: [0.0, 0.0625, 1.0, 0.125],
            },
        ],
        speed: 1.0,
        trigger: Trigger::Always,
        playback: Playback::Running,
        repeat: Repeat::Once,
        delay_ms: 0,
        gap_ms: 0,
    }))
    .sized(Size::Fixed(24.0), Size::Fixed(24.0));
    let draw = |time| {
        ctx.run(
            RawInput {
                time: Some(time),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show_compact(
                        ui,
                        Rect::from_min_size(pos2(20.0, 20.0), Vec2::splat(24.0)),
                        &ExtensionId::builtin("test.sprite"),
                        &node,
                        &icons,
                        1.0,
                    );
                });
            },
        )
        .shapes
        .into_iter()
        .find_map(|entry| match entry.shape {
            egui::Shape::Rect(rect) => rect.brush.map(|brush| brush.uv),
            _ => None,
        })
        .unwrap()
    };
    assert_eq!(draw(0.0).top(), 0.0);
    assert_eq!(draw(0.125).top(), 0.0625);
    assert_eq!(draw(1.0).top(), 0.0625);
    set_reduced_motion(&ctx, true);
    assert_eq!(draw(1.25).top(), 0.0);
}

#[test]
fn host_svg_style_and_image_fit_render_from_registered_assets() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(30.0, 24.0));
    let asset = "test/colored-svg";
    register_svg(
        &ctx,
        asset,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 24" fill="none" stroke="currentColor"><rect x="5" y="5" width="38" height="14"/></svg>"#,
    );
    let mut node = ViewNode::new(Node::Svg {
        asset: asset.into(),
        svg_style: SvgStyle {
            fit: ImageFit::Cover,
            fill: Some(ColorSpec::rgb(100, 150, 210)),
            stroke: Some(ColorSpec::rgb(224, 190, 86)),
            stroke_width: Some(2.0),
        },
    })
    .sized(Size::Fixed(30.0), Size::Fixed(24.0));
    node.style.normal.corner_radius = Some(3.0);
    let output = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            show_compact(
                ui,
                rect,
                &ExtensionId::builtin("test.svg"),
                &node,
                &icons,
                1.0,
            );
        });
    });
    assert!(output.shapes.iter().any(|entry| {
        matches!(&entry.shape, egui::Shape::Rect(shape)
            if shape.rect == rect && shape.corner_radius.nw == 3
                && shape.brush.as_ref().is_some_and(|brush| brush.uv.left() > 0.0))
    }));
    let test_color = Color32::from_rgb(100, 150, 210);
    assert!(output.textures_delta.set.iter().any(|(_, delta)| {
        let egui::ImageData::Color(image) = &delta.image else {
            return false;
        };
        image.pixels.contains(&test_color)
    }));
}

#[test]
fn compact_can_anchor_its_leading_and_center_short_content_across_the_chip() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(360.0, 28.0));
    let marker = Color32::from_rgb(45, 156, 210);
    let mut leading = spacer(Size::Fixed(24.0), Size::Fixed(24.0));
    leading.style.normal.background = Some(ColorSpec::rgb(45, 156, 210));
    let draw = |text: &str| {
        let node = ViewNode::compact(
            leading.clone(),
            ViewNode::text(text, TextStyle::default()),
            CompactLayout {
                width: Size::Fill(1.0),
                content_size: SizeSpec::default(),
                content_align: Align::Center,
                ..Default::default()
            },
        );
        ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_compact(
                    ui,
                    rect,
                    &ExtensionId::builtin("test.lyric-alignment"),
                    &node,
                    &icons,
                    1.0,
                );
            });
        })
    };

    for (lyric, short) in [
        ("Short lyric".to_owned(), true),
        ("A longer lyric ".repeat(25), false),
    ] {
        let output = draw(&lyric);
        let icon = output
            .shapes
            .iter()
            .find_map(|item| match &item.shape {
                egui::Shape::Rect(shape) if shape.fill == marker => Some(shape.rect),
                _ => None,
            })
            .unwrap();
        let (label, clip) = output
            .shapes
            .iter()
            .find_map(|item| match &item.shape {
                egui::Shape::Text(shape) if shape.galley.job.text == lyric => {
                    Some((shape, item.clip_rect))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(icon.left(), rect.left());
        assert!(clip.left() >= icon.right() + CompactLayout::default().gap);
        assert!(clip.right() <= rect.right());
        if short {
            let center = label.pos.x + label.galley.rect.center().x;
            assert!((center - rect.center().x).abs() < 0.5);
            assert!(!label.galley.elided);
        } else {
            assert_eq!(clip.right(), rect.right());
            assert!(label.galley.elided);
        }
    }
}
