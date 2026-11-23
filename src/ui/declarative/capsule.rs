use super::{Align, Overflow, Size, SizeSpec, ViewNode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Capsule {
    pub leading: ViewNode,
    pub content: ViewNode,
    #[serde(default)]
    pub layout: CapsuleLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapsuleLayout {
    pub leading_size: [f32; 2],
    pub leading_min_size: [f32; 2],
    pub leading_max_size: [Option<f32>; 2],
    /// Defaults to the remaining width and the intrinsic height.
    pub content_size: SizeSpec,
    pub leading_align: [Align; 2],
    /// Align Content within the full inner width, constrained to the space after Leading.
    pub content_align: Align,
    pub gap: f32,
    pub align: Align,
    /// Left, top, right, bottom.
    pub padding: [f32; 4],
    pub width: Size,
    pub height: Size,
    /// Scrollable overflow is confined to Content.
    pub overflow: Overflow,
}

impl Default for CapsuleLayout {
    fn default() -> Self {
        Self {
            leading_size: [24.0; 2],
            leading_min_size: [0.0; 2],
            leading_max_size: [None; 2],
            content_size: SizeSpec {
                width: Size::Fill(1.0),
                ..Default::default()
            },
            leading_align: [Align::Center; 2],
            content_align: Align::Start,
            gap: 4.0,
            align: Align::Center,
            padding: [0.0; 4],
            width: Size::Auto,
            height: Size::Auto,
            overflow: Overflow::Clip,
        }
    }
}
