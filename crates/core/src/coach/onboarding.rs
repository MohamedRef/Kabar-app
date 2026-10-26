//! Onboarding: why, minutes per day, a target, study times, reminder style,
//! then where the journey starts.

use kabar_protocol::coach::{CoachProfile, LocalDate, PlanSize, StartingPoint, Target};

/// Curriculum level ids (spec §21): Indonesian BIPA 1–7, then Javanese J1–J6.
pub const LEVELS: [&str; 13] =
    ["bipa1", "bipa2", "bipa3", "bipa4", "bipa5", "bipa6", "bipa7", "j1", "j2", "j3", "j4", "j5", "j6"];

/// Rough hours of study to finish each BIPA level, from the level before it;
/// used only for a suggested target date. BIPA programmes list roughly
/// 100–136 contact hours per level, so the levels climb through that range
/// (later levels need more).
pub const BIPA_HOURS: [u32; 7] = [100, 106, 112, 118, 124, 130, 136];

/// Days a week we assume the learner actually studies, for target dates.
pub const STUDY_DAYS_PER_WEEK: u32 = 6;

/// First units by starting point (curriculum ids, `<level>.uNN`).
pub const FIRST_UNIT_BRAND_NEW: &str = "bipa1.u01";
pub const FIRST_UNIT_KNOWS_BASICS: &str = "bipa2.u01";
pub const FIRST_UNIT_CAN_CHAT: &str = "bipa3.u01";

/// The onboarding screens, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Name,
    Why,
    Focus,
    Minutes,
    Target,
    Times,
    Reminders,
    Start,
    Done,
}

impl Step {
    pub const ALL: [Self; 9] = [
        Self::Name,
        Self::Why,
        Self::Focus,
        Self::Minutes,
        Self::Target,
        Self::Times,
        Self::Reminders,
        Self::Start,
        Self::Done,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    /// The next screen; `Done` stays `Done`.
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1).min(Self::ALL.len() - 1)]
    }

    /// The previous screen; `Name` stays `Name`.
    pub fn prev(self) -> Self {
        Self::ALL[self.index().saturating_sub(1)]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    NoStudyTime,
    TargetInPast,
    UnknownLevel(String),
}

/// Whether `level` is a curriculum level id (see [`LEVELS`]).
pub fn is_known_level(level: &str) -> bool {
    LEVELS.contains(&level)
}

/// Checks a finished profile; sorts and dedups study times.
///
/// Also trims the name, drops an empty "why" note and sorts motivations.
pub fn finish(mut profile: CoachProfile, today: LocalDate) -> Result<CoachProfile, ProfileError> {
    profile.study_times.sort_unstable();
    profile.study_times.dedup();
    if profile.study_times.is_empty() {
        return Err(ProfileError::NoStudyTime);
    }
    if let Some(target) = &profile.target {
        if !is_known_level(&target.level) {
            return Err(ProfileError::UnknownLevel(target.level.clone()));
        }
        if target.by <= today {
            return Err(ProfileError::TargetInPast);
        }
    }
    profile.name = profile.name.trim().to_owned();
    profile.why_note = profile.why_note.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
    profile.motivations.sort_unstable();
    profile.motivations.dedup();
    Ok(profile)
}

/// A realistic default target for this pace and starting point.
///
/// Brand new aims at BIPA 1, "knows the basics" at BIPA 2, "can chat" at
/// BIPA 3. The date assumes [`BIPA_HOURS`] for that level at the daily
/// minutes, six study days a week, rounded up to the first of a month.
pub fn suggest_target(start: StartingPoint, size: PlanSize, today: LocalDate) -> Target {
    let level = match start {
        StartingPoint::BrandNew => 1,
        StartingPoint::KnowsBasics => 2,
        StartingPoint::CanChat => 3,
    };
    let minutes = BIPA_HOURS[level - 1] * 60;
    let study_days = minutes.div_ceil(u32::from(size.minutes()));
    let days = (study_days * 7).div_ceil(STUDY_DAYS_PER_WEEK);
    Target { level: format!("bipa{level}"), by: first_of_next_month(today.add_days(days as i32)) }
}

