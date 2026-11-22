use chrono::{Datelike, NaiveDate, Utc};
use chrono_tz::Tz;
use crate::extension::{SettingValue, Settings};

/// Locale is derived from the machine's IANA time zone. The user does not
/// need to enter ISO country or region codes in extension settings.
#[derive(Clone, Debug)]
pub(super) struct CalendarSettings {
    pub country: String,
    timezone: Tz,
    pub show_lunar: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        let name = iana_time_zone::get_timezone()
            .unwrap_or_else(|_| "Etc/UTC".to_owned());
        Self::for_timezone(&name)
    }
}

impl CalendarSettings {
    fn for_timezone(name: &str) -> Self {
        let timezone = name.parse::<Tz>().unwrap_or(chrono_tz::UTC);
        let country = country_from_timezone(timezone.name());
        Self { country: country.into(), timezone, show_lunar: true }
    }

    pub fn apply(&mut self, values: &Settings) {
        if let Some(SettingValue::Boolean(value)) = values.get("show_lunar") {
            self.show_lunar = *value;
        }
        // OS region and timezone remain authoritative; never retain stale
        // country / region text fields from the old extension settings.
        let current = Self::default();
        self.country = current.country;
        self.timezone = current.timezone;
    }

    pub fn timezone(&self) -> Tz {
        self.timezone
    }

    pub fn today(&self) -> NaiveDate {
        Utc::now().with_timezone(&self.timezone()).date_naive()
    }

    pub fn lunar_enabled(&self) -> bool {
        matches!(self.country.as_str(), "CN" | "TW") && self.show_lunar
    }

    pub fn country_label(&self) -> &'static str {
        match self.country.as_str() {
            "CN" => "中国",
            "TW" => "台湾地区",
            "JP" => "日本",
            "US" => "美国",
            "GB" => "英国",
            "DE" => "德国",
            "FR" => "法国",
            "CA" => "加拿大",
            "AU" => "澳大利亚",
            "ID" => "印度尼西亚",
            _ => "本地",
        }
    }
}

fn country_from_timezone(timezone: &str) -> &'static str {
    match timezone {
        "Asia/Shanghai" | "Asia/Chongqing" | "Asia/Harbin"
        | "Asia/Urumqi" | "Asia/Hong_Kong" | "Asia/Macau" => "CN",
        "Asia/Taipei" => "TW",
        "Asia/Tokyo" => "JP",
        "Europe/London" | "Europe/Belfast" => "GB",
        "Europe/Berlin" | "Europe/Busingen" => "DE",
        "Europe/Paris" => "FR",
        "Asia/Jakarta" | "Asia/Makassar" | "Asia/Jayapura" => "ID",
        zone if zone.starts_with("Australia/") => "AU",
        "America/Vancouver" | "America/Toronto" | "America/Montreal"
        | "America/Edmonton" | "America/Winnipeg" | "America/Halifax"
        | "America/St_Johns" | "America/Regina" | "America/Whitehorse" => "CA",
        "America/New_York" | "America/Chicago" | "America/Denver"
        | "America/Los_Angeles" | "America/Phoenix" | "America/Anchorage"
        | "America/Detroit" | "America/Boise" | "Pacific/Honolulu" => "US",
        _ => "",
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
    fn timezone_drives_country_without_an_editable_country_or_region() {
        let us = CalendarSettings::for_timezone("America/Los_Angeles");
        assert_eq!(us.timezone().name(), "America/Los_Angeles");
        assert_eq!(us.country, "US");
        assert!(!us.lunar_enabled());
        let cn = CalendarSettings::for_timezone("Asia/Shanghai");
        assert_eq!(cn.country, "CN");
        assert!(cn.lunar_enabled());
        assert_eq!(CalendarSettings::for_timezone("Asia/Tokyo").country, "JP");
    }
}
