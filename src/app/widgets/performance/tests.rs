use super::{expanded_width, paint_metrics};
use crate::metrics::Snapshot;
use eframe::egui::{self, Pos2, Rect, Vec2};

#[test]
fn expanded_metrics_fit_inside_the_capsule() {
    let context = egui::Context::default();
    let mut metrics_rect = Rect::NOTHING;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 204.0))),
        ..Default::default()
    };
    let _ = context.run(input, |context| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(context, |ui| {
                let capsule = Rect::from_min_size(Pos2::new(15.0, 7.0), Vec2::new(330.0, 140.0));
                let icons = crate::app::icons::Icons::load(context);
                metrics_rect = paint_metrics(ui, capsule, &Snapshot::default(), 1.0, &icons);
                assert!(metrics_rect.left() >= capsule.left());
                assert!(metrics_rect.right() <= capsule.right());
                assert!(metrics_rect.bottom() < capsule.bottom());
            });
    });
    assert!(metrics_rect.width() > 240.0);
    assert!(metrics_rect.height() > 60.0);
}

#[test]
fn expanded_width_stays_stable_as_live_network_rates_change() {
    let context = egui::Context::default();
    let mut widths = (0.0, 0.0);
    let _ = context.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 204.0))),
            ..Default::default()
        },
        |context| {
            egui::CentralPanel::default().show(context, |_ui| {
                let compact = Snapshot::default();
                let busy = Snapshot {
                    download_bytes_per_second: 8_000_000.0,
                    upload_bytes_per_second: 3_000_000.0,
                    ..Snapshot::default()
                };
                widths = (
                    expanded_width(context, &compact),
                    expanded_width(context, &busy),
                );
            });
        },
    );
    assert_eq!(widths.1, widths.0, "widths: {widths:?}");
    assert!(widths.1 <= 360.0, "capsule should stay compact: {widths:?}");
}

#[test]
fn network_directions_have_equal_typography_and_distinct_colors() {
    assert_eq!(super::NETWORK_SPEED_SIZE, super::NETWORK_UPLOAD_SIZE);
    assert_ne!(super::BLUE, super::UPLOAD);
    let cards = super::card_models(&Snapshot::default());
    assert!(cards[0].subtitle.is_empty());
    assert!(cards[1].subtitle.is_empty());
    assert!(cards[3].subtitle.is_empty());
}

#[test]
fn charging_battery_is_included_in_expanded_metrics() {
    let cards = super::card_models(&Snapshot {
        charging: true,
        battery_percent: Some(42),
        ..Snapshot::default()
    });
    assert!(cards[3].battery);
    assert!(cards[3].subtitle.is_empty());
}
