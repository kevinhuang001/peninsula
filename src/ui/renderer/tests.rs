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
fn text_input_routes_edits_to_its_extension_and_retains_keyboard_focus() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let source = ExtensionId::builtin("test.text-input");
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(260.0, 30.0));
    let view = WidgetView::new(
        ViewNode::new(Node::TextInput {
            key: "feed-url".into(),
            value: String::new(),
            hint: "RSS URL".into(),
        })
        .sized(Size::Fill(1.0), Size::Fixed(30.0)),
        [260.0, 30.0],
    );
    let frame = |events| {
        let mut result = None;
        let _ = ctx.run(
            RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    result = show_widget(ui, rect, &source, &view, &icons, 1.0);
                });
            },
        );
        result
    };
    frame(vec![]);
    frame(vec![Event::PointerMoved(rect.center())]);
    for pressed in [true, false] {
        frame(vec![Event::PointerButton {
            pos: rect.center(),
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }]);
    }
    let result = frame(vec![Event::Text("https://example.com/rss".into())]).unwrap();
    assert_eq!(result.source_extension_id, source);
    assert_eq!(
        result.action,
        Action::SetText {
            key: "feed-url".into(),
            value: "https://example.com/rss".into()
        }
    );
    assert!(ctx.wants_keyboard_input());
}

#[test]
fn progress_ring_strokes_fit_their_clip_bounds_at_each_display_scale() {
    for scale in [1.0, 1.5, 2.0] {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        let icons = Icons::load(&ctx);
        let rect = Rect::from_min_size(pos2(20.0, 20.0), Vec2::splat(70.0));
        let view = WidgetView::new(ViewNode::ring(0.82, "", 70.0, 4.0, 14.0), [70.0; 2]);
        let output = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_widget(
                    ui,
                    rect,
                    &ExtensionId::builtin("test.ring"),
                    &view,
                    &icons,
                    1.0,
                );
            });
        });
        let paths: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Path(path) => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 2);
        assert!(paths[0].closed);
        assert!(!paths[1].closed);
        for path in paths {
            for point in &path.points {
                let outer_radius = point.distance(rect.center())
                    + path.stroke.width * 0.5
                    + 0.5 / ctx.pixels_per_point();
                assert!(outer_radius <= rect.width() * 0.5 + 0.001);
            }
            assert!(path
                .points
                .windows(2)
                .all(|points| points[0].distance(points[1]) * ctx.pixels_per_point() <= 2.01));
        }
    }
}

