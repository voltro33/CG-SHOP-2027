//! Times as the menus need them: seconds since 1970, how long ago, and
//! UTC dates and stamps for names, without a date crate.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since 1970 now.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn seconds_of(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `"5 min ago"` and the like, for `seconds` since 1970.
pub fn ago(seconds: u64) -> String {
    format!("{} ago", span(now().saturating_sub(seconds)))
}

/// A length of time, coarsely: `"42 s"`, `"7 min"`, `"1.5 h"`, `"2.0 days"`.
pub fn span(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds} s"),
        60..=3599 => format!("{} min", seconds / 60),
        3600..=86_399 => format!("{:.1} h", seconds as f64 / 3600.0),
        _ => format!("{:.1} days", seconds as f64 / 86_400.0),
    }
}

/// `YYYY-MM-DD` in UTC.
pub fn date(seconds: u64) -> String {
    let (y, m, d) = civil(seconds / 86_400);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `YYYYMMDD-HHMMSS` in UTC, for folder names that sort by time.
pub fn stamp(seconds: u64) -> String {
    let (y, m, d) = civil(seconds / 86_400);
    let s = seconds % 86_400;
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

/// The date of `days` since 1970 (Howard Hinnant's `civil_from_days`).
fn civil(days: u64) -> (i64, u64, u64) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe as i64 + era * 400 + i64::from(month <= 2);

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_stamps_are_utc() {
        assert_eq!(date(0), "1970-01-01");
        // 2026-11-15 13:05:09 UTC
        let t = 1_794_747_909;
        assert_eq!(date(t), "2026-11-15");
        assert_eq!(stamp(t), "20261115-130509");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(stamp(61), "19700101-000101");
    }

    #[test]
    fn spans_round_down_to_a_unit() {
        assert_eq!(span(42), "42 s");
        assert_eq!(span(420), "7 min");
        assert_eq!(span(5400), "1.5 h");
    }
}
