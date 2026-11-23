use crate::{
    core::{mascot::MascotExpression, worker::Worker},
    extension::*,
    ui::declarative::*,
};
use super::{power, power::PowerSnapshot};
const ID: &str = "builtin.device/battery";
const PROMPT_ID: &str = "builtin.device/power-prompt";
#[derive(Default)]
pub(super) struct BatteryExtension {
    updates: Option<Worker<PowerSnapshot>>,
    snapshot: PowerSnapshot,
    sampled: bool,
}
impl NotificationPresentationProvider for BatteryExtension {}
impl RuntimeExtension for BatteryExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        ExtensionDescriptor::builtin(
            "builtin.device",
            "电池",
            "电量、充电状态和电源动画",
            vec![
                ContributionDescriptor::builtin(
                    ID,
                    ContributionKind::Capsule,
                    "电池",
                    "battery",
                    true,
                    1,
                )
                .with_size_hint(64.0, 28.0),
                ContributionDescriptor::builtin(
                    PROMPT_ID,
                    ContributionKind::Prompt,
                    "电源提示",
                    "battery-charging",
                    true,
                    1,
                ),
            ],
        )
    }
    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.updates = Some(power::start(&ctx.tasks()).map_err(|error| error.to_string())?);
        Ok(())
    }
    fn on_stop(&mut self, _: &ExtensionContext) -> Result<(), String> {
        self.updates = None;
        Ok(())
    }
    fn update(&mut self, _: std::time::Instant) -> ExtensionOutput {
        if let Some(snapshot) = self.updates.as_ref().and_then(Worker::take) {
            if snapshot != self.snapshot {
                let was_sampled = self.sampled;
                let was_connected = self.snapshot.external == Some(true);
                let connected = snapshot.external == Some(true);
                self.snapshot = snapshot;
                self.sampled = true;
                let prompts = if was_sampled && !was_connected && connected {
                    vec![crate::extension::Prompt::new(
                        ExtensionId::builtin("builtin.device"),
                        ExtensionId::builtin(PROMPT_ID),
                        battery_prompt(&self.snapshot),
                        std::time::Duration::from_secs(3),
                    )
                    .with_width(204.0, 222.0)
                    .with_accent([78, 218, 151])
                    .with_layers(battery_prompt_layers(&self.snapshot))]
                } else {
                    Vec::new()
                };
                return ExtensionOutput {
                    invalidated: vec![ExtensionId::builtin(ID)],
                    prompts,
                    ..Default::default()
                };
            }
        }
        Default::default()
    }
    fn capsule_views(&self) -> Vec<CapsuleView> {
        vec![CapsuleView {
            id: ExtensionId::builtin(ID),
            source_extension_id: ExtensionId::builtin("builtin.device"),
            content: battery_capsule(&self.snapshot),
            visibility: Default::default(),
            min_width: 64.0,
            preferred_width: 64.0,
            fit_width: true,
            priority: 0,
            tooltip: Some(power_label(&self.snapshot).into()),
        }]
    }
    fn mascot_state(&self) -> std::collections::BTreeMap<String, String> {
        [("charging".into(), self.snapshot.charging.to_string())].into()
    }
    fn mascot_expressions(&self) -> Vec<MascotExpression> {
        vec![MascotExpression::state(
            "charging",
            "charging",
            "true",
            crate::core::mascot::MascotMotion::Charging,
        )]
    }
}
fn battery_capsule(metrics: &PowerSnapshot) -> ViewNode {
    battery_view(metrics, battery_label(metrics))
}

fn battery_prompt(metrics: &PowerSnapshot) -> ViewNode {
    battery_view(
        metrics,
        if metrics.charging {
            "正在充电".into()
        } else {
            "已接通电源".into()
        },
    )
}

fn battery_prompt_layers(metrics: &PowerSnapshot) -> Vec<PromptLayer> {
    let label = if metrics.charging {
        "正在充电"
    } else {
        "已接通电源"
    };
    let color = battery_color(metrics);
    vec![
        PromptLayer::new(
            [0.06, 0.04, 0.19, 0.92],
            battery_symbol(metrics, 1.15, true),
        ),
        PromptLayer::new(
            [0.28, 0.08, 0.47, 0.84],
            ViewNode::text(
                label,
                TextStyle {
                    font_size: FontSize::Relative(1),
                    ..Default::default()
                },
            )
            .color(ColorSpec::PrimaryText),
        ),
        PromptLayer::new(
            [0.78, 0.08, 0.17, 0.84],
            ViewNode::text(
                battery_label(metrics),
                TextStyle {
                    font_size: FontSize::Relative(0.9167),
                    ..Default::default()
                },
            )
            .color(color),
        ),
    ]
}

