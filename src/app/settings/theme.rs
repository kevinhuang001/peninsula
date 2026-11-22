use crate::ui::palette::{self, TextPalette};
use eframe::egui::{self, Color32};

pub(super) const TEXT: Color32 = TextPalette::DARK.primary;
pub(super) const SECONDARY_TEXT: Color32 = TextPalette::DARK.secondary;
pub(super) const PANEL: Color32 = Color32::from_rgb(27, 29, 34);
pub(super) const ROW: Color32 = Color32::from_rgb(36, 39, 46);
pub(super) const ROW_DISABLED: Color32 = Color32::from_rgb(30, 32, 37);
pub(super) const BORDER: Color32 = Color32::from_rgb(47, 51, 59);

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
pub(in crate::app) const WIDGET_SURFACE_LIGHT: Color32 = palette::SURFACE_LIGHT;

pub(super) fn accent_color(ui: &egui::Ui) -> Color32 {
    palette::accent_tokens(ui.ctx(), true).on_accent_subtle
}

pub(super) fn accent_selected(ui: &egui::Ui) -> Color32 {
    palette::accent_tokens(ui.ctx(), true).accent_subtle
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppearanceTheme {
    #[default]
    Dark,
    Light,
    System,
}

impl AppearanceTheme {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "system" => Some(Self::System),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }

    pub fn install(self, context: &egui::Context, accent: [u8; 3]) {
        use egui::{CornerRadius, ThemePreference};
        context.set_theme(match self {
            Self::Light => ThemePreference::Light,
            Self::Dark => ThemePreference::Dark,
            Self::System => ThemePreference::System,
        });
        palette::install_accent_tokens(context, accent);
        context.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(6.0, 6.0);
            let visuals = &mut style.visuals;
            let dark = visuals.dark_mode;
            let tokens = palette::accent_tokens(context, dark);
            visuals.selection.bg_fill = tokens.accent;
            visuals.selection.stroke.color = tokens.on_accent;
            let text = TextPalette::for_mode(dark);
            visuals.hyperlink_color = palette::readable_accent(tokens.accent, dark);
            visuals.warn_fg_color = text.warning;
            visuals.error_fg_color = text.danger;
            visuals.panel_fill = if dark { WIDGET_SURFACE_DARK } else { WIDGET_SURFACE_LIGHT };
            visuals.widgets.noninteractive.fg_stroke.color = text.primary;
            visuals.widgets.noninteractive.bg_fill = if dark {
                Color32::from_rgb(25, 34, 48)
            } else {
                Color32::from_rgb(235, 241, 249)
            };
            visuals.widgets.noninteractive.bg_stroke.color = if dark {
                Color32::from_rgb(42, 56, 76)
            } else {
                Color32::from_rgb(206, 218, 234)
            };
            for (widget, fill) in [
                (&mut visuals.widgets.inactive, tokens.accent),
                (&mut visuals.widgets.hovered, tokens.accent_hover),
                (&mut visuals.widgets.active, tokens.accent_pressed),
            ] {
                widget.bg_fill = fill;
                widget.weak_bg_fill = fill;
                widget.fg_stroke.color = palette::readable_on(fill);
                widget.bg_stroke.color = tokens.accent_border;
                widget.corner_radius = CornerRadius::same(7);
            }
            visuals.widgets.active.bg_stroke.color = tokens.accent_focus;
        });
    }
}

pub(super) fn parse_accent_color(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}
