use super::{
    anchor,
    animation::Effects,
    assets::{self, image},
    layout, point, Renderer, MAX_DEPTH,
};
use crate::{app::icons::Icon, ui::declarative::*};
use eframe::egui::{
    self, pos2, vec2, Color32, FontFamily, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2,
};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

fn font_size(style: TextStyle) -> f32 {
    match style.font_size {
        FontSize::Small => 10.0,
        FontSize::Body => 12.0,
        FontSize::Title => 14.0,
        FontSize::Heading => 18.0,
        FontSize::Custom(size) => point(size).clamp(1.0, 256.0),
    }
}

fn galley(ui: &Ui, text: &str, style: TextStyle, width: f32, elide: bool) -> Arc<egui::Galley> {
    use egui::text::{LayoutJob, TextFormat};
    let bold = style.font_weight == FontWeight::Bold;
    let mut job = LayoutJob::single_section(
        text.into(),
        TextFormat {
            font_id: FontId::new(
                font_size(style),
                if style.monospace {
                    FontFamily::Monospace
                } else {
                    FontFamily::Proportional
                },
            ),
            color: Color32::PLACEHOLDER,
            extra_letter_spacing: if bold { 0.25 } else { 0.0 },
            line_height: style.line_height.map(point).filter(|v| *v > 0.0),
            ..Default::default()
        },
    );
    if style.wrap || elide {
        job.wrap.max_width = width.max(1.0);
    }
    if let Some(max_lines) = style.max_lines.filter(|_| style.wrap) {
        job.wrap.max_rows = max_lines.max(1);
    }
    if elide && !style.wrap {
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
    }
    job.halign = match style.align {
        Align::Start | Align::Stretch => egui::Align::LEFT,
        Align::Center => egui::Align::Center,
        Align::End => egui::Align::RIGHT,
    };
    ui.fonts(|fonts| fonts.layout_job(job))
}

fn table_header_style() -> TextStyle {
    TextStyle {
        font_size: FontSize::Small,
        font_weight: FontWeight::Bold,
        align: Align::Start,
        ..TextStyle::default()
    }
}

pub(super) fn table_header_width(ui: &Ui, title: &str) -> f32 {
    galley(ui, title, table_header_style(), f32::INFINITY, false)
        .size()
        .x
}

fn text(ui: &Ui, rect: Rect, value: &str, style: TextStyle, color: Color32) {
    if value.is_empty() || !rect.is_positive() {
        return;
    }
    let galley = galley(ui, value, style, rect.width(), true);
    let size = galley.size();
    let at = anchor(rect, size, [style.align, Align::Center])
        - vec2(galley.rect.left(), galley.rect.top());
    let painter = ui.painter().with_clip_rect(rect);
    if style.font_weight == FontWeight::Bold {
        painter.galley(at + vec2(0.4, 0.0), galley.clone(), color);
    }
    painter.galley(at, galley, color);
}

fn named_icon(name: &str) -> Option<Icon> {
    Some(match name {
        "battery" => Icon::Battery,
        "battery-charging" => Icon::BatteryCharging,
        "cpu" => Icon::Cpu,
        "chart" => Icon::Chart,
        "gpu" => Icon::Gpu,
        "clock" => Icon::Clock,
        "moon" => Icon::Moon,
        "music" => Icon::Music,
        "music-note" => Icon::MusicNote,
        "speaker" => Icon::Speaker,
        "speaker-playing" => Icon::SpeakerPlaying,
        "equalizer" => Icon::Equalizer,
        "equalizer-playing" => Icon::EqualizerPlaying,
        "media-play" => Icon::MediaPlay,
        "media-pause" => Icon::MediaPause,
        "media-previous" => Icon::MediaPrevious,
        "media-next" => Icon::MediaNext,
        "arrow-down" | "download" => Icon::ArrowDown,
        "arrow-up" => Icon::ArrowUp,
        "close" => Icon::Close,
        "timer" => Icon::Clock,
        "bell" => Icon::Bell,
        _ => return None,
    })
}

