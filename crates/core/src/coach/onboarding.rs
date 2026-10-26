//! Onboarding: why, minutes per day, a target, study times, reminder style,
//! then where the journey starts.

use std::sync::LazyLock;

use kabar_protocol::coach::{CoachProfile, LocalDate, PlanSize, StartingPoint, Target};

use super::plan::LESSON_MINUTES;
use crate::journey::parse_curriculum;

/// Curriculum level ids (spec §21): Indonesian BIPA 1–7, then Javanese J1–J6.
pub const LEVELS: [&str; 13] =
    ["bipa1", "bipa2", "bipa3", "bipa4", "bipa5", "bipa6", "bipa7", "j1", "j2", "j3", "j4", "j5", "j6"];

/// Share of a planned study day that goes to new lessons: a 15-minute day
/// holds one [`LESSON_MINUTES`]-minute lesson, the rest is reviews, weak
/// spots and reading. Dividing a level's lesson time by it gives the whole
/// time the level takes.
pub const LESSON_SHARE: f32 = LESSON_MINUTES as f32 / 15.0;

/// Hours of study to finish each BIPA level, from the level before it, for
/// the suggested target and the pace forecast. Measured from Kabar's own
/// curriculum (every lesson's minutes plus the level test, over
/// [`LESSON_SHARE`]), not classroom contact hours: docs/PEDAGOGY.md §5 keeps
/// those as planning ceilings, never as a promise of dates.
pub static BIPA_HOURS: LazyLock<[u32; 7]> = LazyLock::new(|| {
    let curriculum = parse_curriculum(CURRICULUM_ID).expect("curriculum/indonesian.yaml is valid");
    let mut hours = [0; 7];
    for (i, h) in hours.iter_mut().enumerate() {
        let Some(level) = curriculum.level(&format!("bipa{}", i + 1)) else { continue };
        let minutes: u32 = level.lessons().map(|l| l.minutes).sum::<u32>()
            + level.level_test.as_ref().map_or(0, |t| t.minutes);
        *h = ((minutes as f32 / LESSON_SHARE / 60.0).ceil() as u32).max(1);
    }
    hours
});

const CURRICULUM_ID: &str = include_str!("../../../../curriculum/indonesian.yaml");

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
    /// Where to start comes before the target, which is suggested from it.
    Start,
    Minutes,
    Target,
    Times,
    Reminders,
    Done,
}

impl Step {
    pub const ALL: [Self; 9] = [
        Self::Name,
        Self::Why,
        Self::Focus,
        Self::Start,
        Self::Minutes,
        Self::Target,
        Self::Times,
        Self::Reminders,
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
/// minutes, six study days a week, rounded up to the first of a month: a
/// few months for a level, not years.
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
        // The target is suggested from the start level and the daily size.
        assert!(Step::Start.index() < Step::Target.index());
        assert!(Step::Minutes.index() < Step::Target.index());
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
        // Hours come from the curriculum: every level has some, and none
        // is anywhere near the classroom ceilings (100+ h) that once gave
        // "BIPA 1 by March 2028" at 15 minutes a day.
        assert!(BIPA_HOURS.iter().all(|h| (1..60).contains(h)), "{:?}", *BIPA_HOURS);
        let bipa1_days = |size: PlanSize| {
            let study = (BIPA_HOURS[0] * 60).div_ceil(u32::from(size.minutes()));
            (study * 7).div_ceil(STUDY_DAYS_PER_WEEK) as i32
        };
        let t = suggest_target(StartingPoint::BrandNew, PlanSize::Steady, today);
        assert_eq!(
            t,
            Target {
                level: "bipa1".into(),
                by: first_of_next_month(today.add_days(bipa1_days(PlanSize::Steady)))
            }
        );
        // BIPA 1 at 15 minutes a day is months away, not years.
        assert!(t.by > today.add_days(30) && t.by < today.add_days(365), "{:?}", t.by);
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
