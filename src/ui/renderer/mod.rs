mod layout;
mod paint;

use crate::{
    app::icons::Icons,
    extension::{ActionInvocation, ExtensionId},
    ui::{declarative::*, palette::TextPalette},
};
use eframe::egui::{self, pos2, vec2, Color32, Id, Rect, Ui, UiBuilder, Vec2};
use std::collections::HashMap;

const MAX_DEPTH: usize = 32;
const MAX_EXTENT: f32 = 16_384.0;
type MeasureKey = (*const ViewNode, u32, u32, [Option<u32>; 2], usize);

pub(crate) fn register_image(context: &egui::Context, asset: &str, texture: egui::TextureHandle) {
    context.data_mut(|data| data.insert_temp(paint::image_id(asset), texture));
}

pub fn show_widget(
    ui: &mut Ui,
    rect: Rect,
    source: &ExtensionId,
    view: &WidgetView,
    icons: &Icons,
    opacity: f32,
) -> Option<ActionInvocation> {
    show_widget_with_overflow(ui, rect, source, view, icons, opacity, view.bounds.overflow)
}

pub fn show_widget_with_overflow(
    ui: &mut Ui,
    rect: Rect,
    source: &ExtensionId,
    view: &WidgetView,
    icons: &Icons,
    opacity: f32,
    overflow: Overflow,
) -> Option<ActionInvocation> {
    let mut renderer = Renderer::new(ui, icons);
    let id = ui.id().with(("widget", source));
    let mut surface = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect));
    surface.set_clip_rect(ui.clip_rect().intersect(rect));
    match overflow {
        Overflow::Clip => renderer.paint_root(&mut surface, rect, &view.root, opacity),
        Overflow::Scroll(axes) => renderer.scroll(
            &mut surface,
            rect,
            &view.root,
            axes,
            id,
            Color32::PLACEHOLDER,
            opacity,
            0,
        ),
    }
    renderer.action.map(|action| ActionInvocation {
        source_extension_id: source.clone(),
        action,
    })
}

pub fn content_width(ctx: &egui::Context, root: &ViewNode, limit: f32, icons: &Icons) -> f32 {
    let ui = Ui::new(
        ctx.clone(),
        Id::new("widget-width-measure"),
        UiBuilder::new().sizing_pass(),
    );
    Renderer::new(&ui, icons)
        .measure(&ui, root, vec2(point(limit), 0.0), [None; 2], 0)
        .x
        .min(limit)
}

/// Measure content without letting horizontal Fill/Stretch consume the limit.
/// The original tree retains its flexible layout when it is painted.
pub fn natural_content_width(
    ctx: &egui::Context,
    root: &ViewNode,
    limit: f32,
    icons: &Icons,
) -> f32 {
    let ui = Ui::new(
        ctx.clone(),
        Id::new("natural-width-measure"),
        UiBuilder::new().sizing_pass(),
    );
    let mut renderer = Renderer::new(&ui, icons);
    renderer.intrinsic_width = true;
    renderer
        .measure(&ui, root, vec2(point(limit), 0.0), [None; 2], 0)
        .x
        .min(limit)
}

pub fn content_height(ctx: &egui::Context, root: &ViewNode, width: f32, icons: &Icons) -> f32 {
    let ui = Ui::new(
        ctx.clone(),
        Id::new("widget-measure"),
        UiBuilder::new().sizing_pass(),
    );
    let width = point(width);
    // A flexible height has no allocation while measuring intrinsic size.
    Renderer::new(&ui, icons)
        .in_cell(
            &ui,
            root,
            vec2(width, 0.0),
            [Align::Stretch, Align::Start],
            0,
        )
        .y
}

pub fn scrolls_horizontally(view: &WidgetView) -> bool {
    matches!(
        view.bounds.overflow,
        Overflow::Scroll(ScrollAxes::Horizontal | ScrollAxes::Both)
    ) || layout::scrolls(&view.root, 0, 0, &mut 4096)
}

pub fn show_compact(
    ui: &mut Ui,
    rect: Rect,
    id: &ExtensionId,
    node: &ViewNode,
    icons: &Icons,
    opacity: f32,
) -> Option<crate::extension::Action> {
    let mut renderer = Renderer::new(ui, icons);
    let mut surface = ui.new_child(UiBuilder::new().id_salt(("compact", id)).max_rect(rect));
    surface.set_clip_rect(ui.clip_rect().intersect(rect));
    let size = renderer.in_cell(&surface, node, rect.size(), [Align::Start; 2], 0);
    let rect = Rect::from_min_size(
        rect.min
            + vec2(
                layout::offset(Align::Center, rect.width() - size.x),
                layout::offset(Align::Center, rect.height() - size.y),
            ),
        size,
    );
    renderer.paint_root(&mut surface, rect, node, opacity);
    renderer.action
}

