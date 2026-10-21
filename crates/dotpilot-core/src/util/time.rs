//! Minimal UTC timestamp formatting.
//!
//! Only whole seconds in UTC are needed, so a small implementation of the
//! civil-calendar conversion is used instead of pulling in a date-time crate.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current time as whole seconds since the Unix epoch.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format Unix seconds as an ISO-8601 UTC timestamp, e.g. `2026-10-22T14:03:09Z`.
pub fn format_unix(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let secs_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

/// The current time formatted as an ISO-8601 UTC timestamp.
pub fn now_iso8601() -> String {
    format_unix(now_unix())
}

/// Convert days since the Unix epoch into a proleptic Gregorian date.
///
/// This is the civil-from-days algorithm, valid for the full range of dates
/// DotPilot can encounter.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as i64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epoch() {
        assert_eq!(format_unix(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn formats_a_known_timestamp() {
        // 2026-10-22T00:00:00Z
        assert_eq!(format_unix(1_792_627_200), "2026-10-22T00:00:00Z");
    }

    #[test]
    fn handles_leap_years() {
        // 2024-02-29T12:00:00Z
        assert_eq!(format_unix(1_709_208_000), "2024-02-29T12:00:00Z");
    }

    #[test]
    fn handles_pre_epoch_times() {
        assert_eq!(format_unix(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn now_is_after_2020() {
        assert!(now_unix() > 1_577_836_800);
    }
}
