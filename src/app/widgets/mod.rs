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
}