#[test]
fn table_shares_column_widths_and_wraps_rows_inside_the_viewport() {
    with_ui(|ui, icons| {
        let details = "The parcel has arrived at the local sorting facility. ".repeat(5);
        let table = Table {
            row_menus: Vec::new(),
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
            row_menus: Vec::new(),
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
fn table_context_menu_keeps_its_target_when_rows_are_reordered() {
    let context = egui::Context::default();
    let icons = Icons::load(&context);
    let source = ExtensionId::builtin("test.widget");
    let rect = Rect::from_min_size(pos2(30.0, 30.0), vec2(180.0, 100.0));
    let frame = |events, names: [&str; 2]| {
        let table = Table {
            columns: vec![TableColumn::new("Application", 1.0)],
            rows: names
                .iter()
                .map(|name| vec![ViewNode::text(*name, TextStyle::default())])
                .collect(),
            row_menus: names
                .iter()
                .map(|name| {
                    Some(TableRowMenu {
                        title: (*name).into(),
                        label: "End program".into(),
                        action: Action::Custom((*name).into()),
                    })
                })
                .collect(),
        };
        let view = WidgetView::new(ViewNode::new(Node::Table(table)), [180.0, 100.0]);
        let mut result = None;
        let _ = context.run(
            RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let capsule =
                        ui.interact(rect.expand(4.0), Id::new("capsule"), egui::Sense::click());
                    let action = show_widget(ui, rect, &source, &view, &icons, 1.0);
                    context_menu(&capsule, |ui| {
                        ui.label("Exit capsule");
                        false
                    });
                    result = Some((action.map(|action| action.action), context_menu_rect(ctx)));
                });
            },
        );
        result.unwrap()
    };

    let click = |pos, button, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let row = pos2(
        rect.left() + 12.0,
        rect.top() + layout::TABLE_HEADER_HEIGHT + 8.0,
    );
    let order = ["Firefox", "Code"];
    assert_eq!(frame(vec![Event::PointerMoved(row)], order), (None, None));
    frame(vec![click(row, PointerButton::Secondary, true)], order);
    assert_eq!(
        frame(vec![click(row, PointerButton::Secondary, false)], order).0,
        None
    );
    let reversed = ["Code", "Firefox"];
    let menu = frame(vec![], reversed).1.unwrap();
    let button = pos2(menu.center().x, menu.bottom() - 12.0);
    frame(vec![Event::PointerMoved(button)], reversed);
    frame(vec![click(button, PointerButton::Primary, true)], reversed);
    assert_eq!(
        frame(vec![click(button, PointerButton::Primary, false)], reversed),
        (Some(Action::Custom("Firefox".into())), None)
    );
}

#[test]
fn sidebar_context_menu_deletes_the_right_clicked_item_after_reordering() {
    let context = egui::Context::default();
    let icons = Icons::load(&context);
    let source = ExtensionId::builtin("test.widget");
    let rect = Rect::from_min_size(pos2(30.0, 30.0), vec2(180.0, 100.0));
    let frame = |events, names: [&str; 2]| {
        let rows = names
            .iter()
            .map(|name| {
                let mut row = ViewNode::new(Node::ActionArea {
                    action: Action::Select((*name).into()),
                    label: (*name).into(),
                    content: Box::new(ViewNode::text(*name, TextStyle::default())),
                    context_menu: Some(ContextMenuAction {
                        title: (*name).into(),
                        label: "Delete".into(),
                        action: Action::Custom((*name).into()),
                    }),
                })
                .sized(Size::Fill(1.0), Size::Fixed(34.0));
                row.key = Some((*name).into());
                row
            })
            .collect();
        let view = WidgetView::new(
            ViewNode::column(
                rows,
                Flow {
                    gap: Some(5.0),
                    ..Default::default()
                },
            ),
            [180.0, 100.0],
        );
        let mut result = None;
        let _ = context.run(
            RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let capsule =
                        ui.interact(rect.expand(4.0), Id::new("capsule"), egui::Sense::click());
                    let action = show_widget(ui, rect, &source, &view, &icons, 1.0);
                    context_menu(&capsule, |ui| {
                        ui.label("Exit capsule");
                        false
                    });
                    result = Some((action.map(|action| action.action), context_menu_rect(ctx)));
                });
            },
        );
        result.unwrap()
    };

    let click = |pos, button, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let row = pos2(rect.left() + 12.0, rect.top() + 8.0);
    let order = ["Firefox", "Code"];
    assert_eq!(frame(vec![Event::PointerMoved(row)], order), (None, None));
    frame(vec![click(row, PointerButton::Secondary, true)], order);
    assert_eq!(
        frame(vec![click(row, PointerButton::Secondary, false)], order).0,
        None
    );
    let reversed = ["Code", "Firefox"];
    let menu = frame(vec![], reversed).1.unwrap();
    let button = pos2(menu.center().x, menu.bottom() - 12.0);
    frame(vec![Event::PointerMoved(button)], reversed);
    frame(vec![click(button, PointerButton::Primary, true)], reversed);
    assert_eq!(
        frame(vec![click(button, PointerButton::Primary, false)], reversed),
        (Some(Action::Custom("Firefox".into())), None)
    );
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
fn calendar_grid_stretches_each_day_to_shared_column_width_and_week_height() {
    with_ui(|ui, icons| {
        let tile = |label: &'static str| {
            let content = ViewNode::column(
                vec![ViewNode::text(
                    label,
                    TextStyle {
                        wrap: true,
                        ..Default::default()
                    },
                )
                .sized(Size::Fill(1.0), Size::Auto)],
                Flow::default(),
            )
            .sized(Size::Fill(1.0), Size::Auto);
            let mut area = ViewNode::new(Node::ActionArea {
                action: Action::Custom("select-day".into()),
                label: label.into(),
                content: Box::new(content),
                context_menu: None,
            })
            .sized(Size::Fill(1.0), Size::Auto);
            area.size.min_height = Some(54.0);
            area.style.padding = [5.0, 9.0, 5.0, 8.0];
            area.style.normal.background = Some(ColorSpec::Surface);
            area
        };
        let mut days = vec![tile("15"); 14];
        days[3] = tile("Thanksgiving\nDay\nCelebration\n2026");
        // Seven weekday columns need not have equal widths; the long
        // holiday only widens its own column, while every week stays aligned.
        let column_widths = [54.0, 54.0, 75.0, 108.0, 54.0, 54.0, 54.0];
        let mut grid = Grid::new(Columns::Tracks(
            column_widths.into_iter().map(Track::Fixed).collect()
        ));
        grid.rows = vec![Track::Auto; 2];
        grid.column_gap = Some(6.0);
        grid.row_gap = Some(6.0);
        let node = ViewNode::new(Node::Grid(grid, days));
        let result = Renderer::new(ui, icons)
            .layout(ui, &node, vec2(840.0, 450.0), 1)
            .unwrap();
        assert_eq!(result.cells.len(), 14);
        let first_week = &result.cells[..7];
        let second_week = &result.cells[7..14];
        let first_height = first_week[0].1.height();
        let second_height = second_week[0].1.height();
        assert!(
            first_height > second_height,
            "the longest holiday must expand every day in that week"
        );
        for week in [first_week, second_week] {
            let height = week[0].1.height();
            for (_, rect) in week {
                assert!((rect.height() - height).abs() < 0.01);
                let col = week.iter().position(|(_, other)| other == rect)
                    .expect("the day belongs to a weekday column");
                assert!((rect.width() - column_widths[col]).abs() < 0.01);
            }
        }
        assert!(first_week[3].1.width() > first_week[0].1.width());
        for column in 0..7 {
            let upper = first_week[column].1;
            let lower = second_week[column].1;
            assert!((upper.left() - lower.left()).abs() < 0.01);
            assert!((upper.width() - lower.width()).abs() < 0.01);
        }
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
fn text_roles_do_not_change_when_accent_changes() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let read = |accent| {
        crate::ui::palette::install_accent_tokens(&ctx, accent);
        let colors = Cell::new(None);
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let theme = Renderer::new(ui, &icons).theme;
                colors.set(Some([
                    theme.color(ui, ColorSpec::PrimaryText),
                    theme.color(ui, ColorSpec::DisabledText),
                    theme.color(ui, ColorSpec::Accent),
                    theme.color(ui, ColorSpec::rgb(24, 89, 146)),
                ]));
            });
        });
        colors.get().unwrap()
    };
    let blue = read([37, 87, 156]);
    let violet = read([109, 69, 160]);
    assert_eq!(blue[0], violet[0]);
    assert_eq!(blue[1], violet[1]);
    assert_ne!(blue[2], violet[2]);
    assert_eq!(blue[3], violet[3]);
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
fn semantic_text_uses_the_dark_palette() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let theme = Renderer::new(ui, &icons).theme;
            let text = TextPalette::DEFAULT;
            for (role, expected) in [
                (ColorSpec::PrimaryText, text.primary),
                (ColorSpec::SecondaryText, text.secondary),
                (ColorSpec::MutedText, text.muted),
                (ColorSpec::DisabledText, text.disabled),
                (ColorSpec::PlaceholderText, text.placeholder),
                (ColorSpec::Success, text.success),
                (ColorSpec::Warning, text.warning),
                (ColorSpec::Danger, text.danger),
                (ColorSpec::Info, text.info),
                (ColorSpec::Cyan, text.cyan),
                (ColorSpec::Violet, text.violet),
            ] {
                assert_eq!(theme.color(ui, role), expected);
            }
        });
    });
}

