
use super::{should_start_drag, window_target_size, COLLAPSED_SIZE, SETTINGS_SIZE};
use crate::app::state::window_left_for_center;
use eframe::egui::{pos2, vec2, Rect};

#[test]
fn window_modes_keep_settings_separate_from_the_capsule_canvas() {
    assert_eq!(COLLAPSED_SIZE, [420.0, 190.0]);
    assert_eq!(SETTINGS_SIZE, [760.0, 520.0]);
    assert_eq!(window_left_for_center(715.0, SETTINGS_SIZE[0]), 335.0);
}

#[test]
fn capsule_drag_starts_only_for_a_held_pointer_moving_inside_it() {
    let capsule = Rect::from_min_size(pos2(10.0, 10.0), vec2(200.0, 60.0));
    assert!(should_start_drag(
        Some(capsule),
        Some(pos2(30.0, 30.0)),
        true,
        vec2(2.0, 0.0)
    ));
    assert!(!should_start_drag(
        Some(capsule),
        Some(pos2(30.0, 30.0)),
        false,
        vec2(2.0, 0.0)
    ));
    assert!(!should_start_drag(
        Some(capsule),
        Some(pos2(250.0, 30.0)),
        true,
        vec2(2.0, 0.0)
    ));
}

#[test]
fn native_window_stays_expanded_until_the_close_animation_settles() {
    assert_eq!(
        window_target_size(false, false, true, 148.0),
        COLLAPSED_SIZE
    );
    assert_eq!(
        window_target_size(false, false, false, 148.0),
        COLLAPSED_SIZE
    );
}
