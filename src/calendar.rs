pub const MONTH_LABELS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

pub fn is_valid_day(year: i32, month: u32, day: u32) -> bool {
    day >= 1 && day <= days_in_month(year, month)
}

/// True when the given date falls on a Saturday or Sunday.
/// Returns false for dates that don't exist (e.g. Feb 30).
pub fn is_weekend(year: i32, month: u32, day: u32) -> bool {
    use chrono::{Datelike, NaiveDate, Weekday};
    NaiveDate::from_ymd_opt(year, month, day)
        .map(|d| matches!(d.weekday(), Weekday::Sat | Weekday::Sun))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_years() {
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2023));
    }

    #[test]
    fn days_per_month() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2024, 4), 30);
        assert_eq!(days_in_month(2024, 1), 31);
        assert_eq!(days_in_month(2024, 13), 0);
        assert_eq!(days_in_month(2024, 0), 0);
    }

    #[test]
    fn weekends() {
        // 2026-08-29 Sat, 2026-08-30 Sun, 2026-08-31 Mon.
        assert!(is_weekend(2026, 8, 29));
        assert!(is_weekend(2026, 8, 30));
        assert!(!is_weekend(2026, 8, 31));
        // Non-existent date is not a weekend.
        assert!(!is_weekend(2026, 2, 30));
    }

    #[test]
    fn valid_days() {
        assert!(is_valid_day(2024, 2, 29));
        assert!(!is_valid_day(2023, 2, 29));
        assert!(!is_valid_day(2024, 4, 31));
        assert!(is_valid_day(2024, 12, 31));
        assert!(!is_valid_day(2024, 1, 0));
    }
}
