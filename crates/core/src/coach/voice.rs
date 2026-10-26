//! Kak Nur's voice: message templates written at build time
//! (`content/coach/kak-nur.toml`), in Egyptian Arabic, MSA and English with
//! Indonesian sprinkled in. Runtime only fills in the personal details
//! (`{name}`, `{minutes}`, …); the wording itself is reviewed content.
//!
//! Template syntax (documented for reviewers in `content/coach/README.md`):
//! - `{name}` is replaced by the argument's value.
//! - `{minutes|one|two|few|many}` (Arabic) or `{minutes|one|other}` (English)
//!   renders a count with the right noun form, e.g. `{words|word|words}`.
//!
//! The tests at the bottom are the content gate: every line exists in every
//! language, placeholders are declared, Latin text in Arabic lines is
//! isolated, and no guilt, fear or fake urgency gets in.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::locale::UiLanguage;

/// Every moment Kak Nur speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Line {
    // Onboarding
    OnboardHello,
    OnboardWhy,
    /// Indonesian, Javanese or both (the onboarding `Focus` step).
    OnboardFocus,
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
    /// Halfway through a 30-minute plan: a break is offered between the blocks.
    PlanBreak,
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
    // Monthly level check
    /// Invitation to the monthly 10-minute level check.
    LevelCheckInvite,
    /// "At this pace you reach {level} around {date}."
    LevelForecast,
    /// The check moved the learner up a level.
    LevelUp,
    /// The check moved the learner down a level: a plan to rebuild, no guilt.
    LevelRebuild,
}

impl Line {
    pub const ALL: [Self; 36] = [
        Self::OnboardHello,
        Self::OnboardWhy,
        Self::OnboardFocus,
        Self::OnboardMinutes,
        Self::OnboardTarget,
        Self::OnboardTimes,
        Self::OnboardReminders,
        Self::OnboardStart,
        Self::OnboardDone,
        Self::PlanMorning,
        Self::PlanComeback,
        Self::PlanAllDone,
        Self::PlanAskSize,
        Self::PlanBreak,
        Self::RecapGoalMet,
        Self::RecapSomeStudy,
        Self::RecapRemember,
        Self::RecapTomorrow,
        Self::RemindStudy,
        Self::RemindGentle,
        Self::RemindWelcomeBack,
        Self::RemindReport,
        Self::ReportGreatWeek,
        Self::ReportSteadyWeek,
        Self::ReportLightWeek,
        Self::ReportComeback,
        Self::PelitaLit,
        Self::PelitaFrozen,
        Self::PelitaRelit,
        Self::FreezeEarned,
        Self::AchievementUnlocked,
        Self::WarungBought,
        Self::LevelCheckInvite,
        Self::LevelForecast,
        Self::LevelUp,
        Self::LevelRebuild,
    ];