struct Renderer<'a> {
    icons: &'a Icons,
    intrinsic_width: bool,
    measured: HashMap<MeasureKey, Vec2>,
    theme: Theme,
    action: Option<crate::extension::Action>,
    remaining: usize,
    measure_remaining: usize,
}

struct Theme {
    gap: f32,
    radius: f32,
    accent: Color32,
    dark: bool,
}

impl Theme {
    fn from_ui(ui: &Ui) -> Self {
        let visuals = ui.visuals();
        Self {
            gap: ui.spacing().item_spacing.x,
            radius: f32::from(visuals.widgets.inactive.corner_radius.nw),
            accent: visuals.selection.bg_fill,
            dark: visuals.dark_mode,
        }
    }

    fn color(&self, ui: &Ui, color: ColorSpec) -> Color32 {
        use ColorSpec::*;
        let text = TextPalette::for_mode(self.dark);
        match color {
            PrimaryText => ui.visuals().text_color(),
            SecondaryText => text.secondary,
            MutedText => text.muted,
            Surface => ui.visuals().panel_fill,
            SurfaceVariant => ui.visuals().widgets.noninteractive.bg_fill,
            Accent => self.accent,
            OnAccent => ui.visuals().selection.stroke.color,
            AccentSubtle => self
                .accent
                .gamma_multiply(if self.dark { 0.26 } else { 0.12 }),
            Border => ui.visuals().widgets.noninteractive.bg_stroke.color,
            Success => text.success,
            Warning => text.warning,
            Danger => text.danger,
            Info => ui.visuals().hyperlink_color,
            Transparent => Color32::TRANSPARENT,
            Rgba { r, g, b, a } => Color32::from_rgba_unmultiplied(r, g, b, a),
        }
    }
}

impl<'a> Renderer<'a> {
    fn new(ui: &Ui, icons: &'a Icons) -> Self {
        Self {
            icons,
            intrinsic_width: false,
            measured: HashMap::new(),
            theme: Theme::from_ui(ui),
            action: None,
            remaining: 4096,
            measure_remaining: 16_384,
        }
    }

    fn paint_root(&mut self, ui: &mut Ui, rect: Rect, node: &ViewNode, opacity: f32) {
        let size = self.in_cell(ui, node, rect.size(), [Align::Stretch; 2], 0);
        self.paint(
            ui,
            node,
            Rect::from_min_size(rect.min, size),
            ui.id().with(node.key.as_deref()),
            Color32::PLACEHOLDER,
            opacity,
            0,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn scroll(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        child: &ViewNode,
        axes: ScrollAxes,
        id: Id,
        foreground: Color32,
        opacity: f32,
        depth: usize,
    ) {
        let enabled = axes.axes();
        let mut scroller = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect));
        scroller.set_clip_rect(ui.clip_rect().intersect(rect));
        egui::ScrollArea::new(enabled)
            .id_salt(id)
            .max_width(rect.width())
            .max_height(rect.height())
            .min_scrolled_width(0.0)
            .min_scrolled_height(0.0)
            .auto_shrink([false; 2])
            .show(&mut scroller, |ui| {
                // Keep percentages relative to the viewport. Fixed dimensions
                // and intrinsic content can exceed it along the scroll axes.
                let viewport = rect.size();
                let available = viewport;
                let size = self.in_cell(
                    ui,
                    child,
                    available,
                    [
                        if enabled[0] {
                            Align::Start
                        } else {
                            Align::Stretch
                        },
                        if enabled[1] {
                            Align::Start
                        } else {
                            Align::Stretch
                        },
                    ],
                    depth,
                );
                let size = vec2(
                    if enabled[0] {
                        size.x
                    } else {
                        size.x.min(viewport.x)
                    },
                    if enabled[1] {
                        size.y
                    } else {
                        size.y.min(viewport.y)
                    },
                );
                let content = Rect::from_min_size(ui.cursor().min, size);
                let extent = self.overflow_extent(ui, child, size, depth, &mut 4096);
                let scroll_size = vec2(
                    if enabled[0] {
                        extent.x.min(MAX_EXTENT)
                    } else {
                        size.x
                    },
                    if enabled[1] {
                        extent.y.min(MAX_EXTENT)
                    } else {
                        size.y
                    },
                );
                ui.allocate_rect(
                    Rect::from_min_size(content.min, scroll_size),
                    egui::Sense::hover(),
                );
                self.paint(
                    ui,
                    child,
                    content,
                    id.with("content"),
                    foreground,
                    opacity,
                    depth,
                );
            });
    }
}

fn point(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, MAX_EXTENT)
    } else {
        0.0
    }
}
fn anchor(rect: Rect, size: Vec2, align: [Align; 2]) -> egui::Pos2 {
    pos2(
        rect.left() + layout::offset(align[0], rect.width() - size.x),
        rect.top() + layout::offset(align[1], rect.height() - size.y),
    )
}

#[cfg(test)]
mod tests;
