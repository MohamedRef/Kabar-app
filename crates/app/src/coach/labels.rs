//! How coach data reads in the UI language.

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::coach::{
    LanguageFocus, LocalDate, LocalTime, Motivation, PlanItem, PlanItemKind, ReminderStyle, Skill,
    StartingPoint, Weekday,
};

use crate::Ui;
use crate::i18n::Msg;

/// Curriculum levels a target can name (spec §21): the ids the core checks.
pub use kabar_core::coach::onboarding::LEVELS;

/// "bipa-2" → "BIPA 2", "j1" → "J1".
pub fn level_name(id: &str) -> String {
    match id.strip_prefix("bipa").map(|n| n.trim_start_matches('-')) {
        Some(n) if !n.is_empty() => format!("BIPA {n}"),
        _ => id.to_uppercase(),
    }
}

pub fn motivation(m: Motivation) -> Msg {
    match m {
        Motivation::Travel => Msg::WhyTravel,
        Motivation::Work => Msg::WhyWork,
        Motivation::FamilyFriends => Msg::WhyFamily,
        Motivation::Media => Msg::WhyMedia,
        Motivation::Faith => Msg::WhyFaith,
        Motivation::Curiosity => Msg::WhyCuriosity,
    }
}

pub const MOTIVATIONS: [Motivation; 6] = [
    Motivation::Travel,
    Motivation::Work,
    Motivation::FamilyFriends,
    Motivation::Media,
    Motivation::Faith,
    Motivation::Curiosity,
];

/// Language names stay in their own language.
pub fn focus(f: LanguageFocus) -> Option<&'static str> {
    match f {
        LanguageFocus::Indonesian => Some("Bahasa Indonesia"),
        LanguageFocus::Javanese => Some("Basa Jawa"),
        LanguageFocus::Both => None,
    }
}

pub const FOCUSES: [LanguageFocus; 3] =
    [LanguageFocus::Indonesian, LanguageFocus::Javanese, LanguageFocus::Both];

pub fn reminder_style(s: ReminderStyle) -> (Msg, Msg) {
    match s {
        ReminderStyle::Gentle => (Msg::StyleGentle, Msg::StyleGentleHint),
        ReminderStyle::Steady => (Msg::StyleSteady, Msg::StyleSteadyHint),
        ReminderStyle::Off => (Msg::StyleOff, Msg::StyleOffHint),
    }
}

pub const STYLES: [ReminderStyle; 3] = [ReminderStyle::Gentle, ReminderStyle::Steady, ReminderStyle::Off];

pub fn starting_point(s: StartingPoint) -> (Msg, Msg) {
    match s {
        StartingPoint::BrandNew => (Msg::StartNew, Msg::StartNewHint),
        StartingPoint::KnowsBasics => (Msg::StartBasics, Msg::StartBasicsHint),
        StartingPoint::CanChat => (Msg::StartChat, Msg::StartChatHint),
    }
}

pub const STARTS: [StartingPoint; 3] =
    [StartingPoint::BrandNew, StartingPoint::KnowsBasics, StartingPoint::CanChat];

pub fn weekday(w: Weekday) -> Msg {
    match w {
        Weekday::Mon => Msg::Mon,
        Weekday::Tue => Msg::Tue,
        Weekday::Wed => Msg::Wed,
        Weekday::Thu => Msg::Thu,
        Weekday::Fri => Msg::Fri,
        Weekday::Sat => Msg::Sat,
        Weekday::Sun => Msg::Sun,
    }
}

pub fn skill(s: Skill) -> Msg {
    match s {
        Skill::Reading => Msg::SkillReading,
        Skill::Listening => Msg::SkillListening,
        Skill::Writing => Msg::SkillWriting,
        Skill::Speaking => Msg::SkillSpeaking,
        Skill::Vocabulary => Msg::SkillVocabulary,
    }
}

/// "دقيقتين", "5 دقايق", "15 دقيقة", "15 min".
pub fn minutes(ui: &Ui, n: u32) -> String {
    minutes_text(ui.lang(), n, &ui.num(n))
}

/// Arabic number agreement: 1 and 2 are said by the noun alone (dual for 2),
/// 3–10 take the plural, everything else the singular. `num` is `n` in the
/// learner's digits.
pub fn minutes_text(lang: UiLanguage, n: u32, num: &str) -> String {
    match (lang, n) {
        (UiLanguage::En, _) => format!("{num} min"),
        (_, 1) => "دقيقة".into(),
        (UiLanguage::ArEg, 2) => "دقيقتين".into(),
        (UiLanguage::Ar, 2) => "دقيقتان".into(),
        (UiLanguage::ArEg, 3..=10) => format!("{num} دقايق"),
        (UiLanguage::Ar, 3..=10) => format!("{num} دقائق"),
        _ => format!("{num} دقيقة"),
    }
}

/// A short date: "26/10", or "26/10/2025" outside the current year.
pub fn date(ui: &Ui, d: LocalDate, today: LocalDate) -> String {
    let (y, m, day) = d.ymd();
    let short = format!("{}/{}", ui.num(day), ui.num(m));
    if y == today.ymd().0 { short } else { format!("{short}/{}", ui.num(y as u32)) }
}

