//! How coach data reads in the UI language.

use kabar_core::locale::UiLanguage;
use kabar_protocol::coach::{
    LanguageFocus, LocalDate, LocalTime, Motivation, PlanItem, PlanItemKind, ReminderStyle, Skill,
    StartingPoint, Weekday,
};

use crate::Ui;
use crate::i18n::Msg;

/// Curriculum levels a target can name (spec §21).
pub const LEVELS: [&str; 13] = [
    "bipa-1", "bipa-2", "bipa-3", "bipa-4", "bipa-5", "bipa-6", "bipa-7", "j1", "j2", "j3", "j4", "j5", "j6",
];

/// "bipa-2" → "BIPA 2", "j1" → "J1".
pub fn level_name(id: &str) -> String {
    match id.strip_prefix("bipa-") {
        Some(n) => format!("BIPA {n}"),
        None => id.to_uppercase(),
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

/// "15 دقيقة", "5 دقايق", "15 min".
pub fn minutes(ui: &Ui, n: u32) -> String {
    let lang = ui.lang();
    let unit = match (lang, n) {
        (UiLanguage::En, _) => "min",
        (UiLanguage::ArEg, 3..=10) => "دقايق",
        (UiLanguage::Ar, 3..=10) => "دقائق",
        _ => "دقيقة",
    };
    format!("{} {unit}", ui.num(n))
}

/// A short date: "26/10", or "26/10/2025" outside the current year.
pub fn date(ui: &Ui, d: LocalDate, today: LocalDate) -> String {
    let (y, m, day) = d.ymd();
    let short = format!("{}/{}", ui.num(day), ui.num(m));
    if y == today.ymd().0 { short } else { format!("{short}/{}", ui.num(y as u32)) }
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
    let pad = |n: u16| if n < 10 { format!("{}{}", ui.num(0u16), ui.num(n)) } else { ui.num(n) };
    format!("{}:{}", pad(t.hour()), pad(t.minute()))
}

/// A plan item's title, e.g. "Review · 12 cards".
pub fn plan_item(ui: &Ui, kind: &PlanItemKind) -> (Msg, Option<String>) {
    match kind {
        PlanItemKind::Review { cards } if *cards > 0 => {
            (Msg::ItemReview, Some(format!("{} {}", ui.num(*cards), ui.t(Msg::Cards))))
        }
        PlanItemKind::Review { .. } => (Msg::ItemReview, None),
        PlanItemKind::Lesson { unit_id } => (Msg::ItemLesson, Some(unit_id.clone())),
        PlanItemKind::WeakSpot { skill: s, topic } => {
            (Msg::ItemWeakSpot, Some(format!("{} · {topic}", ui.t(skill(*s)))))
        }
        PlanItemKind::Reading => (Msg::ItemReading, None),
    }
}

/// The same title as one string, for Kak Nur's `{step}`.
pub fn plan_item_text(ui: &Ui, kind: &PlanItemKind) -> String {
    match plan_item(ui, kind) {
        (msg, Some(detail)) => format!("{} ({detail})", ui.t(msg)),
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
    fn level_names() {
        assert_eq!(level_name("bipa-2"), "BIPA 2");
        assert_eq!(level_name("j3"), "J3");
    }
}
