//! The device time zone's UTC offsets over time, for the study-day clock
//! (`kabar_core::srs::DayClock::with_history`, engineering audit F19): an
//! answer given in summer time keeps its study day after the clocks go back.
// adapted from batna-app main: crates/app/src/store/mod.rs (zone)

use std::sync::OnceLock;

const HOUR: i64 = 3_600_000;
/// 2020-01-01 .. 2040-01-01 UTC: Kabar's whole history and well beyond.
const SPAN: (i64, i64) = (1_577_836_800_000, 2_208_988_800_000);

/// `(from_ms, minutes east of UTC)`, the first from `i64::MIN`, sorted.
/// Worked out once per run from the device's zone rules.
pub fn history() -> &'static [(i64, i32)] {
    static HISTORY: OnceLock<&'static [(i64, i32)]> = OnceLock::new();
    HISTORY.get_or_init(|| Box::leak(build(local_offset).into_boxed_slice()))
}

/// Minutes east of UTC at `ms` in the device's zone.
fn local_offset(ms: i64) -> i32 {
    use chrono::{Offset, TimeZone};
    chrono::Local.timestamp_millis_opt(ms).single().map_or(0, |t| t.offset().fix().local_minus_utc() / 60)
}

/// Scans hour by hour and narrows each change to the minute.
fn build(offset: impl Fn(i64) -> i32) -> Vec<(i64, i32)> {
    let (from, to) = SPAN;
    let mut last = offset(from);
    let mut out = vec![(i64::MIN, last)];
    let mut t = from;
    while t < to {
        let o = offset(t + HOUR);
        if o != last {
            let (mut lo, mut hi) = (t, t + HOUR);
            while hi - lo > 60_000 {
                let mid = lo + (hi - lo) / 2;
                if offset(mid) == last { lo = mid } else { hi = mid }
            }
            out.push((hi, o));
            last = o;
        }
        t += HOUR;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_are_found_to_the_minute() {
        // A made-up zone: +180 until 2026-10-30 00:00 UTC, then +120.
        let change = 1_793_318_400_000;
        let h = build(|ms| if ms < change { 180 } else { 120 });
        assert_eq!(h, vec![(i64::MIN, 180), (change, 120)]);
        assert_eq!(build(|_| 60), vec![(i64::MIN, 60)]);
    }

    #[test]
    fn the_device_history_is_sorted_and_agrees_with_now() {
        let h = history();
        assert_eq!(h[0].0, i64::MIN);
        assert!(h.windows(2).all(|w| w[0].0 < w[1].0));
        let now = crate::store::now_ms() as i64;
        let at_now = h[h.partition_point(|(f, _)| *f <= now) - 1].1;
        assert_eq!(at_now, local_offset(now));
    }
}
