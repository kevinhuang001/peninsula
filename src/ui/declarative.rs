//! Serializable extension UI contract. All dimensions are in egui points.
use crate::extension::Action;
use serde::{Deserialize, Serialize};

mod animation;
mod compact;
pub use animation::*;
pub use compact::*;

/// Generous host width for content-fitted, single-line surfaces (egui points).
pub const MAX_CONTENT_WIDTH: f32 = 4096.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WidgetView {
    pub root: ViewNode,
    pub preferred_size: [f32; 2],
    #[serde(default)]
    pub bounds: WidgetBounds,
    #[serde(default)]
    pub refresh_hint: RefreshHint,
}

impl WidgetView {
    pub fn new(root: ViewNode, preferred_size: [f32; 2]) -> Self {
        Self {
            root,
            preferred_size,
            bounds: WidgetBounds::default(),
            refresh_hint: RefreshHint::OnInvalidation,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum RefreshHint {
    #[default]
    OnInvalidation,
    EverySecond,
    Static,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WidgetBounds {
    pub min: [f32; 2],
    pub max: [Option<f32>; 2],
    pub resizable: bool,
    /// Measure the root and use its natural width, up to the host limit.
    pub fit_width: bool,
    /// Measure the root at the resolved widget width and use its natural height.
    pub fit_height: bool,
    pub overflow: Overflow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Overflow {
    #[default]
    Clip,
    Scroll(ScrollAxes),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScrollAxes {
    Horizontal,
    Vertical,
    Both,
}

impl ScrollAxes {
    pub fn axes(self) -> [bool; 2] {
        [self != Self::Vertical, self != Self::Horizontal]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Size {
    #[default]
    Auto,
    Fixed(f32),
    Fill(f32),
    /// 0.0–1.0, relative to the parent's inner area.
    Percent(f32),
    MinMax {
        min: f32,
        max: f32,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SizeSpec {
    pub width: Size,
    pub height: Size,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub aspect_ratio: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
    Stretch,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Flow {
    pub gap: Option<f32>,
    pub line_gap: Option<f32>,
    pub align: Align,
    pub justify: Justify,
    pub wrap: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Track {
    Auto,
    Fixed(f32),
    Fraction(f32),
    MinMax { min: f32, max: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Columns {
    Tracks(Vec<Track>),
    AutoFit {
        min_item_width: f32,
        max_columns: usize,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum AutoFlow {
    #[default]
    Row,
    Column,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub columns: Columns,
    pub rows: Vec<Track>,
    pub column_gap: Option<f32>,
    pub row_gap: Option<f32>,
    pub auto_flow: AutoFlow,
}

impl Grid {
    pub fn new(columns: Columns) -> Self {
        Self {
            columns,
            rows: vec![],
            column_gap: None,
            row_gap: None,
            auto_flow: AutoFlow::Row,
        }
    }
}

/// A table has one shared set of columns for its header and all rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<TableColumn>,
    /// Cells follow column order. An omitted cell is empty.
    pub rows: Vec<Vec<ViewNode>>,
    /// Optional right-click action for each row, in the same order as the cells.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_menus: Vec<Option<TableRowMenu>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableRowMenu {
    pub title: String,
    pub label: String,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableColumn {
    pub title: String,
    /// Relative share of the width available for cells.
    pub weight: f32,
}

impl TableColumn {
    pub fn new(title: impl Into<String>, weight: f32) -> Self {
        Self {
            title: title.into(),
            weight,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Placement {
    pub column: Option<usize>,
    pub row: Option<usize>,
    /// Zero is treated as one.
    pub column_span: usize,
    pub row_span: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ColorSpec {
    #[default]
    PrimaryText,
    SecondaryText,
    MutedText,
    Surface,
    SurfaceVariant,
    Accent,
    OnAccent,
    AccentSubtle,
    Border,
    Success,
    Warning,
    Danger,
    Info,
    Transparent,
    Rgba {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    },
}

impl ColorSpec {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::Rgba { r, g, b, a: 255 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: ColorSpec,
    pub offset: [i8; 2],
    pub blur: u8,
    pub spread: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paint {
    pub foreground: Option<ColorSpec>,
    pub background: Option<ColorSpec>,
    pub border_color: Option<ColorSpec>,
    pub border_width: Option<f32>,
    pub corner_radius: Option<f32>,
    pub opacity: Option<f32>,
    pub shadow: Option<Shadow>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub normal: Paint,
    pub hovered: Paint,
    pub pressed: Paint,
    pub disabled: Paint,
    /// Left, top, right, bottom.
    pub padding: [f32; 4],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FontSize {
    Small,
    #[default]
    Body,
    Title,
    Heading,
    Custom(f32),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub font_size: FontSize,
    pub font_weight: FontWeight,
    pub align: Align,
    pub line_height: Option<f32>,
    pub wrap: bool,
    /// Maximum visible rows when wrapping, with an ellipsis on the final row.
    pub max_lines: Option<usize>,
    pub monospace: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct IconStyle {
    pub size: f32,
    pub color: Option<ColorSpec>,
    pub alignment: [Align; 2],
}

impl Default for IconStyle {
    fn default() -> Self {
        Self {
            size: 16.0,
            color: None,
            alignment: [Align::Center; 2],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ControlKind {
    Button,
    Toggle { checked: bool },
    Link,
    Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ChartKind {
    Line,
    Bar,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineChartPoint {
    pub label: String,
    pub value: Option<f32>,
    pub image: Option<String>,
    pub tooltip: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RasterImage {
    pub size: [u16; 2],
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ImageFit {
    #[default]
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgStyle {
    pub fit: ImageFit,
    pub fill: Option<ColorSpec>,
    pub stroke: Option<ColorSpec>,
    pub stroke_width: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FillDirection {
    #[default]
    LeftToRight,
    RightToLeft,
    BottomToTop,
    TopToBottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RingStyle {
    pub track: ColorSpec,
    /// Radians measured clockwise from the positive x-axis.
    pub start_angle: f32,
    pub clockwise: bool,
}

impl Default for RingStyle {
    fn default() -> Self {
        Self {
            track: ColorSpec::Border,
            start_angle: -std::f32::consts::FRAC_PI_2,
            clockwise: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewNode {
    pub kind: Node,
    #[serde(default)]
    pub size: SizeSpec,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub placement: Placement,
    #[serde(default)]
    pub clip: bool,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_fit: Option<ImageFit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Compact(Box<Compact>),
    Motion {
        motion: Box<Motion>,
        child: Box<ViewNode>,
    },
    Row(Flow, Vec<ViewNode>),
    Column(Flow, Vec<ViewNode>),
    Wrap(Flow, Vec<ViewNode>),
    Grid(Grid, Vec<ViewNode>),
    Table(Table),
    /// Children are painted in declaration order, back to front.
    Stack([Align; 2], Vec<ViewNode>),
    Container(Box<ViewNode>),
    Scroll(ScrollAxes, Box<ViewNode>),
    Text {
        text: String,
        text_style: TextStyle,
    },
    Icon {
        name: String,
        icon_style: IconStyle,
    },
    /// Key of a host-registered texture.
    Image {
        asset: String,
    },
    Svg {
        /// Key of a host-registered SVG resource.
        asset: String,
        #[serde(default)]
        svg_style: SvgStyle,
    },
    RasterImage(RasterImage),
    SpriteSequence(SpriteSequence),
    SegmentedIndicator {
        segments: u8,
        fraction: f32,
        gap: f32,
        direction: FillDirection,
        inactive: ColorSpec,
    },
    Sprite {
        frames: Vec<String>,
        frame_ms: u32,
    },
    Metric {
        label: String,
        value: String,
        unit: String,
    },
    Badge(String),
    Divider {
        vertical: bool,
    },
    Spacer,
    TextInput {
        key: String,
        value: String,
        hint: String,
    },
    Slider {
        fraction: f32,
        label: String,
        action: String,
        enabled: bool,
    },
    ProgressBar {
        fraction: f32,
        label: String,
        #[serde(default)]
        marker: bool,
    },
    ProgressRing {
        fraction: f32,
        label: String,
        stroke: f32,
        text_style: TextStyle,
        #[serde(default)]
        ring_style: RingStyle,
    },
    Sparkline(Vec<f32>),
    RangeChart {
        labels: Vec<String>,
        high: Vec<f32>,
        low: Vec<f32>,
        unit: String,
    },
    LineChart {
        points: Vec<LineChartPoint>,
        unit: String,
    },
    Chart {
        kind: ChartKind,
        values: Vec<f32>,
    },
    Gauge {
        fraction: f32,
        label: String,
    },
    Control {
        kind: ControlKind,
        label: String,
        action: Action,
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
    },
    /// Interactive wrapper for a composed action, e.g. a notification body.
    ActionArea {
        action: Action,
        label: String,
        content: Box<ViewNode>,
    },
}

impl ViewNode {
    pub fn new(kind: Node) -> Self {
        Self {
            kind,
            size: SizeSpec::default(),
            style: Style::default(),
            placement: Placement::default(),
            clip: false,
            key: None,
            image_fit: None,
        }
    }
    pub fn compact(leading: Self, content: Self, layout: CompactLayout) -> Self {
        Self::new(Node::Compact(Box::new(Compact {
            leading,
            content,
            layout,
        })))
    }
    pub fn motion(self, motion: Motion) -> Self {
        Self::new(Node::Motion {
            motion: Box::new(motion),
            child: Box::new(self),
        })
    }
    pub fn svg(asset: impl Into<String>, size: f32) -> Self {
        Self::new(Node::Svg {
            asset: asset.into(),
            svg_style: SvgStyle::default(),
        })
        .sized(Size::Fixed(size), Size::Fixed(size))
    }
    pub fn row(children: Vec<Self>, flow: Flow) -> Self {
        Self::new(Node::Row(flow, children))
    }
    pub fn column(children: Vec<Self>, flow: Flow) -> Self {
        Self::new(Node::Column(flow, children))
    }
    pub fn text(text: impl Into<String>, text_style: TextStyle) -> Self {
        Self::new(Node::Text {
            text: text.into(),
            text_style,
        })
    }
    pub fn icon(name: impl Into<String>, size: f32) -> Self {
        Self::new(Node::Icon {
            name: name.into(),
            icon_style: IconStyle {
                size,
                ..Default::default()
            },
        })
    }
    pub fn button(label: impl Into<String>, action: Action) -> Self {
        Self::new(Node::Control {
            kind: ControlKind::Button,
            label: label.into(),
            action,
            enabled: true,
            icon: None,
        })
    }
    pub fn subtle_button(label: impl Into<String>, action: Action) -> Self {
        Self::button(label, action).styled(Style {
            normal: Paint {
                foreground: Some(ColorSpec::Info),
                background: Some(ColorSpec::AccentSubtle),
                ..Default::default()
            },
            hovered: Paint {
                foreground: Some(ColorSpec::OnAccent),
                background: Some(ColorSpec::Accent),
                ..Default::default()
            },
            pressed: Paint {
                foreground: Some(ColorSpec::OnAccent),
                background: Some(ColorSpec::Accent),
                opacity: Some(0.82),
                ..Default::default()
            },
            ..Default::default()
        })
    }
    pub fn ring(
        fraction: f32,
        label: impl Into<String>,
        diameter: f32,
        stroke: f32,
        font: f32,
    ) -> Self {
        Self::new(Node::ProgressRing {
            fraction,
            label: label.into(),
            stroke,
            ring_style: RingStyle::default(),
            text_style: TextStyle {
                font_size: FontSize::Custom(font),
                ..Default::default()
            },
        })
        .sized(Size::Fixed(diameter), Size::Fixed(diameter))
    }
    pub fn sized(mut self, width: Size, height: Size) -> Self {
        self.size.width = width;
        self.size.height = height;
        self
    }
    pub fn color(mut self, color: ColorSpec) -> Self {
        self.style.normal.foreground = Some(color);
        self
    }
    pub fn styled(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}