    /// The line's key in `content/coach/kak-nur.toml` (`[lines.<key>]`).
    pub fn key(self) -> &'static str {
        match self {
            Self::OnboardHello => "onboard_hello",
            Self::OnboardWhy => "onboard_why",
            Self::OnboardFocus => "onboard_focus",
            Self::OnboardMinutes => "onboard_minutes",
            Self::OnboardTarget => "onboard_target",
            Self::OnboardTimes => "onboard_times",
            Self::OnboardReminders => "onboard_reminders",
            Self::OnboardStart => "onboard_start",
            Self::OnboardDone => "onboard_done",
            Self::PlanMorning => "plan_morning",
            Self::PlanComeback => "plan_comeback",
            Self::PlanAllDone => "plan_all_done",
            Self::PlanAskSize => "plan_ask_size",
            Self::PlanBreak => "plan_break",
            Self::RecapGoalMet => "recap_goal_met",
            Self::RecapSomeStudy => "recap_some_study",
            Self::RecapRemember => "recap_remember",
            Self::RecapTomorrow => "recap_tomorrow",
            Self::RemindStudy => "remind_study",
            Self::RemindGentle => "remind_gentle",
            Self::RemindWelcomeBack => "remind_welcome_back",
            Self::RemindReport => "remind_report",
            Self::ReportGreatWeek => "report_great_week",
            Self::ReportSteadyWeek => "report_steady_week",
            Self::ReportLightWeek => "report_light_week",
            Self::ReportComeback => "report_comeback",
            Self::PelitaLit => "pelita_lit",
            Self::PelitaFrozen => "pelita_frozen",
            Self::PelitaRelit => "pelita_relit",
            Self::FreezeEarned => "freeze_earned",
            Self::AchievementUnlocked => "achievement_unlocked",
            Self::WarungBought => "warung_bought",
            Self::LevelCheckInvite => "level_check_invite",
            Self::LevelForecast => "level_forecast",
            Self::LevelUp => "level_up",
            Self::LevelRebuild => "level_rebuild",
        }
    }

    /// The placeholders this line may use, as declared in the content file.
    pub fn placeholders(self) -> &'static [String] {
        catalog().lines.get(self.key()).map(|e| e.placeholders.as_slice()).unwrap_or(&[])
    }
}

impl Line {
    /// When Kak Nur says this line (for the content review screen).
    pub fn moment(self) -> &'static str {
        catalog().lines.get(self.key()).map(|e| e.moment.as_str()).unwrap_or_default()
    }
}

/// Whether the learner has approved Kak Nur's wording. Until then the coach
/// content is flagged unverified, like other reviewed content.
pub fn reviewed() -> bool {
    catalog().reviewed
}

/// Version of `content/coach/kak-nur.toml`, for content packs.
pub fn content_version() -> u32 {
    catalog().version
}

/// Every placeholder a template may use.
pub const PLACEHOLDERS: [&str; 13] = [
    "name", "minutes", "count", "words", "topic", "skill", "streak", "days", "item", "koin", "level", "date",
    "proverb",
];

/// Placeholders that hold a whole number and may take a plural group
/// (`{days|day|days}`). The others may hold Latin text, so Arabic lines must
/// wrap them in FSI … PDI.
pub const NUMERIC_PLACEHOLDERS: [&str; 6] = ["minutes", "count", "words", "streak", "days", "koin"];

/// First-strong isolate: opens a Latin snippet inside Arabic text.
pub const FSI: char = '\u{2068}';
/// Pop directional isolate: closes it.
pub const PDI: char = '\u{2069}';

/// Kak Nur's line for `line` in `lang`, with `{placeholders}` filled from
/// `args`. `seed` picks among variants (use the date's day number, so every
/// device shows the same wording on the same day).
///
/// Missing arguments degrade gracefully: starting at variant `seed % n`, the
/// first variant whose placeholders are all supplied is used. If none is,
/// missing text placeholders render empty (with the sentence tidied) and a
/// missing count renders just its plural noun, so no raw `{…}` ever shows.
pub fn say(lang: UiLanguage, line: Line, seed: u64, args: &[(&str, &str)]) -> String {
    let variants = variants(lang, line);
    if variants.is_empty() {
        return String::new();
    }
    let parsed: Vec<Vec<Seg<'_>>> = variants.iter().map(|v| parse(v).unwrap_or_default()).collect();
    let n = parsed.len();
    let start = (seed % n as u64) as usize;
    let complete =
        |segs: &[Seg<'_>]| segs.iter().all(|s| matches!(s, Seg::Text(_)) || arg(args, s.name()).is_some());
    let pick = (0..n).map(|i| (start + i) % n).find(|&i| complete(&parsed[i])).unwrap_or(start);
    render(lang, &parsed[pick], args)
}

// --- content ------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    reviewed: bool,
    version: u32,
    lines: BTreeMap<String, Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    /// For the reviewer: when Kak Nur says this.
    moment: String,
    placeholders: Vec<String>,
    #[serde(rename = "ar-eg")]
    ar_eg: Vec<String>,
    ar: Vec<String>,
    en: Vec<String>,
}

