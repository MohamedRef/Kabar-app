//! Calendar helpers for quota periods (UTC), without a date library.

/// `(year, month, day)` for a Unix time in milliseconds (proleptic Gregorian, UTC).
pub fn civil_date(epoch_ms: u64) -> (i64, u32, u32) {
    // Howard Hinnant's days_from_civil inverse.
    let days = (epoch_ms / 86_400_000) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// "2026-10-26"
pub fn day_key(epoch_ms: u64) -> String {
    let (y, m, d) = civil_date(epoch_ms);
    format!("{y:04}-{m:02}-{d:02}")
}

/// "2026-10"
pub fn month_key(epoch_ms: u64) -> String {
    let (y, m, _) = civil_date(epoch_ms);
    format!("{y:04}-{m:02}")
}

/// Milliseconds since the epoch at the start of the next UTC day.
pub fn next_day_ms(epoch_ms: u64) -> u64 {
    (epoch_ms / 86_400_000 + 1) * 86_400_000
}

/// Milliseconds since the epoch at the start of the next UTC month.
pub fn next_month_ms(epoch_ms: u64) -> u64 {
    let (y, m, _) = civil_date(epoch_ms);
    let (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    days_from_civil(y, m, 1) as u64 * 86_400_000
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(day_key(0), "1970-01-01");
        // 2026-10-26T10:50:56Z
        assert_eq!(day_key(1_793_011_856_000), "2026-10-26");
        assert_eq!(month_key(1_793_011_856_000), "2026-10");
        // 2024-02-29T23:59:59.999Z (leap day)
        assert_eq!(day_key(1_709_251_199_999), "2024-02-29");
        assert_eq!(day_key(1_709_251_200_000), "2024-03-01");
    }

    #[test]
    fn resets() {
        let now = 1_793_011_856_000;
        assert_eq!(day_key(next_day_ms(now)), "2026-10-27");
        assert_eq!(next_day_ms(now) % 86_400_000, 0);
        assert_eq!(day_key(next_month_ms(now)), "2026-11-01");
        assert_eq!(day_key(next_month_ms(1_796_700_000_000)), "2027-01-01"); // 8 Dec 2026
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }
}
