use crate::extension::ExtensionId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Direction {
    Previous,
    Next,
}

impl Direction {
    pub(super) fn from_swipe(delta_x: f32) -> Self {
        // Dragging the current page left reveals the next page on its right.
        if delta_x < 0.0 {
            Self::Next
        } else {
            Self::Previous
        }
    }

    fn offset(self) -> isize {
        match self {
            Self::Previous => -1,
            Self::Next => 1,
        }
    }
}

pub(super) fn key_direction(input: &eframe::egui::InputState) -> Option<Direction> {
    use eframe::egui::Key;
    match (
        input.key_pressed(Key::ArrowLeft),
        input.key_pressed(Key::ArrowRight),
    ) {
        (true, false) => Some(Direction::Previous),
        (false, true) => Some(Direction::Next),
        _ => None,
    }
}

pub fn default_widget_id() -> ExtensionId {
    ExtensionId::builtin(crate::extension::NOTIFICATION_CENTER_ID)
}

pub(super) fn next_widget(
    current: &ExtensionId,
    ids: &[ExtensionId],
    direction: Direction,
) -> ExtensionId {
    if ids.is_empty() {
        return default_widget_id();
    }
    let index = ids.iter().position(|id| id == current).unwrap_or(0);
    let target = (index as isize + direction.offset()).rem_euclid(ids.len() as isize) as usize;
    ids[target].clone()
}

#[cfg(test)]
mod tests {
    use super::{default_widget_id, key_direction, next_widget, Direction};
    use crate::{builtins, extension::ExtensionId};

    fn widget_ids() -> Vec<ExtensionId> {
        std::iter::once(ExtensionId::builtin(
            crate::extension::NOTIFICATION_CENTER_ID,
        ))
        .chain(
            builtins::registry()
                .by_kind(crate::extension::ContributionKind::Widget)
                .into_iter()
                .map(|descriptor| descriptor.id.clone()),
        )
        .collect()
    }

    #[test]
    fn notification_center_is_always_first_and_is_the_default_widget() {
        let ids = widget_ids();
        assert_eq!(ids[0], default_widget_id());
        assert_eq!(
            ids.iter().map(ExtensionId::as_str).collect::<Vec<_>>(),
            [
                crate::extension::NOTIFICATION_CENTER_ID,
                "builtin.system-monitor/overview",
                "builtin.pomodoro/timer",
                "builtin.kuaidi/shipments",
                "builtin.calendar/month",
                "builtin.weather/forecast",
                #[cfg(any(target_os = "linux", target_os = "windows"))]
                "builtin.music/now-playing",
                "builtin.market/overview",
                "builtin.health/overview",
                "builtin.rss/reader",
                "builtin.mail/inbox",
            ]
        );
    }

    #[test]
    fn widget_cycling_uses_registry_order() {
        let ids = widget_ids();
        assert_eq!(next_widget(&ids[1], &ids, Direction::Next), ids[2]);
        assert_eq!(next_widget(&ids[1], &ids, Direction::Previous), ids[0]);
    }

    #[test]
    fn arrow_keys_move_in_both_directions_and_wrap_at_the_ends() {
        use eframe::egui::{Context, Event, Key, Modifiers, RawInput};
        let ids = widget_ids();
        let last = ids.len() - 1;
        for (start, key, target) in [
            (2, Key::ArrowLeft, 1),
            (0, Key::ArrowLeft, last),
            (1, Key::ArrowRight, 2),
            (last, Key::ArrowRight, 0),
        ] {
            let ctx = Context::default();
            let _ = ctx.run(
                RawInput {
                    events: vec![Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }],
                    ..RawInput::default()
                },
                |ctx| {
                    let direction = ctx.input(key_direction).unwrap();
                    assert_eq!(next_widget(&ids[start], &ids, direction), ids[target]);
                },
            );
        }
    }
}