#[test]
fn capsule_preserves_independent_regions_and_draws_a_center_overlay() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(92.0, 32.0));
    let ring = ViewNode::ring(0.5, "", 24.0, 2.0, 8.0).color(ColorSpec::Info);
    let heading = "A heading that wraps and should be ellipsized".repeat(3);
    let node = ViewNode::capsule(
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
        CapsuleLayout {
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
            show_capsule(
                ui,
                rect,
                &ExtensionId::builtin("test.capsule"),
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
            egui::Shape::Path(path) if path.closed => {
                Some(Rect::from_points(&path.points).center())
            }
            _ => None,
        })
        .unwrap();
    assert!((center - ring_center).length() < 1.0);
    assert!((ring_center.y - rect.center().y).abs() < 0.1);
    assert!(leading_clip.right() + 5.0 <= content_clip.left());
    assert!(content_clip.right() <= rect.right() - 4.0);
    assert!(title.galley.elided);
    assert_eq!(title.galley.rows.len(), 1);
}

#[test]
fn capsule_centers_its_leading_and_content_together_in_wide_chips() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let bounds = Rect::from_min_size(pos2(20.0, 20.0), vec2(360.0, 28.0));
    let marker = Color32::from_rgb(35, 178, 220);
    let mut leading = spacer(Size::Fixed(24.0), Size::Fixed(24.0));
    leading.style.normal.background = Some(ColorSpec::rgb(35, 178, 220));
    let label = "Short lyric";
    let node = ViewNode::capsule(
        leading,
        ViewNode::text(
            label,
            TextStyle {
                align: Align::Start,
                ..Default::default()
            },
        ),
        CapsuleLayout {
            gap: 5.0,
            ..Default::default()
        },
    );
    let output = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            show_capsule(
                ui,
                bounds,
                &ExtensionId::builtin("test.centered-capsule"),
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
fn motion_transforms_paint_without_moving_capsule_regions_and_reduced_motion_is_static() {
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
    let node = ViewNode::capsule(
        motion,
        ViewNode::text("status", TextStyle::default()),
        CapsuleLayout {
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
                    show_capsule(
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
    let atlas = ctx.load_texture(
        "test.sprite-atlas",
        egui::ColorImage::new([16, 256], Color32::WHITE),
        egui::TextureOptions::NEAREST,
    );
    register_image(&ctx, "test.sprite-atlas", atlas);
    let node = ViewNode::new(Node::SpriteSequence(SpriteSequence {
        frames: vec![
            SpriteFrame {
                asset: "test.sprite-atlas".into(),
                duration_ms: 100,
                uv: [0.0, 0.0, 1.0, 0.0625],
            },
            SpriteFrame {
                asset: "test.sprite-atlas".into(),
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
                    show_capsule(
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
            show_capsule(
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
fn capsule_can_anchor_its_leading_and_center_short_content_across_the_chip() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(360.0, 28.0));
    let marker = Color32::from_rgb(45, 156, 210);
    let mut leading = spacer(Size::Fixed(24.0), Size::Fixed(24.0));
    leading.style.normal.background = Some(ColorSpec::rgb(45, 156, 210));
    let draw = |text: &str| {
        let node = ViewNode::capsule(
            leading.clone(),
            ViewNode::text(text, TextStyle::default()),
            CapsuleLayout {
                width: Size::Fill(1.0),
                content_size: SizeSpec::default(),
                content_align: Align::Center,
                ..Default::default()
            },
        );
        ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_capsule(
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
        assert!(clip.left() >= icon.right() + CapsuleLayout::default().gap);
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

#[test]
fn progress_slider_previews_drag_and_dispatches_one_bounded_value_on_release() {
    let ctx = egui::Context::default();
    let icons = Icons::load(&ctx);
    let source = ExtensionId::builtin("test.progress");
    let rect = Rect::from_min_size(pos2(30.0, 30.0), vec2(100.0, 24.0));
    let frame = |events, enabled| {
        let mut action = None;
        let view = WidgetView::new(
            ViewNode::new(Node::Slider {
                fraction: 0.2,
                label: "Progress".into(),
                action: "seek".into(),
                enabled,
            })
            .sized(Size::Fill(1.0), Size::Fixed(24.0)),
            [100.0, 24.0],
        );
        let _ = ctx.run(
            RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = show_widget(ui, rect, &source, &view, &icons, 1.0);
                });
            },
        );
        action
    };
    frame(vec![], true);
    frame(vec![Event::PointerMoved(pos2(50.0, 42.0))], true);
    let button = |x, pressed| Event::PointerButton {
        pos: pos2(x, 42.0),
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    assert!(frame(vec![button(50.0, true)], true).is_none());
    assert!(frame(vec![Event::PointerMoved(pos2(110.0, 42.0))], true).is_none());
    let action = frame(vec![button(110.0, false)], true).unwrap();
    assert_eq!(
        action.action,
        Action::SetProgress {
            key: "seek".into(),
            basis_points: 8000
        }
    );
    let key_action = frame(
        vec![Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        true,
    )
    .unwrap();
    assert_eq!(
        key_action.action,
        Action::SetProgress {
            key: "seek".into(),
            basis_points: 2100
        }
    );
    // The focused slider consumes the arrow so the host cannot also turn pages.
    assert!(!ctx.input(|input| input.key_pressed(egui::Key::ArrowRight)));
    frame(vec![], false);
    frame(vec![button(50.0, true)], false);
    assert!(frame(vec![button(50.0, false)], false).is_none());
}

#[test]
fn small_svg_uses_target_physical_size_and_antialiased_edges_at_fractional_dpi() {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(1.5);
    let icons = Icons::load(&ctx);
    let source = ExtensionId::builtin("test.smooth-clock");
    let node = ViewNode::svg(
        r#"inline:<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.3" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><path d="M12 12L12 5M12 12L17 12"/></svg>"#,
        18.0,
    );
    let mut physical_size = 0;
    let output = ctx.run(RawInput::default(), |ctx| {
        physical_size = (18.0 * ctx.pixels_per_point()).ceil() as usize;
        egui::CentralPanel::default().show(ctx, |ui| {
            show_capsule(
                ui,
                Rect::from_min_size(pos2(30.0, 30.0), vec2(18.0, 18.0)),
                &source,
                &node,
                &icons,
                1.0,
            );
        });
    });
    assert!(output.textures_delta.set.iter().any(|(_, delta)| {
        let egui::ImageData::Color(image) = &delta.image else {
            return false;
        };
        image.size == [physical_size, physical_size]
            && image.pixels.iter().any(|p| p.a() > 0 && p.a() < 255)
    }));
}

#[test]
fn reader_sidebars_fill_height_and_keep_the_footer_inside_at_multiple_sizes() {
    let views: Vec<_> = crate::builtins::runtimes()
        .into_iter()
        .filter_map(|extension| match extension.descriptor().id.as_str() {
            "builtin.rss" => extension.widget_view(&ExtensionId::builtin("builtin.rss/reader")),
            "builtin.mail" => extension.widget_view(&ExtensionId::builtin("builtin.mail/inbox")),
            _ => None,
        })
        .collect();
    assert_eq!(views.len(), 2);
    with_ui(|ui, icons| {
        for view in views {
            let row = match &view.root.kind {
                Node::Column(_, children) => children.last().unwrap(),
                _ => &view.root,
            };
            let Node::Row(_, children) = &row.kind else {
                panic!("reader must have two columns")
            };
            for available in [vec2(500.0, 300.0), vec2(720.0, 450.0), vec2(900.0, 720.0)] {
                let mut renderer = Renderer::new(ui, icons);
                let columns = renderer.layout(ui, row, available, 0).unwrap();
                let left = columns.cells[0].1;
                let right = columns.cells[1].1;
                assert_eq!(left.height(), available.y);
                assert!(right.left() >= left.right() + 9.0);
                assert!(right.right() <= available.x + 0.01);
                let sidebar = &children[0];
                let inner = layout::inset(
                    Rect::from_min_size(Pos2::ZERO, left.size()),
                    sidebar.style.padding,
                );
                let rows = renderer.layout(ui, sidebar, inner.size(), 1).unwrap();
                let footer = rows.cells.last().unwrap().1;
                assert!((footer.bottom() - inner.height()).abs() < 0.01);
                assert!(rows.cells[1].1.bottom() < footer.top());
            }
        }
    });
}
