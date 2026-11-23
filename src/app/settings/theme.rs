use crate::ui::palette::{self, TextPalette};
use eframe::egui::{self, Color32};

pub(super) const TEXT: Color32 = TextPalette::DEFAULT.primary;
pub(super) const SECONDARY_TEXT: Color32 = TextPalette::DEFAULT.secondary;
pub(super) const PANEL: Color32 = Color32::from_rgb(27, 29, 34);
pub(super) const ROW: Color32 = palette::SURFACE_VARIANT;
pub(super) const ROW_DISABLED: Color32 = palette::CONTROL_DISABLED;
pub(super) const BORDER: Color32 = palette::BORDER;

pub(super) const ACCENT_PRESETS: [(&str, [u8; 3]); 6] = [
    ("蓝色", [37, 87, 156]),
    ("紫色", [109, 69, 160]),
    ("青色", [20, 112, 122]),
    ("绿色", [43, 116, 75]),
    ("橙色", [167, 85, 31]),
    ("玫红", [160, 55, 98]),
];

pub(super) const DEFAULT_ACCENT_COLOR: [u8; 3] = [37, 87, 156];
pub(in crate::app) const WIDGET_SURFACE_DARK: Color32 = palette::SURFACE_DARK;

pub(super) fn accent_color(ui: &egui::Ui) -> Color32 {
    palette::accent_tokens(ui.ctx()).on_accent_subtle
}

pub(super) fn accent_selected(ui: &egui::Ui) -> Color32 {
    palette::accent_tokens(ui.ctx()).accent_subtle
}

/// Install a single dark UI style for native controls, popups and the renderer.
pub(in crate::app) fn install(context: &egui::Context, accent: [u8; 3]) {
    use egui::{CornerRadius, ThemePreference};
    context.set_theme(ThemePreference::Dark);
    palette::install_accent_tokens(context, accent);
    let tokens = palette::accent_tokens(context);
    let text = TextPalette::DEFAULT;
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
        let visuals = &mut style.visuals;
        *visuals = egui::Visuals::dark();
        visuals.selection.bg_fill = tokens.accent;
        visuals.selection.stroke.color = tokens.on_accent;
        visuals.hyperlink_color = palette::readable_accent(tokens.accent);
        visuals.warn_fg_color = text.warning;
        visuals.error_fg_color = text.danger;
        visuals.panel_fill = WIDGET_SURFACE_DARK;
        visuals.extreme_bg_color = palette::INPUT_BACKGROUND;
        visuals.widgets.noninteractive.fg_stroke.color = text.primary;
        visuals.widgets.noninteractive.bg_fill = palette::SURFACE_VARIANT;
        visuals.widgets.noninteractive.bg_stroke.color = palette::BORDER;
        for (widget, fill) in [
            (&mut visuals.widgets.inactive, palette::CONTROL_INACTIVE),
            (&mut visuals.widgets.hovered, palette::CONTROL_HOVERED),
            (&mut visuals.widgets.active, palette::CONTROL_ACTIVE),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.fg_stroke.color = text.primary;
            widget.bg_stroke.color = palette::BORDER;
            widget.corner_radius = CornerRadius::same(7);
        }
        visuals.widgets.active.bg_stroke.color = tokens.accent_focus;
    });
}

pub(super) fn parse_accent_color(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}
