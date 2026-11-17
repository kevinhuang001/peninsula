use eframe::egui::Color32;

pub(super) const TEXT: Color32 = Color32::from_rgb(255, 255, 255);
pub(super) const SECONDARY_TEXT: Color32 = Color32::from_rgb(220, 225, 234);
pub(in crate::app) const ACCENT: Color32 = Color32::from_rgb(45, 164, 255);
pub(super) const CHECKBOX_ON: Color32 = Color32::from_rgb(48, 121, 184);
pub(super) const SWITCH_ON: Color32 = Color32::from_rgb(24, 62, 105);
pub(super) const ACCENT_SELECTED: Color32 = Color32::from_rgb(42, 76, 111);
pub(super) const BACKGROUND: Color32 = Color32::from_rgb(23, 25, 30);
pub(super) const PANEL: Color32 = Color32::from_rgb(27, 29, 34);
pub(super) const ROW: Color32 = Color32::from_rgb(36, 39, 46);
pub(super) const ROW_DISABLED: Color32 = Color32::from_rgb(30, 32, 37);
pub(super) const BORDER: Color32 = Color32::from_rgb(47, 51, 59);
pub(super) const SWITCH_OFF: Color32 = Color32::from_rgb(75, 81, 92);

pub(super) const DEFAULT_WIDGET_ACCENT: [u8; 3] = [37, 87, 156];

pub(in crate::app) const WIDGET_SURFACE_DARK: Color32 = Color32::from_rgb(16, 21, 30);
pub(in crate::app) const WIDGET_SURFACE_LIGHT: Color32 = Color32::from_rgb(247, 249, 253);

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
            visuals.hyperlink_color = if dark {
                accent.lerp_to_gamma(Color32::WHITE, 0.48)
            } else {
                accent.lerp_to_gamma(Color32::BLACK, 0.10)
            };
            visuals.panel_fill = if dark {
                WIDGET_SURFACE_DARK
            } else {
                WIDGET_SURFACE_LIGHT
            };
            visuals.widgets.noninteractive.fg_stroke.color = if dark {
                Color32::from_rgb(231, 237, 246)
            } else {
                Color32::from_rgb(29, 42, 62)
            };
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

pub fn parse_accent(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}
