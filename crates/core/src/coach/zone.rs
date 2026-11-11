//! The learner's time zone for reading *past* moments (engineering audit
//! F19): a review answered in summer time stays on its own local day and
//! hour after the clocks go back.
//!
//! "Now" always uses the profile's offset, which each device refreshes from
//! its own zone. The history comes from the device's zone rules
//! (`kabar_app::zone::history`). A history that disagrees with the offset in
//! use now is ignored, so a zone never contradicts the profile. Reminder
//! timing stays on the profile's offset alone: Kabar Cloud's cron has no zone
//! history, and the device and the cron must reach the same answer.

use kabar_protocol::coach::{LocalDate, LocalTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zone {
    /// Minutes east of UTC now (the profile's offset).
    now: i16,
    /// `(from_ms, minutes east of UTC)`, the first from `i64::MIN`, sorted.
    history: &'static [(i64, i32)],
}

impl Zone {
    /// One offset for every moment.
    pub const fn fixed(utc_offset_min: i16) -> Self {
        Self { now: utc_offset_min, history: &[] }
    }

    /// `utc_offset_min` now, and `history` for the past when it agrees with
    /// that offset at `now_ms`.
    pub fn with_history(utc_offset_min: i16, history: &'static [(i64, i32)], now_ms: u64) -> Self {
        let candidate = Self { now: utc_offset_min, history };
        let agrees = !history.is_empty()
            && history[0].0 == i64::MIN
            && history.windows(2).all(|w| w[0].0 < w[1].0)
            && candidate.offset_at(now_ms) == utc_offset_min;
        if agrees { candidate } else { Self::fixed(utc_offset_min) }
    }

    /// Minutes east of UTC now.
    pub fn now_offset(&self) -> i16 {
        self.now
    }

    /// Minutes east of UTC at `ms`.
    pub fn offset_at(&self, ms: u64) -> i16 {
        let ms = i64::try_from(ms).unwrap_or(i64::MAX);
        match self.history.partition_point(|(from, _)| *from <= ms) {
            0 => self.now,
            i => i16::try_from(self.history[i - 1].1).unwrap_or(self.now),
        }
    }

    /// The local day `ms` fell on.
    pub fn date(&self, ms: u64) -> LocalDate {
        LocalDate::from_unix_ms(ms, self.offset_at(ms))
    }

    /// The local clock time at `ms`.
    pub fn time(&self, ms: u64) -> LocalTime {
        LocalTime::from_unix_ms(ms, self.offset_at(ms))
    }
}

impl From<i16> for Zone {
    fn from(utc_offset_min: i16) -> Self {
        Self::fixed(utc_offset_min)
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::to_unix_ms;

    use super::*;

    /// Cairo: +180 (summer) until 2026-10-29 22:00 UTC, then +120.
    const CHANGE: i64 = 1_793_311_200_000;
    static CAIRO: [(i64, i32); 2] = [(i64::MIN, 180), (CHANGE, 120)];

    #[test]
    fn a_summer_moment_keeps_its_day_and_hour_after_the_change() {
        let after = CHANGE as u64 + 7 * 86_400_000;
        let z = Zone::with_history(120, &CAIRO, after);
        // 2026-10-20 23:30 summer time = 20:30 UTC.
        let day = LocalDate::from_unix_ms(CHANGE as u64 - 10 * 86_400_000, 180);
        let at = to_unix_ms(day, LocalTime::hm(23, 30), 180);
        assert_eq!(z.date(at), day);
        assert_eq!(z.time(at), LocalTime::hm(23, 30));
        // On today's offset alone it would be 22:30 the same day…
        assert_eq!(Zone::fixed(120).time(at), LocalTime::hm(22, 30));
        // …and 00:30 summer time would move to the day before.
        let late = to_unix_ms(day, LocalTime::hm(0, 30), 180);
        assert_eq!(z.date(late), day);
        assert_ne!(Zone::fixed(120).date(late), day);
        // Now uses the profile's offset.
        assert_eq!(z.offset_at(after), 120);
        assert_eq!(z.now_offset(), 120);
    }

    #[test]
    fn a_history_that_disagrees_with_the_profile_is_ignored() {
        let after = CHANGE as u64 + 86_400_000;
        // The profile says +420 (Jakarta) but the device zone says +120.
        assert_eq!(Zone::with_history(420, &CAIRO, after), Zone::fixed(420));
        assert_eq!(Zone::with_history(120, &[], after), Zone::fixed(120));
        static UNSORTED: [(i64, i32); 2] = [(i64::MIN, 120), (i64::MIN, 120)];
        assert_eq!(Zone::with_history(120, &UNSORTED, after), Zone::fixed(120));
        assert_eq!(Zone::from(60), Zone::fixed(60));
    }
}
