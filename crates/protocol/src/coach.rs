//! Kak Nur, the coach (spec §6): the data it keeps and syncs.
//!
//! Where each type lives in sync (`crates/protocol/src/sync.rs` table names):
//! - [`CoachProfile`] → `goals`, one LWW row with id [`PROFILE_ROW_ID`].
//! - [`DailyPlan`] → `daily_plans`, LWW keyed by the date, so every device
//!   shows the same plan.
//! - [`ActivityEvent`] → `xp_events`, append-only. Pelita, Koin, the league,
//!   recaps, reports and "goal met today" are all derived from these, so two
//!   devices studying offline never conflict.
//! - [`ReminderState`] → `reminders`, one LWW row with id [`REMINDER_ROW_ID`].
//! - [`WeeklyReport`] → `reports`, LWW keyed by the week's Monday.
//!
//! Dates are the learner's local calendar days; the device that writes an
//! event decides its date, so a late-night session never lands on the wrong
//! day for the streak.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Hlc;

/// Bumped when a coach type changes incompatibly.
pub const COACH_VERSION: u32 = 1;
pub const PROFILE_ROW_ID: &str = "profile";
pub const REMINDER_ROW_ID: &str = "reminders";

// --- calendar ----------------------------------------------------------------

/// A calendar day, stored as days since 1970-01-01. Serialized as `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDate(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub const ALL: [Self; 7] = [Self::Mon, Self::Tue, Self::Wed, Self::Thu, Self::Fri, Self::Sat, Self::Sun];

    /// 0 = Monday … 6 = Sunday.
    pub fn index(self) -> u8 {
        self as u8
    }
}

impl LocalDate {
    /// `None` for an impossible date such as 2026-02-30.
    pub fn from_ymd(y: i32, m: u32, d: u32) -> Option<Self> {
        if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
            return None;
        }
        // Howard Hinnant's days_from_civil.
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe as u32 * 365 + yoe as u32 / 4 - yoe as u32 / 100 + doy;
        Some(Self(era * 146_097 + doe as i32 - 719_468))
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        let z = self.0 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe as i32 + era * 400 + i32::from(m <= 2);
        (y, m, d)
    }

    /// The local day of a moment, given the learner's UTC offset.
    pub fn from_unix_ms(ms: u64, utc_offset_min: i16) -> Self {
        let local = ms as i64 + i64::from(utc_offset_min) * 60_000;
        Self(local.div_euclid(86_400_000) as i32)
    }

    pub fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday.
        Weekday::ALL[(self.0 + 3).rem_euclid(7) as usize]
    }

    pub fn add_days(self, n: i32) -> Self {
        Self(self.0 + n)
    }

    pub fn days_since(self, earlier: Self) -> i32 {
        self.0 - earlier.0
    }

    /// The Monday that starts this date's week.
    pub fn week_start(self) -> Self {
        self.add_days(-i32::from(self.weekday().index()))
    }
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

impl fmt::Display for LocalDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl FromStr for LocalDate {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let mut parts = s.splitn(3, '-');
        let mut next = || parts.next().and_then(|p| p.parse::<i64>().ok());
        match (next(), next(), next()) {
            (Some(y), Some(m), Some(d)) => i32::try_from(y)
                .ok()
                .and_then(|y| Self::from_ymd(y, u32::try_from(m).ok()?, u32::try_from(d).ok()?)),
            _ => None,
        }
        .ok_or_else(|| format!("not a date: {s:?}"))
    }
}

impl Serialize for LocalDate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LocalDate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

/// A local time of day, minutes after midnight. Serialized as `HH:MM`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalTime(pub u16);

impl LocalTime {
    pub fn hm(h: u16, m: u16) -> Self {
        Self((h % 24) * 60 + m % 60)
    }

    pub fn hour(self) -> u16 {
        self.0 / 60
    }

    pub fn minute(self) -> u16 {
        self.0 % 60
    }

