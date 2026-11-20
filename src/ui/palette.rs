//! Shared text colors for the renderer, native egui labels and settings.
//! Tune the hierarchy here instead of dimming labels in individual widgets.
use eframe::egui::{Color32, Rgba};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextPalette {
    pub primary: Color32,
    pub secondary: Color32,
    pub muted: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub info: Color32,
    pub cyan: Color32,
    pub violet: Color32,
}

impl TextPalette {
    pub const DARK: Self = Self {
        primary: Color32::WHITE,
        secondary: Color32::from_rgb(235, 240, 248),
        muted: Color32::from_rgb(203, 214, 230),
        success: Color32::from_rgb(97, 222, 174),
        warning: Color32::from_rgb(255, 153, 77),
        danger: Color32::from_rgb(255, 158, 170),
        info: Color32::from_rgb(143, 202, 255),
        cyan: Color32::from_rgb(91, 219, 226),
        violet: Color32::from_rgb(205, 180, 255),
    };
    pub const LIGHT: Self = Self {
        primary: Color32::from_rgb(29, 42, 62),
        secondary: Color32::from_rgb(55, 69, 89),
        muted: Color32::from_rgb(65, 79, 99),
        success: Color32::from_rgb(0, 90, 56),
        warning: Color32::from_rgb(125, 55, 0),
        danger: Color32::from_rgb(140, 32, 47),
        info: Color32::from_rgb(26, 77, 150),
        cyan: Color32::from_rgb(0, 86, 104),
        violet: Color32::from_rgb(91, 53, 156),
    };

    pub const fn for_mode(dark: bool) -> Self {
        if dark {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

pub(crate) const SETTINGS_ACCENT_TEXT: Color32 = Color32::from_rgb(146, 206, 255);

pub(crate) const SURFACE_DARK: Color32 = Color32::from_rgb(16, 21, 30);
pub(crate) const SURFACE_LIGHT: Color32 = Color32::from_rgb(247, 249, 253);
// Include the lighter settings row / darker light-theme chip in the budget.
const TEXT_BACKGROUND_DARK: Color32 = Color32::from_rgb(36, 39, 46);
const TEXT_BACKGROUND_LIGHT: Color32 = Color32::from_rgb(235, 241, 249);

/// Keep colored small text readable even with black, white or vivid accents.
/// This changes text only; the user's button/background accent is retained.
pub(crate) fn readable_accent(accent: Color32, dark: bool) -> Color32 {
    let target = if dark { Color32::WHITE } else { Color32::BLACK };
    let background = if dark {
        TEXT_BACKGROUND_DARK
    } else {
        TEXT_BACKGROUND_LIGHT
    };
    let start = accent.lerp_to_gamma(target, if dark { 0.68 } else { 0.10 });
    for step in 0..=20 {
        let color = start.lerp_to_gamma(target, step as f32 / 20.0);
        if contrast(color, background) >= 7.0 {
            return color;
        }
    }
    target
}

fn luminance(color: Color32) -> f32 {
    let [r, g, b, _] = Rgba::from(color).to_array();
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast(a: Color32, b: Color32) -> f32 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_text_levels_meet_high_contrast_on_their_surfaces() {
        for (palette, backgrounds) in [
            (TextPalette::DARK, [SURFACE_DARK, TEXT_BACKGROUND_DARK]),
            (TextPalette::LIGHT, [SURFACE_LIGHT, TEXT_BACKGROUND_LIGHT]),
        ] {
            for text in [
                palette.primary,
                palette.secondary,
                palette.muted,
                palette.success,
                palette.warning,
                palette.danger,
                palette.info,
                palette.cyan,
                palette.violet,
            ] {
                for background in backgrounds {
                    assert!(
                        contrast(text, background) >= 7.0,
                        "{text:?} on {background:?}"
                    );
                }
            }
            assert!(
                contrast(palette.primary, backgrounds[0])
                    > contrast(palette.secondary, backgrounds[0])
            );
            assert!(
                contrast(palette.secondary, backgrounds[0])
                    > contrast(palette.muted, backgrounds[0])
            );
        }
    }

    #[test]
    fn custom_accent_text_remains_readable_in_both_themes() {
        for dark in [false, true] {
            let background = if dark {
                TEXT_BACKGROUND_DARK
            } else {
                TEXT_BACKGROUND_LIGHT
            };
            for r in [0, 128, 255] {
                for g in [0, 128, 255] {
                    for b in [0, 128, 255] {
                        let text = readable_accent(Color32::from_rgb(r, g, b), dark);
                        assert_eq!(text.a(), 255);
                        assert!(contrast(text, background) >= 7.0);
                    }
                }
            }
        }
    }
}
