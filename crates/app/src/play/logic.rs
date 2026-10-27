//! Pure helpers behind the Play screens, kept out of the components so they
//! can be tested without a WebView (and without the game engines).

use std::collections::HashSet;

use kabar_core::locale::UiLanguage;
use kabar_protocol::coach::{ActivityEvent, ActivityKind, LocalDate};
use kabar_protocol::content::Trilingual;

/// Practice topics the Play screens log under.
pub const TOPIC_PAGI: &str = "kabar-pagi";
pub const TOPIC_ALARM: &str = "false-friends";
pub const TOPIC_RUPIAH: &str = "rupiah";
pub const TOPIC_PASAR: &str = "pasar";

/// `bridge:<rule id>`, or `bridge:all` when every rule is mixed.
pub fn detective_topic(rule: Option<&str>) -> String {
    format!("bridge:{}", rule.unwrap_or("all"))
}

/// The meaning in the explanation voice that matches the UI language.
pub fn meaning(lang: UiLanguage, t: &Trilingual) -> &str {
    match lang {
        UiLanguage::ArEg => &t.arz,
        UiLanguage::Ar => &t.ar,
        UiLanguage::En => &t.en,
    }
}

/// Every word already logged as met (`NewWords`), on any device, lowercase.
pub fn logged_words(events: &[ActivityEvent]) -> HashSet<String> {
    events
        .iter()
        .filter_map(|e| match &e.kind {
            ActivityKind::NewWords { words } => Some(words),
            _ => None,
        })
        .flatten()
        .map(|w| w.trim().to_lowercase())
        .collect()
}

/// `words` minus those already logged, without repeats, in order.
pub fn unlogged(words: &[String], logged: &HashSet<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    words
        .iter()
        .map(|w| w.trim().to_lowercase())
        .filter(|w| !w.is_empty() && !logged.contains(w) && seen.insert(w.clone()))
        .collect()
}

/// All words met so far, in the order they were met, without repeats.
pub fn known_words(events: &[ActivityEvent]) -> Vec<String> {
    let mut seen = HashSet::new();
    events
        .iter()
        .filter_map(|e| match &e.kind {
            ActivityKind::NewWords { words } => Some(words),
            _ => None,
        })
        .flatten()
        .map(|w| w.trim().to_lowercase())
        .filter(|w| seen.insert(w.clone()))
        .collect()
}