    /// Time of day of a moment, given the learner's UTC offset.
    pub fn from_unix_ms(ms: u64, utc_offset_min: i16) -> Self {
        let local = ms as i64 + i64::from(utc_offset_min) * 60_000;
        Self((local.rem_euclid(86_400_000) / 60_000) as u16)
    }
}

impl fmt::Display for LocalTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour(), self.minute())
    }
}

impl FromStr for LocalTime {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        s.split_once(':')
            .and_then(|(h, m)| Some((h.parse::<u16>().ok()?, m.parse::<u16>().ok()?)))
            .filter(|&(h, m)| h < 24 && m < 60)
            .map(|(h, m)| Self::hm(h, m))
            .ok_or_else(|| format!("not a time: {s:?}"))
    }
}

impl Serialize for LocalTime {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LocalTime {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

/// Converts a local day + time back to a Unix timestamp.
pub fn to_unix_ms(date: LocalDate, time: LocalTime, utc_offset_min: i16) -> u64 {
    let local_min = i64::from(date.0) * 1440 + i64::from(time.0);
    ((local_min - i64::from(utc_offset_min)) * 60_000).max(0) as u64
}

// --- onboarding --------------------------------------------------------------

/// Why the learner is learning; several can apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Motivation {
    Travel,
    Work,
    FamilyFriends,
    /// Films, music, social media.
    Media,
    /// Islam in Indonesia and Java, pesantren, travel for faith.
    Faith,
    Curiosity,
}

/// "5, 15 or 30 minutes today?"
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanSize {
    Quick,
    #[default]
    Steady,
    Deep,
}

impl PlanSize {
    pub const ALL: [Self; 3] = [Self::Quick, Self::Steady, Self::Deep];

    pub fn minutes(self) -> u16 {
        match self {
            Self::Quick => 5,
            Self::Steady => 15,
            Self::Deep => 30,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageFocus {
    #[default]
    Indonesian,
    Javanese,
    Both,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReminderStyle {
    /// At most one reminder a day, at the first study time.
    Gentle,
    /// One at each study time until today's goal is met.
    #[default]
    Steady,
    /// No reminders; the weekly report still arrives in the app.
    Off,
}

/// A target such as "BIPA 2 by 2027-03-01".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// Curriculum level id, e.g. `bipa-2` or `j1`.
    pub level: String,
    pub by: LocalDate,
}

/// Where the journey starts. Until the placement chat with Mas Ardi exists
/// (personas, M12), onboarding asks a self-assessment and M8's placement
/// test refines it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartingPoint {
    #[default]
    BrandNew,
    KnowsBasics,
    CanChat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuietHours {
    pub start: LocalTime,
    pub end: LocalTime,
}

impl Default for QuietHours {
    fn default() -> Self {
        Self { start: LocalTime::hm(22, 30), end: LocalTime::hm(8, 0) }
    }
}

impl QuietHours {
    /// Whether `t` falls inside the quiet window (which may wrap midnight).
    pub fn contains(self, t: LocalTime) -> bool {
        if self.start <= self.end { t >= self.start && t < self.end } else { t >= self.start || t < self.end }
    }
}

/// Everything onboarding asks. Personal: never sent to an engine that trains
/// on inputs (the router enforces it for every coach task).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachProfile {
    /// The name Kak Nur uses.
    pub name: String,
    pub motivations: Vec<Motivation>,
    /// The learner's own words on why, if they wrote any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why_note: Option<String>,
    pub focus: LanguageFocus,
    /// The default answer to "5, 15 or 30 minutes today?".
    pub daily_size: PlanSize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    pub start: StartingPoint,
    /// Preferred study times, sorted.
    pub study_times: Vec<LocalTime>,
    pub reminder_style: ReminderStyle,
    pub quiet_hours: QuietHours,
    pub report_day: Weekday,
    pub report_time: LocalTime,
    /// The learner's current UTC offset in minutes; every device refreshes it
    /// on launch, so Kabar Cloud schedules reminders in local time.
    pub utc_offset_min: i16,
}

impl Default for CoachProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            motivations: Vec::new(),
            why_note: None,
            focus: LanguageFocus::default(),
            daily_size: PlanSize::default(),
            target: None,
            start: StartingPoint::default(),
            study_times: vec![LocalTime::hm(20, 0)],
            reminder_style: ReminderStyle::default(),
            quiet_hours: QuietHours::default(),
            report_day: Weekday::Thu,
            report_time: LocalTime::hm(19, 0),
            utc_offset_min: 120,
        }
    }
}

