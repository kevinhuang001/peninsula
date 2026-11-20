pub(crate) mod power;
use crate::{
    core::{mascot::MascotExpression, worker::Worker},
    extension::*,
    ui::declarative::*,
};
use power::PowerSnapshot;
const ID: &str = "builtin.battery/status";
const PROMPT_ID: &str = "builtin.battery/prompt";
#[derive(Default)]
pub struct BatteryExtension {
    updates: Option<Worker<PowerSnapshot>>,
    snapshot: PowerSnapshot,
    sampled: bool,
}
impl NotificationPresentationProvider for BatteryExtension {}
impl RuntimeExtension for BatteryExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        ExtensionDescriptor::builtin(
            "builtin.battery",
            "电池",
            "电量、充电状态和电源动画",
            vec![
                ContributionDescriptor::builtin(
                    ID,
                    ContributionKind::Compact,
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
    fn on_start(&mut self, _: &ExtensionContext) -> Result<(), String> {
        self.updates = Some(power::start());
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
                        ExtensionId::builtin("builtin.battery"),
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
    fn compact_views(&self) -> Vec<CompactView> {
        vec![CompactView {
            id: ExtensionId::builtin(ID),
            source_extension_id: ExtensionId::builtin("builtin.battery"),
            content: battery_compact(&self.snapshot),
            visibility: Default::default(),
            min_width: 64.0,
            preferred_width: 64.0,
            fit_width: false,
            priority: 0,
            tooltip: Some(power_label(&self.snapshot).into()),
        }]
    }
    fn mascot_state(&self) -> std::collections::BTreeMap<String, String> {
        [("charging".into(), self.snapshot.charging.to_string())].into()
    }
    fn mascot_expressions(&self) -> Vec<MascotExpression> {
        vec![MascotExpression::state("charging", "charging", "true", 6)]
    }
}
fn battery_compact(metrics: &PowerSnapshot) -> ViewNode {
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
    let glow = ColorSpec::rgb(99, 238, 174);
    let entrance = Motion {
        duration_ms: 650,
        easing: Easing::EaseOut,
        tracks: vec![
            MotionTrack::new(MotionProperty::Scale, &[(0, 0.78), (320, 1.08), (650, 1.0)]),
            MotionTrack::new(MotionProperty::Opacity, &[(0, 0.0), (180, 1.0)]),
        ],
        ..Default::default()
    };
    vec![
        PromptLayer::new(
            [0.06, 0.04, 0.19, 0.92],
            ViewNode::svg("compact/battery", 27.0)
                .color(glow)
                .motion(entrance),
        ),
        PromptLayer::new(
            [0.28, 0.08, 0.47, 0.84],
            ViewNode::text(
                label,
                TextStyle {
                    font_size: FontSize::Custom(12.0),
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
                    font_size: FontSize::Custom(11.0),
                    ..Default::default()
                },
            )
            .color(glow),
        ),
    ]
}

fn battery_view(metrics: &PowerSnapshot, label: String) -> ViewNode {
    let level = f32::from(metrics.percent.unwrap_or(0)) / 100.0;
    let color = battery_color(metrics);
    let indicator = |segments, fraction, width, inactive, color| {
        ViewNode::new(Node::SegmentedIndicator {
            segments,
            fraction,
            gap: 0.9,
            direction: FillDirection::LeftToRight,
            inactive,
        })
        .sized(Size::Fixed(width), Size::Fixed(5.0))
        .color(color)
    };
    let leading = ViewNode::new(Node::Stack(
        [Align::Center; 2],
        vec![
            ViewNode::svg("compact/battery", 24.0).color(color),
            indicator(5, level, 17.0, ColorSpec::Border, color)
                .motion(Motion::progress(level, 200)),
            indicator(1, 1.0, 1.8, ColorSpec::Transparent, ColorSpec::OnAccent)
                .styled(Style {
                    normal: Paint {
                        foreground: Some(ColorSpec::OnAccent),
                        opacity: Some(if metrics.charging { 0.55 } else { 0.0 }),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .motion(Motion {
                    trigger: Trigger::While(metrics.charging),
                    duration_ms: 2400,
                    gap_ms: 500,
                    repeat: Repeat::Infinite,
                    tracks: vec![
                        MotionTrack::new(MotionProperty::TranslateX, &[(0, -8.0), (2400, 8.0)]),
                        MotionTrack::new(
                            MotionProperty::Opacity,
                            &[(0, 0.0), (400, 1.0), (2000, 1.0), (2400, 0.0)],
                        ),
                    ],
                    ..Default::default()
                }),
        ],
    ))
    .sized(Size::Fixed(25.0), Size::Fixed(24.0));
    ViewNode::compact(
        leading,
        super::compact_value(label, false).color(color),
        CompactLayout {
            leading_size: [25.0, 24.0],
            gap: 4.0,
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
        ColorSpec::PrimaryText
    }
}