/// The words of the most recent `NewWords` event: Kabar Pagi's stand-in for
/// "due" words until the review engine (M6) schedules them.
pub fn latest_new_words(events: &[ActivityEvent]) -> Vec<String> {
    events
        .iter()
        .filter(|e| matches!(e.kind, ActivityKind::NewWords { .. }))
        .max_by(|a, b| a.date.cmp(&b.date).then(a.at_ms.cmp(&b.at_ms)).then(a.hlc.cmp(&b.hlc)))
        .map(|e| match &e.kind {
            ActivityKind::NewWords { words } => words.iter().map(|w| w.trim().to_lowercase()).collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

/// Whether a practice on `topic` was logged on `date`.
pub fn practiced_on(events: &[ActivityEvent], date: LocalDate, topic: &str) -> bool {
    events
        .iter()
        .any(|e| e.date == date && matches!(&e.kind, ActivityKind::Practice { topic: t, .. } if t == topic))
}

/// The day number as a seed: the same story all day on every device.
pub fn day_seed(date: LocalDate) -> u64 {
    u64::try_from(date.0).unwrap_or_default()
}

/// "bipa1.u01" → "bipa1".
pub fn level_of_unit(unit: &str) -> &str {
    unit.split('.').next().unwrap_or(unit)
}

/// The first `n` words of `content/freq/id.tsv` (rank, word, zipf, sources).
pub fn top_words(freq_tsv: &str, n: usize) -> Vec<String> {
    freq_tsv
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split('\t').nth(1))
        .map(|w| w.trim().to_lowercase())
        .filter(|w| !w.is_empty())
        .take(n)
        .collect()
}

const PREFIXES: &[&str] = &[
    "memper", "menge", "meng", "meny", "mem", "men", "me", "di", "ber", "be", "ter", "per", "peng", "peny",
    "pem", "pen", "pe", "ke", "se",
];
const SUFFIXES: &[&str] = &["kannya", "annya", "kan", "an", "i", "nya", "lah", "kah", "pun", "ku", "mu"];

/// Whether `token` is `target` or an affixed form of it: `dibaca`, `membaca`,
/// `bacaan`, and nasal forms like `menulis` (tulis) or `memukul` (pukul).
pub fn is_form_of(token: &str, target: &str) -> bool {
    let token = token.to_lowercase();
    let target = target.to_lowercase();
    if target.is_empty() {
        return false;
    }
    if token == target {
        return true;
    }
    // Reduplication: anak-anak.
    if token.split('-').any(|part| part == target) {
        return true;
    }
    let mut stems = vec![token.clone()];
    for p in PREFIXES {
        if let Some(rest) = token.strip_prefix(p) {
            stems.push(rest.to_string());
            // Nasal assimilation drops the root's first consonant.
            let restored = match *p {
                "meny" | "peny" => Some(format!("s{rest}")),
                "men" | "pen" => Some(format!("t{rest}")),
                "mem" | "pem" => Some(format!("p{rest}")),
                "meng" | "peng" => Some(format!("k{rest}")),
                _ => None,
            };
            stems.extend(restored);
        }
    }
    stems.iter().any(|stem| {
        stem == &target || SUFFIXES.iter().any(|s| stem.strip_suffix(s).is_some_and(|base| base == target))
    })
}

/// What the learner can afford in Pasar!: the fair price plus a 10% cushion,
/// rounded up to a step a seller would name.
pub fn budget_for(fair: u64) -> u64 {
    let raw = fair + fair / 10;
    let step = match raw {
        0..=9_999 => 500,
        10_000..=99_999 => 1_000,
        100_000..=999_999 => 5_000,
        1_000_000..=9_999_999 => 50_000,
        _ => 500_000,
    };
    raw.div_ceil(step) * step
}

/// An amount typed into the offer box: digits in either script, with any
/// dots, commas or spaces people put between thousands. Ignores "Rp".
pub fn parse_amount(input: &str) -> Option<u64> {
    let mut digits = String::new();
    for ch in input.chars() {
        match ch {
            '0'..='9' => digits.push(ch),
            '\u{0660}'..='\u{0669}' => digits.push(char::from(b'0' + (ch as u32 - 0x0660) as u8)),
            '\u{06F0}'..='\u{06F9}' => digits.push(char::from(b'0' + (ch as u32 - 0x06F0) as u8)),
            '.' | ',' | ' ' | '\u{066C}' | '\u{00A0}' => {}
            'R' | 'r' | 'p' | 'P' if digits.is_empty() => {}
            _ => return None,
        }
    }
    if digits.is_empty() { None } else { digits.parse().ok().filter(|n| *n > 0) }
}

/// The Bridge deck's hero line, in Indonesian (Western digits, as Indonesians write them).
pub fn deck_headline(n: usize) -> String {
    format!("Kamu sudah tahu {n} kata")
}

/// A sound-rule side that is a description, not letters ("(vowel)",
/// "(dropped)", "(سكون)", "(آخر الكلمة)"), shown from the UI text tables in
/// the learner's language instead of as raw content.
pub fn rule_placeholder(side: &str) -> Option<crate::i18n::Msg> {
    use crate::i18n::Msg;
    match side.trim() {
        "(vowel)" => Some(Msg::SoundVowel),
        "(dropped)" => Some(Msg::SoundDropped),
        "(سكون)" => Some(Msg::SoundSukun),
        "(آخر الكلمة)" => Some(Msg::SoundWordEnd),
        _ => None,
    }
}

/// The rules the Sound lab picker shows before "show every rule": the most
/// common shifts, in this order.
pub const COMMON_RULES: [&str; 5] = ["fa_p", "qaf_k", "kha_k", "ta_marbuta_at", "shin_sy"];

/// Which rule ids the picker shows: all of them when expanded, else the
/// common ones plus the selected rule (so a pick never disappears).
pub fn picker_rules<'a>(all: &[&'a str], expanded: bool, selected: Option<&str>) -> Vec<&'a str> {
    all.iter().copied().filter(|id| expanded || COMMON_RULES.contains(id) || selected == Some(*id)).collect()
}