// --- skills and activity -----------------------------------------------------

/// The five axes of the skill radar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Skill {
    Reading,
    Listening,
    Writing,
    Speaking,
    Vocabulary,
}

impl Skill {
    pub const ALL: [Self; 5] =
        [Self::Reading, Self::Listening, Self::Writing, Self::Speaking, Self::Vocabulary];
}

/// Something the learner did. Written by whichever feature did it (reviews,
/// lessons, reader, …) and never changed afterwards.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityEvent {
    /// Unique per event; replays ignore duplicates.
    pub id: String,
    pub hlc: Hlc,
    /// Local day the activity counts for.
    pub date: LocalDate,
    pub at_ms: u64,
    pub kind: ActivityKind,
    /// Active study time, in seconds.
    #[serde(default)]
    pub seconds: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActivityKind {
    /// A batch of reviews.
    Reviews {
        cards: u32,
        correct: u32,
    },
    /// Words met for the first time (saved in the reader, taught in a lesson).
    NewWords {
        words: Vec<String>,
    },
    LessonStep {
        unit_id: String,
        done: bool,
    },
    /// A practice result on one skill and topic (e.g. writing, "-kan verbs").
    Practice {
        skill: Skill,
        topic: String,
        correct: u32,
        total: u32,
    },
    /// An item of today's plan was finished.
    PlanItemDone {
        item_id: String,
    },
    /// Koin spent in the Warung.
    Purchase {
        item_id: String,
        koin: u32,
    },
}

// --- daily plan --------------------------------------------------------------

/// *Rencana Hari Ini*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DailyPlan {
    pub date: LocalDate,
    pub size: PlanSize,
    /// After a break: a short, light session (spec §6, "welcome back").
    pub comeback: bool,
    pub items: Vec<PlanItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanItem {
    /// Stable within the day, e.g. `review`, `lesson:bipa1.u03`, `weak:writing:-kan`.
    pub id: String,
    pub kind: PlanItemKind,
    pub minutes: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanItemKind {
    Review {
        cards: u32,
    },
    Lesson {
        unit_id: String,
    },
    WeakSpot {
        skill: Skill,
        topic: String,
    },
    /// A short read or listen at the learner's level.
    Reading,
}

// --- recap and report --------------------------------------------------------

/// The 30-second end-of-day recap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DayRecap {
    pub date: LocalDate,
    pub minutes: u32,
    pub goal_met: bool,
    pub items_done: u32,
    pub reviews: u32,
    pub new_words: Vec<String>,
    /// The one thing to remember: the topic missed most today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remember: Option<Focus>,
    /// Tomorrow's first step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tomorrow: Option<PlanItemKind>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Focus {
    pub skill: Skill,
    pub topic: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeeklyReport {
    /// The Monday the week starts on.
    pub week_start: LocalDate,
    pub minutes: u32,
    pub days_studied: u32,
    pub new_words: u32,
    /// Share of reviews answered correctly, 0.0–1.0; `None` without reviews.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention: Option<f32>,
    /// Pelita at the end of the week.
    pub streak: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strongest: Option<Skill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weakest: Option<Skill>,
    pub league: LeagueResult,
    /// Next week's focus, most important first.
    pub next_focus: Vec<Focus>,
}

/// Liga Pribadi: this week against the learner's own recent weeks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeagueResult {
    /// XP this week.
    pub xp: u32,
    /// 1 = best of the weeks compared.
    pub rank: u32,
    /// How many weeks were compared, including this one.
    pub of: u32,
}