impl Renderer<'_> {
    pub(super) fn leaf_size(&self, ui: &Ui, node: &ViewNode, available: Vec2) -> Vec2 {
        let measure_text = |value, style| galley(ui, value, style, available.x, false).size();
        match &node.kind {
            Node::Text { text, text_style } => {
                measure_text(text.as_str(), *text_style).min(vec2(available.x, f32::INFINITY))
            }
            Node::Icon { icon_style, .. } => Vec2::splat(point(icon_style.size)),
            Node::Metric { label, value, unit } => {
                let caption = measure_text(
                    label,
                    TextStyle {
                        font_size: FontSize::Small,
                        ..Default::default()
                    },
                );
                let data = measure_text(
                    &format!("{value}{unit}"),
                    TextStyle {
                        font_size: FontSize::Title,
                        ..Default::default()
                    },
                );
                vec2(caption.x.max(data.x), caption.y + data.y)
            }
            Node::Badge(label) => measure_text(label, TextStyle::default()) + vec2(12.0, 4.0),
            Node::Divider { vertical } => {
                if *vertical {
                    vec2(1.0, 16.0)
                } else {
                    vec2(32.0, 1.0)
                }
            }
            Node::ProgressBar { .. } => vec2(100.0, 16.0),
            Node::Slider { .. } => vec2(100.0, 24.0),
            Node::TextInput { .. } | Node::PasswordInput { .. } => vec2(180.0, 30.0),
            Node::ProgressRing { .. } | Node::Gauge { .. } => Vec2::splat(60.0),
            Node::Chart { .. } | Node::Sparkline(_) | Node::RangeChart { .. } => vec2(100.0, 36.0),
            Node::LineChart { .. } => vec2(120.0, 104.0),
            Node::Image { asset } => image_size(ui, asset),
            Node::Svg { asset, svg_style } => assets::svg(ui, &self.theme, asset, *svg_style, None)
                .map_or(Vec2::ZERO, |texture| texture.size_vec2()),
            Node::SpriteSequence(spec) => spec.frames.first().map_or(Vec2::ZERO, |frame| {
                image_size(ui, &frame.asset) * assets::uv_rect(frame.uv).size()
            }),
            Node::SegmentedIndicator { .. } => vec2(24.0, 10.0),
            Node::RasterImage(image) => vec2(f32::from(image.size[0]), f32::from(image.size[1])),
            Node::Sprite { frames, .. } => frames
                .first()
                .map_or(Vec2::ZERO, |asset| image_size(ui, asset)),
            Node::Control { label, kind, .. } => {
                measure_text(label, TextStyle::default())
                    + vec2(
                        if matches!(kind, ControlKind::Toggle { .. }) {
                            30.0
                        } else {
                            14.0
                        },
                        8.0,
                    )
            }
            _ => Vec2::ZERO,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint(
        &mut self,
        ui: &mut Ui,
        node: &ViewNode,
        rect: Rect,
        id: Id,
        inherited: Color32,
        opacity: f32,
        depth: usize,
    ) {
        self.paint_with_effects(
            ui,
            node,
            rect,
            id,
            inherited,
            opacity,
            depth,
            Effects::default(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_with_effects(
        &mut self,
        ui: &mut Ui,
        node: &ViewNode,
        rect: Rect,
        id: Id,
        inherited: Color32,
        opacity: f32,
        depth: usize,
        effects: Effects,
    ) {
        let visible = rect.is_positive() && ui.is_rect_visible(rect);
        if depth >= MAX_DEPTH
            || self.remaining == 0
            || (!visible && (node.clip || layout::children(node).is_none()))
        {
            return;
        }
        self.remaining -= 1;
        let (interactive, enabled) = match node.kind {
            Node::Control { enabled, .. } | Node::Slider { enabled, .. } => {
                (true, enabled && ui.is_enabled())
            }
            Node::ActionArea { .. } => (true, ui.is_enabled()),
            _ => (false, true),
        };
        let response = if interactive && visible {
            // Use the same hit rectangle and focus behavior as egui widgets.
            let target = rect.intersect(ui.clip_rect());
            if enabled {
                Some(ui.interact(
                    target,
                    id,
                    if matches!(node.kind, Node::Slider { .. }) {
                        Sense::click_and_drag()
                    } else {
                        Sense::click()
                    },
                ))
            } else {
                let disabled =
                    ui.new_child(UiBuilder::new().id_salt(id).max_rect(target).disabled());
                Some(disabled.interact(target, id, Sense::hover()))
            }
        } else {
            None
        };
        let active = if !enabled {
            node.style.disabled
        } else if response
            .as_ref()
            .is_some_and(|r| r.is_pointer_button_down_on())
        {
            node.style.pressed
        } else if response
            .as_ref()
            .is_some_and(|r| r.hovered() || r.has_focus())
        {
            node.style.hovered
        } else {
            Paint::default()
        };
        let normal = node.style.normal;
        let foreground = effects.color.unwrap_or_else(|| {
            active.foreground.or(normal.foreground).map_or_else(
                || {
                    let default = match node.kind {
                        Node::Control {
                            kind: ControlKind::Button | ControlKind::Action,
                            ..
                        } => ColorSpec::OnAccent,
                        Node::Control {
                            kind: ControlKind::Link,
                            ..
                        } => ColorSpec::Info,
                        _ if inherited != Color32::PLACEHOLDER => return inherited,
                        _ => ColorSpec::PrimaryText,
                    };
                    self.theme.color(ui, default)
                },
                |color| self.theme.color(ui, color),
            )
        });
        let opacity = opacity
            * active
                .opacity
                .or(normal.opacity)
                .filter(|v| v.is_finite())
                .unwrap_or(if enabled { 1.0 } else { 0.55 })
                .clamp(0.0, 1.0);
        let color = |color| self.theme.color(ui, color).linear_multiply(opacity);
        let background = active
            .background
            .or(normal.background)
            .map(color)
            .unwrap_or_else(|| match node.kind {
                Node::Control {
                    kind: ControlKind::Link,
                    ..
                }
                | Node::ActionArea { .. } => Color32::TRANSPARENT,
                Node::Control { .. } => response.as_ref().map_or(Color32::TRANSPARENT, |r| {
                    ui.style().interact(r).bg_fill.linear_multiply(opacity)
                }),
                Node::Badge(_) => color(ColorSpec::SurfaceVariant),
                _ => Color32::TRANSPARENT,
            });
        let radius = point(active.corner_radius.or(normal.corner_radius).unwrap_or(
            if interactive || matches!(node.kind, Node::Badge(_)) {
                self.theme.radius
            } else {
                0.0
            },
        ));
        if let Some(shadow) = active.shadow.or(normal.shadow) {
            ui.painter().add(
                egui::Shadow {
                    offset: shadow.offset,
                    blur: shadow.blur,
                    spread: shadow.spread,
                    color: color(shadow.color),
                }
                .as_shape(rect, radius),
            );
        }
        if background != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, radius, background);
        }
        if let Some(width) = active
            .border_width
            .or(normal.border_width)
            .filter(|w| *w > 0.0)
        {
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(
                    point(width),
                    color(
                        active
                            .border_color
                            .or(normal.border_color)
                            .unwrap_or(ColorSpec::Border),
                    ),
                ),
                StrokeKind::Inside,
            );
        } else if response.as_ref().is_some_and(|r| r.has_focus()) {
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(1.0_f32, color(ColorSpec::Accent)),
                StrokeKind::Inside,
            );
        }
        if response.as_ref().is_some_and(|r| r.clicked()) {
            if let Node::Control { action, .. } | Node::ActionArea { action, .. } = &node.kind {
                self.action = Some(action.clone());
            }
        }
        if let (
            Node::ActionArea {
                context_menu: Some(menu),
                ..
            },
            Some(response),
        ) = (&node.kind, response.as_ref())
        {
            super::context_menu(response, |ui| {
                ui.label(&menu.title);
                let clicked = ui.button(&menu.label).clicked();
                if clicked {
                    self.action = Some(menu.action.clone());
                }
                clicked
            });
        }
        if let Node::Control { label, .. } | Node::ActionArea { label, .. } = &node.kind {
            if let Some(r) = &response {
                let info = match &node.kind {
                    Node::Control {
                        kind: ControlKind::Toggle { checked },
                        ..
                    } => egui::WidgetInfo::selected(
                        egui::WidgetType::Checkbox,
                        enabled,
                        *checked,
                        label,
                    ),
                    Node::Control {
                        kind: ControlKind::Link,
                        ..
                    } => egui::WidgetInfo::labeled(egui::WidgetType::Link, enabled, label),
                    _ => egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label),
                };
                r.widget_info(|| info.clone());
                if matches!(node.kind, Node::Control { icon: Some(_), .. }) {
                    r.clone().on_hover_text(label);
                }
            }
        }
        let inner = layout::inset(rect, node.style.padding);
        let mut scope;
        let ui = if node.clip {
            scope = ui.new_child(UiBuilder::new().id_salt(id).max_rect(inner));
            scope.set_clip_rect(ui.clip_rect().intersect(inner));
            &mut scope
        } else {
            ui
        };

        if let Node::Compact(compact) = &node.kind {
            self.paint_compact(ui, compact, inner, id, foreground, opacity, depth + 1);
            return;
        }
        if let Node::Motion { motion, child } = &node.kind {
            if visible {
                let child_id = id.with(("motion-child", &child.key));
                let mut current = self.effects(ui, id, motion, supports_progress(child));
                current.color = effects.color.or(current.color);
                current.progress = effects.progress.or(current.progress);
                let size = self.in_cell(ui, child, inner.size(), [Align::Stretch; 2], depth + 1);
                let rect = Rect::from_min_size(inner.min, size);
                let start = current.has_transform().then(|| {
                    ui.ctx()
                        .graphics_mut(|graphics| graphics.entry(ui.layer_id()).next_idx())
                });
                self.paint_with_effects(
                    ui,
                    child,
                    rect,
                    child_id,
                    foreground,
                    opacity * current.opacity,
                    depth + 1,
                    current,
                );
                if let Some(start) = start {
                    current.transform_shapes(ui, start, rect.center());
                }
            }
            return;
        }
        if let Node::Scroll(axes, child) = &node.kind {
            self.scroll(ui, inner, child, *axes, id, foreground, opacity, depth + 1);
            return;
        }
        if let Node::Table(table) = &node.kind {
            self.paint_table(ui, inner, table, id, foreground, opacity, depth + 1);
            return;
        }
        if let Some(layout) = self.layout(ui, node, inner.size(), depth + 1) {
            let Some(children) = layout::children(node) else {
                return;
            };
            for (index, cell) in layout.cells {
                let child = &children[index];
                let child_id = child
                    .key
                    .as_ref()
                    .map_or_else(|| id.with(index), |key| id.with(key));
                self.paint(
                    ui,
                    child,
                    cell.translate(inner.min.to_vec2()),
                    child_id,
                    foreground,
                    opacity,
                    depth + 1,
                );
            }
            return;
        }
        self.paint_leaf(ui, inner, node, id, foreground, opacity, effects);
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_table(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        table: &Table,
        id: Id,
        foreground: Color32,
        opacity: f32,
        depth: usize,
    ) {
        use layout::{TABLE_GAP, TABLE_HEADER_HEIGHT, TABLE_X_PADDING, TABLE_Y_PADDING};
        let layout = self.table(ui, table, rect.width(), depth);
        let painter = ui.painter().with_clip_rect(rect);
        let color = |spec| self.theme.color(ui, spec).linear_multiply(opacity);
        let surface = color(ColorSpec::SurfaceVariant);
        let divider = Stroke::new(1.0_f32, color(ColorSpec::Border).gamma_multiply(0.58));
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), TABLE_HEADER_HEIGHT));
        painter.rect_filled(
            header,
            egui::CornerRadius {
                nw: 7,
                ne: 7,
                ..Default::default()
            },
            surface,
        );
        painter.hline(header.x_range(), header.bottom(), divider);
        let mut x = rect.left() + TABLE_X_PADDING;
        for (column, &width) in table.columns.iter().zip(&layout.columns) {
            text(
                ui,
                Rect::from_min_size(pos2(x, rect.top()), vec2(width, TABLE_HEADER_HEIGHT)),
                &column.title,
                table_header_style(),
                color(ColorSpec::SecondaryText),
            );
            x += width + TABLE_GAP;
        }
        let mut y = header.bottom();
        for (index, (row, height)) in table.rows.iter().zip(layout.rows).enumerate() {
            let row_rect = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), height));
            y += height;
            if row_rect.top() >= rect.bottom() {
                break;
            }
            if !ui.clip_rect().intersects(row_rect) {
                continue;
            }
            if index % 2 == 0 {
                painter.rect_filled(row_rect, 0.0, surface.gamma_multiply(0.36));
            }
            if let Some(Some(menu)) = table.row_menus.get(index) {
                let response = ui.interact(
                    row_rect.intersect(rect),
                    id.with(("row-menu", &menu.action)),
                    Sense::click(),
                );
                super::context_menu(&response, |ui| {
                    ui.label(&menu.title);
                    let clicked = ui.button(&menu.label).clicked();
                    if clicked {
                        self.action = Some(menu.action.clone());
                    }
                    clicked
                });
            }
            painter.hline(row_rect.x_range(), row_rect.bottom(), divider);
            let mut x = rect.left() + TABLE_X_PADDING;
            for (column, (cell, &width)) in row.iter().zip(&layout.columns).enumerate() {
                let bounds = Rect::from_min_size(
                    pos2(x, row_rect.top() + TABLE_Y_PADDING),
                    vec2(width, height - 2.0 * TABLE_Y_PADDING),
                );
                let child_id = id.with((index, column, cell.key.as_deref()));
                let mut child = ui.new_child(UiBuilder::new().id_salt(child_id).max_rect(bounds));
                child.set_clip_rect(ui.clip_rect().intersect(rect).intersect(bounds));
                let size = self.in_cell(
                    &child,
                    cell,
                    vec2(width, 0.0),
                    [Align::Stretch, Align::Start],
                    depth,
                );
                self.paint(
                    &mut child,
                    cell,
                    Rect::from_min_size(anchor(bounds, size, [Align::Start, Align::Center]), size),
                    child_id,
                    foreground,
                    opacity,
                    depth,
                );
                x += width + TABLE_GAP;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_leaf(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        node: &ViewNode,
        id: Id,
        foreground: Color32,
        opacity: f32,
        effects: Effects,
    ) {
        let painter = ui.painter().with_clip_rect(rect);
        let fg = foreground.linear_multiply(opacity);
        let color = |color| self.theme.color(ui, color).linear_multiply(opacity);
        match &node.kind {
            Node::TextInput { key, value, hint } => {
                let mut value = value.clone();
                let mut child = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect));
                let response = child.add_sized(
                    rect.size(),
                    egui::TextEdit::singleline(&mut value)
                        .id(id.with(key))
                        .hint_text(hint)
                        .char_limit(2048),
                );
                if response.changed() {
                    self.action = Some(crate::extension::Action::SetText {
                        key: key.clone(),
                        value,
                    });
                }
            }
            Node::PasswordInput { key, value, hint } => {
                let mut value = value.clone();
                let mut child = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect));
                let response = child.add_sized(
                    rect.size(),
                    egui::TextEdit::singleline(&mut value)
                        .id(id.with(key))
                        .hint_text(hint)
                        .char_limit(2048)
                        .password(true),
                );
                if response.changed() {
                    self.action = Some(crate::extension::Action::SetText {
                        key: key.clone(),
                        value,
                    });
                }
            }
            Node::Text {
                text: value,
                text_style,
            } => text(ui, rect, value, *text_style, fg),
            Node::Icon { name, icon_style } => {
                let size = Vec2::splat(point(icon_style.size)).min(rect.size());
                let icon_rect = Rect::from_min_size(anchor(rect, size, icon_style.alignment), size);
                let tint = if effects.color.is_some() {
                    fg
                } else {
                    icon_style.color.map_or(fg, color)
                };
                if let Some(icon) = named_icon(name) {
                    self.icons.paint(&painter, icon, icon_rect, tint, 1.0);
                } else if let Some(texture) = image(ui, name) {
                    painter.image(
                        texture.id(),
                        icon_rect,
                        Rect::from_min_max(egui::Pos2::ZERO, pos2(1.0, 1.0)),
                        tint,
                    );
                }
            }
            Node::Image { asset } => {
                if let Some(texture) = image(ui, asset) {
                    assets::paint_texture(
                        &painter,
                        &texture,
                        rect,
                        assets::uv_rect([0.0, 0.0, 1.0, 1.0]),
                        node.image_fit.unwrap_or_default(),
                        point(node.style.normal.corner_radius.unwrap_or(0.0)),
                        effects
                            .color
                            .map_or(Color32::WHITE, |_| foreground)
                            .linear_multiply(opacity),
                    );
                }
            }
            Node::Svg { asset, svg_style } => {
                if let Some(texture) =
                    assets::svg(ui, &self.theme, asset, *svg_style, Some(rect.size()))
                {
                    assets::paint_texture(
                        &painter,
                        &texture,
                        rect,
                        assets::uv_rect([0.0, 0.0, 1.0, 1.0]),
                        svg_style.fit,
                        point(node.style.normal.corner_radius.unwrap_or(0.0)),
                        effects
                            .color
                            .map_or_else(|| assets::svg_tint(*svg_style, fg, opacity), |_| fg),
                    );
                }
            }
            Node::RasterImage(image) => paint_raster(
                ui,
                &painter,
                id,
                image,
                rect,
                effects
                    .color
                    .map_or(Color32::WHITE, |_| foreground)
                    .linear_multiply(opacity),
                point(node.style.normal.corner_radius.unwrap_or(0.0)),
                node.image_fit.unwrap_or(ImageFit::Stretch),
            ),
            Node::Sprite { frames, frame_ms } => {
                let index = self.uniform_sprite_index(ui, id, frames.len(), *frame_ms);
                if let Some(asset) = frames.get(index) {
                    paint_image(ui, &painter, asset, rect, opacity);
                }
            }
            Node::SpriteSequence(spec) => {
                let index = self.sprite_index(ui, id, spec);
                if let Some(frame) = spec.frames.get(index) {
                    if let Some(texture) = image(ui, &frame.asset) {
                        assets::paint_texture(
                            &painter,
                            &texture,
                            rect,
                            assets::uv_rect(frame.uv),
                            node.image_fit.unwrap_or_default(),
                            point(node.style.normal.corner_radius.unwrap_or(0.0)),
                            effects
                                .color
                                .map_or(Color32::WHITE, |_| foreground)
                                .linear_multiply(opacity),
                        );
                    }
                }
            }
            Node::SegmentedIndicator {
                segments,
                fraction,
                gap,
                direction,
                inactive,
            } => {
                segments_paint(
                    &painter,
                    rect,
                    *segments,
                    ratio(effects.progress.unwrap_or(*fraction)),
                    *gap,
                    *direction,
                    fg,
                    color(*inactive),
                );
            }
            Node::Metric { label, value, unit } => {
                let mid = rect.top() + rect.height() * 0.42;
                text(
                    ui,
                    Rect::from_min_max(rect.min, pos2(rect.right(), mid)),
                    label,
                    TextStyle {
                        font_size: FontSize::Small,
                        ..Default::default()
                    },
                    color(ColorSpec::SecondaryText),
                );
                let value = format!("{value}{unit}");
                text(
                    ui,
                    Rect::from_min_max(pos2(rect.left(), mid), rect.max),
                    &value,
                    TextStyle {
                        font_size: FontSize::Title,
                        ..Default::default()
                    },
                    fg,
                );
            }
            Node::Badge(label) => text(
                ui,
                rect.shrink2(vec2(5.0, 1.0)),
                label,
                TextStyle::default(),
                fg,
            ),
            Node::Divider { vertical } => {
                let c = rect.center();
                let line = if *vertical {
                    [pos2(c.x, rect.top()), pos2(c.x, rect.bottom())]
                } else {
                    [pos2(rect.left(), c.y), pos2(rect.right(), c.y)]
                };
                painter.line_segment(line, Stroke::new(1.0_f32, fg));
            }
            Node::Slider {
                fraction,
                label,
                action,
                enabled,
            } => {
                let enabled = *enabled && ui.is_enabled();
                let response = ui.ctx().read_response(id);
                let mut value = ratio(*fraction);
                if let Some(response) = &response {
                    if enabled && (response.drag_started() || response.clicked()) {
                        response.request_focus();
                    }
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Slider, enabled, label)
                    });
                    let preview_id = id.with("slider-preview");
                    if enabled
                        && (response.dragged() || response.drag_stopped() || response.clicked())
                    {
                        if let Some(position) = response.interact_pointer_pos() {
                            value = ((position.x - rect.left()) / rect.width().max(1.0))
                                .clamp(0.0, 1.0);
                            ui.data_mut(|data| data.insert_temp(preview_id, value));
                        } else if let Some(preview) =
                            ui.data(|data| data.get_temp::<f32>(preview_id))
                        {
                            value = preview;
                        }
                        if response.clicked() || response.drag_stopped() {
                            self.action = Some(crate::extension::Action::SetProgress {
                                key: action.clone(),
                                basis_points: (value * 10_000.0).round() as u16,
                            });
                        }
                    }
                    if enabled && response.has_focus() {
                        let step = ui.input_mut(|input| {
                            i32::from(
                                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
                            ) - i32::from(
                                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                            )
                        });
                        if step != 0 {
                            value = (value + step as f32 * 0.01).clamp(0.0, 1.0);
                            self.action = Some(crate::extension::Action::SetProgress {
                                key: action.clone(),
                                basis_points: (value * 10_000.0).round() as u16,
                            });
                        }
                    }
                }
                let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 3.0));
                let tint = if enabled {
                    fg
                } else {
                    color(ColorSpec::MutedText)
                };
                painter.rect_filled(track, 1.5, color(ColorSpec::SurfaceVariant));
                let x = egui::lerp(track.x_range(), value);
                painter.rect_filled(
                    Rect::from_min_max(track.min, pos2(x, track.bottom())),
                    1.5,
                    tint,
                );
                painter.circle_filled(pos2(x, track.center().y), 3.0, tint);
            }
            Node::ProgressBar {
                fraction,
                label,
                marker,
            } => {
                let fraction = ratio(effects.progress.unwrap_or(*fraction));
                let track = color(
                    node.style
                        .normal
                        .background
                        .unwrap_or(ColorSpec::SurfaceVariant),
                );
                if *marker {
                    let center = rect.center().y;
                    let radius = (rect.height() * 0.5).min(2.5);
                    let x = egui::lerp(rect.left() + radius..=rect.right() - radius, fraction);
                    let line = |right| {
                        Rect::from_min_max(
                            pos2(rect.left(), center - radius * 0.7),
                            pos2(right, center + radius * 0.7),
                        )
                    };
                    painter.rect_filled(line(rect.right()), radius, track);
                    painter.rect_filled(line(x), radius, fg.gamma_multiply(0.77));
                    painter.circle_filled(pos2(x, center), radius, fg);
                    return;
                }
                let label_color = color(ColorSpec::PrimaryText);
                let mut scope = ui.new_child(
                    UiBuilder::new()
                        .id_salt(id)
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                scope.set_clip_rect(ui.clip_rect().intersect(rect));
                scope.visuals_mut().extreme_bg_color = track;
                scope.visuals_mut().override_text_color = Some(label_color);
                let bar = egui::ProgressBar::new(fraction)
                    .desired_width(rect.width())
                    .desired_height(rect.height())
                    .corner_radius(point(node.style.normal.corner_radius.unwrap_or(4.0)))
                    .fill(if fraction > 0.0 {
                        fg
                    } else {
                        Color32::TRANSPARENT
                    });
                scope.add(if label.is_empty() {
                    bar
                } else {
                    bar.text(label.as_str())
                });
            }
            Node::ProgressRing {
                fraction,
                label,
                stroke,
                text_style,
                ring_style,
            } => {
                let radius = (rect.width().min(rect.height()) - point(*stroke)) * 0.5
                    - 0.5 / ui.ctx().pixels_per_point();
                if radius > 0.0 {
                    arc(
                        &painter,
                        rect.center(),
                        radius,
                        0.0,
                        std::f32::consts::TAU,
                        Stroke::new(point(*stroke), color(ring_style.track)),
                    );
                    arc(
                        &painter,
                        rect.center(),
                        radius,
                        if ring_style.start_angle.is_finite() {
                            ring_style.start_angle
                        } else {
                            RingStyle::default().start_angle
                        },
                        std::f32::consts::TAU
                            * ratio(effects.progress.unwrap_or(*fraction))
                            * if ring_style.clockwise { 1.0 } else { -1.0 },
                        Stroke::new(point(*stroke), fg),
                    );
                }
                text(ui, rect, label, *text_style, color(ColorSpec::PrimaryText));
            }
            Node::Gauge { fraction, label } => {
                let radius = (rect.width() * 0.5).min(rect.height()) - 3.0;
                if radius > 0.0 {
                    let center = pos2(rect.center().x, rect.bottom() - 2.0);
                    let pi = std::f32::consts::PI;
                    arc(
                        &painter,
                        center,
                        radius,
                        pi,
                        pi,
                        Stroke::new(3.0_f32, color(ColorSpec::Border)),
                    );
                    arc(
                        &painter,
                        center,
                        radius,
                        pi,
                        pi * ratio(effects.progress.unwrap_or(*fraction)),
                        Stroke::new(3.0_f32, fg),
                    );
                }
                text(ui, rect, label, TextStyle::default(), fg);
            }
            Node::Sparkline(values) => chart(&painter, rect, ChartKind::Line, values, fg),
            Node::Chart { kind, values } => chart(&painter, rect, *kind, values, fg),
            Node::LineChart { points, unit } => line_chart(
                ui,
                rect,
                id,
                points,
                unit,
                fg,
                color(ColorSpec::MutedText),
                opacity,
            ),
            Node::RangeChart {
                labels,
                high,
                low,
                unit,
            } => range_chart(
                &painter,
                rect,
                labels,
                high,
                low,
                unit,
                self.theme.dark,
                opacity,
                color(ColorSpec::MutedText),
            ),
            Node::Control {
                kind, label, icon, ..
            } => {
                if let ControlKind::Toggle { checked } = kind {
                    let track = Rect::from_center_size(
                        pos2(rect.left() + 14.0, rect.center().y),
                        vec2(22.0, 12.0),
                    );
                    painter.rect_filled(
                        track,
                        6.0,
                        color(if *checked {
                            ColorSpec::Accent
                        } else {
                            ColorSpec::Border
                        }),
                    );
                    painter.circle_filled(
                        pos2(
                            if *checked {
                                track.right() - 6.0
                            } else {
                                track.left() + 6.0
                            },
                            track.center().y,
                        ),
                        4.0,
                        fg,
                    );
                    text(
                        ui,
                        Rect::from_min_max(rect.min + vec2(28.0, 0.0), rect.max),
                        label,
                        TextStyle {
                            align: Align::Start,
                            ..Default::default()
                        },
                        fg,
                    );
                } else if let Some(icon) = icon.as_deref().and_then(named_icon) {
                    let extent = rect.width().min(rect.height()) * 0.76;
                    let target = Rect::from_center_size(rect.center(), Vec2::splat(extent));
                    self.icons.paint(&painter, icon, target, fg, 1.0);
                } else {
                    text(
                        ui,
                        rect.shrink2(vec2(4.0, 0.0)),
                        label,
                        TextStyle::default(),
                        fg,
                    );
                }
            }
            _ => {}
        }
    }
}

