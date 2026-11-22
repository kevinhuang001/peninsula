use super::{icons::Icons, settings::AppPreferences};
use crate::extension::{CapsuleView, Prompt};
use crate::ui::{declarative::MAX_CONTENT_WIDTH, renderer};
use eframe::egui;

pub(super) const CAPSULE_HEIGHT: f32 = 44.0;
pub(super) const CHIP_HEIGHT: f32 = 28.0;
pub(super) const ITEM_GAP: f32 = 6.0;
pub(super) const HORIZONTAL_PADDING: f32 = 16.0;

const MIN_WIDTH: f32 = 64.0;
const MAX_WIDTH: f32 = 600.0;

pub(super) struct CapsuleItem<'a> {
    pub view: &'a CapsuleView,
    preferred_width: f32,
    min_width: f32,
    pub exclusive: bool,
    pub prompt: Option<Prompt>,
}

impl<'a> From<&'a CapsuleView> for CapsuleItem<'a> {
    fn from(view: &'a CapsuleView) -> Self {
        Self {
            view,
            preferred_width: view.preferred_width,
            min_width: view.min_width,
            exclusive: false,
            prompt: None,
        }
    }
}

/// Resolve font-dependent widths using the same renderer as paint.
pub(super) fn measure_views<'a>(
    context: &egui::Context,
    available: impl IntoIterator<Item = &'a CapsuleView>,
    icons: &Icons,
    text_size: f32,
) -> Vec<CapsuleItem<'a>> {
    available
        .into_iter()
        .map(|view| {
            let mut item = CapsuleItem::from(view);
            if view.fit_width {
                // Include the chip insets and round up so fractional metrics
                // cannot introduce an ellipsis on the last glyph.
                let width = (renderer::capsule_content_width(
                    context,
                    &view.content,
                    MAX_CONTENT_WIDTH - 6.0,
                    icons,
                    text_size,
                ) + 6.0)
                    .ceil();
                item.preferred_width = width.max(item.min_width);
                item.min_width = item.preferred_width;
            }
            item
        })
        .collect()
}

