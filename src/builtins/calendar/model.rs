use chrono::{Datelike, NaiveDate, Utc};
use chrono_tz::Tz;
use crate::extension::{SettingValue, Settings};

#[derive(Clone, Debug)]
pub(super) struct CalendarSettings {
    pub country: String,
    pub region: String,
    pub show_lunar: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        Self { country: "CN".into(), region: "CN-BJ".into(), show_lunar: true }
    }
}

impl CalendarSettings {
    pub fn apply(&mut self, values: &Settings) {
        if let Some(SettingValue::Enum(value)) = values.get("country") {
            self.country.clone_from(value);
        }
        if let Some(SettingValue::Enum(value)) = values.get("region") {
            self.region.clone_from(value);
        }
        if let Some(SettingValue::Boolean(value)) = values.get("show_lunar") {
            self.show_lunar = *value;
        }
    }

    /// Region selects the user's preferred civil timezone, not a guess from IP.
    /// An incompatible region falls back to the selected country's capital.
    pub fn timezone(&self) -> Tz {
        let key = match (self.country.as_str(), self.region.as_str()) {
            ("CN", "CN-HK") => "Asia/Hong_Kong",
            ("CN", "CN-TW") => "Asia/Taipei",
            ("US", "US-CA") => "America/Los_Angeles",
            ("US", "US-NY") => "America/New_York",
            ("CA", "CA-ON") => "America/Toronto",
            ("AU", "AU-NSW") => "Australia/Sydney",
            ("CN", _) => "Asia/Shanghai",
            ("US", _) => "America/New_York",
            ("GB", _) => "Europe/London",
            ("DE", _) => "Europe/Berlin",
            ("FR", _) => "Europe/Paris",
            ("CA", _) => "America/Toronto",
            ("AU", _) => "Australia/Sydney",
            ("ID", _) => "Asia/Jakarta",
            _ => "UTC",
        };
        key.parse().expect("built-in IANA time zones are valid")
    }

    pub fn today(&self) -> NaiveDate {
        Utc::now().with_timezone(&self.timezone()).date_naive()
    }

    pub fn lunar_enabled(&self) -> bool {
        self.country == "CN" && self.show_lunar
    }

    pub fn country_label(&self) -> &'static str {
        match self.country.as_str() {
            "CN" => "中国",
            "US" => "美国",
            "GB" => "英国",
            "DE" => "德国",
            "FR" => "法国",
            "CA" => "加拿大",
            "AU" => "澳大利亚",
            "ID" => "印度尼西亚",
            _ => "国际",
        }
    }
}

pub(super) fn start_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("valid first of month")
}

pub(super) fn shift_month(date: NaiveDate, direction: i32) -> NaiveDate {
    let month = date.year() * 12 + date.month0() as i32 + direction;
    let year = month.div_euclid(12);
    let month = month.rem_euclid(12) as u32 + 1;
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(date)
}

pub(super) fn month_dates(date: NaiveDate) -> Vec<Option<NaiveDate>> {
    use chrono::{Duration, Weekday};
    let first = start_month(date);
    let weekday = match first.weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    };
    let next = shift_month(first, 1);
    let days = (next - first).num_days() as usize;
    (0..42).map(|slot| {
        let day = slot as i64 - weekday as i64;
        (day >= 0 && (day as usize) < days).then(|| first + Duration::days(day))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_week_grid_starts_on_monday_and_never_bleeds_across_months() {
        let grid = month_dates(NaiveDate::from_ymd_opt(2026, 11, 22).unwrap());
        assert_eq!(grid.len(), 42);
        assert_eq!(grid.iter().flatten().count(), 30);
        assert_eq!(grid.iter().flatten().next().unwrap().day(), 1);
    }
    #[test]
    fn regions_select_dst_aware_timezones_without_changing_country() {
        let mut preferences = CalendarSettings { country: "US".into(), region: "US-CA".into(), show_lunar: false };
        assert_eq!(preferences.timezone().name(), "America/Los_Angeles");
        preferences.country = "CN".into();
        assert_eq!(preferences.timezone().name(), "Asia/Shanghai");
    }
}
