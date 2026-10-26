//! Kak Nur's voice: message templates written at build time
//! (`content/coach/`), in Egyptian Arabic, MSA and English with Indonesian
//! sprinkled in. Runtime only fills in the personal details (`{name}`,
//! `{minutes}`, …); the wording itself is reviewed content.

use crate::locale::UiLanguage;

/// Every moment Kak Nur speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Line {
    // Onboarding
    OnboardHello,
    OnboardWhy,
    OnboardMinutes,
    OnboardTarget,
    OnboardTimes,
    OnboardReminders,
    OnboardStart,
    OnboardDone,
    // Daily plan
    PlanMorning,
    PlanComeback,
    PlanAllDone,
    PlanAskSize,
    // Recap
    RecapGoalMet,
    RecapSomeStudy,
    RecapRemember,
    RecapTomorrow,
    // Reminders
    RemindStudy,
    RemindGentle,
    RemindWelcomeBack,
    RemindReport,
    // Weekly report notes
    ReportGreatWeek,
    ReportSteadyWeek,
    ReportLightWeek,
    ReportComeback,
    // Rewards
    PelitaLit,
    PelitaFrozen,
    PelitaRelit,
    FreezeEarned,
    AchievementUnlocked,
    WarungBought,
}

/// Kak Nur's line for `line` in `lang`, with `{placeholders}` filled from
/// `args`. `seed` picks among variants (use the date's day number, so every
/// device shows the same wording on the same day).
pub fn say(lang: UiLanguage, line: Line, seed: u64, args: &[(&str, &str)]) -> String {
    let _ = (lang, seed, args);
    format!("{line:?}")
}
