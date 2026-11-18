use crate::{
    extension::{Action, CompactView, ExtensionId},
    ui::declarative::{
        CompactLayout, Node, Playback, Repeat, Size, SpriteFrame, SpriteSequence, Trigger, ViewNode,
    },
};

const ID: &str = super::MASCOT_SLOT_ID;

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

pub fn register_assets(ctx: &eframe::egui::Context) {
    for (asset, bytes) in SHEETS {
        crate::ui::renderer::register_png(ctx, asset, bytes);
    }
}

#[derive(Default)]
pub(crate) struct Mascot {
    expression: usize,
    revision: u64,
}

impl Mascot {
    fn timings(&self) -> &[u32; 16] {
        if self.expression >= 9 {
            &IDLE_TIMES
        } else {
            &EXPRESSION_TIMES
        }
    }
}

impl Mascot {
    pub fn select(&mut self, expression: usize) {
        let expression = expression.min(SHEETS.len() - 1);
        if expression != self.expression {
            self.expression = expression;
            self.revision += 1;
        }
    }

    pub fn compact_views(&self) -> Vec<CompactView> {
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

pub(super) fn freeze(node: &mut ViewNode) {
    match &mut node.kind {
        Node::SpriteSequence(sequence) => {
            sequence.playback = Playback::Paused;
            sequence.trigger = Trigger::While(false);
        }
        Node::Compact(compact) => {
            freeze(&mut compact.leading);
            freeze(&mut compact.content);
        }
        Node::Motion { child, motion } => {
            motion.playback = Playback::Paused;
            freeze(child);
        }
        Node::ActionArea { content, .. } | Node::Container(content) | Node::Scroll(_, content) => {
            freeze(content)
        }
        Node::Sprite { frames, .. } => {
            frames.truncate(1);
        }
        Node::Row(_, children)
        | Node::Column(_, children)
        | Node::Wrap(_, children)
        | Node::Stack(_, children)
        | Node::Grid(_, children) => {
            for child in children {
                freeze(child);
            }
        }
        _ => {}
    }
}