pub(super) fn item_widths(
    preferences: &AppPreferences,
    available: &[CapsuleItem<'_>],
) -> Vec<f32> {
    if available.iter().any(|item| item.exclusive) {
        return available
            .iter()
            .filter(|item| item.exclusive)
            .map(|item| item.preferred_width)
            .collect();
    }
    let mut widths = available
        .iter()
        .filter(|v| {
            preferences.mascot_enabled && v.view.id.as_str() == crate::core::mascot::MASCOT_SLOT_ID
        })
        .map(|v| v.preferred_width)
        .collect::<Vec<_>>();
    for item in preferences.ordered_capsule_items() {
        if preferences.is_enabled(&item.id) {
            if let Some(view) = available.iter().find(|view| view.view.id == item.id) {
                widths.push(view.preferred_width);
            }
        }
    }
    widths
}

pub(super) fn width(preferences: &AppPreferences, available: &[CapsuleItem<'_>]) -> f32 {
    if let Some(prompt) = available.iter().find_map(|item| item.prompt.as_ref()) {
        return prompt.preferred_width.clamp(MIN_WIDTH, MAX_WIDTH);
    }
    let widths = item_widths(preferences, available);
    if widths.is_empty() {
        return MIN_WIDTH;
    }
    let required = widths.iter().sum::<f32>()
        + ITEM_GAP * widths.len().saturating_sub(1) as f32
        + HORIZONTAL_PADDING;
    let fits_content = available
        .iter()
        .any(|view| view.view.fit_width && preferences.is_enabled(&view.view.id));
    if fits_content {
        required.clamp(MIN_WIDTH, MAX_CONTENT_WIDTH)
    } else {
        required.clamp(MIN_WIDTH, MAX_WIDTH)
    }
}

pub(super) fn fitted_widths(views: &[&CapsuleItem<'_>], capsule_width: f32) -> Vec<f32> {
    let available =
        (capsule_width - HORIZONTAL_PADDING - ITEM_GAP * views.len().saturating_sub(1) as f32)
            .max(0.0);
    let preferred: f32 = views.iter().map(|view| view.preferred_width).sum();
    let minimum: f32 = views
        .iter()
        .map(|view| view.min_width.min(view.preferred_width))
        .sum();
    views
        .iter()
        .map(|view| {
            let floor = view.min_width.min(view.preferred_width);
            if available >= preferred {
                view.preferred_width
            } else if available >= minimum {
                floor
                    + (view.preferred_width - floor)
                        * ((available - minimum) / (preferred - minimum))
            } else {
                floor * (available / minimum)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        fitted_widths, item_widths, width, CapsuleItem, CAPSULE_HEIGHT, CHIP_HEIGHT,
        HORIZONTAL_PADDING, ITEM_GAP, MAX_WIDTH,
    };
    use crate::{
        app::settings::AppPreferences,
        builtins,
        extension::{CapsuleView, ContributionKind, ExtensionId, ViewNode},
    };

    fn available_views() -> Vec<CapsuleView> {
        builtins::registry()
            .by_kind(ContributionKind::Capsule)
            .into_iter()
            .map(|descriptor| CapsuleView {
                id: descriptor.id.clone(),
                source_extension_id: descriptor.id.clone(),
                content: ViewNode::text(String::new(), Default::default()),
                visibility: Default::default(),
                min_width: descriptor.size_hint[0],
                preferred_width: descriptor.size_hint[0],
                fit_width: false,
                priority: 0,
                tooltip: None,
            })
            .collect()
    }

    #[test]
    fn capsule_capsule_has_small_even_padding_around_its_contents() {
        let preferences = AppPreferences::default();
        let views = available_views();
        let widths = item_widths(
            &preferences,
            &views.iter().map(CapsuleItem::from).collect::<Vec<_>>(),
        );
        let content_width = widths.iter().sum::<f32>() + ITEM_GAP * (widths.len() - 1) as f32;
        assert_eq!(
            width(
                &preferences,
                &views.iter().map(CapsuleItem::from).collect::<Vec<_>>()
            ) - content_width,
            HORIZONTAL_PADDING
        );
        assert_eq!(HORIZONTAL_PADDING / 2.0, 8.0);
        assert_eq!(CAPSULE_HEIGHT - CHIP_HEIGHT, 16.0);
    }

    #[test]
    fn prompt_owns_the_whole_capsule_width_without_capsule_chip_padding() {
        let view = CapsuleView {
            id: ExtensionId::builtin("builtin.usb/prompt"),
            source_extension_id: ExtensionId::builtin("builtin.usb"),
            content: ViewNode::text("USB", Default::default()),
            visibility: Default::default(),
            min_width: 230.0,
            preferred_width: 266.0,
            fit_width: false,
            priority: 0,
            tooltip: None,
        };
        let mut item = CapsuleItem::from(&view);
        item.exclusive = true;
        item.prompt = Some(
            crate::extension::Prompt::new(
                view.source_extension_id.clone(),
                view.id.clone(),
                view.content.clone(),
                std::time::Duration::from_secs(4),
            )
            .with_width(230.0, 266.0),
        );
        assert_eq!(width(&AppPreferences::default(), &[item]), 266.0);
    }

    #[test]
    fn capsule_width_tracks_available_view_content_without_feature_checks() {
        let views: Vec<_> = available_views()
            .into_iter()
            .filter(|view| view.id.as_str() != "builtin.downloads/status")
            .collect();
        let mut preferences = AppPreferences::default();
        preferences.set_enabled(&ExtensionId::builtin("builtin.downloads/status"), true);
        preferences.set_enabled(
            &ExtensionId::builtin("builtin.system-monitor/network"),
            true,
        );
        let without_download = width(
            &preferences,
            &views.iter().map(CapsuleItem::from).collect::<Vec<_>>(),
        );
        let mut active_views = views.clone();
        active_views.push(CapsuleView {
            id: ExtensionId::builtin("builtin.downloads/status"),
            source_extension_id: ExtensionId::builtin("builtin.downloads/notifications"),
            content: ViewNode::text("download", Default::default()),
            visibility: Default::default(),
            min_width: 88.0,
            preferred_width: 100.0,
            fit_width: false,
            priority: 0,
            tooltip: None,
        });
        assert!(
            width(
                &preferences,
                &active_views
                    .iter()
                    .map(CapsuleItem::from)
                    .collect::<Vec<_>>()
            ) > without_download
        );
        for item in &mut preferences.capsule_items {
            item.enabled = false;
        }
        assert_eq!(
            width(
                &preferences,
                &active_views
                    .iter()
                    .map(CapsuleItem::from)
                    .collect::<Vec<_>>()
            ),
            64.0
        );
    }

    #[test]
    fn every_enabled_capsule_item_fits_inside_its_capsule() {
        let mut preferences = AppPreferences::default();
        for item in &mut preferences.capsule_items {
            item.enabled = true;
        }
        let views = available_views();
        let widths = item_widths(
            &preferences,
            &views.iter().map(CapsuleItem::from).collect::<Vec<_>>(),
        );
        let required =
            widths.iter().sum::<f32>() + ITEM_GAP * (widths.len() - 1) as f32 + HORIZONTAL_PADDING;
        assert_eq!(
            width(
                &preferences,
                &views.iter().map(CapsuleItem::from).collect::<Vec<_>>()
            ),
            required.min(MAX_WIDTH)
        );
        let items: Vec<_> = views.iter().map(CapsuleItem::from).collect();
        let references: Vec<_> = items.iter().collect();
        let fitted: f32 = fitted_widths(&references, MAX_WIDTH).into_iter().sum();
        assert!(
            fitted + ITEM_GAP * (views.len() - 1) as f32 + HORIZONTAL_PADDING <= MAX_WIDTH + 0.01
        );
    }

    #[test]
    fn capsule_item_order_follows_the_saved_preference_order() {
        let mut preferences = AppPreferences::default();
        let views = available_views();
        preferences.move_capsule_item(&ExtensionId::builtin("builtin.battery/status"), -1);
        assert_eq!(
            item_widths(
                &preferences,
                &views.iter().map(CapsuleItem::from).collect::<Vec<_>>()
            ),
            [
                64.0,
                88.0,
                68.0,
                #[cfg(any(target_os = "linux", target_os = "windows"))]
                176.0,
            ]
        );
    }
}

#[cfg(test)]
mod lyric_tests {
    use super::*;
    use crate::{extension::ExtensionId, ui::declarative::*};

    #[test]
    fn long_lyric_measures_with_fonts_and_exceeds_old_capsule_limit() {
        let context = egui::Context::default();
        super::super::fonts::install(&context);
        let icons = Icons::load(&context);
        // Exercise host layout with a Capsule ID registered on every platform;
        // the music extension itself is intentionally unavailable on macOS.
        let id = ExtensionId::builtin("builtin.calendar/time");
        let view = CapsuleView {
            id: id.clone(),
            source_extension_id: id.clone(),
            content: ViewNode::capsule(
                ViewNode::icon("speaker-playing", 16.0),
                ViewNode::text(
                    "♪把这一整句长长的歌词完整地显示出来，直到最后一个音符也不会省略♪".repeat(3),
                    TextStyle {
                        font_size: FontSize::Custom(11.0),
                        ..Default::default()
                    },
                ),
                CapsuleLayout {
                    leading_size: [16.0, 20.0],
                    ..Default::default()
                },
            ),
            visibility: Default::default(),
            min_width: 360.0,
            preferred_width: 360.0,
            fit_width: true,
            priority: 0,
            tooltip: None,
        };
        let mut preferences = AppPreferences::default();
        preferences.set_enabled(&id, true);
        let _ = context.run(egui::RawInput::default(), |context| {
            let measured = measure_views(context, [&view], &icons, preferences.capsule_text_size);
            let references = [&measured[0]];
            let capsule = width(&preferences, &measured);
            assert!(capsule > 600.0);
            assert_eq!(
                fitted_widths(&references, capsule)[0],
                measured[0].preferred_width
            );
            let natural = renderer::content_width(context, &view.content, 16_384.0, &icons);
            assert!(measured[0].preferred_width >= natural + 6.0);
            let large = measure_views(context, [&view], &icons, 18.0);
            assert!(large[0].preferred_width > measured[0].preferred_width);
            let mut short = view.clone();
            let Node::Capsule(capsule) = &mut short.content.kind else {
                unreachable!("lyric fixture is a Capsule node");
            };
            capsule.content = ViewNode::text("♪短句♪", Default::default());
            let short = measure_views(context, [&short], &icons, preferences.capsule_text_size);
            assert_eq!(short[0].preferred_width, 360.0);
            assert_eq!(short[0].min_width, 360.0);
            assert_eq!(width(&preferences, &short), 360.0 + HORIZONTAL_PADDING);
        });
    }
}
