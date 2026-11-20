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
    assert!((Duration::from_secs(3)..=Duration::from_secs(5)).contains(&(start - now)));
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

#[test]
fn compact_slot_is_fixed_and_passive() {
    let view = MascotHost::default().compact_slot();
    assert_eq!(view.min_width, 38.0);
    assert_eq!(view.preferred_width, view.min_width);
    assert!(!view.fit_width);
    assert!(view.tooltip.is_none());
    assert!(matches!(view.content.kind, Node::CoreMascot(_)));
}

#[test]
fn idle_distribution_matches_weights_without_blink_or_adjacent_repeats() {
    use MascotMotion::*;
    let weights = [
        (LookAround, 30),
        (Happy, 20),
        (Thinking, 15),
        (Sleepy, 15),
        (Surprised, 10),
        (Love, 7),
        (Excited, 3),
    ];
    let mut scheduler = scheduler::Scheduler::default();
    let mut rng = StdRng::seed_from_u64(526);
    let mut counts = [0; 7];
    let mut last = None;
    for _ in 0..50_000 {
        let motion = scheduler.choose(&[], &mut rng).motion;
        assert_ne!(Some(motion), last);
        last = Some(motion);
        counts[weights
            .iter()
            .position(|&(candidate, _)| candidate == motion)
            .unwrap()] += 1;
    }
    for ((motion, weight), count) in weights.into_iter().zip(counts) {
        assert!(
            (count as f64 / 500.0 - f64::from(weight)).abs() < 0.65,
            "{motion:?}: {count}"
        );
    }
}

#[test]
fn blink_and_full_expression_deadlines_are_independent() {
    let now = Instant::now();
    let mut host = MascotHost::default();
    host.update(&[], true, now, MotionPolicy::Full);
    let first_blink = host.idle_clock.blink_at.unwrap();
    assert!((Duration::from_secs(3)..=Duration::from_secs(7)).contains(&(first_blink - now)));
    let first_action = host.idle_at.unwrap();
    host.update(&[], true, first_action, MotionPolicy::Full);
    let playback = host.playback.as_ref().unwrap();
    let end = first_action + playback.action.duration;
    let next_action = host.idle_at.unwrap();
    assert!((Duration::from_secs(8)..=Duration::from_secs(18)).contains(&(next_action - end)));
    let blink = end + Duration::from_secs(1);
    host.idle_clock.blink_at = Some(blink);
    host.update(
        &[],
        true,
        blink + Duration::from_millis(95),
        MotionPolicy::Full,
    );
    assert!(host.pose.eyes[0] < 0.1);
    assert_eq!(host.idle_at, Some(next_action));
    assert!(host.playback.is_none());
}

#[test]
fn hiding_a_playing_expression_pauses_it_without_restarting() {
    let mut host = MascotHost::default();
    let now = Instant::now();
    let sources = events();
    host.event("a", &sources, now);
    host.update(&sources, true, now, MotionPolicy::Full);
    let hidden = now + Duration::from_millis(260);
    host.update(&sources, true, hidden, MotionPolicy::Full);
    let before = host.pose;
    assert_eq!(
        host.update(&sources, false, hidden, MotionPolicy::Full),
        None
    );
    host.update(
        &sources,
        true,
        hidden + Duration::from_secs(5),
        MotionPolicy::Full,
    );
    assert_eq!(host.pose, before);
    assert_eq!(host.playback.unwrap().start, now + Duration::from_secs(5));
}

#[test]
fn music_pause_removes_listening_from_the_candidates() {
    let mut scheduler = scheduler::Scheduler::default();
    let mut rng = StdRng::seed_from_u64(46);
    let mut source = state("music");
    assert!((0..30).any(|_| scheduler
        .choose(std::slice::from_ref(&source), &mut rng)
        .motion
        == MascotMotion::Listening));
    source.2.clear();
    assert!((0..30).all(|_| scheduler
        .choose(std::slice::from_ref(&source), &mut rng)
        .motion
        != MascotMotion::Listening));
}
