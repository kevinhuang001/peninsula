mod performance;
pub mod pomodoro;
pub(in crate::app) use performance::{expanded_width, paint_metrics};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IslandTab {
    #[default]
    Performance,
    Pomodoro,
}

impl IslandTab {
    pub const ALL: [Self; 2] = [Self::Performance, Self::Pomodoro];

    pub fn other(self) -> Self {
        match self {
            Self::Performance => Self::Pomodoro,
            Self::Pomodoro => Self::Performance,
        }
    }

    pub fn extra_height(self) -> f32 {
        match self {
            Self::Performance => 76.0,
            Self::Pomodoro => 126.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IslandTab;

    #[test]
    fn island_has_performance_and_pomodoro_tabs() {
        assert_eq!(
            IslandTab::ALL,
            [IslandTab::Performance, IslandTab::Pomodoro]
        );
    }

    #[test]
    fn pomodoro_has_more_room_than_performance() {
        assert!(IslandTab::Pomodoro.extra_height() > IslandTab::Performance.extra_height());
    }

    #[test]
    fn performance_height_leaves_a_compact_margin_below_cards() {
        let capsule_height = 52.0 + IslandTab::Performance.extra_height();
        let cards_bottom = 48.0 + 64.0;
        assert!((8.0..=20.0).contains(&(capsule_height - cards_bottom)));
    }
}