const SOURCE: &str = include_str!("../../../../content/coach/kak-nur.toml");

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(SOURCE).expect("content/coach/kak-nur.toml is valid (see voice tests)")
    })
}

fn variants(lang: UiLanguage, line: Line) -> &'static [String] {
    let Some(entry) = catalog().lines.get(line.key()) else { return &[] };
    match lang {
        UiLanguage::ArEg => &entry.ar_eg,
        UiLanguage::Ar => &entry.ar,
        UiLanguage::En => &entry.en,
    }
}

// --- templates ----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Seg<'a> {
    Text(&'a str),
    /// `{name}` or `{name|form|form…}`.
    Slot {
        name: &'a str,
        forms: Vec<&'a str>,
    },
}

impl Seg<'_> {
    fn name(&self) -> &str {
        match self {
            Seg::Text(_) => "",
            Seg::Slot { name, .. } => name,
        }
    }
}

fn parse(template: &str) -> Result<Vec<Seg<'_>>, String> {
    let mut segs = Vec::new();
    let mut rest = template;
    while !rest.is_empty() {
        match rest.find(['{', '}']) {
            None => {
                segs.push(Seg::Text(rest));
                break;
            }
            Some(i) if rest.as_bytes()[i] == b'}' => return Err(format!("stray '}}' in {template:?}")),
            Some(i) => {
                if i > 0 {
                    segs.push(Seg::Text(&rest[..i]));
                }
                let after = &rest[i + 1..];
                let close = after.find('}').ok_or_else(|| format!("unclosed '{{' in {template:?}"))?;
                let inner = &after[..close];
                if inner.contains('{') {
                    return Err(format!("nested '{{' in {template:?}"));
                }
                let mut parts = inner.split('|');
                let name = parts.next().unwrap_or_default();
                if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                    return Err(format!("bad placeholder {{{inner}}} in {template:?}"));
                }
                let forms: Vec<&str> = parts.collect();
                if forms.iter().any(|f| f.trim().is_empty()) {
                    return Err(format!("empty plural form in {{{inner}}} in {template:?}"));
                }
                segs.push(Seg::Slot { name, forms });
                rest = &after[close + 1..];
            }
        }
    }
    Ok(segs)
}

/// The argument's value, if supplied and not blank.
fn arg<'a>(args: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    args.iter().find(|(k, _)| *k == name).map(|(_, v)| *v).filter(|v| !v.trim().is_empty())
}

