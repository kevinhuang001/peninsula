use super::*;
use rand::{rngs::StdRng, SeedableRng};

fn state(id: &str) -> MascotSource {
    (
        ExtensionId::new(id).unwrap(),
        vec![MascotExpression::state(
            id,
            "playing",
            "yes",
            MascotMotion::Listening,
        )],
        [("playing".into(), "yes".into())].into(),
    )
}

fn events() -> Vec<MascotSource> {
    vec![(
        ExtensionId::builtin("test"),
        vec![
            MascotExpression::event("a", "a", MascotMotion::Happy),
            MascotExpression::event("b", "b", MascotMotion::Celebrate),
        ],
        BTreeMap::new(),
    )]
}

#[test]
fn category_ratio_is_independent_of_extension_count() {
    for (count, expected) in [(0, 0.0_f64), (1, 0.75), (20, 0.75)] {
        let mut rng = StdRng::seed_from_u64(345);
        let mut scheduler = scheduler::Scheduler::default();
        let sources: Vec<_> = (0..count).map(|i| state(&format!("test-{i}"))).collect();
        let registered = (0..10_000)
            .filter(|_| scheduler.choose(&sources, &mut rng).registration.is_some())
            .count();
        assert!((registered as f64 / 10_000.0 - expected).abs() < 0.02);
    }
}

#[test]
fn event_queue_coalesces_bursts_and_preserves_replays_and_order() {
    let now = Instant::now();
    let sources = events();
    let mut host = MascotHost::default();
    host.event("a", &sources, now);
    host.event("a", &sources, now + Duration::from_millis(50));
    host.event("b", &sources, now + Duration::from_millis(100));
    host.event("a", &sources, now + Duration::from_millis(250));
    host.update(
        &sources,
        false,
        now + Duration::from_secs(1),
        MotionPolicy::Full,
    );
    for (index, motion) in [
        MascotMotion::Happy,
        MascotMotion::Celebrate,
        MascotMotion::Happy,
    ]
    .into_iter()
    .enumerate()
    {
        let start = now + Duration::from_secs(2 + 2 * index as u64);
        host.update(&sources, true, start, MotionPolicy::Full);
        let playback = host.playback.as_ref().unwrap();
        assert_eq!(playback.action.motion, motion);
        assert_eq!(playback.start, start);
    }
    host.update(
        &sources,
        true,
        now + Duration::from_secs(7),
        MotionPolicy::Full,
    );
    assert!(host.playback.is_none());
}

#[test]
fn disabled_owners_and_expired_events_are_removed_even_when_hidden() {
    let now = Instant::now();
    let sources = events();
    let mut host = MascotHost::default();
    host.event("a", &sources, now);
    host.update(&[], false, now, MotionPolicy::Full);
    host.event("b", &sources, now);
    host.update(
        &sources,
        false,
        now + Duration::from_secs(16),
        MotionPolicy::Full,
    );
    host.update(
        &sources,
        true,
        now + Duration::from_secs(17),
        MotionPolicy::Full,
    );
    assert!(host.playback.is_none());
}

#[test]
fn idle_autostarts_and_event_interruption_preserves_current_pose() {
    let now = Instant::now();
    let mut host = MascotHost::default();
    let sources = events();
    host.update(&sources, true, now, MotionPolicy::Full);
    let start = host.idle_at.unwrap();
    assert!((Duration::from_secs(4)..=Duration::from_secs(9)).contains(&(start - now)));
    host.update(&sources, true, start, MotionPolicy::Full);
    assert!(host.playback.is_some());
    let interrupt = start + Duration::from_millis(100);
    host.update(&sources, true, interrupt, MotionPolicy::Full);
    let pose = host.pose;
    host.event("a", &sources, interrupt);
    host.update(&sources, true, interrupt, MotionPolicy::Full);
    assert_eq!(host.pose, pose);
    assert!(host.playback.unwrap().action.event);
}

#[test]
fn off_keeps_static_unlock_feedback_without_continuous_repaint() {
    let now = Instant::now();
    let mut host = MascotHost::default();
    assert_eq!(
        host.update_unlock(UnlockMascotState::Failed, now, MotionPolicy::Off),
        None
    );
    let error = host.pose;
    assert!(error.brow > 0.5);
    host.update_unlock(
        UnlockMascotState::Failed,
        now + Duration::from_millis(800),
        MotionPolicy::Off,
    );
    assert_eq!(host.pose, error);
    host.update_unlock(UnlockMascotState::Succeeded, now, MotionPolicy::Off);
    assert!(host.pose.smile > 0.8);
}

#[test]
fn repeat_during_the_merge_window_does_not_requeue_a_playing_event() {
    let now = Instant::now();
    let sources = events();
    let mut host = MascotHost::default();
    host.event("a", &sources, now);
    host.update(&sources, true, now, MotionPolicy::Full);
    host.event("a", &sources, now + Duration::from_millis(100));
    host.update(
        &sources,
        true,
        now + Duration::from_secs(1),
        MotionPolicy::Full,
    );
    assert!(host.playback.is_none());
}

#[test]
fn every_procedural_motion_fits_both_small_and_large_slots() {
    use eframe::egui::{self, pos2, vec2};
    use MascotMotion::*;
    let motions = [
        Idle, Blink, LookAround, Sleepy, Happy, Excited, Thinking, Surprised, Love, Celebrate,
        Listening, Focused, Charging, Typing, Verifying, Error, Success,
    ];
    let ctx = egui::Context::default();
    for size in [vec2(18.0, 38.0), vec2(38.0, 38.0), vec2(64.0, 48.0)] {
        let rect = egui::Rect::from_min_size(pos2(20.0, 20.0), size);
        for motion in motions {
            for step in 0..=16 {
                let phase = step as f32 / 16.0;
                let pose = animation::sample(motion, phase, 10.0 * phase, 0.0, MotionPolicy::Full);
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    let painter = ctx.layer_painter(egui::LayerId::background());
                    paint(&painter, rect, pose, true);
                });
                assert!(!output.shapes.is_empty(), "{motion:?} emitted no shapes");
                for entry in output.shapes {
                    let bounds = entry.shape.visual_bounding_rect();
                    assert!(
                        rect.contains_rect(bounds),
                        "{motion:?} at phase {phase}: {bounds:?} exceeds {rect:?}",
                    );
                }
            }
        }
    }
}
