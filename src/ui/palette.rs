//! Shared text colors for the renderer, native egui labels and settings.
//! Tune the hierarchy here instead of dimming labels in individual widgets.
use eframe::egui::Color32;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextPalette {
    pub primary: Color32,
    pub secondary: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub placeholder: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub info: Color32,
    pub cyan: Color32,
    pub violet: Color32,
}

impl TextPalette {
    pub const DEFAULT: Self = Self {
        primary: Color32::WHITE,
        secondary: Color32::from_rgb(235, 240, 248),
        muted: Color32::from_rgb(203, 214, 230),
        disabled: Color32::from_rgb(185, 196, 213),
        placeholder: Color32::from_rgb(185, 196, 213),
        success: Color32::from_rgb(97, 222, 174),
        warning: Color32::from_rgb(255, 153, 77),
        danger: Color32::from_rgb(255, 158, 170),
        info: Color32::from_rgb(143, 202, 255),
        cyan: Color32::from_rgb(91, 219, 226),
        violet: Color32::from_rgb(205, 180, 255),
    };
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
    pub accent_subtle: Color32,
    pub on_accent_subtle: Color32,
    pub accent_focus: Color32,
}

impl ThemeTokens {
    pub(crate) fn new(rgb: [u8; 3]) -> Self {
        let scale = AccentScale::new(rgb);
        let accent = scale.at(5);
        let accent_subtle = scale.at(9);
        Self {
            scale,
            accent,
            on_accent: readable_on(accent),
            accent_subtle,
            on_accent_subtle: scale.at(1),
            accent_focus: scale.at(3),
        }
    }
}
fn accent_store_id() -> eframe::egui::Id {
    eframe::egui::Id::new("peninsula-accent-theme-tokens")
}
/// Foreground selected against the actual colored background.
pub(crate) fn readable_on(background: Color32) -> Color32 {
    if contrast(background, Color32::WHITE) >= contrast(background, Color32::BLACK) {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}
pub(crate) fn install_accent_tokens(ctx: &eframe::egui::Context, rgb: [u8; 3]) {
    ctx.data_mut(|data| data.insert_temp(accent_store_id(), ThemeTokens::new(rgb)));
}
pub(crate) fn accent_tokens(ctx: &eframe::egui::Context) -> ThemeTokens {
    ctx.data(|data| data.get_temp::<ThemeTokens>(accent_store_id()))
        .unwrap_or_else(|| ThemeTokens::new([37, 87, 156]))
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

// Neutral surface colors never depend on the accent seed.
pub(crate) const SURFACE_DARK: Color32 = Color32::from_rgb(16, 21, 30);

/// Host-owned capsule text choices. Bright tints remain legible on both the
/// black capsule shell and its dark chips. None uses the default white text.
pub(crate) const CAPSULE_TEXT_PRESETS: [(&str, Option<[u8; 3]>); 8] = [
    ("默认", None),
    ("纯白", Some([255, 255, 255])),
    ("暖白", Some([255, 238, 211])),
    ("浅蓝", Some([158, 214, 255])),
    ("薄荷绿", Some([154, 242, 204])),
    ("浅金", Some([255, 226, 156])),
    ("淡紫", Some([218, 195, 255])),
    ("珊瑚粉", Some([255, 187, 199])),
];
pub(crate) const SURFACE_VARIANT: Color32 = Color32::from_rgb(36, 39, 46);
pub(crate) const CONTROL_INACTIVE: Color32 = Color32::from_rgb(44, 49, 59);
pub(crate) const CONTROL_HOVERED: Color32 = Color32::from_rgb(55, 62, 74);
pub(crate) const CONTROL_ACTIVE: Color32 = Color32::from_rgb(62, 72, 87);
pub(crate) const CONTROL_DISABLED: Color32 = Color32::from_rgb(37, 41, 49);
pub(crate) const INPUT_BACKGROUND: Color32 = Color32::from_rgb(29, 33, 41);
pub(crate) const BORDER: Color32 = Color32::from_rgb(58, 66, 79);
/// Accents used as small text are lightened independently of accent surfaces.
pub(crate) fn readable_accent(accent: Color32) -> Color32 {
    ensure_text_contrast(accent, SURFACE_VARIANT, Color32::WHITE)
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
    fn accent_scale_is_monotonic_and_foregrounds_are_readable() {
        for rgb in [
            [37, 87, 156], [109, 69, 160], [20, 112, 122],
            [43, 116, 75], [167, 85, 31], [160, 55, 98],
            [0, 0, 0], [255, 255, 255], [255, 0, 255],
        ] {
            let scale = AccentScale::new(rgb);
            for index in 0..10 {
                let a = scale.at(index);
                let b = scale.at(index + 1);
                assert!(
                    rgb_to_oklab([a.r(), a.g(), a.b()])[0] >
                    rgb_to_oklab([b.r(), b.g(), b.b()])[0],
                    "{rgb:?}, shade {index}"
                );
            }
            let token = ThemeTokens::new(rgb);
            assert!(contrast(token.accent, token.on_accent) >= 4.5);
            assert!(contrast(token.accent_subtle, token.on_accent_subtle) >= 4.5);
        }
    }

    #[test]
    fn text_roles_remain_readable_on_dark_surfaces() {
        let text = TextPalette::DEFAULT;
        for background in [SURFACE_DARK, SURFACE_VARIANT, INPUT_BACKGROUND] {
            for foreground in [
                text.primary, text.secondary, text.muted, text.disabled,
                text.placeholder, text.success, text.warning, text.danger,
                text.info, text.cyan, text.violet,
            ] {
                assert!(contrast(foreground, background) >= 4.5,
                    "{foreground:?} on {background:?}");
            }
        }
        assert!(contrast(text.primary, SURFACE_DARK) > contrast(text.secondary, SURFACE_DARK));
        assert!(contrast(text.secondary, SURFACE_DARK) > contrast(text.muted, SURFACE_DARK));
    }

    #[test]
    fn custom_text_changes_only_when_contrast_is_low() {
        let primary = TextPalette::DEFAULT.primary;
        assert_eq!(ensure_text_contrast(primary, SURFACE_DARK, primary), primary);
        assert!(contrast(ensure_text_contrast(SURFACE_DARK, SURFACE_DARK, primary), SURFACE_DARK) >= 7.0);
    }

    #[test]
    fn capsule_text_presets_have_strong_contrast_on_dark_surfaces() {
        assert_eq!(CAPSULE_TEXT_PRESETS[0].1, None);
        for (name, value) in CAPSULE_TEXT_PRESETS {
            let [r, g, b] = value.unwrap_or([255, 255, 255]);
            let color = Color32::from_rgb(r, g, b);
            for background in [SURFACE_DARK, SURFACE_VARIANT] {
                assert!(
                    contrast(color, background) >= 7.0,
                    "{name} ({r}, {g}, {b}) is not readable on {background:?}"
                );
            }
        }
    }

    #[test]
    fn small_accent_text_is_readable_for_extreme_seeds() {
        for r in [0, 128, 255] {
            for g in [0, 128, 255] {
                for b in [0, 128, 255] {
                    let color = readable_accent(Color32::from_rgb(r, g, b));
                    assert_eq!(color.a(), 255);
                    assert!(contrast(color, SURFACE_VARIANT) >= 7.0);
                }
            }
        }
    }
}