/// A month's name in the UI language (1 = January).
pub fn month(ui: &Ui, m: u32) -> &'static str {
    const AR: [&str; 12] = [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ];
    const EN: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let names = match ui.settings.read().ui_language {
        UiLanguage::ArEg | UiLanguage::Ar => AR,
        UiLanguage::En => EN,
    };
    names[(m.clamp(1, 12) - 1) as usize]
}

/// A month label for chart axes: "Jul" / "يوليو". Never "1/7", which reads
/// as a date (#434).
pub fn month_short(ui: &Ui, m: u32) -> &'static str {
    let name = month(ui, m);
    match ui.settings.read().ui_language {
        UiLanguage::En => &name[..3],
        UiLanguage::ArEg | UiLanguage::Ar => name,
    }
}

/// "1 مارس 2028" / "1 March 2028": a month name, never an ambiguous
/// "1/3/2028", for dates the learner plans around (targets).
pub fn long_date(ui: &Ui, d: LocalDate) -> String {
    let (y, m, day) = d.ymd();
    format!("{} {} {}", ui.num(day), month(ui, m), ui.num(y as u32))
}

/// "05" with the UI's digits: two places, for 24-hour clock fields.
pub fn two_digits(ui: &Ui, n: u16) -> String {
    if n < 10 { format!("{}{}", ui.num(0u16), ui.num(n)) } else { ui.num(n) }
}

/// "Monday 26/10".
pub fn day_heading(ui: &Ui, d: LocalDate, today: LocalDate) -> String {
    if d == today {
        return ui.t(Msg::TodayWord).to_string();
    }
    if d == today.add_days(-1) {
        return ui.t(Msg::Yesterday).to_string();
    }
    format!("{} {}", ui.t(weekday(d.weekday())), date(ui, d, today))
}

/// "20:00" with the UI's digits.
pub fn time(ui: &Ui, t: LocalTime) -> String {
    format!("{}:{}", two_digits(ui, t.hour()), two_digits(ui, t.minute()))
}

/// A plan item's title, e.g. "Review · 12 cards".
pub fn plan_item(ui: &Ui, kind: &PlanItemKind) -> (Msg, Option<String>) {
    match kind {
        PlanItemKind::Review { cards } if *cards > 0 => {
            (Msg::ItemReview, Some(format!("{} {}", ui.num(*cards), ui.t(Msg::Cards))))
        }
        PlanItemKind::Review { .. } => (Msg::ItemReview, None),
        PlanItemKind::Lesson { unit_id } => {
            let lang = ui.settings.read().ui_language;
            (
                Msg::ItemLesson,
                Some(crate::journey::unit_title(lang, unit_id).unwrap_or_else(|| unit_id.clone())),
            )
        }
        PlanItemKind::WeakSpot { skill: s, topic } => {
            (Msg::ItemWeakSpot, Some(format!("{} · {topic}", ui.t(skill(*s)))))
        }
        PlanItemKind::Reading => (Msg::ItemReading, None),
    }
}

/// The same title as one string, for Kak Nur's `{step}`.
pub fn plan_item_text(ui: &Ui, kind: &PlanItemKind) -> String {
    match plan_item(ui, kind) {
        // Isolated so a Latin title keeps its punctuation inside Arabic text.
        (msg, Some(detail)) => format!("{} (\u{2068}{detail}\u{2069})", ui.t(msg)),
        (msg, None) => ui.t(msg).to_string(),
    }
}

/// Kak Nur's `{count}` (reviews due; empty when none, so a variant without
/// it is picked) and `{topic}` (the first non-review step) for a plan.
pub fn plan_voice_args(ui: &Ui, items: &[PlanItem]) -> (String, String) {
    let count = items
        .iter()
        .find_map(|i| match i.kind {
            PlanItemKind::Review { cards } if cards > 0 => Some(ui.num(cards)),
            _ => None,
        })
        .unwrap_or_default();
    let topic = items
        .iter()
        .find(|i| !matches!(i.kind, PlanItemKind::Review { .. }))
        .map(|i| plan_item_text(ui, &i.kind))
        .unwrap_or_default();
    (count, topic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_agree_with_the_number() {
        let eg = |n: u32| minutes_text(UiLanguage::ArEg, n, &n.to_string());
        assert_eq!(eg(1), "دقيقة");
        assert_eq!(eg(2), "دقيقتين");
        assert_eq!(eg(5), "5 دقايق");
        assert_eq!(eg(15), "15 دقيقة");
        assert_eq!(minutes_text(UiLanguage::Ar, 2, "٢"), "دقيقتان");
        assert_eq!(minutes_text(UiLanguage::Ar, 7, "٧"), "٧ دقائق");
        assert_eq!(minutes_text(UiLanguage::En, 2, "2"), "2 min");
    }

    #[test]
    fn level_names() {
        assert_eq!(level_name("bipa-2"), "BIPA 2");
        assert_eq!(level_name("bipa2"), "BIPA 2");
        assert_eq!(level_name("j3"), "J3");
        // The target picker offers exactly the ids `onboarding::finish` accepts.
        assert!(LEVELS.iter().all(|l| kabar_core::coach::onboarding::is_known_level(l)));
        assert_eq!(level_name(LEVELS[1]), "BIPA 2");
    }
}
