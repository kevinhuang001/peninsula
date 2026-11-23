//! Shared text colors for the renderer, native egui labels and settings.
//! Tune the hierarchy here instead of dimming labels in individual widgets.
use eframe::egui::Color32;

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



/// A perceptually spaced 11-stop accent palette, generated without color-name branches.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AccentScale {
    shades: [Color32; 11],
}

impl AccentScale {
    pub(crate) fn new(rgb: [u8; 3]) -> Self {
        let [_, a, b] = rgb_to_oklab(rgb);
        let shades = [0.98, 0.94, 0.89, 0.82, 0.73, 0.64, 0.55, 0.46, 0.36, 0.27, 0.19]
            .map(|lightness| gamut_mapped_oklab(lightness, a, b));
        Self { shades }
    }

    pub(crate) fn at(self, index: usize) -> Color32 {
        self.shades[index.min(10)]
    }
}

/// Semantic colors shared by egui controls and the declarative renderer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ThemeTokens {
    pub scale: AccentScale,
    pub accent: Color32,
    pub on_accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub accent_subtle: Color32,
    pub on_accent_subtle: Color32,
    pub accent_border: Color32,
    pub accent_focus: Color32,
}

impl ThemeTokens {
    pub(crate) fn new(rgb: [u8; 3], dark: bool) -> Self {
        let scale = AccentScale::new(rgb);
        let accent = scale.at(if dark { 5 } else { 7 });
        let accent_subtle = scale.at(if dark { 9 } else { 1 });
        let contrasting = readable_on;
        Self {
            scale,
            accent,
            on_accent: contrasting(accent),
            accent_hover: scale.at(if dark { 4 } else { 6 }),
            accent_pressed: scale.at(if dark { 6 } else { 8 }),
            accent_subtle,
            on_accent_subtle: scale.at(if dark { 1 } else { 9 }),
            accent_border: scale.at(if dark { 6 } else { 3 }),
            accent_focus: scale.at(if dark { 3 } else { 6 }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct InstalledAccent {
    dark: ThemeTokens,
    light: ThemeTokens,
}

fn accent_store_id() -> eframe::egui::Id {
    eframe::egui::Id::new("peninsula-accent-theme-tokens")
}

/// Foreground chosen for the actual colored surface, not a fixed white label.
pub(crate) fn readable_on(background: Color32) -> Color32 {
    if contrast(background, Color32::WHITE) >= contrast(background, Color32::BLACK) {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

pub(crate) fn install_accent_tokens(ctx: &eframe::egui::Context, rgb: [u8; 3]) {
    let current = InstalledAccent {
        dark: ThemeTokens::new(rgb, true),
        light: ThemeTokens::new(rgb, false),
    };
    ctx.data_mut(|data| data.insert_temp(accent_store_id(), current));
}

pub(crate) fn accent_tokens(ctx: &eframe::egui::Context, dark: bool) -> ThemeTokens {
    let saved = ctx.data(|data| data.get_temp::<InstalledAccent>(accent_store_id()));
    let palette = saved.unwrap_or_else(|| InstalledAccent {
        dark: ThemeTokens::new([37, 87, 156], true),
        light: ThemeTokens::new([37, 87, 156], false),
    });
    if dark { palette.dark } else { palette.light }
}

// Linear sRGB / OKLab conversion. Mapping chroma down keeps all shades in gamut.
fn linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn gamma(channel: f32) -> f32 {
    if channel <= 0.0031308 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
    let r = linear(f32::from(rgb[0]) / 255.0);
    let g = linear(f32::from(rgb[1]) / 255.0);
    let b = linear(f32::from(rgb[2]) / 255.0);
    let l = (0.41222146 * r + 0.53633255 * g + 0.05144599 * b).cbrt();
    let m = (0.21190350 * r + 0.68069955 * g + 0.10739696 * b).cbrt();
    let s = (0.08830246 * r + 0.28171884 * g + 0.62997870 * b).cbrt();
    [
        0.21045426 * l + 0.7936178 * m - 0.00407205 * s,
        1.9779985 * l - 2.4285922 * m + 0.4505937 * s,
        0.02590404 * l + 0.78277177 * m - 0.80867577 * s,
    ]
}

fn oklab_to_rgb(lightness: f32, a: f32, b: f32) -> [f32; 3] {
    let l = (lightness + 0.39633778 * a + 0.21580376 * b).powi(3);
    let m = (lightness - 0.10556135 * a - 0.06385417 * b).powi(3);
    let s = (lightness - 0.08948418 * a - 1.2914855 * b).powi(3);
    [
        4.0767417 * l - 3.3077116 * m + 0.23096994 * s,
        -1.268438 * l + 2.6097574 * m - 0.3413194 * s,
        -0.0041960863 * l - 0.7034186 * m + 1.7076147 * s,
    ]
}

fn gamut_mapped_oklab(lightness: f32, a: f32, b: f32) -> Color32 {
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for _ in 0..20 {
        let mid = (lo + hi) * 0.5;
        let rgb = oklab_to_rgb(lightness, a * mid, b * mid);
        if rgb.iter().all(|value| (0.0..=1.0).contains(value)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let [r, g, b] = oklab_to_rgb(lightness, a * lo, b * lo)
        .map(|value| (gamma(value.clamp(0.0, 1.0)) * 255.0).round() as u8);
    Color32::from_rgb(r, g, b)
}

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
    ensure_text_contrast(start, background, target)
}

pub(crate) fn ensure_text_contrast(
    foreground: Color32,
    background: Color32,
    target: Color32,
) -> Color32 {
    for step in 0..20 {
        let color = foreground.lerp_to_gamma(target, step as f32 / 20.0);
        if contrast(color, background) >= 7.0 {
            return color;
        }
    }
    target
}

fn luminance(color: Color32) -> f32 {
    let r = linear(f32::from(color.r()) / 255.0);
    let g = linear(f32::from(color.g()) / 255.0);
    let b = linear(f32::from(color.b()) / 255.0);
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
    fn accent_scale_is_monotonic_for_presets_and_extreme_seeds() {
        for rgb in [[37, 87, 156], [109, 69, 160], [20, 112, 122],
                    [43, 116, 75], [167, 85, 31], [160, 55, 98],
                    [0, 0, 0], [255, 255, 255], [255, 0, 255]] {
            let scale = AccentScale::new(rgb);
            for index in 0..10 {
                let lighter = rgb_to_oklab([scale.at(index).r(), scale.at(index).g(), scale.at(index).b()])[0];
                let darker = rgb_to_oklab([scale.at(index + 1).r(), scale.at(index + 1).g(), scale.at(index + 1).b()])[0];
                assert!(lighter > darker, "{rgb:?} {index}: {lighter} !> {darker}");
            }
            for dark in [false, true] {
                let token = ThemeTokens::new(rgb, dark);
                assert!(contrast(token.accent, token.on_accent) >= 4.5);
                assert!(contrast(token.accent_subtle, token.on_accent_subtle) >= 4.5);
            }
        }
    }


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
    fn custom_capsule_text_is_only_adjusted_when_contrast_is_low() {
        for (background, primary) in [
            (Color32::from_rgb(25, 34, 48), TextPalette::DARK.primary),
            (Color32::from_rgb(235, 241, 249), TextPalette::LIGHT.primary),
        ] {
            assert_eq!(ensure_text_contrast(primary, background, primary), primary);
            let corrected = ensure_text_contrast(background, background, primary);
            assert!(contrast(corrected, background) >= 7.0);
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