// --- reminders ---------------------------------------------------------------

/// Synced reminder state, so a snooze on the phone also quiets the tablet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReminderState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until_ms: Option<u64>,
    /// Last reminder sent, so a day never gets more than the style allows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sent_ms: Option<u64>,
    #[serde(default)]
    pub sent_today: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_through_days() {
        for (y, m, d) in [(1970, 1, 1), (2000, 2, 29), (2026, 10, 26), (2027, 3, 1), (1969, 12, 31)] {
            let date = LocalDate::from_ymd(y, m, d).unwrap();
            assert_eq!(date.ymd(), (y, m, d));
        }
        assert_eq!(LocalDate::from_ymd(1970, 1, 1), Some(LocalDate(0)));
        assert_eq!(LocalDate::from_ymd(2026, 2, 29), None);
        assert_eq!(LocalDate::from_ymd(2026, 2, 28).unwrap().add_days(1).ymd(), (2026, 3, 1));
    }

    #[test]
    fn weekdays_and_weeks() {
        let d = LocalDate::from_ymd(2026, 10, 26).unwrap();
        assert_eq!(d.weekday(), Weekday::Mon);
        assert_eq!(LocalDate::from_ymd(2026, 10, 29).unwrap().weekday(), Weekday::Thu);
        assert_eq!(LocalDate::from_ymd(2026, 11, 1).unwrap().week_start(), d);
        assert_eq!(LocalDate(0).weekday(), Weekday::Thu);
    }

    #[test]
    fn local_day_follows_the_offset() {
        // 2026-10-26 22:30 UTC is already the 27th in Jakarta (+7) but not in Cairo (+2).
        let ms = to_unix_ms(LocalDate::from_ymd(2026, 10, 26).unwrap(), LocalTime::hm(22, 30), 0);
        assert_eq!(LocalDate::from_unix_ms(ms, 420).to_string(), "2026-10-27");
        assert_eq!(LocalDate::from_unix_ms(ms, 120).to_string(), "2026-10-27");
        assert_eq!(LocalDate::from_unix_ms(ms, 60).to_string(), "2026-10-26");
        assert_eq!(LocalTime::from_unix_ms(ms, 120), LocalTime::hm(0, 30));
    }

    #[test]
    fn serde_uses_readable_strings() {
        let json = serde_json::to_string(&(LocalDate::from_ymd(2026, 10, 26).unwrap(), LocalTime::hm(7, 5)))
            .unwrap();
        assert_eq!(json, r#"["2026-10-26","07:05"]"#);
        assert!(serde_json::from_str::<LocalDate>(r#""2026-13-01""#).is_err());
        assert!(serde_json::from_str::<LocalTime>(r#""24:00""#).is_err());
    }

    #[test]
    fn quiet_hours_wrap_midnight() {
        let q = QuietHours::default();
        assert!(q.contains(LocalTime::hm(23, 0)));
        assert!(q.contains(LocalTime::hm(3, 0)));
        assert!(!q.contains(LocalTime::hm(8, 0)));
        assert!(!q.contains(LocalTime::hm(20, 0)));
        let day = QuietHours { start: LocalTime::hm(13, 0), end: LocalTime::hm(14, 0) };
        assert!(day.contains(LocalTime::hm(13, 30)) && !day.contains(LocalTime::hm(14, 0)));
    }

    #[test]
    fn profile_and_events_round_trip() {
        let p = CoachProfile {
            name: "Mohamed".into(),
            motivations: vec![Motivation::Travel],
            ..Default::default()
        };
        let back: CoachProfile = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        let kind =
            ActivityKind::Practice { skill: Skill::Writing, topic: "-kan".into(), correct: 3, total: 5 };
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.starts_with(r#"{"type":"practice""#));
        assert_eq!(serde_json::from_str::<ActivityKind>(&json).unwrap(), kind);
    }
}
