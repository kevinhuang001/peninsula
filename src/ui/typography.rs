//! Shared relative typography for native egui and declarative extensions.
//! The OS handles physical pixel density; only the monitor size in logical points
//! is used for small, bounded responsive adjustments.
use crate::ui::declarative::FontSize;
use eframe::egui::{self, FontFamily, FontId};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextScale {
    Small,
    #[default]
    Standard,
    Large,
    ExtraLarge,
}

impl TextScale {
    pub const ALL: [Self; 4] = [Self::Small, Self::Standard, Self::Large, Self::ExtraLarge];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Standard => "standard",
            Self::Large => "large",
            Self::ExtraLarge => "extra_large",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Small => "较小",
            Self::Standard => "标准",
            Self::Large => "较大",
            Self::ExtraLarge => "特大",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input.trim() {
            "small" => Some(Self::Small),
            "standard" => Some(Self::Standard),
            "large" => Some(Self::Large),
            "extra_large" => Some(Self::ExtraLarge),
            _ => None,
        }
    }

    pub const fn factor(self) -> f32 {
        match self {
            Self::Small => 0.90,
            Self::Standard => 1.0,
            Self::Large => 1.12,
            Self::ExtraLarge => 1.25,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct InstalledTypography {
    preset: TextScale,
    factor: f32,
}

fn store_id() -> egui::Id {
    egui::Id::new("peninsula-typography")
}

/// A modest response to usable desktop width, not raw 1080p/4K pixel counts.
pub(crate) fn monitor_factor(width_in_points: f32) -> f32 {
    if !width_in_points.is_finite() || width_in_points <= 0.0 {
        return 1.0;
    }
    (width_in_points / 1440.0).sqrt().clamp(0.95, 1.12)
}

pub(crate) struct Typography;
impl Typography {
    pub const SMALL: f32 = 10.0;
    pub const CAPTION: f32 = 10.5;
    pub const FORM_HINT: f32 = 11.0;
    pub const FORM_LABEL: f32 = 11.5;
    pub const BODY: f32 = 12.0;
    pub const SETTINGS_HEADING: f32 = 12.5;
    pub const TITLE: f32 = 14.0;
    pub const DIALOG_TITLE: f32 = 17.0;
    pub const HEADING: f32 = 18.0;
    pub const CAPSULE_DEFAULT: f32 = 11.0;
    pub const CAPSULE_MIN: f32 = 8.0;
    pub const CAPSULE_MAX: f32 = 18.0;

    /// Keep the per-viewport font atlas and layout metrics in sync when the
    /// preference changes or the window moves between monitors.
    pub fn install(ctx: &egui::Context, preset: TextScale) {
        let width = ctx.input(|input| {
            input.viewport().monitor_size.map(|size| size.x).unwrap_or(1440.0)
        });
        let factor = preset.factor() * monitor_factor(width);
        let previous = ctx.data(|data| data.get_temp::<InstalledTypography>(store_id()));
        if previous.is_some_and(|installed| {
            installed.preset == preset && (installed.factor - factor).abs() < 0.001
        }) {
            return;
        }
        ctx.data_mut(|data| data.insert_temp(store_id(), InstalledTypography { preset, factor }));
        ctx.all_styles_mut(|style| {
            for (role, size) in [
                (egui::TextStyle::Small, Self::SMALL),
                (egui::TextStyle::Body, Self::BODY),
                (egui::TextStyle::Button, Self::BODY),
                (egui::TextStyle::Heading, Self::HEADING),
            ] {
                style.text_styles.insert(role, FontId::new(size * factor, FontFamily::Proportional));
            }
            style.text_styles.insert(
                egui::TextStyle::Monospace,
                FontId::new(Self::BODY * factor, FontFamily::Monospace),
            );
        });
        ctx.request_repaint();
    }

    pub fn factor(ctx: &egui::Context) -> f32 {
        ctx.data(|data| data.get_temp::<InstalledTypography>(store_id()))
            .map_or(1.0, |state| state.factor)
    }

    pub fn scaled(ctx: &egui::Context, baseline: f32) -> f32 {
        (baseline * Self::factor(ctx)).clamp(1.0, 256.0)
    }

    pub fn font_id(ctx: &egui::Context, baseline: f32) -> FontId {
        FontId::proportional(Self::scaled(ctx, baseline))
    }

    pub fn size(ctx: &egui::Context, value: FontSize) -> f32 {
        let baseline = match value {
            FontSize::Small => Self::SMALL,
            FontSize::Body => Self::BODY,
            FontSize::Title => Self::TITLE,
            FontSize::Heading => Self::HEADING,
            // 1.0 is Body; callers declare a proportion, not a point size.
            FontSize::Relative(scale) if scale.is_finite() => Self::BODY * scale.clamp(0.5, 3.0),
            FontSize::Relative(_) => Self::BODY,
        };
        Self::scaled(ctx, baseline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_scaling_uses_logical_size_and_is_bounded() {
        assert_eq!(monitor_factor(1440.0), 1.0);
        assert_eq!(monitor_factor(600.0), 0.95);
        assert_eq!(monitor_factor(3840.0), 1.12);
        assert_eq!(monitor_factor(f32::NAN), 1.0);
        assert!(monitor_factor(1920.0) > monitor_factor(1440.0));
    }

    #[test]
    fn every_preset_roundtrips_and_scales_monotonically() {
        let scales = TextScale::ALL;
        for scale in scales {
            assert_eq!(TextScale::parse(scale.key()), Some(scale));
        }
        assert!(scales.windows(2).all(|pair| pair[0].factor() < pair[1].factor()));
    }

    #[test]
    fn relative_font_sizes_use_body_as_the_unit() {
        let ctx = egui::Context::default();
        assert_eq!(Typography::size(&ctx, FontSize::Relative(1.0)), Typography::BODY);
        assert_eq!(Typography::size(&ctx, FontSize::Relative(1.5)), Typography::BODY * 1.5);
    }

    #[test]
    fn dpi_changes_pixel_density_not_the_user_selected_logical_font_size() {
        for dpi in [1.0, 1.25, 1.5, 2.0, 3.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(dpi);
            Typography::install(&ctx, TextScale::Large);
            let expected = Typography::BODY * TextScale::Large.factor();
            assert!((Typography::size(&ctx, FontSize::Body) - expected).abs() < 0.01);
        }
    }

    #[test]
    fn user_preference_scales_native_and_declarative_text_together() {
        let ctx = egui::Context::default();
        Typography::install(&ctx, TextScale::Large);
        let expected = Typography::BODY * TextScale::Large.factor();
        assert!((Typography::size(&ctx, FontSize::Body) - expected).abs() < 0.01);
        let native = ctx.style().text_styles.get(&egui::TextStyle::Body).unwrap().size;
        assert!((native - expected).abs() < 0.01);
    }
}
