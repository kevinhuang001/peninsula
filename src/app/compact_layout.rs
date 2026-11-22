use super::{icons::Icons, settings::CompactPreferences};
use crate::extension::CompactView;
use crate::ui::{declarative::MAX_CONTENT_WIDTH, renderer};
use eframe::egui;

pub(super) const CAPSULE_HEIGHT: f32 = 44.0;
pub(super) const CHIP_HEIGHT: f32 = 28.0;
pub(super) const ITEM_GAP: f32 = 6.0;
pub(super) const HORIZONTAL_PADDING: f32 = 16.0;

const MIN_WIDTH: f32 = 64.0;
const MAX_WIDTH: f32 = 600.0;

/// Resolve font-dependent widths in the host, using the same renderer as paint.
pub(super) fn measure_views(
    context: &egui::Context,
    available: &[&CompactView],
    icons: &Icons,
) -> Vec<CompactView> {
    available
        .iter()
        .map(|view| {
            let mut view = (*view).clone();
            if view.fit_width {
                // Include both three-point chip insets and round up to avoid
                // fractional font metrics causing an ellipsis on the last glyph.
                let width = (renderer::content_width(
                    context,
                    &view.content,
                    MAX_CONTENT_WIDTH - 6.0,
                    icons,
                ) + 6.0)
                    .ceil();
                view.preferred_width = width.max(view.min_width);
                view.min_width = view.preferred_width;
            }
            view
        })
        .collect()
}

pub(super) fn item_widths(
    preferences: &CompactPreferences,
    available: &[&CompactView],
) -> Vec<f32> {
    let mut widths = Vec::new();
    for item in preferences.ordered_compact_items() {
        if preferences.is_enabled(&item.id) {
            if let Some(view) = available.iter().find(|view| view.id == item.id) {
                widths.push(view.preferred_width);
            }
        }
    }
    widths
}

pub(super) fn width(preferences: &CompactPreferences, available: &[&CompactView]) -> f32 {
    let widths = item_widths(preferences, available);
    if widths.is_empty() {
        return MIN_WIDTH;
    }
    let required = widths.iter().sum::<f32>()
        + ITEM_GAP * widths.len().saturating_sub(1) as f32
        + HORIZONTAL_PADDING;
    let fits_content = available
        .iter()
        .any(|view| view.fit_width && preferences.is_enabled(&view.id));
    if fits_content {
        required.clamp(MIN_WIDTH, MAX_CONTENT_WIDTH)
    } else {
        required.clamp(MIN_WIDTH, MAX_WIDTH)
    }
}

