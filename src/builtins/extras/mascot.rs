use crate::{
    extension::{
        Action, CompactView, ExtensionDescriptor, ExtensionId, ExtensionKind, ExtensionOutput,
        RuntimeExtension,
    },
    ui::declarative::{
        CompactLayout, Node, Playback, Repeat, Size, SpriteFrame, SpriteSequence, Trigger, ViewNode,
    },
};
use std::time::{Duration, Instant};

const ID: &str = "builtin.mascot/face";
const START_DELAY: Duration = Duration::from_secs(4);
const IDLE_DELAY: Duration = Duration::from_secs(7);

const SHEETS: [(&str, &[u8]); 12] = [
    (
        "mascot/wink",
        include_bytes!("../../../assets/mascot/expressions/00-wink.png"),
    ),
    (
        "mascot/smug",
        include_bytes!("../../../assets/mascot/expressions/01-smug.png"),
    ),
    (
        "mascot/blush",
        include_bytes!("../../../assets/mascot/expressions/02-blush.png"),
    ),
    (
        "mascot/delighted",
        include_bytes!("../../../assets/mascot/expressions/03-delighted.png"),
    ),
    (
        "mascot/side-glance",
        include_bytes!("../../../assets/mascot/expressions/04-side-glance.png"),
    ),
    (
        "mascot/praised",
        include_bytes!("../../../assets/mascot/expressions/05-praised.png"),
    ),
    (
        "mascot/heart",
        include_bytes!("../../../assets/mascot/expressions/06-heart.png"),
    ),
    (
        "mascot/giggle",
        include_bytes!("../../../assets/mascot/expressions/07-giggle.png"),
    ),
    (
        "mascot/shy-peek",
        include_bytes!("../../../assets/mascot/expressions/08-shy-peek.png"),
    ),
    (
        "mascot/scan",
        include_bytes!("../../../assets/mascot/expressions/09-scan.png"),
    ),
    (
        "mascot/peek",
        include_bytes!("../../../assets/mascot/expressions/10-peek.png"),
    ),
    (
        "mascot/search",
        include_bytes!("../../../assets/mascot/expressions/11-search.png"),
    ),
];

const EXPRESSION_TIMES: [u32; 16] = [
    350, 120, 120, 120, 130, 130, 140, 400, 180, 140, 120, 120, 120, 140, 140, 450,
];
const IDLE_TIMES: [u32; 16] = [
    350, 130, 150, 180, 350, 280, 160, 110, 110, 160, 350, 280, 160, 140, 200, 350,
];

pub(super) fn descriptor() -> ExtensionDescriptor {
    ExtensionDescriptor::builtin(ID, ExtensionKind::Compact, "表情", "mascot", true, 0)
        .with_size_hint(38.0, 28.0)
        .with_capabilities(vec![crate::extension::Capability::Action])
}

pub(crate) fn register_assets(ctx: &eframe::egui::Context) {
    for (asset, bytes) in SHEETS {
        crate::ui::renderer::register_png(ctx, asset, bytes);
    }
}

pub(crate) struct Mascot {
    expression: usize,
    next_expression_at: Instant,
    revision: u64,
}

impl Default for Mascot {
    fn default() -> Self {
        Self {
            expression: 0,
            next_expression_at: Instant::now() + START_DELAY,
            revision: 0,
        }
    }
}

impl Mascot {
    fn timings(&self) -> &[u32; 16] {
        if self.expression >= 9 {
            &IDLE_TIMES
        } else {
            &EXPRESSION_TIMES
        }
    }

    fn advance(&mut self, now: Instant) -> ExtensionOutput {
        self.expression = (self.expression + 1) % SHEETS.len();
        self.revision += 1;
        let duration_ms = u64::from(self.timings().iter().sum::<u32>());
        self.next_expression_at = now + Duration::from_millis(duration_ms) + IDLE_DELAY;
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(ID)],
            ..Default::default()
        }
    }
}

impl crate::extension::NotificationPresentationProvider for Mascot {}

impl RuntimeExtension for Mascot {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        vec![descriptor()]
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        if now >= self.next_expression_at {
            self.advance(now)
        } else {
            ExtensionOutput::default()
        }
    }

    fn on_action(&mut self, action: Action, now: Instant) -> ExtensionOutput {
        if action == Action::Advance {
            self.advance(now)
        } else {
            ExtensionOutput::default()
        }
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let (asset, _) = SHEETS[self.expression];
        let mut frames: Vec<_> = self
            .timings()
            .iter()
            .enumerate()
            .map(|(index, duration_ms)| SpriteFrame {
                asset: asset.into(),
                duration_ms: *duration_ms,
                uv: [0.0, index as f32 / 16.0, 1.0, (index + 1) as f32 / 16.0],
            })
            .collect();
        frames.push(SpriteFrame {
            duration_ms: 16,
            ..frames[0].clone()
        });
        let mut sprite = ViewNode::new(Node::SpriteSequence(SpriteSequence {
            frames,
            trigger: Trigger::While(self.revision > 0),
            playback: Playback::Running,
            repeat: Repeat::Once,
            delay_ms: 0,
            gap_ms: 0,
            speed: 1.0,
        }))
        .sized(Size::Fill(1.0), Size::Fill(1.0));
        sprite.key = Some(self.revision.to_string());
        let leading = ViewNode::new(Node::ActionArea {
            action: Action::Advance,
            label: "切换吉祥物表情".into(),
            content: Box::new(sprite),
        })
        .sized(Size::Fill(1.0), Size::Fill(1.0));
        vec![CompactView {
            id: ExtensionId::builtin(ID),
            source_extension_id: ExtensionId::builtin(ID),
            content: ViewNode::compact(
                leading,
                ViewNode::new(Node::Spacer),
                CompactLayout {
                    leading_size: [32.0, 28.0],
                    gap: 0.0,
                    ..Default::default()
                },
            ),
            visibility: Default::default(),
            min_width: 38.0,
            preferred_width: 38.0,
            fit_width: false,
            priority: 0,
            tooltip: Some("点击切换吉祥物表情".into()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[test]
    fn clicking_or_reaching_the_idle_deadline_changes_the_declared_sprite() {
        let mut mascot = Mascot::default();
        let initial = mascot.compact_views();
        assert!(mascot
            .update(mascot.next_expression_at - Duration::from_millis(1))
            .invalidated
            .is_empty());
        assert_eq!(
            mascot.update(mascot.next_expression_at).invalidated,
            vec![ExtensionId::builtin(ID)]
        );
        assert_ne!(mascot.compact_views(), initial);
        let before_click = mascot.compact_views();
        mascot.on_action(Action::Advance, Instant::now());
        assert_ne!(mascot.compact_views(), before_click);
    }

    #[test]
    fn bundled_reference_art_has_sixteen_transparent_animation_frames() {
        for (_, bytes) in SHEETS {
            let image = image::load_from_memory(bytes).expect("valid expression PNG");
            assert_eq!(image.dimensions(), (144, 1440));
            assert!(image.color().has_alpha());
        }
    }
}
