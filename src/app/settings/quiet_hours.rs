use time::{OffsetDateTime, Weekday};

pub const MAX_QUIET_PERIODS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuietPeriod {
    pub enabled: bool,
    pub days: u8,
    pub start_minute: u16,
    pub end_minute: u16,
}

impl Default for QuietPeriod {
    fn default() -> Self {
        Self {
            enabled: true,
            days: 0b0001_1111,
            start_minute: 22 * 60,
            end_minute: 7 * 60,
        }
    }
}

impl QuietPeriod {
    pub fn is_active_at(self, weekday: Weekday, minute: u16) -> bool {
        if !self.enabled || self.days & 0x7f == 0 {
            return false;
        }
        let minute = minute.min(1439);
        let day = weekday.number_from_monday() - 1;
        if self.start_minute == self.end_minute {
            return self.includes(day);
        }
        if self.start_minute < self.end_minute {
            self.includes(day) && (self.start_minute..self.end_minute).contains(&minute)
        } else {
            let previous = (day + 6) % 7;
            (self.includes(day) && minute >= self.start_minute)
                || (self.includes(previous) && minute < self.end_minute)
        }
    }

    pub fn includes(self, day: u8) -> bool {
        day < 7 && self.days & (1 << day) != 0
    }

    pub fn toggle_day(&mut self, day: u8) {
        if day < 7 {
            self.days ^= 1 << day;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuietHours {
    pub manual_enabled: bool,
    periods: [QuietPeriod; MAX_QUIET_PERIODS],
    count: u8,
}

impl Default for QuietHours {
    fn default() -> Self {
        Self {
            manual_enabled: false,
            periods: [QuietPeriod::default(); MAX_QUIET_PERIODS],
            count: 0,
        }
    }
}

impl QuietHours {
    pub fn periods(&self) -> &[QuietPeriod] {
        &self.periods[..usize::from(self.count)]
    }

    pub fn period(&self, index: usize) -> Option<QuietPeriod> {
        self.periods().get(index).copied()
    }

    pub fn add_period(&mut self, period: QuietPeriod) -> bool {
        let index = usize::from(self.count);
        if index == MAX_QUIET_PERIODS {
            return false;
        }
        self.periods[index] = normalize_period(period);
        self.count += 1;
        true
    }

    pub fn update_period(&mut self, index: usize, period: QuietPeriod) -> bool {
        let Some(slot) = self
            .periods
            .get_mut(index)
            .filter(|_| index < usize::from(self.count))
        else {
            return false;
        };
        *slot = normalize_period(period);
        true
    }

    pub fn remove_period(&mut self, index: usize) -> bool {
        let count = usize::from(self.count);
        if index >= count {
            return false;
        }
        self.periods.copy_within(index + 1..count, index);
        self.count -= 1;
        self.periods[usize::from(self.count)] = QuietPeriod::default();
        true
    }

    pub fn toggle_manual(&mut self, scheduled_now: bool, bypassed: &mut bool) {
        if self.manual_enabled {
            self.manual_enabled = false;
            *bypassed |= scheduled_now;
        } else if scheduled_now && !*bypassed {
            *bypassed = true;
        } else {
            self.manual_enabled = true;
        }
    }

    pub fn is_scheduled_now(self) -> bool {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        self.is_active_at(
            now.weekday(),
            u16::from(now.hour()) * 60 + u16::from(now.minute()),
        )
    }

    pub fn is_active_at(self, weekday: Weekday, minute: u16) -> bool {
        self.periods()
            .iter()
            .any(|period| period.is_active_at(weekday, minute))
    }
}

fn normalize_period(mut period: QuietPeriod) -> QuietPeriod {
    period.days &= 0x7f;
    period.start_minute = period.start_minute.min(1439);
    period.end_minute = period.end_minute.min(1439);
    period
}

#[cfg(test)]
mod tests {
    use super::{QuietHours, QuietPeriod, MAX_QUIET_PERIODS};
    use time::Weekday;

    fn period(days: u8, start: u16, end: u16) -> QuietPeriod {
        QuietPeriod {
            enabled: true,
            days,
            start_minute: start,
            end_minute: end,
        }
    }

    #[test]
    fn multiple_periods_match_independently_across_weekdays() {
        let mut quiet = QuietHours::default();
        quiet.add_period(period(0b0001_1111, 22 * 60, 7 * 60));
        quiet.add_period(period(0b0010_0000, 12 * 60, 14 * 60));
        assert!(quiet.is_active_at(Weekday::Monday, 23 * 60));
        assert!(quiet.is_active_at(Weekday::Saturday, 13 * 60));
        assert!(!quiet.is_active_at(Weekday::Saturday, 15 * 60));
        assert!(!quiet.is_active_at(Weekday::Sunday, 13 * 60));
    }

    #[test]
    fn schedule_add_edit_remove_and_capacity_are_bounded() {
        let mut quiet = QuietHours::default();
        for _ in 0..MAX_QUIET_PERIODS {
            assert!(quiet.add_period(QuietPeriod::default()));
        }
        assert!(!quiet.add_period(QuietPeriod::default()));
        assert!(quiet.update_period(1, period(0x7f, 480, 1020)));
        assert_eq!(quiet.period(1).unwrap().start_minute, 480);
        assert!(quiet.remove_period(1));
        assert_eq!(quiet.periods().len(), MAX_QUIET_PERIODS - 1);
        assert!(!quiet.remove_period(MAX_QUIET_PERIODS));
    }

    #[test]
    fn period_days_and_times_are_clamped_to_valid_values() {
        let mut quiet = QuietHours::default();
        quiet.add_period(period(0xff, 2000, 2001));
        let period = quiet.period(0).unwrap();
        assert_eq!(period.days, 0x7f);
        assert_eq!(period.start_minute, 1439);
        assert_eq!(period.end_minute, 1439);
    }

    #[test]
    fn manual_mode_and_scheduled_bypass_are_separate() {
        let mut quiet = QuietHours::default();
        let mut bypassed = false;
        quiet.toggle_manual(false, &mut bypassed);
        assert!(quiet.manual_enabled);
        quiet.toggle_manual(true, &mut bypassed);
        assert!(!quiet.manual_enabled);
        assert!(bypassed);
        quiet.toggle_manual(true, &mut bypassed);
        assert!(quiet.manual_enabled);
    }
}