/// Where a Rupiah Reader best score is kept on this device.
pub fn best_key(level: &str) -> String {
    format!("play.rupiah.best.{level}")
}

#[cfg(test)]
mod tests {
    use kabar_protocol::{DeviceId, Hlc};

    use super::*;

    fn ev(date: i32, at_ms: u64, kind: ActivityKind) -> ActivityEvent {
        ActivityEvent {
            id: format!("{date}-{at_ms}"),
            hlc: Hlc { wall_ms: at_ms, counter: 0, device: DeviceId("t".into()) },
            date: LocalDate(date),
            at_ms,
            kind,
            seconds: 0,
        }
    }

    fn words(ws: &[&str]) -> ActivityKind {
        ActivityKind::NewWords { words: ws.iter().map(|w| w.to_string()).collect() }
    }

    fn s(ws: &[&str]) -> Vec<String> {
        ws.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn meaning_follows_the_ui_language() {
        let t = Trilingual { arz: "جملة".into(), ar: "جملة تامة".into(), en: "sentence".into() };
        assert_eq!(meaning(UiLanguage::ArEg, &t), "جملة");
        assert_eq!(meaning(UiLanguage::Ar, &t), "جملة تامة");
        assert_eq!(meaning(UiLanguage::En, &t), "sentence");
    }

    #[test]
    fn logged_words_are_not_logged_twice() {
        let events = vec![ev(1, 1, words(&["kabar", "Waktu"])), ev(2, 2, words(&["ilmu"]))];
        let logged = logged_words(&events);
        assert_eq!(
            unlogged(&s(&["kabar", "waktu", "kursi", "kursi", " Paham ", ""]), &logged),
            s(&["kursi", "paham"])
        );
        assert_eq!(known_words(&events), s(&["kabar", "waktu", "ilmu"]));
    }

    #[test]
    fn latest_new_words_stand_in_for_due_words() {
        assert!(latest_new_words(&[]).is_empty());
        let events = vec![
            ev(5, 50, words(&["selamat"])),
            ev(3, 90, words(&["lezat"])),
            ev(5, 10, words(&["kursi"])),
            ev(6, 1, ActivityKind::PlanItemDone { item_id: "x".into() }),
        ];
        assert_eq!(latest_new_words(&events), s(&["selamat"]));
    }

    #[test]
    fn pagi_counts_as_read_only_on_its_day() {
        let practice = |topic: &str| ActivityKind::Practice {
            skill: kabar_protocol::coach::Skill::Listening,
            topic: topic.into(),
            correct: 1,
            total: 1,
        };
        let events = vec![ev(10, 1, practice(TOPIC_PAGI)), ev(11, 1, practice(TOPIC_RUPIAH))];
        assert!(practiced_on(&events, LocalDate(10), TOPIC_PAGI));
        assert!(!practiced_on(&events, LocalDate(11), TOPIC_PAGI));
    }

    #[test]
    fn small_helpers() {
        assert_eq!(level_of_unit("bipa1.u01"), "bipa1");
        assert_eq!(level_of_unit("bipa2"), "bipa2");
        assert_eq!(detective_topic(Some("fa_p")), "bridge:fa_p");
        assert_eq!(detective_topic(None), "bridge:all");
        assert_eq!(day_seed(LocalDate(20_000)), 20_000);
        assert_eq!(day_seed(LocalDate(-1)), 0);
        assert_eq!(best_key("Millions"), "play.rupiah.best.Millions");
        assert_eq!(deck_headline(312), "Kamu sudah tahu 312 kata");
    }

    #[test]
    fn top_words_skip_the_header() {
        let tsv =
            "# header\n# Columns: rank, word\n1\tyang\t7.41\twordfreq\n2\tDan\t7.27\tx\n3\taku\t7.2\tx\n";
        assert_eq!(top_words(tsv, 2), s(&["yang", "dan"]));
        let real = include_str!("../../../../content/freq/id.tsv");
        assert_eq!(top_words(real, 3), s(&["yang", "dan", "aku"]));
    }

    #[test]
    fn affixed_forms_count_as_the_target() {
        for (token, target) in [
            ("membaca", "baca"),
            ("dibaca", "baca"),
            ("bacaan", "baca"),
            ("dibersihkan", "bersih"),
            ("Kesehatan", "sehat"),
            ("menulis", "tulis"),
            ("memukul", "pukul"),
            ("menyapu", "sapu"),
            ("mengirim", "kirim"),
            ("Anak-anak", "anak"),
            ("rumahnya", "rumah"),
            ("bermain", "main"),
            ("kabar", "kabar"),
        ] {
            assert!(is_form_of(token, target), "{token} should count as {target}");
        }
        for (token, target) in [("makan", "akan"), ("buku", "baca"), ("ini", "")] {
            assert!(!is_form_of(token, target), "{token} is not {target}");
        }
    }

    #[test]
    fn budgets_are_round_and_above_fair() {
        assert_eq!(budget_for(35_000), 39_000);
        assert_eq!(budget_for(8_000), 9_000);
        assert_eq!(budget_for(20_000_000), 22_000_000);
        for fair in [1_000, 7_500, 150_000, 2_750_000] {
            assert!(budget_for(fair) >= fair);
        }
    }

    #[test]
    fn offers_parse_like_people_type_them() {
        assert_eq!(parse_amount("25000"), Some(25_000));
        assert_eq!(parse_amount("Rp25.000"), Some(25_000));
        assert_eq!(parse_amount("26 000 000"), Some(26_000_000));
        assert_eq!(parse_amount("٢٥٠٠٠"), Some(25_000));
        assert_eq!(parse_amount(""), None);
        assert_eq!(parse_amount("0"), None);
        assert_eq!(parse_amount("25rb"), None);
    }

    /// Every descriptive rule side in the shipped content is localized, and
    /// the Arabic UIs never show "vowel" or "dropped" (#165).
    #[test]
    fn sound_rule_placeholders_are_localized() {
        use crate::i18n::t;
        use kabar_core::locale::UiLanguage::{Ar, ArEg, En};
        let data = super::super::content::bridge().as_ref().expect("bridge content");
        for r in &data.rules {
            for side in [&r.from, &r.to] {
                let looks_descriptive = side.starts_with('(');
                assert_eq!(rule_placeholder(side).is_some(), looks_descriptive, "{}: {side}", r.id);
                if let Some(msg) = rule_placeholder(side) {
                    for lang in [ArEg, Ar] {
                        assert!(!t(lang, msg).chars().any(|c| c.is_ascii_alphabetic()), "{side} in {lang:?}");
                    }
                    assert!(t(En, msg).is_ascii());
                }
            }
        }
        assert_eq!(rule_placeholder("p"), None);
    }

    #[test]
    fn the_rule_picker_starts_short() {
        let data = super::super::content::bridge().as_ref().expect("bridge content");
        let ids: Vec<&str> = data.rules.iter().map(|r| r.id.as_str()).collect();
        for id in COMMON_RULES {
            assert!(ids.contains(&id), "{id} is not a rule");
        }
        assert_eq!(picker_rules(&ids, false, None).len(), COMMON_RULES.len());
        assert_eq!(picker_rules(&ids, true, None), ids);
        // A picked rule outside the common ones stays visible.
        let short = picker_rules(&ids, false, Some("ayn_dropped"));
        assert_eq!(short.len(), COMMON_RULES.len() + 1);
        assert!(short.contains(&"ayn_dropped"));
    }
}