/// The 1st of the month after `date` (or `date` itself if it is a 1st).
fn first_of_next_month(date: LocalDate) -> LocalDate {
    let (y, m, d) = date.ymd();
    if d == 1 {
        return date;
    }
    let (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    LocalDate::from_ymd(y, m, 1).unwrap_or(date)
}

/// The first unit of the journey for a starting point (refined by the M8
/// placement test).
pub fn starting_unit(start: StartingPoint) -> &'static str {
    match start {
        StartingPoint::BrandNew => FIRST_UNIT_BRAND_NEW,
        StartingPoint::KnowsBasics => FIRST_UNIT_KNOWS_BASICS,
        StartingPoint::CanChat => FIRST_UNIT_CAN_CHAT,
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::{LocalTime, Motivation};

    use super::*;
    use crate::coach::history::tests::d;

    #[test]
    fn steps_walk_in_order_and_stop_at_the_ends() {
        assert_eq!(Step::Name.prev(), Step::Name);
        assert_eq!(Step::Done.next(), Step::Done);
        for pair in Step::ALL.windows(2) {
            assert_eq!(pair[0].next(), pair[1]);
            assert_eq!(pair[1].prev(), pair[0]);
        }
        let mut s = Step::Name;
        for _ in 0..20 {
            s = s.next();
        }
        assert_eq!(s, Step::Done);
    }

    fn profile() -> CoachProfile {
        CoachProfile {
            name: "  Mohamed ".into(),
            motivations: vec![Motivation::Travel, Motivation::Faith, Motivation::Travel],
            why_note: Some("   ".into()),
            study_times: vec![LocalTime::hm(20, 0), LocalTime::hm(7, 30), LocalTime::hm(20, 0)],
            target: Some(Target { level: "bipa2".into(), by: d(2027, 3, 1) }),
            ..CoachProfile::default()
        }
    }

    #[test]
    fn finish_tidies_a_good_profile() {
        let p = finish(profile(), d(2026, 10, 26)).unwrap();
        assert_eq!(p.name, "Mohamed");
        assert_eq!(p.study_times, [LocalTime::hm(7, 30), LocalTime::hm(20, 0)]);
        assert_eq!(p.motivations, [Motivation::Travel, Motivation::Faith]);
        assert_eq!(p.why_note, None);
        let note = CoachProfile { why_note: Some(" family in Surabaya ".into()), ..profile() };
        assert_eq!(finish(note, d(2026, 10, 26)).unwrap().why_note.as_deref(), Some("family in Surabaya"));
        let no_target = CoachProfile { target: None, ..profile() };
        assert!(finish(no_target, d(2026, 10, 26)).is_ok());
    }

    #[test]
    fn finish_rejects_what_cannot_work() {
        let today = d(2026, 10, 26);
        let p = CoachProfile { study_times: vec![], ..profile() };
        assert_eq!(finish(p, today), Err(ProfileError::NoStudyTime));
        let p = CoachProfile { target: Some(Target { level: "bipa2".into(), by: today }), ..profile() };
        assert_eq!(finish(p, today), Err(ProfileError::TargetInPast));
        for bad in ["bipa-2", "bipa8", "j7", "BIPA2", ""] {
            let p =
                CoachProfile { target: Some(Target { level: bad.into(), by: d(2027, 3, 1) }), ..profile() };
            assert_eq!(finish(p, today), Err(ProfileError::UnknownLevel(bad.into())));
        }
        for good in LEVELS {
            let p =
                CoachProfile { target: Some(Target { level: good.into(), by: d(2027, 3, 1) }), ..profile() };
            assert!(finish(p, today).is_ok(), "{good}");
        }
    }

    #[test]
    fn suggested_targets_fit_the_pace() {
        let today = d(2026, 10, 26);
        // 100 h at 15 min a day: 400 study days, 467 calendar days → 2028-02-05 → 2028-03-01.
        let t = suggest_target(StartingPoint::BrandNew, PlanSize::Steady, today);
        assert_eq!(t, Target { level: "bipa1".into(), by: d(2028, 3, 1) });
        // 30 min a day: 234 days → 2027-06-17 → 2027-07-01.
        assert_eq!(suggest_target(StartingPoint::BrandNew, PlanSize::Deep, today).by, d(2027, 7, 1));
        assert!(BIPA_HOURS.windows(2).all(|w| w[0] < w[1]));
        assert!(BIPA_HOURS.iter().all(|h| (100..=136).contains(h)));
        assert_eq!(suggest_target(StartingPoint::KnowsBasics, PlanSize::Deep, today).level, "bipa2");
        assert_eq!(suggest_target(StartingPoint::CanChat, PlanSize::Deep, today).level, "bipa3");
        for start in [StartingPoint::BrandNew, StartingPoint::KnowsBasics, StartingPoint::CanChat] {
            let quick = suggest_target(start, PlanSize::Quick, today);
            let steady = suggest_target(start, PlanSize::Steady, today);
            let deep = suggest_target(start, PlanSize::Deep, today);
            assert!(quick.by > steady.by && steady.by > deep.by && deep.by > today);
            assert_eq!(deep.by.ymd().2, 1);
            // Every suggestion passes finish().
            let p = CoachProfile { target: Some(quick), ..profile() };
            assert!(finish(p, today).is_ok());
        }
    }

    #[test]
    fn rounding_to_the_first_of_a_month() {
        assert_eq!(first_of_next_month(d(2026, 12, 15)), d(2027, 1, 1));
        assert_eq!(first_of_next_month(d(2027, 3, 1)), d(2027, 3, 1));
        assert_eq!(first_of_next_month(d(2027, 2, 28)), d(2027, 3, 1));
    }

    #[test]
    fn journeys_start_at_the_right_unit() {
        assert_eq!(starting_unit(StartingPoint::BrandNew), "bipa1.u01");
        assert_eq!(starting_unit(StartingPoint::KnowsBasics), "bipa2.u01");
        assert_eq!(starting_unit(StartingPoint::CanChat), "bipa3.u01");
    }
}
