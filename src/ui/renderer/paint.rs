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
              m«ëŒ+Š×®º+º$zzb¥â7W'&VçBæ6öÆ÷"ÒVffV7G2æ6öÆ÷"æ÷"†7W'&VçBæ6öÆ÷"“°¢7W'&VçBç&öw&W72ÒVffV7G2ç&öw&W72æ÷"†7W'&VçBç&öw&W72“°¢ÆWB6—¦RÒ6VÆbæ–åö6VÆÂ‡V’Â6†–ÆBÂ–ææW"ç6—¦R‚’Â´Æ–vã£¥7G&WF6ƒ²%ÒÂFWF‚²“°¢ÆWB&V7BÒ&V7C£¦g&öÕöÖ–å÷6—¦R†–ææW"æÖ–âÂ6—¦R“°¢ÆWB7F'BÒ7W'&VçBæ†5÷G&ç6f÷&Ò‚’çF†Vâ‡ÇÂ°¢V’æ7G‚‚¢æw&†–75ö×WB‡Æw&†–77Âw&†–72æVçG'’‡V’æÆ–W%ö–B‚’’ææW‡Eö–G‚‚’¢Ò“°¢6VÆbç–çE÷v—F…öVffV7G2€¢V’À¢6†–ÆBÀ¢&V7BÀ¢6†–ÆEö–BÀ¢f÷&Vw&÷VæBÀ¢÷6—G’¢7W'&VçBæ÷6—G’À¢FWF‚²À¢7W'&VçBÀ¢“°¢–bÆWB6öÖR‡7F'B’Ò7F'B°¢7W'&VçBçG&ç6f÷&Õ÷6†W2‡V’Â7F'BÂ&V7Bæ6VçFW"‚’“°¢Ğ¢Ğ¢&WGW&ã°¢Ğ¢–bÆWBæöFS£¥67&öÆÂ††W2Â6†–ÆB’ÒfæöFRæ¶–æB°¢6VÆbç67&öÆÂ‡V’Â–ææW"Â6†–ÆBÂ¦†W2Â–BÂf÷&Vw&÷VæBÂ÷6—G’ÂFWF‚²“°¢&WGW&ã°¢Ğ¢–bÆWBæöFS£¥F&ÆR‡F&ÆR’ÒfæöFRæ¶–æB°¢6VÆbç–çE÷F&ÆR‡V’Â–ææW"ÂF&ÆRÂ–BÂf÷&Vw&÷VæBÂ÷6—G’ÂFWF‚²“°¢&WGW&ã°¢Ğ¢–bÆWB6öÖR†Æ–÷WB’Ò6VÆbæÆ–÷WB‡V’ÂæöFRÂ–ææW"ç6—¦R‚’ÂFWF‚²’°¢ÆWB6öÖR†6†–ÆG&Vâ’ÒÆ–÷WC£¦6†–ÆG&Vâ†æöFR’VÇ6R°¢&WGW&ã°¢Ó°¢f÷"†–æFW‚Â6VÆÂ’–âÆ–÷WBæ6VÆÇ2°¢ÆWB6†–ÆBÒf6†–ÆG&Vå¶–æFW…Ó°¢ÆWB6†–ÆEö–BÒ6†–Æ@¢æ¶W¢æ5÷&Vb‚¢æÖö÷%öVÇ6R‡ÇÂ–Bçv—F‚†–æFW‚’ÂÆ¶W—Â–Bçv—F‚†¶W’’“°¢6VÆbç–çB€¢V’À¢6†–ÆBÀ¢6VÆÂçG&ç6ÆFR†–ææW"æÖ–âçFõ÷fV3"‚’’À¢6†–ÆEö–BÀ¢f÷&Vw&÷VæBÀ¢÷6—G’À¢FWF‚²À¢“°¢Ğ¢&WGW&ã°¢Ğ¢6VÆbç–çEöÆVb‡V’Â–ææW"ÂæöFRÂ–BÂf÷&Vw&÷VæBÂ÷6—G’ÂVffV7G2“°¢Ğ ¢5¶ÆÆ÷r†6Æ—“£§FöõöÖç•ö&wVÖVçG2•Ğ¢fâ–çE÷F&ÆR€¢f×WB6VÆbÀ¢V“¢f×WBV’À¢&V7C¢&V7BÀ¢F&ÆS¢eF&ÆRÀ¢–C¢–BÀ¢f÷&Vw&÷VæC¢6öÆ÷#3"À¢÷6—G“¢c3"À¢FWFƒ¢W6—¦RÀ¢’°¢W6RÆ–÷WC£§µD$ÄUôtÂD$ÄUô„TDU%ô„T”t…BÂD$ÄUõ…õDD”ärÂD$ÄUõ•õDD”äwÓ°¢ÆWBÆ–÷WBÒ6VÆbçF&ÆR‡V’ÂF&ÆRÂ&V7Bçv–GF‚‚’ÂFWF‚“°¢ÆWB–çFW"ÒV’ç–çFW"‚’çv—F…ö6Æ—÷&V7B‡&V7B“°¢ÆWB6öÆ÷"ÒÇ7V7Â6VÆbçF†VÖRæ6öÆ÷"‡V’Â7V2’æÆ–æV%ö×VÇF—Ç’†÷6—G’“°¢ÆWB7W&f6RÒ6öÆ÷"„6öÆ÷%7V3£¥7W&f6Uf&–çB“°¢ÆWBF—f–FW"Ò7G&ö¶S£¦æWrƒãöc3"Â6öÆ÷"„6öÆ÷%7V3£¤&÷&FW"’ævÖÖö×VÇF—Ç’ƒãS‚’“°¢ÆWB†VFW"Ò&V7C£¦g&öÕöÖ–å÷6—¦R‡&V7BæÖ–âÂfV3"‡&V7Bçv–GF‚‚’ÂD$ÄUô„TDU%ô„T”t…B’“°¢–çFW"ç&V7Eöf–ÆÆVB€¢†VFW"À¢VwV“£¤6÷&æW%&F—W2°¢çs¢rÀ¢æS¢rÀ¢âäFVfVÇC£¦FVfVÇB‚¢ÒÀ¢7W&f6RÀ¢“°¢–çFW"æ†Æ–æR††VFW"ç…÷&ævR‚’Â†VFW"æ&÷GFöÒ‚’ÂF—f–FW"“°¢ÆWB×WB‚Ò&V7BæÆVgB‚’²D$ÄUõ…õDD”äs°¢f÷"†6öÇVÖâÂgv–GF‚’–âF&ÆRæ6öÇVÖç2æ—FW"‚’ç¦—‚fÆ–÷WBæ6öÇVÖç2’°¢FW‡B€¢V’À¢&V7C£¦g&öÕöÖ–å÷6—¦R‡÷3"‡‚Â&V7BçF÷‚’’ÂfV3"‡v–GF‚ÂD$ÄUô„TDU%ô„T”t…B’’À¢f6öÇVÖâçF—FÆRÀ¢F&ÆUö†VFW%÷7G–ÆR‚’À¢6öÆ÷"„6öÆ÷%7V3£¥6V6öæF'•FW‡B’À¢“°¢‚³Òv–GF‚²D$ÄUôt°¢Ğ¢ÆWB×WB’Ò†VFW"æ&÷GFöÒ‚“°¢f÷"†–æFW‚Â‡&÷rÂ†V–v‡B’’–âF&ÆRç&÷w2æ—FW"‚’ç¦—†Æ–÷WBç&÷w2’æVçVÖW&FR‚’°¢ÆWB&÷u÷&V7BÒ&V7C£¦g&öÕöÖ–å÷6—¦R‡÷3"‡&V7BæÆVgB‚’Â’’ÂfV3"‡&V7Bçv–GF‚‚’Â†V–v‡B’“°¢’³Ò†V–v‡C°¢–b&÷u÷&V7BçF÷‚’ãÒ&V7Bæ&÷GFöÒ‚’°¢'&V³°¢Ğ¢–bV’æ6Æ—÷&V7B‚’æ–çFW'6V7G2‡&÷u÷&V7B’°¢6öçF–çVS°¢Ğ¢–b–æFW‚R"ÓÒ°¢–çFW"ç&V7Eöf–ÆÆVB‡&÷u÷&V7BÂãÂ7W&f6RævÖÖö×VÇF—Ç’ƒã3b’“°¢Ğ¢–bÆWB6öÖR…6öÖR†ÖVçR’’ÒF&ÆRç&÷uöÖVçW2ævWB†–æFW‚’°¢ÆWB&W7öç6RÒV’æ–çFW&7B€¢&÷u÷&V7Bæ–çFW'6V7B‡&V7B’À¢–Bçv—F‚‚‚'&÷rÖÖVçR"ÂfÖVçRæ7F–öâ’’À¢6Vç6S£¦6Æ–6²‚’À¢“°¢7WW#£¦6öçFW‡EöÖVçR‚g&W7öç6RÂÇV—Â°¢V’æÆ&VÂ‚fÖVçRçF—FÆR“°¢ÆWB6Æ–6¶VBÒV’æ'WGFöâ‚fÖVçRæÆ&VÂ’æ6Æ–6¶VB‚“°¢–b6Æ–6¶VB°¢6VÆbæ7F–öâÒ6öÖR†ÖVçRæ7F–öâæ6ÆöæR‚’“°¢Ğ¢6Æ–6¶V@¢Ò“°¢Ğ¢–çFW"æ†Æ–æR‡&÷u÷&V7Bç…÷&ævR‚’Â&÷u÷&V7Bæ&÷GFöÒ‚’ÂF—f–FW"“°¢ÆWB×WB‚Ò&V7BæÆVgB‚’²D$ÄUõ…õDD”äs°¢f÷"†6öÇVÖâÂ†6VÆÂÂgv–GF‚’’–â&÷ræ—FW"‚’ç¦—‚fÆ–÷WBæ6öÇVÖç2’æVçVÖW&FR‚’°¢ÆWB&÷VæG2Ò&V7C£¦g&öÕöÖ–å÷6—¦R€¢÷3"‡‚Â&÷u÷&V7BçF÷‚’²D$ÄUõ•õDD”är’À¢fV3"‡v–GF‚Â†V–v‡BÒ"ã¢D$ÄUõ•õDD”är’À¢“°¢ÆWB6†–ÆEö–BÒ–Bçv—F‚‚†–æFW‚Â6öÇVÖâÂ6VÆÂæ¶W’æ5öFW&Vb‚’’“°¢ÆWB×WB6†–ÆBÒV’ææWuö6†–ÆB…V”'V–ÆFW#£¦æWr‚’æ–E÷6ÇB†6†–ÆEö–B’æÖ…÷&V7B†&÷VæG2’“°¢6†–ÆBç6WEö6Æ—÷&V7B‡V’æ6Æ—÷&V7B‚’æ–çFW'6V7B‡&V7B’æ–çFW'6V7B†&÷VæG2’“°¢ÆWB6—¦RÒ6VÆbæ–åö6VÆÂ€¢f6†–ÆBÀ¢6VÆÂÀ¢fV3"‡v–GF‚Âã’À¢´Æ–vã£¥7G&WF6‚ÂÆ–vã£¥7F'EÒÀ¢FWF‚À¢“°¢6VÆbç–çB€¢f×WB6†–ÆBÀ¢6VÆÂÀ¢&V7C£¦g&öÕöÖ–å÷6—¦R†æ6†÷"†&÷VæG2Â6—¦RÂ´Æ–vã£¥7F'BÂÆ–vã£¤6VçFW%Ò’Â6—¦R’À¢6†–ÆEö–BÀ¢f÷&Vw&÷VæBÀ¢÷6—G’À¢FWF‚À¢“°¢‚³Òv–GF‚²D$ÄUôt°¢Ğ¢Ğ¢Ğ ¢5¶ÆÆ÷r†6Æ—“£§FöõöÖç•ö&wVÖVçG2•Ğ¢fâ–çEöÆVb€¢f×WB6VÆbÀ¢V“¢f×WBV’À¢&V7C¢&V7BÀ¢æöFS¢ef–WtæöFRÀ¢–C¢–BÀ¢f÷&Vw&÷VæC¢6öÆ÷#3"À¢÷6—G“¢c3"À¢VffV7G3¢VffV7G2À¢’°¢ÆWB–çFW"ÒV’ç–çFW"‚’çv—F…ö6Æ—÷&V7B‡&V7B“°¢ÆWBfrÒf÷&Vw&÷VæBæÆ–æV%ö×VÇF—Ç’†÷6—G’“°¢ÆWB6öÆ÷"ÒÆ6öÆ÷'Â6VÆbçF†VÖRæ6öÆ÷"‡V’Â6öÆ÷"’æÆ–æV%ö×VÇF—Ç’†÷6—G’“°¢ÖF6‚fæöFRæ¶–æB°¢æöFS£¥FW‡D–çWB²¶W’ÂfÇVRÂ†–çBÒÓâ°¢ÆWB×WBfÇVRÒfÇVRæ6ÆöæR‚“°¢ÆWB×WB6†–ÆBÒV’ææWuö6†–ÆB…V”'V–ÆFW#£¦æWr‚’æ–E÷6ÇB†–B’æÖ…÷&V7B‡&V7B’“°¢ÆWB&W7öç6RÒ6†–ÆBæFE÷6—¦VB€¢&V7Bç6—¦R‚’À¢VwV“£¥FW‡DVF—C£§6–ævÆVÆ–æR‚f×WBfÇVR¢æ–B†–Bçv—F‚†¶W’’¢æ†–çE÷FW‡B††–çB¢æ6†%öÆ–Ö—Bƒ#C‚’À¢“°¢–b&W7öç6Ræ6†ævVB‚’°¢6VÆbæ7F–öâÒ6öÖR†7&FS£¦W‡FVç6–öã£¤7F–öã£¥6WEFW‡B°¢¶W“¢¶W’æ6ÆöæR‚’À¢fÇVRÀ¢Ò“°¢Ğ¢Ğ¢æöFS£¥77v÷&D–çWB²¶W’ÂfÇVRÂ†–çBÒÓâ°¢ÆWB×WBfÇVRÒfÇVRæ6ÆöæR‚“°¢ÆWB×WB6†–ÆBÒV’ææWuö6†–ÆB…V”'V–ÆFW#£¦æWr‚’æ–E÷6ÇB†–B’æÖ…÷&V7B‡&V7B’“°¢ÆWB&W7öç6RÒ6†–ÆBæFE÷6—¦VB€¢&V7Bç6—¦R‚’À¢VwV“£¥FW‡DVF—C£§6–ævÆVÆ–æR‚f×WBfÇVR¢æ–B†–Bçv—F‚†¶W’’¢æ†–çE÷FW‡B††–çB¢æ6†%öÆ–Ö—Bƒ#C‚¢ç77v÷&B‡G'VR’À¢“°¢–b&W7öç6Ræ6†ævVB‚’°¢6VÆbæ7F–öâÒ6öÖR†7&FS£¦W‡FVç6–öã£¤7F–öã£¥6WEFW‡B°¢¶W“¢¶W’æ6ÆöæR‚’À¢fÇVRÀ¢Ò“°¢Ğ¢Ğ¢æöFS£¥FW‡B°¢FW‡C¢fÇVRÀ¢FW‡E÷7G–ÆRÀ¢ÒÓâFW‡B‡V’Â&V7BÂfÇVRÂ§FW‡E÷7G–ÆRÂfr’À¢æöFS£¤–6öâ²æÖRÂ–6öå÷7G–ÆRÒÓâ°¢ÆWB6—¦RÒfV3#£§7ÆB‡ö–çB†–6öå÷7G–ÆRç6—¦R’’æÖ–â‡&V7Bç6—¦R‚’“°¢ÆWB–6öå÷&V7BÒ&V7C£¦g&öÕöÖ–å÷6—¦R†æ6†÷"‡&V7BÂ6—¦RÂ–6öå÷7G–ÆRæÆ–væÖVçB’Â6—¦R“°¢ÆWBF–çBÒ–bVffV7G2æ6öÆ÷"æ—5÷6öÖR‚’°¢fp¢ÒVÇ6R°¢–6öå÷7G–ÆRæ6öÆ÷"æÖö÷"†frÂ6öÆ÷"¢Ó°¢–bÆWB6öÖR†–6öâ’ÒæÖVEö–6öâ†æÖR’°¢6VÆbæ–6öç2ç–çB‚g–çFW"Â–6öâÂ–6öå÷&V7BÂF–çBÂã“°¢ÒVÇ6R–bÆWB6öÖR‡FW‡GW&R’Ò–ÖvR‡V’ÂæÖR’°¢–çFW"æ–ÖvR€¢FW‡GW&Ræ–B‚’À¢–6öå÷&V7BÀ¢&V7C£¦g&öÕöÖ–åöÖ‚†VwV“£¥÷3#£¥¤U$òÂ÷3"ƒãÂã’’À¢F–çBÀ¢“°¢Ğ¢Ğ¢æöFS£¤–ÖvR²76WBÒÓâ°¢–bÆWB6öÖR‡FW‡GW&R’Ò–ÖvR‡V’Â76WB’°¢76WG3£§–çE÷FW‡GW&R€¢g–çFW"À¢gFW‡GW&RÀ¢&V7BÀ¢76WG3£§We÷&V7B…³ãÂãÂãÂãÒ’À¢æöFRæ–ÖvUöf—BçVçw&ö÷%öFVfVÇB‚’À¢ö–çB†æöFRç7G–ÆRææ÷&ÖÂæ6÷&æW%÷&F—W2çVçw&ö÷"ƒã’’À¢VffV7G0¢æ6öÆ÷ ¢æÖö÷"„6öÆ÷#3#£¥t„•DRÂÅ÷Âf÷&Vw&÷VæB¢æÆ–æV%ö×VÇF—Ç’†÷6—G’’À¢“°¢Ğ¢Ğ¢æöFS£¥7fr²76WBÂ7fu÷7G–ÆRÒÓâ°¢–bÆWB6öÖR‡FW‡GW&R’Ğ¢76WG3£§7fr‡V’Âg6VÆbçF†VÖRÂ76WBÂ§7fu÷7G–ÆRÂ6öÖR‡&V7Bç6—¦R‚’’¢°¢76WG3£§–çE÷FW‡GW&R€¢g–çFW"À¢gFW‡GW&RÀ¢&V7BÀ¢76WG3£§We÷&V7B…³ãÂãÂãÂãÒ’À¢7fu÷7G–ÆRæf—BÀ¢ö–çB†æöFRç7G–ÆRææ÷&ÖÂæ6÷&æW%÷&F—W2çVçw&ö÷"ƒã’’À¢VffV7G0¢æ6öÆ÷ ¢æÖö÷%öVÇ6R‡ÇÂ76WG3£§7fu÷F–çB‚§7fu÷7G–ÆRÂfrÂ÷6—G’’ÂÅ÷Âfr’À¢“°¢Ğ¢Ğ¢æöFS£¥&7FW$–ÖvR†–ÖvR’Óâ–çE÷&7FW"€¢V’À¢g–çFW"À¢–BÀ¢–ÖvRÀ¢&V7BÀ¢VffV7G0¢æ6öÆ÷ ¢æÖö÷"„6öÆ÷#3#£¥t„•DRÂÅ÷Âf÷&Vw&÷VæB¢æÆ–æV%ö×VÇF—Ç’†÷6—G’’À¢ö–çB†æöFRç7G–ÆRææ÷&ÖÂæ6÷&æW%÷&F—W2çVçw&ö÷"ƒã’’À¢æöFRæ–ÖvUöf—BçVçw&ö÷"„–ÖvTf—C£¥7G&WF6‚’À¢’À¢æöFS£¥7&—FR²g&ÖW2Âg&ÖUö×2ÒÓâ°¢ÆWB–æFW‚Ò6VÆbçVæ–f÷&Õ÷7&—FUö–æFW‚‡V’Â–BÂg&ÖW2æÆVâ‚’Â¦g&ÖUö×2“°¢–bÆWB6öÖR†76WB’Òg&ÖW2ævWB†–æFW‚’°¢–çEö–ÖvR‡V’Âg–çFW"Â76WBÂ&V7BÂ÷6—G’“°¢Ğ¢Ğ¢æöFS£¥7&—FU6WVVæ6R‡7V2’Óâ°¢ÆWB–æFW‚Ò6VÆbç7&—FUö–æFW‚‡V’Â–BÂ7V2“°¢–bÆWB6öÖR†g&ÖR’Ò7V2æg&ÖW2ævWB†–æFW‚’°¢–bÆWB6öÖR‡FW‡GW&R’Ò–ÖvR‡V’Âfg&ÖRæ76WB’°¢76WG3£§–çE÷FW‡GW&R€¢g–çFW"À¢gFW‡GW&RÀ¢&V7BÀ¢76WG3£§We÷&V7B†g&ÖRçWb’À¢æöFRæ–ÖvUöf—BçVçw&ö÷%öFVfVÇB‚’À¢ö–çB†æöFRç7G–ÆRææ÷&ÖÂæ6÷&æW%÷&F—W2çVçw&ö÷"ƒã’’À¢VffV7G0¢æ6öÆ÷ ¢æÖö÷"„6öÆ÷#3#£¥t„•DRÂÅ÷Âf÷&Vw&÷VæB¢æÆ–æV%ö×VÇF—Ç’†÷6—G’’À¢“°¢Ğ¢Ğ¢Ğ¢æöFS£¥6VvÖVçFVD–æF–6F÷"°¢6VvÖVçG2À¢g&7F–öâÀ¢vÀ¢F—&V7F–öâÀ¢–æ7F—fRÀ¢ÒÓâ°¢6VvÖVçG5÷–çB€¢g–çFW"À¢&V7BÀ¢§6VvÖVçG2À¢&F–ò†VffV7G2ç&öw&W72çVçw&ö÷"‚¦g&7F–öâ’’À¢¦vÀ¢¦F—&V7F–öâÀ¢frÀ¢6öÆ÷"‚¦–æ7F—fR’À¢“°¢Ğ¢æöFS£¤ÖWG&–2²Æ&VÂÂfÇVRÂVæ—BÒÓâ°¢ÆWBÖ–BÒ&V7BçF÷‚’²&V7Bæ†V–v‡B‚’¢ãC#°¢FW‡B€¢V’À¢&V7C£¦g&öÕöÖ–åöÖ‚‡&V7BæÖ–âÂ÷3"‡&V7Bç&–v‡B‚’ÂÖ–B’’À¢Æ&VÂÀ¢FW‡E7G–ÆR°¢föçE÷6—¦S¢föçE6—¦S£¥6ÖÆÂÀ¢âäFVfVÇC£¦FVfVÇB‚¢ÒÀ¢6öÆ÷"„6öÆ÷%7V3£¥6V6öæF'•FW‡B’À¢“°¢ÆWBfÇVRÒf÷&ÖB‚'·fÇVW×·Væ—GÒ"“°¢FW‡B€¢V’À¢&V7C£¦g&öÕöÖ–åöÖ‚‡÷3"‡&V7BæÆVgB‚’ÂÖ–B’Â&V7BæÖ‚’À¢gfÇVRÀ¢FW‡E7G–ÆR°¢föçE÷6—¦S¢föçE6—¦S£¥F—FÆRÀ¢âäFVfVÇC£¦FVfVÇB‚¢ÒÀ¢frÀ¢“°¢Ğ¢æöFS£¤&FvR†Æ&VÂ’ÓâFW‡B€¢V’À¢&V7Bç6‡&–æ³"‡fV3"ƒRãÂã’’À¢Æ&VÂÀ¢FW‡E7G–ÆS£¦FVfVÇB‚’À¢frÀ¢’À¢æöFS£¤F—f–FW"²fW'F–6ÂÒÓâ°¢ÆWB2Ò&V7Bæ6VçFW"‚“°¢ÆWBÆ–æRÒ–b§fW'F–6Â°¢·÷3"†2ç‚Â&V7BçF÷‚’’Â÷3"†2ç‚Â&V7Bæ&÷GFöÒ‚’•Ğ¢ÒVÇ6R°¢·÷3"‡&V7BæÆVgB‚’Â2ç’’Â÷3"‡&V7Bç&–v‡B‚’Â2ç’•Ğ¢Ó°¢–çFW"æÆ–æU÷6VvÖVçB†Æ–æRÂ7G&ö¶S£¦æWrƒãöc3"Âfr’“°¢Ğ¢æöFS£¥6Æ–FW"°¢g&7F–öâÀ¢Æ&VÂÀ¢7F–öâÀ¢Væ&ÆVBÀ¢ÒÓâ°¢ÆWBVæ&ÆVBÒ¦Væ&ÆVBbbV’æ—5öVæ&ÆVB‚“°¢ÆWB&W7öç6RÒV’æ7G‚‚’ç&VE÷&W7öç6R†–B“°¢ÆWB×WBfÇVRÒ&F–ò‚¦g&7F–öâ“°¢–bÆWB6öÖR‡&W7öç6R’Òg&W7öç6R°¢–bVæ&ÆVBbb‡&W7öç6RæG&u÷7F'FVB‚’ÇÂ&W7öç6Ræ6Æ–6¶VB‚’’°¢&W7öç6Rç&WVW7Eöfö7W2‚“°¢Ğ¢&W7öç6Rçv–FvWEö–æfò‡ÇÂ°¢VwV“£¥v–FvWD–æfó£¦Æ&VÆVB†VwV“£¥v–FvWEG—S£¥6Æ–FW"ÂVæ&ÆVBÂÆ&VÂ¢Ò“°¢ÆWB&Wf–Wuö–BÒ–Bçv—F‚‚'6Æ–FW"×&Wf–Wr"“°¢–bVæ&ÆV@¢bb‡&W7öç6RæG&vvVB‚’ÇÂ&W7öç6RæFÚ±î¸Â¸­yêë¢°k¢G§¦*^rag_stopped() || response.clicked())
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
                "â€”",
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
