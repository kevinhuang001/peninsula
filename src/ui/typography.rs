//! Shared semantic font metrics, independent from accent and text colors.
use crate::ui::declarative::FontSize;
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
    pub fn size(value: FontSize) -> f32 {
        match value {
            FontSize::Small => Self::SMALL,
            FontSize::Body => Self::BODY,
            FontSize::Title => Self::TITLE,
            FontSize::Heading => Self::HEADING,
            FontSize::Custom(size) if size.is_finite() => size.clamp(1.0, 256.0),
            FontSize::Custom(_) => Self::BODY,
        }
    }
    pub fn capsule_navigation(screen_width: f32) -> f32 {
        let width = if screen_width.is_finite() { screen_width.max(1.0) } else { 1440.0 };
        Self::SMALL * (width / 1440.0).sqrt().clamp(0.9, 1.2)
    }
}
