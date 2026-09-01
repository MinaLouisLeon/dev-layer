//! One timestamp format, converted once.
//!
//! GitHub answers in RFC 3339 - `2026-08-30T09:41:12Z` - and every time on
//! this transport interface is Unix epoch milliseconds. Converting here rather
//! than in the UI is the same decision the git history parser makes about
//! git's `%at`: a renderer that parsed dates would be a second implementation
//! of a format, and with two transports eventually two disagreeing ones.
//!
//! No date crate for eight lines of arithmetic. The civil-to-days algorithm
//! below is Howard Hinnant's `days_from_civil`, which is exact for every date
//! this will ever see and has no timezone or locale behaviour to be surprised
//! by.
//!
//! A value this cannot read answers `None` rather than an error. A run whose
//! timestamp is unreadable is still a run worth listing, and refusing a whole
//! page over one field would be the worse trade.

/// Epoch milliseconds for an RFC 3339 instant in UTC, or `None`.
///
/// Only the `Z` form is accepted, because it is the only form GitHub sends.
/// Fractional seconds are tolerated and discarded: the interface's unit is
/// milliseconds and nothing here is timing anything.
pub fn epoch_ms(text: &str) -> Option<u64> {
    let text = text.trim();
    let (date, rest) = text.split_once('T')?;
    let time = rest
        .split_once('.')
        .map_or(rest, |(before, _)| before)
        .trim_end_matches('Z');

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..=60).contains(&second) {
        return None;
    }

    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second;
    u64::try_from(seconds).ok()?.checked_mul(1_000)
}

/// Days between 1970-01-01 and the given civil date. Exact for every
/// proleptic Gregorian date; see Hinnant, "chrono-Compatible Low-Level Date
/// Algorithms".
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_itself_is_zero() {
        assert_eq!(epoch_ms("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn a_known_instant_converts() {
        // 2026-08-30T00:00:00Z. 20_695 days after the epoch.
        assert_eq!(
            epoch_ms("2026-08-30T00:00:00Z"),
            Some(20_695 * 86_400 * 1_000)
        );
    }

    #[test]
    fn the_time_of_day_is_added() {
        let midnight = epoch_ms("2026-08-30T00:00:00Z").unwrap();
        assert_eq!(
            epoch_ms("2026-08-30T09:41:12Z"),
            Some(midnight + (9 * 3_600 + 41 * 60 + 12) * 1_000)
        );
    }

    #[test]
    fn fractional_seconds_are_tolerated_and_dropped() {
        assert_eq!(
            epoch_ms("2026-08-30T09:41:12.482Z"),
            epoch_ms("2026-08-30T09:41:12Z")
        );
    }

    #[test]
    fn a_leap_day_is_a_day() {
        let leap = epoch_ms("2024-02-29T00:00:00Z").unwrap();
        let after = epoch_ms("2024-03-01T00:00:00Z").unwrap();
        assert_eq!(after - leap, 86_400_000);
    }

    #[test]
    fn nonsense_is_none_and_never_a_panic() {
        for text in [
            "",
            "not a date",
            "2026-13-01T00:00:00Z",
            "2026-08-30",
            "T::",
        ] {
            assert_eq!(epoch_ms(text), None, "{text:?}");
        }
    }
}
