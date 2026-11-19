use crate::ui::palette::{self, TextPalette};
use eframe::egui::Color32;

pub(super) const TEXT: Color32 = TextPalette::DARK.primary;
pub(super) const SECONDARY_TEXT: Color32 = TextPalette::DARK.secondary;
pub(in crate::app) const ACCENT: Color32 = palette::SETTINGS_ACCENT_TEXT;
pub(super) const ACCENT_SELECTED: Color32 = Color32::from_rgb(42, 76, 111);
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

pub(super) const DEFAULT_WIDGET_ACCENT: [u8; 3] = [37, 87, 156];

pub(in crate::app) const WIDGET_SURFACE_DARK: Color32 = palette::SURFACE_DARK;
pub(in crate::app) const WIDGET_SURFACE_LIGHT: Color32 = palette::SURFACE_LIGHT;

// Widget colors follow the configured egui theme. The existing settings
// panels keep their own shell colors.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WidgetTheme {
    #[default]
    Dark,
    Light,
    System,
}

impl WidgetTheme {
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

    pub fn install(self, context: &eframe::egui::Context, accent: [u8; 3]) {
        use eframe::egui::{CornerRadius, ThemePreference};
        context.set_theme(match self {
            Self::Light => ThemePreference::Light,
            Self::Dark => ThemePreference::Dark,
            Self::System => ThemePreference::System,
        });
        let [r, g, b] = accent;
        let accent = contrasting_button_fill(Color32::from_rgb(r, g, b));
        context.all_styles_mut(|style| {
            style.spacing.item_spacing = eframe::egui::vec2(6.0, 6.0);
            let visuals = &mut style.visuals;
            let dark = visuals.dark_mode;
            visuals.selection.bg_fill = accent;
            visuals.selection.stroke.color = Color32::WHITE;
            let text = TextPalette::for_mode(dark);
            visuals.hyperlink_color = palette::readable_accent(accent, dark);
            visuals.warn_fg_color = text.warning;
            visuals.error_fg_color = text.danger;
            visuals.panel_fill = if dark {
                WIDGET_SURFACE_DARK
            } else {
                WIDGET_SURFACE_LIGHT
            };
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
            let hover = contrasting_button_fill(accent.lerp_to_gamma(Color32::WHITE, 0.10));
            let press = accent.lerp_to_gamma(Color32::BLACK, 0.16);
            for (widget, fill) in [
                (&mut visuals.widgets.inactive, accent),
                (&mut visuals.widgets.hovered, hover),
                (&mut visuals.widgets.active, press),
            ] {
                widget.bg_fill = fill;
                widget.weak_bg_fill = fill;
                widget.fg_stroke.color = Color32::WHITE;
                widget.corner_radius = CornerRadius::same(7);
            }
        });
    }
}

fn contrasting_button_fill(color: Color32) -> Color32 {
    use eframe::egui::Rgba;
    let [r, g, b, _] = Rgba::from(color).to_array();
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    // Keep white button labels readable with light custom accent colors.
    let scale = (0.17 / luminance).min(1.0);
    Rgba::from_rgb(r * scale, g * scale, b * scale).into()
}

pub(super) fn parse_preset_accent(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    let color = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    ACCENT_PRESETS
        .iter()
        .find_map(|(_, preset)| (*preset == color).then_some(color))
}
