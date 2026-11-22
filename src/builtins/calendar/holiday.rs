//! Gregorian public holidays from holidays-rs; lunar conversion from lunar-lite.
//! Chinese traditional observances are NOT a State Council leave/shift calendar.
use chrono::{Datelike, Duration, NaiveDate};
use holidays_rs::{Calendar, Date};
use lunar_lite::{solar_to_lunar, SolarDate};
use super::model::CalendarSettings;

fn national_calendar(country: &str) -> Option<Calendar> {
    match country {
        "US" => Some(Calendar::united_states()),
        "GB" => Some(Calendar::united_kingdom()),
        "DE" => Some(Calendar::germany()),
        "FR" => Some(Calendar::france()),
        "CA" => Some(Calendar::canada()),
        "AU" => Some(Calendar::australia()),
        "ID" => Some(Calendar::indonesia()),
        _ => None,
    }
}

fn lunar(date: NaiveDate) -> Option<lunar_lite::LunarDate> {
    solar_to_lunar(SolarDate { year: date.year(), month: date.month() as u8, day: date.day() as u8 }).ok()
}

pub(super) fn chinese_lunar_label(date: NaiveDate) -> Option<String> {
    let lunar = lunar(date)?;
    const DAYS: [&str; 30] = [
        "初一","初二","初三","初四","初五","初六","初七","初八","初九","初十",
        "十一","十二","十三","十四","十五","十六","十七","十八","十九","二十",
        "廿一","廿二","廿三","廿四","廿五","廿六","廿七","廿八","廿九","三十",
    ];
    const MONTHS: [&str; 12] = ["正月","二月","三月","四月","五月","六月","七月","八月","九月","十月","冬月","腊月"];
    if lunar.day == 1 {
        let month = MONTHS.get(lunar.month.saturating_sub(1) as usize)?;
        Some(format!("{}{}", if lunar.is_leap_month { "闰" } else { "" }, month))
    } else {
        DAYS.get(lunar.day.saturating_sub(1) as usize).map(|s| (*s).into())
    }
}

/// Festival labels are civil-day observances, NOT official leave/shift dates.
/// The principal festival is first for capsule calendar cells.
pub(super) fn festivals(date: NaiveDate, locale: &CalendarSettings) -> Vec<String> {
    let mut names = Vec::new();
    if locale.country == "CN" {
        if let Some(l) = lunar(date).filter(|l| !l.is_leap_month) {
            let lunar_festival = match (l.month, l.day) {
                (1, 1) => Some("春节"),
                (1, 15) => Some("元宵节"),
                (5, 5) => Some("端午节"),
                (7, 7) => Some("七夕节"),
                (8, 15) => Some("中秋节"),
                (9, 9) => Some("重阳节"),
                (12, 8) => Some("腊八节"),
                _ => None,
            };
            if let Some(name) = lunar_festival {
                names.push(name.into());
            }
        }
        if let Some(tomorrow) = date.checked_add_signed(Duration::days(1)) {
            if let Some(l) = lunar(tomorrow) {
                if l.month == 1 && l.day == 1 && !l.is_leap_month {
                    names.insert(0, "除夕".into());
                }
            }
        }
        let solar_festival = match (date.month(), date.day()) {
            (1, 1) => Some("元旦"),
            (5, 1) => Some("劳动节"),
            (10, 1) => Some("国庆节"),
            _ => None,
        };
        if let Some(name) = solar_festival {
            names.push(name.into());
        }
    } else if let Some(calendar) = national_calendar(&locale.country) {
        if let Some(holiday) = Date::new(date.year(), date.month() as u8, date.day() as u8)
            .and_then(|day| calendar.holiday_name(day))
        {
            names.push(holiday.into());
        }
    }
    names
}

/// Core requirement: a festival replaces (not accompanies) the lunar date.
pub(super) fn day_subtitle(date: NaiveDate, locale: &CalendarSettings) -> Option<String> {
    festivals(date, locale).into_iter().next()
        .or_else(|| locale.lunar_enabled().then(|| chinese_lunar_label(date)).flatten())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spring_festival_overrides_the_lunar_day() {
        let locale = CalendarSettings::default();
        let d = NaiveDate::from_ymd_opt(2026, 2, 17).unwrap();
        assert_eq!(day_subtitle(d, &locale).as_deref(), Some("春节"));
        let weekday = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_ne!(day_subtitle(weekday, &locale).as_deref(), Some("春节"));
    }
    #[test]
    fn international_holidays_come_from_holidays_rs() {
        let locale = CalendarSettings { country: "US".into(), region: "US-NY".into(), show_lunar: false };
        let d = NaiveDate::from_ymd_opt(2026, 7, 3).unwrap();
        assert!(!festivals(d, &locale).is_empty());
        assert_eq!(chinese_lunar_label(d).is_some(), true);
        assert_eq!(day_subtitle(NaiveDate::from_ymd_opt(2026, 7, 8).unwrap(), &locale), None);
    }
}
