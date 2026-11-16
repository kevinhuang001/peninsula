use super::settings::{CompactPreferences, OptionItem};

pub(super) const CAPSULE_HEIGHT: f32 = 44.0;
pub(super) const CHIP_HEIGHT: f32 = 28.0;
pub(super) const ITEM_GAP: f32 = 6.0;
pub(super) const HORIZONTAL_PADDING: f32 = 16.0;
pub(super) const MASCOT_SIZE: [f32; 2] = [38.0, 28.0];
pub(super) const COMPACT_ICON_SIZE: f32 = 16.0;
pub(super) const COMPACT_TEXT_SIZE: f32 = 10.0;
pub(super) const COMPACT_ICON_GAP: f32 = 4.0;

const MIN_WIDTH: f32 = 64.0;
const MAX_WIDTH: f32 = 480.0;

pub(super) fn item_widths(preferences: &CompactPreferences, downloading: bool) -> Vec<f32> {
    let mut widths = Vec::new();
    for item in preferences.ordered_compact_items() {
        if !item.enabled(preferences) || (item == OptionItem::Downloads && !downloading) {
            continue;
        }
        widths.push(item_width(item));
    }
    widths
}

pub(super) fn item_width(item: OptionItem) -> f32 {
    match item {
        OptionItem::Mascot => MASCOT_SIZE[0],
        OptionItem::Battery => 54.0,
        OptionItem::Time => 56.0,
        OptionItem::Cpu | OptionItem::Gpu => 56.0,
        OptionItem::Network => 80.0,
        OptionItem::Downloads => 88.0,
    }
}

pub(super) fn width(preferences: &CompactPreferences, downloading: bool) -> f32 {
    let widths = item_widths(preferences, downloading);
    if widths.is_empty() {
        return MIN_WIDTH;
    }
    (widths.iter().sum::<f32>()
        + ITEM_GAP * widths.len().saturating_sub(1) as f32
        + HORIZONTAL_PADDING)
        .clamp(MIN_WIDTH, MAX_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::{item_widths, width, CAPSULE_HEIGHT, CHIP_HEIGHT, HORIZONTAL_PADDING, ITEM_GAP};
    use crate::app::settings::{CompactPreferences, OptionItem};

    #[test]
    fn compact_capsule_has_small_even_padding_around_its_contents() {
        let preferences = CompactPreferences::default();
        let widths = item_widths(&preferences, false);
        let content_width = widths.iter().sum::<f32>() + ITEM_GAP * (widths.len() - 1) as f32;
        assert_eq!(
            width(&preferences, false) - content_width,
            HORIZONTAL_PADDING
        );
        assert_eq!(HORIZONTAL_PADDING / 2.0, 8.0);
        assert_eq!(CAPSULE_HEIGHT - CHIP_HEIGHT, 16.0);
    }

    #[test]
    fn compact_width_tracks_content_and_only_adds_download_space_when_active() {
        let preferences = CompactPreferences {
            downloads: true,
            network: true,
            ..CompactPreferences::default()
        };
        assert!(width(&preferences, true) > width(&preferences, false));
        assert_eq!(
            width(
                &CompactPreferences {
                    cpu: false,
                    battery: false,
                    mascot: false,
                    gpu: false,
                    network: false,
                    downloads: false,
                    time: false,
                    compact_order: std::array::from_fn(|index| index as u8),
                    quiet_hours: crate::app::settings::quiet_hours::QuietHours::default(),
                },
                false
            ),
            64.0
        );
    }

    #[test]
    fn every_enabled_compact_item_fits_inside_its_capsule() {
        let preferences = CompactPreferences {
            mascot: true,
            battery: true,
            cpu: true,
            gpu: true,
            network: true,
            downloads: true,
            time: true,
            compact_order: std::array::from_fn(|index| index as u8),
            quiet_hours: crate::app::settings::quiet_hours::QuietHours::default(),
        };
        let widths = item_widths(&preferences, true);
        let required =
            widths.iter().sum::<f32>() + ITEM_GAP * (widths.len() - 1) as f32 + HORIZONTAL_PADDING;
        assert_eq!(width(&preferences, true), required);
    }

    #[test]
    fn compact_item_order_follows_the_saved_preference_order() {
        let mut preferences = CompactPreferences {
            battery: true,
            ..CompactPreferences::default()
        };
        preferences.move_compact_item(OptionItem::Battery, -1);
        assert_eq!(item_widths(&preferences, false), [54.0, 38.0].to_vec());
    }
}
