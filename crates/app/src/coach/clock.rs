//! "Today" for the learner: the system clock plus the profile's UTC offset,
//! which every device refreshes from its own time zone (spec §6, so Kabar
//! Cloud schedules reminders in local time).

use kabar_protocol::coach::{CoachProfile, LocalDate, LocalTime};

/// Offsets beyond these don't exist on Earth (UTC−12 … UTC+14).
const MIN_OFFSET: i16 = -12 * 60;
const MAX_OFFSET: i16 = 14 * 60;

/// JS: `-new Date().getTimezoneOffset()` minutes east of UTC. JS reports
/// minutes *west* (Cairo in summer is −180), so the sign flips.
pub fn offset_from_js(minutes_west: f64) -> Option<i16> {
    if !minutes_west.is_finite() {
        return None;
    }
    let east = -minutes_west.round();
    (f64::from(MIN_OFFSET)..=f64::from(MAX_OFFSET)).contains(&east).then_some(east as i16)
}

/// The profile with the device's offset, if it changed (travel, DST).
pub fn with_device_offset(profile: &CoachProfile, device_offset: i16) -> Option<CoachProfile> {
    (profile.utc_offset_min != device_offset)
        .then(|| CoachProfile { utc_offset_min: device_offset, ..profile.clone() })
}

pub fn today(now_ms: u64, utc_offset_min: i16) -> LocalDate {
    LocalDate::from_unix_ms(now_ms, utc_offset_min)
}

pub fn time_of_day(now_ms: u64, utc_offset_min: i16) -> LocalTime {
    LocalTime::from_unix_ms(now_ms, utc_offset_min)
}

/// Seed for Kak Nur's wording: the day number, so every device shows the
/// same variant all day.
pub fn seed(date: LocalDate) -> u64 {
    u64::try_from(date.0).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::to_unix_ms;

    use super::*;

    #[test]
    fn js_offsets_flip_sign_and_reject_nonsense() {
        assert_eq!(offset_from_js(-120.0), Some(120)); // Cairo, winter
        assert_eq!(offset_from_js(-180.0), Some(180)); // Cairo, summer
        assert_eq!(offset_from_js(-420.0), Some(420)); // Jakarta
        assert_eq!(offset_from_js(-330.0), Some(330)); // India
        assert_eq!(offset_from_js(300.0), Some(-300));
        assert_eq!(offset_from_js(f64::NAN), None);
        assert_eq!(offset_from_js(-2000.0), None);
    }

    #[test]
    fn offset_refresh_only_when_changed() {
        let p = CoachProfile { utc_offset_min: 120, ..Default::default() };
        assert_eq!(with_device_offset(&p, 120), None);
        assert_eq!(with_device_offset(&p, 420).map(|p| p.utc_offset_min), Some(420));
    }

    #[test]
    fn today_follows_the_offset() {
        let d = LocalDate::from_ymd(2026, 10, 26).unwrap();
        let ms = to_unix_ms(d, LocalTime::hm(23, 30), 120);
        assert_eq!(today(ms, 120), d);
        assert_eq!(time_of_day(ms, 120), LocalTime::hm(23, 30));
        // The same moment is already tomorrow in Jakarta.
        assert_eq!(today(ms, 420), d.add_days(1));
        assert_eq!(seed(d), d.0 as u64);
    }
}