pub(super) fn fitted_widths(views: &[&CompactView], capsule_width: f32) -> Vec<f32> {
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
        fitted_widths, item_widths, width, CAPSULE_HEIGHT, CHIP_HEIGHT, HORIZONTAL_PADDING,
        ITEM_GAP, MAX_WIDTH,
    };
    use crate::{
        app::settings::CompactPreferences,
        builtins,
        extension::{CompactView, ExtensionId, ExtensionKind, ViewNode},
    };

    fn available_views() -> Vec<CompactView> {
        builtins::registry()
            .by_kind(ExtensionKind::Compact)
            .into_iter()
            .map(|descriptor| CompactView {
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
    fn compact_capsule_has_small_even_padding_around_its_contents() {
        let preferences = CompactPreferences::default();
        let views = available_views();
        let widths = item_widths(&preferences, &views.iter().collect::<Vec<_>>());
        let content_width = widths.iter().sum::<f32>() + ITEM_GAP * (widths.len() - 1) as f32;
        assert_eq!(
            width(&preferences, &views.iter().collect::<Vec<_>>()) - content_width,
            HORIZONTAL_PADDING
        );
        assert_eq!(HORIZONTAL_PADDING / 2.0, 8.0);
        assert_eq!(CAPSULE_HEIGHT - CHIP_HEIGHT, 16.0);
    }

    #[test]
    fn compact_width_tracks_available_view_content_without_feature_checks() {
        let views: Vec<_> = available_views()
            .into_iter()
            .filter(|view| view.id.as_str() != "builtin.downloads/status")
            .collect();
        let mut preferences = CompactPreferences::default();
        preferences.set_enabled(&ExtensionId::builtin("builtin.downloads/status"), true);
        preferences.set_enabled(
            &ExtensionId::builtin("builtin.system-monitor/network"),
            true,
        );
        let without_download = width(&preferences, &views.iter().collect::<Vec<_>>());
        let mut active_views = views.clone();
        active_views.push(CompactView {
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
        assert!(width(&preferences, &active_views.iter().collect::<Vec<_>>()) > without_download);
        for item in &mut preferences.compact_items {
            item.enabled = false;
        }
        assert_eq!(
            width(&preferences, &active_views.iter().collect::<Vec<_>>()),
            64.0
        );
    }

    #[test]
    fn every_enabled_compact_item_fits_inside_its_capsule() {
        let mut preferences = CompactPreferences::default();
        for item in &mut preferences.compact_items {
            item.enabled = true;
        }
        let views = available_views();
        let widths = item_widths(&preferences, &views.iter().collect::<Vec<_>>());
        let required =
            widths.iter().sum::<f32>() + ITEM_GAP * (widths.len() - 1) as f32 + HORIZONTAL_PADDING;
        assert_eq!(
            width(&preferences, &views.iter().collect::<Vec<_>>()),
            required.min(MAX_WIDTH)
        );
        let references: Vec<_> = views.iter().collect();
        let fitted: f32 = fitted_widths(&references, MAX_WIDTH).into_iter().sum();
        assert!(
            fitted + ITEM_GAP * (views.len() - 1) as f32 + HORIZONTAL_PADDING <= MAX_WIDTH + 0.01
        );
    }

    #[test]
    fn compact_item_order_follows_the_saved_preference_order() {
        let mut preferences = CompactPreferences::default();
        let views = available_views();
        preferences.move_compact_item(&ExtensionId::builtin("builtin.system-monitor/battery"), -1);
        assert_eq!(
            item_widths(&preferences, &views.iter().collect::<Vec<_>>()),
            [
                54.0,
                38.0,
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
        let id = ExtensionId::builtin("builtin.music/lyrics");
        let view = CompactView {
            id: id.clone(),
            source_extension_id: id.clone(),
            content: ViewNode::row(
                vec![
                    ViewNode::icon("speaker-playing", 16.0),
                    ViewNode::text(
                        "♪把这一整句长长的歌词完整地显示出来，直到最后一个音符也不会省略♪"
                            .repeat(3),
                        TextStyle {
                            font_size: FontSize::Custom(11.0),
                            ..Default::default()
                        },
                    ),
                ],
                Flow {
                    gap: Some(4.0),
                    ..Default::default()
                },
            ),
            visibility: Default::default(),
            min_width: 84.0,
            preferred_width: 176.0,
            fit_width: true,
            priority: 0,
            tooltip: None,
        };
        let preferences = CompactPreferences::default();
        let _ = context.run(egui::RawInput::default(), |context| {
            let measured = measure_views(context, &[&view], &icons);
            let references = [&measured[0]];
            let capsule = width(&preferences, &references);
            assert!(capsule > 600.0);
            assert_eq!(
                fitted_widths(&references, capsule)[0],
                measured[0].preferred_width
            );
            let natural = renderer::content_width(context, &view.content, 16_384.0, &icons);
            assert!(measured[0].preferred_width >= natural + 6.0);
            let mut short = view.clone();
            short.content = ViewNode::text("♪短句♪", Default::default());
            assert!(
                measure_views(context, &[&short], &icons)[0].preferred_width
                    < measured[0].preferred_width
            );
        });
    }
}