/// Shared vector shell and continuous fill. The charging sweep is clipped to
/// the interior, so neither the outline nor the text moves or pulses.
fn battery_symbol(metrics: &PowerSnapshot, scale: f32, connection_prompt: bool) -> ViewNode {
    let level = f32::from(metrics.percent.unwrap_or(0).min(100)) / 100.0;
    let color = battery_color(metrics);
    let animate = metrics.charging || connection_prompt;
    let track = ViewNode::new(Node::Spacer)
        .sized(Size::Fill(1.0), Size::Fill(1.0))
        .styled(Style {
            normal: Paint {
                background: Some(ColorSpec::Rgba {
                    r: 140,
                    g: 160,
                    b: 180,
                    a: 35,
                }),
                corner_radius: Some(1.5 * scale),
                ..Default::default()
            },
            ..Default::default()
        });
    let fill = ViewNode::new(Node::Spacer)
        .sized(Size::Percent(level), Size::Fill(1.0))
        .styled(Style {
            normal: Paint {
                background: Some(color),
                corner_radius: Some(1.5 * scale),
                ..Default::default()
            },
            ..Default::default()
        });
    let mut inside = vec![track, fill];
    if animate {
        // A soft, three-part highlight travels through the cell, not around it.
        let band = ViewNode::row(
            [30, 85, 30]
                .into_iter()
                .map(|alpha| {
                    ViewNode::new(Node::Spacer)
                        .sized(Size::Fixed(1.5 * scale), Size::Fill(1.0))
                        .styled(Style {
                            normal: Paint {
                                background: Some(ColorSpec::Rgba {
                                    r: 255,
                                    g: 255,
                                    b: 255,
                                    a: alpha,
                                }),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                })
                .collect(),
            Flow {
                gap: Some(0.0),
                ..Default::default()
            },
        )
        .sized(Size::Fixed(4.5 * scale), Size::Fill(1.0))
        .motion(Motion {
            duration_ms: 1600,
            gap_ms: 350,
            repeat: Repeat::Infinite,
            easing: Easing::EaseInOut,
            tracks: vec![
                MotionTrack::new(
                    MotionProperty::TranslateX,
                    &[(0, -5.0 * scale), (1600, 18.0 * scale)],
                ),
                MotionTrack::new(
                    MotionProperty::Opacity,
                    &[(0, 0.0), (220, 1.0), (1300, 1.0), (1600, 0.0)],
                ),
            ],
            ..Default::default()
        });
        inside.push(band);
    }
    let mut cell = ViewNode::new(Node::Stack([Align::Start; 2], inside))
        .sized(Size::Fill(1.0), Size::Fill(1.0));
    cell.clip = true;
    let interior = ViewNode::new(Node::Container(Box::new(cell)))
        .sized(Size::Fill(1.0), Size::Fill(1.0))
        .styled(Style {
            padding: [3.5 * scale, 5.0 * scale, 6.5 * scale, 5.0 * scale],
            ..Default::default()
        });
    let mut layers = vec![
        ViewNode::svg("capsule/battery", 28.0 * scale)
            .sized(Size::Fixed(28.0 * scale), Size::Fixed(18.0 * scale))
            .color(ColorSpec::MutedText),
        interior,
    ];
    if metrics.charging {
        layers.push(
            ViewNode::new(Node::Container(Box::new(
                ViewNode::new(Node::Stack(
                    [Align::Center; 2],
                    vec![ViewNode::svg("capsule/bolt", 12.0 * scale)
                        .sized(Size::Fixed(7.0 * scale), Size::Fixed(12.0 * scale))
                        .color(ColorSpec::OnAccent)],
                ))
                .sized(Size::Fill(1.0), Size::Fill(1.0)),
            )))
            .sized(Size::Fill(1.0), Size::Fill(1.0))
            .styled(Style {
                padding: [0.0, 0.0, 3.0 * scale, 0.0],
                ..Default::default()
            }),
        );
    }
    ViewNode::new(Node::Stack([Align::Center; 2], layers))
        .sized(Size::Fixed(28.0 * scale), Size::Fixed(18.0 * scale))
}

fn battery_view(metrics: &PowerSnapshot, label: String) -> ViewNode {
    ViewNode::capsule(
        battery_symbol(metrics, 1.0, false),
        ViewNode::text(
            label,
            TextStyle {
                font_size: FontSize::Relative(0.9167),
                align: Align::Start,
                ..Default::default()
            },
        )
        .color(ColorSpec::PrimaryText),
        CapsuleLayout {
            leading_size: [28.0, 18.0],
            gap: 5.0,
            ..Default::default()
        },
    )
}

fn power_label(metrics: &PowerSnapshot) -> &'static str {
    if metrics.charging {
        "正在充电"
    } else if metrics.external == Some(true) && metrics.percent.is_none() {
        "已接通电源 · 电池电量未知或无电池"
    } else if metrics.external == Some(true) {
        "已接通电源 · 未充电"
    } else if metrics.external == Some(false) {
        "使用电池供电"
    } else {
        "电源状态未知"
    }
}

pub(super) fn battery_label(metrics: &PowerSnapshot) -> String {
    metrics
        .percent
        .map_or_else(|| "--".into(), |v| format!("{v}%"))
}

pub(super) fn battery_color(metrics: &PowerSnapshot) -> ColorSpec {
    if metrics.percent.is_some_and(|v| v < 20) {
        ColorSpec::Danger
    } else if metrics.charging || metrics.external == Some(true) {
        ColorSpec::Success
    } else {
        ColorSpec::Info
    }
}

#[cfg(test)]
mod appearance_tests {
    use super::*;
    use crate::{app::icons::Icons, ui::renderer};
    use eframe::egui::{self, pos2, vec2, Rect};

    #[test]
    fn continuous_fill_matches_charge_at_capsule_and_prompt_sizes() {
        for scale in [1.0, 1.15] {
            for percent in [10, 50, 100] {
                let ctx = egui::Context::default();
                ctx.set_visuals(egui::Visuals::dark());
                let icons = Icons::load(&ctx);
                let metrics = PowerSnapshot {
                    percent: Some(percent),
                    external: Some(true),
                    charging: percent < 100,
                };
                let view = WidgetView::new(
                    battery_symbol(&metrics, scale, false),
                    [28.0 * scale, 18.0 * scale],
                );
                let bounds =
                    Rect::from_min_size(pos2(40.0, 40.0), vec2(28.0 * scale, 18.0 * scale));
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        renderer::show_widget(
                            ui,
                            bounds,
                            &ExtensionId::builtin(ID),
                            &view,
                            &icons,
                            1.0,
                        );
                    });
                });
                if metrics.charging {
                    let bolt = output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect) if rect.brush.is_some() => Some(rect.rect),
                            _ => None,
                        })
                        .next_back()
                        .expect("charging bolt must render");
                    let center = bounds.min + vec2(12.5 * scale, 9.0 * scale);
                    assert!(
                        bolt.center().distance(center) < 0.1,
                        "bolt must be centered in the cell: {bolt:?}"
                    );
                    assert!(bolt.height() >= 11.0 * scale);
                }
                let width = 18.0 * scale * f32::from(percent) / 100.0;
                assert!(
                    output.shapes.iter().any(|shape| {
                        matches!(&shape.shape, egui::Shape::Rect(rect)
                        if (rect.rect.width() - width).abs() < 0.05
                        && (rect.rect.height() - 8.0 * scale).abs() < 0.05
                        && rect.fill.a() == 255)
                    }),
                    "charge fill must occupy {percent}% of the cell at scale {scale}"
                );
            }
        }
    }
    #[test]
    fn full_charge_percentage_is_not_elided_at_measured_width() {
        let ctx = egui::Context::default();
        let icons = Icons::load(&ctx);
        let extension = BatteryExtension {
            snapshot: PowerSnapshot {
                percent: Some(100),
                external: Some(true),
                charging: false,
            },
            ..Default::default()
        };
        let views = extension.capsule_views();
        let view = &views[0];
        assert!(view.fit_width);
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            let width = (renderer::natural_content_width(ctx, &view.content, 4096.0, &icons) + 6.0)
                .ceil()
                .max(view.min_width);
            egui::CentralPanel::default().show(ctx, |ui| {
                let chip = Rect::from_min_size(pos2(40.0, 40.0), vec2(width, 28.0));
                renderer::show_capsule(
                    ui,
                    chip.shrink2(vec2(3.0, 0.0)),
                    &view.id,
                    &view.content,
                    &icons,
                    1.0,
                );
            });
        });
        let label = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "100%" => Some(&text.galley),
                _ => None,
            })
            .expect("full percentage must render");
        assert!(!label.elided);
    }
}
