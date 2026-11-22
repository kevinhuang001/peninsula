use super::{anchor, layout, point, Renderer};
use crate::ui::declarative::{Align, Capsule, Overflow, Size};
use eframe::egui::{vec2, Color32, Id, Pos2, Rect, Ui, UiBuilder, Vec2};

impl Renderer<'_> {
    pub(super) fn measure_capsule(
        &mut self,
        ui: &Ui,
        capsule: &Capsule,
        available: Vec2,
        depth: usize,
    ) -> Vec2 {
        let (inner, leading, gap) = regions(capsule, available, true);
        let room = vec2((inner.width() - leading.x - gap).max(0.0), inner.height());
        let content = self.capsule_content_size(ui, capsule, room, true, depth);
        let mut padding = available - inner.size();
        if available.y == 0.0 {
            padding.y = point(capsule.layout.padding[1]) + point(capsule.layout.padding[3]);
        }
        vec2(
            leading.x + gap + content.x + padding.x,
            leading.y.max(content.y) + padding.y,
        )
    }

    fn capsule_content_size(
        &mut self,
        ui: &Ui,
        capsule: &Capsule,
        available: Vec2,
        intrinsic: bool,
        depth: usize,
    ) -> Vec2 {
        if available.x <= 0.0 {
            return Vec2::ZERO;
        }
        let axes = layout::axes(capsule.layout.content_size);
        let resolve = |axis: usize, value: f32| {
            axes[axis].definite(value).or_else(|| {
                (!intrinsic && matches!(axes[axis].mode, Size::Fill(_)))
                    .then(|| axes[axis].bound(value))
            })
        };
        let width = resolve(0, available.x);
        let height = resolve(1, available.y);
        let room = vec2(
            width
                .unwrap_or_else(|| axes[0].bound(available.x))
                .min(available.x),
            height.unwrap_or_else(|| axes[1].bound(available.y)),
        );
        let measured = self.in_cell(ui, &capsule.content, room, [Align::Start; 2], depth);
        let mut size = vec2(
            width.unwrap_or_else(|| axes[0].bound(measured.x)),
            height.unwrap_or_else(|| axes[1].bound(measured.y)),
        );
        if let Some(ratio) = capsule
            .layout
            .content_size
            .aspect_ratio
            .filter(|r| r.is_finite() && *r > 0.0)
        {
            if height.is_some() && width.is_none() {
                size.x = axes[0].bound(size.y * ratio);
            } else if height.is_none() {
                size.y = axes[1].bound(size.x / ratio);
            }
        }
        size.x = size.x.min(available.x);
        if !intrinsic {
            size.y = size.y.min(available.y);
        }
        size
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_capsule(
        &mut self,
        ui: &mut Ui,
        capsule: &Capsule,
        rect: Rect,
        id: Id,
        foreground: Color32,
        opacity: f32,
        depth: usize,
    ) {
        let (inner, leading, gap) = regions(capsule, rect.size(), false);
        let inner = inner.translate(rect.min.to_vec2());
        let leading_rect = Rect::from_min_size(
            inner.min
                + vec2(
                    0.0,
                    layout::offset(capsule.layout.align, inner.height() - leading.y),
                ),
            leading,
        );
        let content_width = (inner.width() - leading.x - gap).max(0.0);
        let content_size = self.capsule_content_size(
            ui,
            capsule,
            vec2(content_width, inner.height()),
            false,
            depth,
        );
        let content_left =
            layout::offset(capsule.layout.content_align, inner.width() - content_size.x)
                .max(leading.x + gap);
        let content_rect = Rect::from_min_size(
            inner.min
                + vec2(
                    content_left,
                    layout::offset(capsule.layout.align, inner.height() - content_size.y),
                ),
            content_size,
        );

        if leading_rect.is_positive() && ui.is_rect_visible(leading_rect) {
            let mut region = ui.new_child(
                UiBuilder::new()
                    .id_salt(id.with("leading"))
                    .max_rect(leading_rect),
            );
            region.set_clip_rect(ui.clip_rect().intersect(leading_rect));
            let child = &capsule.leading;
            let align = capsule.layout.leading_align;
            let size = self
                .in_cell(&region, child, leading, align, depth)
                .min(leading);
            let rect = Rect::from_min_size(anchor(leading_rect, size, align), size);
            self.paint(
                &mut region,
                child,
                rect,
                id.with(("leading", &child.key)),
                foreground,
                opacity,
                depth,
            );
        }
        if content_rect.is_positive() && ui.is_rect_visible(content_rect) {
            let mut region = ui.new_child(
                UiBuilder::new()
                    .id_salt(id.with("content"))
                    .max_rect(content_rect),
            );
            region.set_clip_rect(ui.clip_rect().intersect(content_rect));
            let child = &capsule.content;
            let child_id = id.with(("content", &child.key));
            match capsule.layout.overflow {
                Overflow::Clip => {
                    let size = self
                        .in_cell(
                            &region,
                            child,
                            content_size,
                            [Align::Stretch, Align::Start],
                            depth,
                        )
                        .min(content_size);
                    let rect = Rect::from_min_size(content_rect.min, size);
                    self.paint(
                        &mut region,
                        child,
                        rect,
                        child_id,
                        foreground,
                        opacity,
                        depth,
                    );
                }
                Overflow::Scroll(axes) => {
                    self.scroll(
                        &mut region,
                        content_rect,
                        child,
                        axes,
                        child_id,
                        foreground,
                        opacity,
                        depth,
                    );
                }
            }
        }
    }
}

fn regions(capsule: &Capsule, size: Vec2, intrinsic: bool) -> (Rect, Vec2, f32) {
    let layout = capsule.layout;
    let inner = layout::inset(Rect::from_min_size(Pos2::ZERO, size), layout.padding);
    let leading = [0, 1].map(|axis| {
        let min = point(layout.leading_min_size[axis]);
        let max = layout.leading_max_size[axis]
            .map_or(super::MAX_EXTENT, point)
            .max(min);
        point(layout.leading_size[axis]).clamp(min, max)
    });
    let width = leading[0].min(inner.width());
    let height = if intrinsic && size.y == 0.0 {
        leading[1]
    } else {
        leading[1].min(inner.height())
    };
    let gap = point(layout.gap).min((inner.width() - width).max(0.0));
    (inner, vec2(width, height), gap)
}
