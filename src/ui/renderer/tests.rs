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