fn render(lang: UiLanguage, segs: &[Seg<'_>], args: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let mut missing = false;
    for seg in segs {
        match seg {
            Seg::Text(t) => out.push_str(t),
            Seg::Slot { name, forms } => {
                let value = arg(args, name);
                missing |= value.is_none();
                if forms.is_empty() {
                    out.push_str(value.unwrap_or_default());
                } else {
                    out.push_str(&plural(lang, value, forms));
                }
            }
        }
    }
    if missing { tidy(&out) } else { out }
}

/// A count with its noun. Arabic forms are `one|two|few[|many]`: 1 and 2 are
/// said by the noun alone ("دقيقة", "دقيقتين"), 3–10 take the plural, 11–99 the
/// `many` form (defaults to `one`, e.g. "15 دقيقة"), and 0 or 100+ the `one`
/// form. English forms are `one|other`. Without a value, the plural noun alone.
fn plural(lang: UiLanguage, value: Option<&str>, forms: &[&str]) -> String {
    let form = |i: usize| forms.get(i).or(forms.last()).copied().unwrap_or_default();
    let Some(value) = value else {
        return match lang {
            UiLanguage::En => form(1),
            UiLanguage::ArEg | UiLanguage::Ar => form(2),
        }
        .to_string();
    };
    let n = parse_count(value);
    match lang {
        UiLanguage::En => format!("{value} {}", if n == Some(1) { form(0) } else { form(1) }),
        UiLanguage::ArEg | UiLanguage::Ar => match n {
            Some(1) => form(0).to_string(),
            Some(2) => form(1).to_string(),
            Some(n) if (3..=10).contains(&(n % 100)) => format!("{value} {}", form(2)),
            Some(n) if (11..=99).contains(&(n % 100)) => {
                format!("{value} {}", forms.get(3).unwrap_or(&form(0)))
            }
            Some(_) => format!("{value} {}", form(0)),
            None => format!("{value} {}", form(2)),
        },
    }
}

/// Parses ASCII, Eastern Arabic (٠–٩) or Persian (۰–۹) digits.
fn parse_count(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.chars().try_fold(0u64, |acc, c| {
        let d = match c {
            '0'..='9' => c as u32 - '0' as u32,
            '\u{0660}'..='\u{0669}' => c as u32 - 0x0660,
            '\u{06F0}'..='\u{06F9}' => c as u32 - 0x06F0,
            _ => return None,
        };
        acc.checked_mul(10)?.checked_add(u64::from(d))
    })
}

/// Cleans up after an empty placeholder: drops empty isolates and quotes,
/// doubled spaces, spaces before punctuation and a dangling vocative "يا".
fn tidy(s: &str) -> String {
    const FIXES: [(&str, &str); 18] = [
        ("\u{2068}\u{2069}", ""),
        ("«»", ""),
        ("“”", ""),
        ("  ", " "),
        (" .", "."),
        (" ,", ","),
        (" ،", "،"),
        (" ؟", "؟"),
        (" ?", "?"),
        (" :", ":"),
        (" ;", ";"),
        (",.", "."),
        ("،.", "."),
        (":.", "."),
        (",?", "?"),
        (" يا.", "."),
        (" يا،", "،"),
        (" يا؟", "؟"),
    ];
    let mut out = s.to_string();
    loop {
        let before = out.clone();
        for (from, to) in FIXES {
            out = out.replace(from, to);
        }
        if out == before {
            break;
        }
    }
    out.trim().trim_start_matches([',', '،', '.', ':']).trim().to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const LANGS: [UiLanguage; 3] = UiLanguage::ALL;

    fn all_args() -> Vec<(&'static str, &'static str)> {
        vec![
            ("name", "Hamo"),
            ("minutes", "15"),
            ("count", "12"),
            ("words", "8"),
            ("topic", "Apa kabar?"),
            ("skill", "Reading"),
            ("streak", "6"),
            ("days", "5"),
            ("item", "Kawung Senja"),
            ("koin", "120"),
            ("level", "BIPA 2"),
            ("date", "2027-03-01"),
            ("proverb", "Sedikit demi sedikit, lama-lama menjadi bukit"),
        ]
    }

    fn entry(line: Line) -> &'static Entry {
        catalog().lines.get(line.key()).unwrap_or_else(|| panic!("{} missing from kak-nur.toml", line.key()))
    }

    fn each_variant(mut f: impl FnMut(Line, UiLanguage, &str)) {
        for line in Line::ALL {
            for lang in LANGS {
                for v in variants(lang, line) {
                    f(line, lang, v);
                }
            }
        }
    }

    fn is_arabic(lang: UiLanguage) -> bool {
        lang != UiLanguage::En
    }

    #[test]
    fn content_parses_and_awaits_review() {
        let c = catalog();
        assert!(!reviewed());
        assert_eq!(content_version(), 1);
        assert!(Line::ALL.iter().all(|l| !l.moment().is_empty()));
        assert!(!c.reviewed, "kak-nur.toml stays reviewed = false until the learner approves it");
        assert_eq!(c.version, 1);
    }

    #[test]
    fn keys_match_lines_both_ways() {
        let keys: BTreeSet<&str> = Line::ALL.iter().map(|l| l.key()).collect();
        assert_eq!(keys.len(), Line::ALL.len(), "duplicate keys");
        for line in Line::ALL {
            assert!(line.key().chars().all(|c| c.is_ascii_lowercase() || c == '_'));
            entry(line);
        }
        for key in catalog().lines.keys() {
            assert!(keys.contains(key.as_str()), "[lines.{key}] matches no voice::Line");
        }
    }

    #[test]
    fn every_line_has_variants_in_every_language() {
        // Frequent moments get variety; rare ones may have one or two.
        let frequent = [
            Line::PlanMorning,
            Line::PlanAllDone,
            Line::PlanAskSize,
            Line::RecapGoalMet,
            Line::RecapSomeStudy,
            Line::RecapRemember,
            Line::RecapTomorrow,
            Line::RemindStudy,
            Line::RemindGentle,
            Line::RemindWelcomeBack,
            Line::RemindReport,
            Line::ReportGreatWeek,
            Line::ReportSteadyWeek,
            Line::ReportLightWeek,
            Line::PelitaLit,
        ];
        for line in Line::ALL {
            for lang in LANGS {
                let n = variants(lang, line).len();
                assert!(n >= 1, "{} has no {lang:?} variant", line.key());
                assert!(n <= 4, "{} has {n} {lang:?} variants (max 4)", line.key());
                if frequent.contains(&line) {
                    assert!(n >= 2, "{} is frequent and needs 2+ {lang:?} variants", line.key());
                }
            }
        }
        each_variant(|line, lang, v| {
            assert!(!v.trim().is_empty(), "{} {lang:?}: empty variant", line.key());
            assert_eq!(v, v.trim(), "{} {lang:?}: stray whitespace", line.key());
            assert!(!v.contains("  "), "{} {lang:?}: double space in {v:?}", line.key());
        });
    }

    #[test]
    fn placeholders_are_declared_and_allowed() {
        for line in Line::ALL {
            let declared = &entry(line).placeholders;
            let set: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
            assert_eq!(set.len(), declared.len(), "{}: duplicate placeholder", line.key());
            for p in &set {
                assert!(PLACEHOLDERS.contains(p), "{}: {{{p}}} is not an allowed placeholder", line.key());
            }
            let mut used = BTreeSet::new();
            for lang in LANGS {
                for v in variants(lang, line) {
                    let segs = parse(v).unwrap_or_else(|e| panic!("{} {lang:?}: {e}", line.key()));
                    for seg in segs {
                        let Seg::Slot { name, forms } = seg else { continue };
                        assert!(
                            set.contains(name),
                            "{} {lang:?}: {{{name}}} not declared in {v:?}",
                            line.key()
                        );
                        used.insert(name);
                        let numeric = NUMERIC_PLACEHOLDERS.contains(&name);
                        if !forms.is_empty() {
                            assert!(numeric, "{} {lang:?}: plural forms on text {{{name}}}", line.key());
                            let ok = if is_arabic(lang) {
                                (3..=4).contains(&forms.len())
                            } else {
                                forms.len() == 2
                            };
                            assert!(
                                ok,
                                "{} {lang:?}: {{{name}}} has {} plural forms",
                                line.key(),
                                forms.len()
                            );
                        }
                    }
                }
            }
            for p in &set {
                assert!(used.contains(p), "{}: {{{p}}} is declared but never used", line.key());
            }
        }
        for p in NUMERIC_PLACEHOLDERS {
            assert!(PLACEHOLDERS.contains(&p));
        }
    }

    fn is_latin_letter(c: char) -> bool {
        c.is_ascii_alphabetic() || ('\u{00C0}'..='\u{024F}').contains(&c)
    }

    fn is_arabic_letter(c: char) -> bool {
        ('\u{0600}'..='\u{06FF}').contains(&c)
    }

    /// Latin letters and text placeholders in Arabic lines sit inside FSI … PDI.
    fn isolation_errors(v: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let mut depth = 0i32;
        let segs = parse(v).unwrap_or_default();
        for seg in segs {
            match seg {
                Seg::Text(t) => {
                    for c in t.chars() {
                        match c {
                            FSI => depth += 1,
                            PDI => {
                                depth -= 1;
                                if depth < 0 {
                                    errors.push("PDI without FSI".into());
                                }
                            }
                            '\u{202A}'..='\u{202E}' | '\u{2066}' | '\u{2067}' => {
                                errors.push(format!("use FSI/PDI, not U+{:04X}", c as u32))
                            }
                            c if is_latin_letter(c) && depth == 0 => {
                                errors.push(format!("Latin {c:?} outside FSI…PDI"));
                            }
                            _ => {}
                        }
                    }
                }
                Seg::Slot { name, forms } => {
                    if !NUMERIC_PLACEHOLDERS.contains(&name) && depth == 0 {
                        errors.push(format!("{{{name}}} may hold Latin text and must sit inside FSI…PDI"));
                    }
                    if forms.iter().any(|f| f.chars().any(is_latin_letter)) {
                        errors.push(format!("Latin in plural forms of {{{name}}}"));
                    }
                }
            }
        }
        if depth != 0 {
            errors.push("unbalanced FSI/PDI".into());
        }
        errors
    }

    #[test]
    fn arabic_isolates_latin_text() {
        each_variant(|line, lang, v| {
            if is_arabic(lang) {
                let errors = isolation_errors(v);
                assert!(errors.is_empty(), "{} {lang:?}: {errors:?} in {v:?}", line.key());
                assert!(v.chars().any(is_arabic_letter), "{} {lang:?}: no Arabic in {v:?}", line.key());
            } else {
                assert!(!v.contains([FSI, PDI]), "{} en: isolates belong to Arabic lines", line.key());
                assert!(!v.chars().any(is_arabic_letter), "{} en: Arabic letters in {v:?}", line.key());
            }
        });
        // The checker itself catches mistakes.
        assert!(!isolation_errors("أهلاً يا {name}").is_empty());
        assert!(!isolation_errors("خطة Rencana").is_empty());
        assert!(!isolation_errors("خطة \u{2068}Rencana").is_empty());
        assert!(isolation_errors("أهلاً يا \u{2068}{name}\u{2069}، {minutes|دقيقة|دقيقتين|دقايق}").is_empty());
    }

    /// Strips Arabic diacritics (tashkeel) and tatweel so "فوّت" matches "فوت".
    fn plain(s: &str) -> String {
        s.chars()
            .filter(|c| !('\u{064B}'..='\u{0652}').contains(c) && *c != '\u{0640}')
            .collect::<String>()
            .to_lowercase()
    }

    #[test]
    fn no_guilt_fear_or_fake_urgency() {
        let en = [
            "don't lose",
            "do not lose",
            "you missed",
            "missed",
            "hurry",
            "last chance",
            "lose your streak",
            "before it's too late",
            "too late",
            "failed",
            "failure",
            "lazy",
            "shame",
            "disappoint",
            "behind",
            "only today",
            "streak is at risk",
            "!!",
            "?!",
        ];
        let ar_eg = [
            "ما تخسرش",
            "متخسرش",
            "هتخسر",
            "خسرت",
            "فوت",
            "فاتك",
            "بسرعة",
            "آخر فرصة",
            "فشلت",
            "فاشل",
            "كسلان",
            "اتأخرت",
            "متأخر",
            "عيب",
            "زعلتني",
            "!!",
            "؟!",
        ];
        let ar = [
            "لا تخسر",
            "ستخسر",
            "خسرت",
            "فوت",
            "فاتك",
            "بسرعة",
            "أسرع",
            "عجل",
            "آخر فرصة",
            "فشلت",
            "أخفقت",
            "كسول",
            "تأخرت",
            "متأخر",
            "خيبت",
            "!!",
            "؟!",
        ];
        each_variant(|line, lang, v| {
            let text = plain(v);
            let banned: &[&str] = match lang {
                UiLanguage::En => &en,
                UiLanguage::ArEg => &ar_eg,
                UiLanguage::Ar => &ar,
            };
            for phrase in banned {
                assert!(!text.contains(&plain(phrase)), "{} {lang:?}: {phrase:?} in {v:?}", line.key());
            }
            let bangs = v.matches('!').count();
            assert!(bangs <= 1, "{} {lang:?}: {bangs} exclamation marks in {v:?}", line.key());
        });
    }

    #[test]
    fn welcome_back_has_no_streak_drama() {
        for line in [Line::PlanComeback, Line::RemindWelcomeBack, Line::PelitaRelit, Line::ReportComeback] {
            let declared = &entry(line).placeholders;
            assert!(!declared.iter().any(|p| p == "streak"), "{}: no streak talk", line.key());
            if line != Line::ReportComeback {
                assert!(!declared.iter().any(|p| p == "days"), "{}: never count the days away", line.key());
            }
            for lang in LANGS {
                for v in variants(lang, line) {
                    let text = plain(v);
                    for word in ["streak", "سلسلة", "سلسله", "break", "انقطاع", "غياب", "غبت"]
                    {
                        assert!(!text.contains(word), "{} {lang:?}: {word:?} in {v:?}", line.key());
                    }
                }
            }
        }
    }

    #[test]
    fn say_fills_placeholders_in_every_language() {
        let args = all_args();
        for line in Line::ALL {
            for lang in LANGS {
                for seed in 0..4 {
                    let s = say(lang, line, seed, &args);
                    assert!(!s.is_empty());
                    assert!(!s.contains(['{', '}', '|']), "{} {lang:?}: unfilled {s:?}", line.key());
                }
            }
        }
        let s = say(UiLanguage::En, Line::RecapRemember, 0, &[("topic", "-kan verbs")]);
        assert_eq!(s, "One thing to remember from today: -kan verbs.");
        let s = say(UiLanguage::ArEg, Line::RecapRemember, 0, &[("topic", "Apa kabar?")]);
        assert_eq!(s, "حاجة واحدة تفتكرها من النهارده: \u{2068}Apa kabar?\u{2069}.");
        let s = say(UiLanguage::En, Line::OnboardTarget, 0, &[("level", "BIPA 2"), ("date", "1 March 2027")]);
        assert_eq!(
            s,
            "How about a target: BIPA 2 by 1 March 2027? It's realistic for your pace, and you can change it any time."
        );
    }

    #[test]
    fn say_is_deterministic_and_seed_covers_every_variant() {
        let args = all_args();
        for line in Line::ALL {
            for lang in LANGS {
                let n = variants(lang, line).len() as u64;
                let seen: BTreeSet<String> = (0..n).map(|seed| say(lang, line, seed, &args)).collect();
                assert_eq!(seen.len() as u64, n, "{} {lang:?}: variants render identically", line.key());
                for seed in 0..n {
                    assert_eq!(say(lang, line, seed, &args), say(lang, line, seed + n, &args));
                }
            }
        }
        assert_eq!(
            say(UiLanguage::Ar, Line::PlanMorning, 20_000, &all_args()),
            say(UiLanguage::Ar, Line::PlanMorning, 20_000, &all_args())
        );
    }

    #[test]
    fn values_are_not_reparsed() {
        let s = say(UiLanguage::En, Line::RecapRemember, 0, &[("topic", "{name}")]);
        assert_eq!(s, "One thing to remember from today: {name}.");
    }

    #[test]
    fn plural_forms() {
        let ar = ["دقيقة", "دقيقتين", "دقايق"];
        let p = |v: &str| plural(UiLanguage::ArEg, Some(v), &ar);
        assert_eq!(p("1"), "دقيقة");
        assert_eq!(p("2"), "دقيقتين");
        assert_eq!(p("5"), "5 دقايق");
        assert_eq!(p("١٠"), "١٠ دقايق");
        assert_eq!(p("15"), "15 دقيقة");
        assert_eq!(p("100"), "100 دقيقة");
        assert_eq!(p("103"), "103 دقايق");
        assert_eq!(p("lots"), "lots دقايق");
        assert_eq!(plural(UiLanguage::ArEg, None, &ar), "دقايق");
        let msa = ["يوم", "يومين", "أيام", "يومًا"];
        assert_eq!(plural(UiLanguage::Ar, Some("15"), &msa), "15 يومًا");
        assert_eq!(plural(UiLanguage::Ar, Some("200"), &msa), "200 يوم");
        let en = ["day", "days"];
        assert_eq!(plural(UiLanguage::En, Some("1"), &en), "1 day");
        assert_eq!(plural(UiLanguage::En, Some("0"), &en), "0 days");
        assert_eq!(plural(UiLanguage::En, Some("7"), &en), "7 days");
        assert_eq!(plural(UiLanguage::En, None, &en), "days");

        let s = say(UiLanguage::En, Line::PelitaLit, 0, &[("streak", "1")]);
        assert_eq!(s, "Your Pelita, your lamp, has been lit for 1 day. Hebat, well done.");
        let s = say(UiLanguage::ArEg, Line::PelitaLit, 0, &[("streak", "2")]);
        assert_eq!(s, "قنديلك (\u{2068}Pelita\u{2069}) منوّر بقاله يومين. \u{2068}Hebat\u{2069}، يعني برافو.");
    }

    #[test]
    fn missing_args_fall_back_gracefully() {
        // With no arguments at all, nothing raw or broken ever shows.
        for line in Line::ALL {
            for lang in LANGS {
                for seed in 0..4 {
                    let s = say(lang, line, seed, &[]);
                    assert!(!s.trim().is_empty(), "{} {lang:?}: empty", line.key());
                    assert!(
                        !s.contains(['{', '}', '|']),
                        "{} {lang:?}: raw placeholder in {s:?}",
                        line.key()
                    );
                    assert!(
                        !s.contains("\u{2068}\u{2069}"),
                        "{} {lang:?}: empty isolate in {s:?}",
                        line.key()
                    );
                    assert!(!s.contains("  ") && !s.contains(" ."), "{} {lang:?}: untidy {s:?}", line.key());
                }
            }
        }
        // A variant whose placeholders are all supplied wins over the seed's.
        let args = [("name", "Hamo"), ("minutes", "15"), ("topic", "-kan verbs")];
        for seed in 0..8 {
            for lang in LANGS {
                let s = say(lang, Line::PlanMorning, seed, &args);
                for word in ["review", "مراجع"] {
                    assert!(!s.contains(word), "{lang:?}: no reviews were due, got {s:?}");
                }
                assert!(s.contains("-kan verbs"));
            }
        }
        // Blank values count as missing.
        let s = say(UiLanguage::En, Line::PlanAllDone, 0, &[("name", "  ")]);
        assert_eq!(s, "Today's plan is done. Hebat, well done. The rest of the day is yours.");
        let s = say(UiLanguage::ArEg, Line::PlanAllDone, 0, &[]);
        assert_eq!(s, "خلّصت خطة النهارده. \u{2068}Hebat\u{2069}، يعني برافو عليك. الباقي من يومك ليك.");
    }

    #[test]
    fn templates_reject_bad_syntax() {
        assert!(parse("a {name").is_err());
        assert!(parse("a } b").is_err());
        assert!(parse("a {Name}").is_err());
        assert!(parse("a {{name}}").is_err());
        assert!(parse("a {count|}").is_err());
        assert_eq!(
            parse("x {count|a|b}").unwrap(),
            vec![Seg::Text("x "), Seg::Slot { name: "count", forms: vec!["a", "b"] }]
        );
    }
}