fn supports_progress(mut node: &ViewNode) -> bool {
    for _ in 0..MAX_DEPTH {
        match &node.kind {
            Node::Motion { child, .. } => node = child,
            Node::ProgressRing { .. }
            | Node::ProgressBar { .. }
            | Node::Gauge { .. }
            | Node::SegmentedIndicator { .. } => return true,
            _ => return false,
        }
    }
    false
}

fn ratio(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start: f32,
    sweep: f32,
    stroke: Stroke,
) {
    if sweep.abs() <= 0.0 || radius <= 0.0 {
        return;
    }
    // Keep adjacent vertices within two physical pixels, including on Retina.
    let segments = (sweep.abs() * radius * painter.ctx().pixels_per_point() / 2.0)
        .ceil()
        .clamp(2.0, 4096.0) as usize;
    let closed = sweep.abs() >= std::f32::consts::TAU - 0.0001;
    let points: Vec<_> = (0..=segments)
        .take(if closed { segments } else { segments + 1 })
        .map(|i| {
            let angle = start + sweep * i as f32 / segments as f32;
            center + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect();
    if closed {
        painter.add(egui::Shape::closed_line(points, stroke));
    } else {
        let first = points[0];
        let last = *points.last().unwrap();
        painter.add(egui::Shape::line(points, stroke));
        painter.circle_filled(first, stroke.width * 0.5, stroke.color);
        painter.circle_filled(last, stroke.width * 0.5, stroke.color);
    }
}

#[allow(clippy::too_many_arguments)]
fn range_chart(
    painter: &egui::Painter,
    rect: Rect,
    labels: &[String],
    high: &[f32],
    low: &[f32],
    unit: &str,
    dark: bool,
    opacity: f32,
    muted: Color32,
) {
    let count = labels.len().min(high.len()).min(low.len()).min(31);
    let pairs = high.iter().copied().zip(low.iter().copied()).take(count);
    let (minimum, maximum) = pairs
        .clone()
        .filter(|(high, low)| high.is_finite() && low.is_finite())
        .fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min, max), (high, low)| (min.min(low), max.max(high)),
        );
    if !minimum.is_finite() || !maximum.is_finite() || rect.width() < 74.0 || rect.height() < 56.0 {
        return;
    }
    let warm = if dark {
        Color32::from_rgb(255, 168, 114)
    } else {
        Color32::from_rgb(157, 65, 17)
    };
    let cool = if dark {
        Color32::from_rgb(105, 180, 255)
    } else {
        Color32::from_rgb(15, 98, 169)
    };
    let warm = warm.linear_multiply(opacity);
    let cool = cool.linear_multiply(opacity);
    let left = rect.left() + 13.0;
    let width = rect.width() - 26.0;
    let top = rect.top() + 15.0;
    let bottom = rect.bottom() - 27.0;
    let span = (maximum - minimum).max(1.0);
    let y = |v: f32| bottom - (v - minimum + 0.12 * span) / (1.24 * span) * (bottom - top);
    let x = |index: usize| {
        if count == 1 {
            rect.center().x
        } else {
            left + width * index as f32 / (count - 1) as f32
        }
    };
    for ratio in [0.0, 0.5, 1.0] {
        painter.hline(
            left..=left + width,
            egui::lerp(top..=bottom, ratio),
            Stroke::new(1.0_f32, muted.gamma_multiply(0.22)),
        );
    }
    let mut previous: Option<(egui::Pos2, egui::Pos2)> = None;
    for (index, (high, low)) in pairs.enumerate() {
        let x = x(index);
        painter.text(
            pos2(x, rect.bottom() - 5.0),
            egui::Align2::CENTER_BOTTOM,
            &labels[index],
            FontId::proportional(10.0),
            muted,
        );
        if !high.is_finite() || !low.is_finite() {
            previous = None;
            continue;
        }
        let high_point = pos2(x, y(high));
        let low_point = pos2(x, y(low));
        if let Some((previous_high, previous_low)) = previous {
            painter.add(egui::Shape::convex_polygon(
                vec![previous_high, high_point, low_point, previous_low],
                cool.gamma_multiply(0.15),
                Stroke::NONE,
            ));
            painter.line_segment([previous_high, high_point], Stroke::new(1.8_f32, warm));
            painter.line_segment([previous_low, low_point], Stroke::new(1.8_f32, cool));
        }
        for (point, value, color, offset, alignment) in [
            (high_point, high, warm, -5.0, egui::Align2::CENTER_BOTTOM),
            (low_point, low, cool, 5.0, egui::Align2::CENTER_TOP),
        ] {
            painter.circle_filled(point, 2.6, color);
            if count <= 7 && width / count as f32 >= 30.0 {
                painter.text(
                    point + vec2(0.0, offset),
                    alignment,
                    format!("{value:.0}{unit}"),
                    FontId::proportional(10.0),
                    color,
                );
            }
        }
        previous = Some((high_point, low_point));
    }
}

