//! "Start with words you already know": the starter Arabic Bridge deck
//! (SPEC §12, §24), offered while the learner has no cards yet.
//!
//! Hand-written, not AI output. Drafted for the learner's review: the cards
//! stay `verified: false` until approved, like all build-time content.

use kabar_protocol::lesson::{Example, Localized};
use kabar_protocol::srs::{ArabicOrigin, Card, CardKind, CardSource, MeaningShift, StudyLanguage};

/// `(ar_eg, ar, en)`.
type L = (&'static str, &'static str, &'static str);

struct SeedWord {
    word: &'static str,
    arabic: &'static str,
    root: &'static str,
    shift: MeaningShift,
    meaning: L,
    /// How the Arabic became the Indonesian (sound change, meaning shift).
    bridge_note: L,
    note: Option<L>,
    example: Option<(&'static str, L)>,
}

use MeaningShift::{FalseFriend, Same, Shifted};

// Latin snippets inside Arabic text are wrapped in FSI…PDI (U+2068/U+2069)
// so they keep their direction in the RTL UI.
const SEED: &[SeedWord] = &[
    SeedWord {
        word: "kabar",
        arabic: "خبر",
        root: "خ ب ر",
        shift: Same,
        meaning: ("خبر، أخبار", "خبر، أخبار", "news"),
        bridge_note: (
            "الخاء بقت k: خبر ← \u{2068}kabar\u{2069}",
            "صارت الخاء k: خبر ← \u{2068}kabar\u{2069}",
            "خ became k",
        ),
        note: None,
        example: Some((
            "Apa kabar?",
            (
                "إزيك؟ (حرفيًا: إيه الأخبار؟)",
                "كيف حالك؟ (حرفيًا: ما الخبر؟)",
                "How are you? (literally: what's the news?)",
            ),
        )),
    },
    SeedWord {
        word: "waktu",
        arabic: "وقت",
        root: "و ق ت",
        shift: Same,
        meaning: ("وقت", "وقت", "time"),
        bridge_note: (
            "القاف بقت k وزادت u في الآخر",
            "صارت القاف k وأضيفت u في آخر الكلمة",
            "ق became k, and a final -u was added",
        ),
        note: None,
        example: Some(("Saya tidak punya waktu.", ("معنديش وقت.", "ليس لديّ وقت.", "I don't have time."))),
    },
    SeedWord {
        word: "selamat",
        arabic: "سلامة",
        root: "س ل م",
        shift: Shifted,
        meaning: (
            "سلامة؛ ومبروك، وبتيجي في التحيات",
            "سلامة ونجاة؛ وتهنئة، وتُستعمل في التحيات",
            "safe; congratulations; used in greetings",
        ),
        bridge_note: (
            "التاء المربوطة بقت -at، والمعنى اتوسّع: \u{2068}Selamat!\u{2069} لوحدها يعني «مبروك!»",
            "صارت التاء المربوطة -at، واتّسع المعنى: \u{2068}Selamat!\u{2069} وحدها تعني «مبروك!»",
            "Final ة became -at, and the meaning widened: “Selamat!” alone means “Congratulations!”",
        ),
        note: None,
        example: Some(("Selamat pagi!", ("صباح الخير!", "صباح الخير!", "Good morning!"))),
    },
    SeedWord {
        word: "lezat",
        arabic: "لذّة",
        root: "ل ذ ذ",
        shift: Shifted,
        meaning: ("لذيذ، طعمه حلو", "لذيذ", "delicious"),
        bridge_note: (
            "الذال بقت z والتاء المربوطة -at. «لذّة» عندنا اسم، لكن \u{2068}lezat\u{2069} صفة: «لذيذ».",
            "صارت الذال z والتاء المربوطة -at. «لذّة» اسم في العربية، و\u{2068}lezat\u{2069} صفة بمعنى «لذيذ».",
            "ذ became z and final ة became -at. لذّة is a noun (pleasure); lezat is an adjective (delicious).",
        ),
        note: Some((
            "في الكلام اليومي \u{2068}enak\u{2069} أشهر.",
            "في الكلام اليومي \u{2068}enak\u{2069} أشيع.",
            "In everyday speech, enak is more common.",
        )),
        example: Some((
            "Makanannya lezat sekali.",
            ("الأكل طعمه حلو أوي.", "الطعام لذيذ جدًّا.", "The food is really delicious."),
        )),
    },
    SeedWord {
        word: "kursi",
        arabic: "كرسي",
        root: "ك ر س",
        shift: Same,
        meaning: ("كرسي", "كرسي", "chair"),
        bridge_note: ("دخلت زي ما هي تقريبًا", "انتقلت كما هي تقريبًا", "Came over almost unchanged"),
        note: None,
        example: Some((
            "Kursi ini kosong?",
            ("الكرسي ده فاضي؟", "هل هذا الكرسي شاغر؟", "Is this chair free?"),
        )),
    },
    SeedWord {
        word: "ilmu",
        arabic: "علم",
        root: "ع ل م",
        shift: Same,
        meaning: ("علم، معرفة", "علم، معرفة", "knowledge; a field of study"),
        bridge_note: (
            "العين اختفت وزادت u في الآخر: علم ← \u{2068}ilmu\u{2069}",
            "سقطت العين وأضيفت u في الآخر: علم ← \u{2068}ilmu\u{2069}",
            "ع dropped out and a final -u was added",
        ),
        note: None,
        example: Some(("Ilmu itu penting.", ("العلم مهم.", "العلمُ مهمّ.", "Knowledge is important."))),
    },
    SeedWord {
        word: "pikir",
        arabic: "فكر",
        root: "ف ك ر",
        shift: Same,
        meaning: ("يفكّر؛ فِكر", "يفكّر؛ فِكر", "to think; thought"),
        bridge_note: (
            "الفاء بقت p: فكر ← \u{2068}pikir\u{2069}",
            "صارت الفاء p: فكر ← \u{2068}pikir\u{2069}",
            "ف became p",
        ),
        note: None,
        example: Some((
            "Saya pikir dia benar.",
            ("أنا شايف إن عنده حق.", "أظن أنه على حق.", "I think he's right."),
        )),
    },
    SeedWord {
        word: "paham",
        arabic: "فهم",
        root: "ف ه م",
        shift: Same,
        meaning: ("يفهم، فاهم", "يفهم، فَهْم", "to understand"),
        bridge_note: (
            "الفاء بقت p تاني: فهم ← \u{2068}paham\u{2069}",
            "صارت الفاء p مرة أخرى: فهم ← \u{2068}paham\u{2069}",
            "ف became p again",
        ),
        note: None,
        example: Some(("Saya sudah paham.", ("خلاص فهمت.", "لقد فهمت.", "I understand now."))),
    },
    SeedWord {
        word: "kalbu",
        arabic: "قلب",
        root: "ق ل ب",
        shift: Same,
        meaning: ("قلب (بالمعنى المعنوي)", "قلب، فؤاد", "heart (in the emotional sense)"),
        bridge_note: (
            "القاف بقت k وزادت u في الآخر",
            "صارت القاف k وأضيفت u في الآخر",
            "ق became k, and a final -u was added",
        ),
        note: Some((
            "كلمة أدبية؛ في الكلام العادي بيقولوا \u{2068}hati\u{2069}.",
            "كلمة أدبية؛ في الكلام اليومي يقال \u{2068}hati\u{2069}.",
            "Literary; in everyday speech people say hati.",
        )),
        example: None,
    },
    SeedWord {
        word: "kalimat",
        arabic: "كلمة",
        root: "ك ل م",
        shift: FalseFriend,
        meaning: ("جملة", "جملة", "sentence"),
        bridge_note: (
            "خلّي بالك! شكلها «كلمة» بس معناها «جملة».",
            "انتبه! تشبه «كلمة» لكنها تعني «جملة».",
            "Careful! It looks like كلمة (“word”) but means “sentence”.",
        ),
        note: Some((
            "«كلمة» بالإندونيسي \u{2068}kata\u{2069}.",
            "«كلمة» بالإندونيسية \u{2068}kata\u{2069}.",
            "The Indonesian for “word” is kata.",
        )),
        example: Some((
            "Kalimat ini panjang sekali.",
            ("الجملة دي طويلة أوي.", "هذه الجملة طويلة جدًّا.", "This sentence is very long."),
        )),
    },
    SeedWord {
        word: "mungkin",
        arabic: "ممكن",
        root: "م ك ن",
        shift: Shifted,
        meaning: ("يمكن (احتمال)", "ربما، لعلّ", "maybe, perhaps"),
        bridge_note: (
            "معناها «يمكن»، مش «ممكن؟» بتاعة الطلب. لو عايز تستأذن قول \u{2068}boleh\u{2069}.",
            "تعني «ربما»، لا «ممكن؟» التي في الطلب؛ وللاستئذان يقال \u{2068}boleh\u{2069}.",
            "It means “maybe”, not the “may I?” of a request; to ask permission, say boleh.",
        ),
        note: None,
        example: Some((
            "Mungkin besok hujan.",
            ("يمكن تمطر بكرة.", "ربما تمطر غدًا.", "Maybe it'll rain tomorrow."),
        )),
    },
];

fn loc((ar_eg, ar, en): L) -> Localized {
    Localized { ar_eg: ar_eg.into(), ar: ar.into(), en: en.into() }
}

/// Words in the starter deck, in teaching order.
pub fn seed_words() -> impl Iterator<Item = &'static str> {
    SEED.iter().map(|w| w.word)
}

/// The starter deck: a recognition card and an Arabic Bridge card per word.
/// Ids are stable, so seeding twice (or on two devices) never duplicates.
pub fn seed_bridge_deck() -> Vec<Card> {
    let mut cards = Vec::with_capacity(SEED.len() * 2);
    for w in SEED {
        for kind in [CardKind::Recognition, CardKind::Bridge] {
            let kind_tag = match kind {
                CardKind::Recognition => "recognition",
                _ => "bridge",
            };
            cards.push(Card {
                v: kabar_protocol::srs::CARD_VERSION,
                id: format!("seed.bridge.{}.{kind_tag}", w.word),
                word: w.word.into(),
                language: StudyLanguage::Id,
                kind,
                meaning: loc(w.meaning),
                note: w.note.map(loc),
                example: w.example.map(|(text, m)| Example { text: text.into(), meaning: loc(m) }),
                arabic: Some(ArabicOrigin {
                    word: w.arabic.into(),
                    root: Some(w.root.into()),
                    shift: w.shift,
                    note: Some(loc(w.bridge_note)),
                }),
                source: CardSource::Seed,
                source_ref: Some("arabic-bridge-starter".into()),
                verified: false,
            });
        }
    }
    cards
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn covers_the_spec_bridge_words() {
        let words: Vec<&str> = seed_words().collect();
        assert_eq!(
            words,
            [
                "kabar", "waktu", "selamat", "lezat", "kursi", "ilmu", "pikir", "paham", "kalbu", "kalimat",
                "mungkin"
            ]
        );
    }

    #[test]
    fn ids_are_unique_and_every_text_is_filled() {
        let deck = seed_bridge_deck();
        assert_eq!(deck.len(), 22);
        let ids: HashSet<&str> = deck.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), deck.len());
        for c in &deck {
            let a = c.arabic.as_ref().unwrap();
            for l in [&c.meaning, a.note.as_ref().unwrap()] {
                assert!(!l.ar_eg.is_empty() && !l.ar.is_empty() && !l.en.is_empty(), "{}", c.id);
            }
        }
    }

    #[test]
    fn false_friend_and_shift_are_flagged() {
        let deck = seed_bridge_deck();
        let shift = |w: &str| deck.iter().find(|c| c.word == w).unwrap().arabic.as_ref().unwrap().shift;
        assert_eq!(shift("kalimat"), MeaningShift::FalseFriend);
        assert_eq!(shift("mungkin"), MeaningShift::Shifted);
        assert_eq!(shift("kursi"), MeaningShift::Same);
    }

    #[test]
    fn examples_contain_their_word() {
        for c in seed_bridge_deck() {
            if let Some(ex) = &c.example {
                assert!(ex.text.to_lowercase().contains(&c.word), "{}", c.id);
            }
        }
    }
}