#[allow(clippy::too_many_arguments)] // Shared chart painter receives layout and visual context.
fn line_chart(
    ui: &Ui,
    rect: Rect,
    id: Id,
    points: &[LineChartPoint],
    unit: &str,
    accent: Color32,
    muted: Color32,
    opacity: f32,
) {
    let points = &points[..points.len().min(31)];
    if points.is_empty() {
        return;
    }
    let values = points
        .iter()
        .filter_map(|point| point.value.filter(|v| v.is_finite()));
    let (min, max) = values
        .map(f64::from)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let span = (max - min).max(1.0) * 1.3;
    let has_images = points.iter().any(|point| point.image.is_some());
    let top = rect.top() + 20.0;
    let bottom = rect.bottom() - if has_images { 43.0 } else { 20.0 };
    if bottom <= top {
        return;
    }
    let width = rect.width() / points.len() as f32;
    let painter = ui.painter().with_clip_rect(rect);
    for level in [0.0, 0.5, 1.0] {
        painter.hline(
            rect.x_range(),
            egui::lerp(top..=bottom, level),
            Stroke::new(1.0_f32, muted.gamma_multiply(0.23)),
        );
    }

    let mut previous: Option<egui::Pos2> = None;
    for (index, point) in points.iter().enumerate() {
        let cell = Rect::from_min_size(
            rect.min + vec2(width * index as f32, 0.0),
            vec2(width, rect.height()),
        );
        let x = cell.center().x;
        let sample = point.value.filter(|v| v.is_finite()).map(|value| {
            let ratio = 0.5 + ((f64::from(value) - (min + max) * 0.5) / span) as f32;
            (pos2(x, egui::lerp(bottom..=top, ratio)), value)
        });
        let position = sample.map(|(position, _)| position);
        if let (Some(a), Some(b)) = (previous, position) {
            painter.add(egui::Shape::convex_polygon(
                vec![a, b, pos2(b.x, bottom), pos2(a.x, bottom)],
                accent.gamma_multiply(0.13),
                Stroke::NONE,
            ));
            painter.line_segment([a, b], Stroke::new(1.8_f32, accent));
        }
        previous = position;
        let labels = painter.with_clip_rect(cell);
        if let Some((position, value)) = sample {
            painter.circle_filled(position, 2.7, accent);
            let value = format!("{value:.1}");
            let value = value.strip_suffix(".0").unwrap_or(&value);
            let value = if value == "-0" { "0" } else { value };
            labels.text(
                position - vec2(0.0, 5.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{value}{unit}"),
                FontId::proportional(11.0),
                accent,
            );
        } else {
            labels.text(
                pos2(x, (top + bottom) * 0.5),
                egui::Align2::CENTER_CENTER,
                "—",
                FontId::proportional(11.0),
                muted,
            );
        }
        if let Some(image) = &point.image {
            paint_image(
                ui,
                &labels,
                image,
                Rect::from_center_size(pos2(x, rect.bottom() - 26.0), Vec2::splat(28.0)),
                opacity,
            );
        }
        labels.text(
            pos2(x, rect.bottom() - 1.0),
            egui::Align2::CENTER_BOTTOM,
            &point.label,
            FontId::proportional(10.0),
            muted,
        );
        if let Some(tooltip) = &point.tooltip {
            ui.interact(
                cell.intersect(ui.clip_rect()),
                id.with(index),
                Sense::hover(),
            )
            .on_hover_text(tooltip);
        }
    }
}

fn chart(painter: &egui::Painter, rect: Rect, kind: ChartKind, values: &[f32], color: Color32) {
    let step = values.len().div_ceil(1024).max(1);
    let data: Vec<_> = values
        .iter()
        .step_by(step)
        .map(|v| v.is_finite().then_some(f64::from(*v)))
        .collect();
    let mut range = if kind == ChartKind::Bar {
        [0.0, 0.0]
    } else {
        [f64::INFINITY, f64::NEG_INFINITY]
    };
    for &value in data.iter().flatten() {
        range = [range[0].min(value), range[1].max(value)];
    }
    let y = |v| {
        if range[0] == range[1] {
            rect.center().y
        } else {
            rect.bottom() - ((v - range[0]) / (range[1] - range[0])) as f32 * rect.height()
        }
    };
    if kind == ChartKind::Bar {
        let width = rect.width() / data.len().max(1) as f32;
        for (i, value) in data.iter().enumerate() {
            if let Some(v) = value {
                let x = rect.left() + i as f32 * width;
                painter.rect_filled(
                    Rect::from_two_pos(
                        pos2(x + width * 0.12, y(0.0)),
                        pos2(x + width * 0.88, y(*v)),
                    ),
                    1.0,
                    color,
                );
            }
        }
    } else {
        let spacing = rect.width() / data.len().saturating_sub(1).max(1) as f32;
        let mut previous = None;
        for (i, value) in data.iter().enumerate() {
            let current = value.map(|v| pos2(rect.left() + i as f32 * spacing, y(v)));
            if let (Some(a), Some(b)) = (previous, current) {
                painter.line_segment([a, b], Stroke::new(1.5_f32, color));
            }
            previous = current;
        }
    }
}

fn image_size(ui: &Ui, asset: &str) -> Vec2 {
    image(ui, asset).map_or(Vec2::ZERO, |texture| texture.size_vec2())
}
fn paint_image(ui: &Ui, painter: &egui::Painter, asset: &str, rect: Rect, opacity: f32) {
    if let Some(texture) = image(ui, asset) {
        assets::paint_texture(
            painter,
            &texture,
            rect,
            assets::uv_rect([0.0, 0.0, 1.0, 1.0]),
            ImageFit::Contain,
            0.0,
            Color32::WHITE.linear_multiply(opacity),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn segments_paint(
    painter: &egui::Painter,
    rect: Rect,
    count: u8,
    fraction: f32,
    gap: f32,
    direction: FillDirection,
    active: Color32,
    inactive: Color32,
) {
    let count = usize::from(count.clamp(1, 64));
    let horizontal = matches!(
        direction,
        FillDirection::LeftToRight | FillDirection::RightToLeft
    );
    let reverse = matches!(
        direction,
        FillDirection::RightToLeft | FillDirection::BottomToTop
    );
    let length = if horizontal {
        rect.width()
    } else {
        rect.height()
    };
    let gap = point(gap).min(length / count as f32 * 0.85);
    let step = (length + gap) / count as f32;
    let active_count = fraction * count as f32;
    for index in 0..count {
        let offset = index as f32 * step;
        let size = step - gap;
        let segment = match (horizontal, reverse) {
            (true, false) => Rect::from_min_size(
                rect.left_top() + vec2(offset, 0.0),
                vec2(size, rect.height()),
            ),
            (true, true) => Rect::from_min_size(
                rect.right_top() - vec2(offset + size, 0.0),
                vec2(size, rect.height()),
            ),
            (false, false) => Rect::from_min_size(
                rect.left_top() + vec2(0.0, offset),
                vec2(rect.width(), size),
            ),
            (false, true) => Rect::from_min_size(
                rect.left_bottom() - vec2(0.0, offset + size),
                vec2(rect.width(), size),
            ),
        };
        let fill = (active_count - index as f32).clamp(0.0, 1.0);
        painter.rect_filled(segment, 1.0, inactive);
        if fill > 0.0 {
            painter.rect_filled(segment, 1.0, active.linear_multiply(fill));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_raster(
    ui: &Ui,
    painter: &egui::Painter,
    id: Id,
    image: &RasterImage,
    rect: Rect,
    tint: Color32,
    radius: f32,
    fit: ImageFit,
) {
    let size = image.size.map(usize::from);
    if size.iter().any(|side| !(1..=256).contains(side))
        || image.rgba.len() != size[0] * size[1] * 4
    {
        return;
    }
    let mut hasher = DefaultHasher::new();
    image.size.hash(&mut hasher);
    image.rgba.hash(&mut hasher);
    let key = hasher.finish();
    let texture_id = id.with("raster");
    let cached = ui.data(|data| data.get_temp::<(u64, egui::TextureHandle)>(texture_id));
    let texture = match cached {
        Some((previous, texture)) if previous == key => texture,
        _ => {
            let texture = ui.ctx().load_texture(
                "widget raster",
                egui::ColorImage::from_rgba_unmultiplied(size, &image.rgba),
                egui::TextureOptions::LINEAR,
            );
            ui.data_mut(|data| data.insert_temp(texture_id, (key, texture.clone())));
            texture
        }
    };
    assets::paint_texture(
        painter,
        &texture,
        rect,
        assets::uv_rect([0.0, 0.0, 1.0, 1.0]),
        fit,
        radius,
        tint,
    );
}
