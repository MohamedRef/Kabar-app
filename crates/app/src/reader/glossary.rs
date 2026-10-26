//! Local word data for the popover: glosses, Arabic Bridge cards and
//! Javanese speech levels, looked up without any network.
//!
//! The content pack from the vocabulary pipeline (top-2,000 glosses and the
//! Bridge cards) plugs in behind [`Glossary`]. Until it lands, [`SeedGlossary`]
//! covers the SPEC §24 test texts. Every seed entry is unverified.

use std::collections::HashMap;

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordLang {
    Indonesian,
    Javanese,
    English,
}

/// One meaning in the three explanation voices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gloss {
    /// Egyptian colloquial (بالمصري).
    pub arz: String,
    /// Modern Standard Arabic.
    pub ar: String,
    pub en: String,
}

impl Gloss {
    pub fn get(&self, voice: UiLanguage) -> &str {
        match voice {
            UiLanguage::ArEg => &self.arz,
            UiLanguage::Ar => &self.ar,
            UiLanguage::En => &self.en,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Same,
    Narrowed,
    Broadened,
    Shifted,
    FalseFriend,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bridge {
    pub arabic: String,
    /// Spaced root letters, e.g. "خ ب ر".
    pub root: Option<String>,
    pub shift: Shift,
    /// What changed, when it did.
    pub note: Option<Gloss>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeechLevel {
    Ngoko,
    Krama,
    KramaInggil,
    /// Set by the content pack's speech-level table; the seed has none.
    #[allow(dead_code)]
    KramaAndhap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Levels {
    /// The level of the word itself.
    pub level: SpeechLevel,
    pub ngoko: String,
    pub krama: Option<String>,
    pub krama_inggil: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordInfo {
    pub lemma: String,
    pub lang: WordLang,
    pub pos: Option<String>,
    pub gloss: Gloss,
    pub bridge: Option<Bridge>,
    pub levels: Option<Levels>,
    /// A usage note ("yang links a noun to its description…").
    pub note: Option<Gloss>,
    /// One example sentence with its translation.
    pub example: Option<(String, Gloss)>,
    /// Reviewed and approved by the learner.
    pub verified: bool,
}

pub trait Glossary {
    /// Looks up a lowercase form: tries it as given, then each fallback
    /// (normalized form, root) in order.
    fn lookup(&self, forms: &[&str]) -> Option<&WordInfo>;
}

pub struct SeedGlossary {
    pub(super) words: HashMap<&'static str, WordInfo>,
}

type Seed = (&'static str, WordLang, &'static str, [&'static str; 3]);

/// (lemma, lang, pos, [arz, ar, en]). Unverified seed for the §24 texts.
const SEED: &[Seed] = &[
    // Javanese chat
    ("sek", WordLang::Javanese, "particle", ["استنى / ثانية", "انتظر لحظة", "hold on, wait"]),
    (
        "tak",
        WordLang::Javanese,
        "prefix",
        ["أنا هـ… (أنا اللي هعمل)", "سأفعل أنا", "I'll (1st-person, tak-)"],
    ),
    ("nang", WordLang::Javanese, "preposition", ["لـ / على", "إلى", "to, toward"]),
    ("piye", WordLang::Javanese, "pronoun", ["إزاي / إيه رأيك", "كيف", "how; what do you think"]),
    (
        "dhe",
        WordLang::Javanese,
        "noun",
        ["يا عم / يا خالة (ندا ودود)", "نداء ودّي لكبير", "friendly address (Pakdhe / Budhe)"],
    ),
    ("arep", WordLang::Javanese, "verb", ["عايز / هـ", "يريد / سوف", "want to, will"]),
    ("cul", WordLang::Javanese, "verb", ["يسيب / يبيع", "يترك / يبيع", "let go; sell (diculke)"]),
    ("pira", WordLang::Javanese, "pronoun", ["بكام / كام", "كم", "how much, how many"]),
    ("kowe", WordLang::Javanese, "pronoun", ["إنت", "أنت", "you (ngoko)"]),
    ("sampeyan", WordLang::Javanese, "pronoun", ["حضرتك", "حضرتك", "you (krama)"]),
    ("panjenengan", WordLang::Javanese, "pronoun", ["سيادتك", "سيادتك", "you (krama inggil)"]),
    ("wis", WordLang::Javanese, "adverb", ["خلاص / بالفعل", "قد / بالفعل", "already"]),
    ("sampun", WordLang::Javanese, "adverb", ["خلاص / بالفعل (باحترام)", "قد / بالفعل", "already (krama)"]),
    ("mangan", WordLang::Javanese, "verb", ["ياكل", "يأكل", "eat (ngoko)"]),
    ("nedha", WordLang::Javanese, "verb", ["ياكل (باحترام)", "يأكل", "eat (krama)"]),
    ("dhahar", WordLang::Javanese, "verb", ["ياكل (لحد بتكبّره)", "يتناول الطعام", "eat (krama inggil)"]),
    ("koen", WordLang::Javanese, "pronoun", ["إنت (لهجة سورابايا)", "أنت", "you (Surabaya Arek)"]),
    ("ta", WordLang::Javanese, "particle", ["ولّا إيه؟", "أليس كذلك؟", "question tag"]),
    ("rek", WordLang::Javanese, "noun", ["يا جماعة / يا صاحبي", "يا رفاق", "mate, guys (Surabaya)"]),
    // Indonesian
    ("lapor", WordLang::Indonesian, "verb", ["يبلّغ", "يُبلغ", "report"]),
    ("pusat", WordLang::Indonesian, "noun", ["المركز / الإدارة", "المركز", "center, HQ"]),
    ("saya", WordLang::Indonesian, "pronoun", ["أنا", "أنا", "I, me"]),
    ("sedang", WordLang::Indonesian, "adverb", ["دلوقتي بـ… (بيعمل)", "يفعل الآن", "in the middle of (-ing)"]),
    ("baca", WordLang::Indonesian, "verb", ["يقرا", "يقرأ", "read"]),
    ("buku", WordLang::Indonesian, "noun", ["كتاب", "كتاب", "book"]),
    ("rumah", WordLang::Indonesian, "noun", ["بيت", "بيت", "house, home"]),
    ("bersih", WordLang::Indonesian, "adjective", ["نضيف", "نظيف", "clean"]),
    ("kemarin", WordLang::Indonesian, "adverb", ["امبارح", "أمس", "yesterday"]),
    ("anak", WordLang::Indonesian, "noun", ["عيّل / ابن", "طفل / ابن", "child"]),
    ("main", WordLang::Indonesian, "verb", ["يلعب", "يلعب", "play"]),
    ("di", WordLang::Indonesian, "preposition", ["في", "في", "in, at"]),
    ("taman", WordLang::Indonesian, "noun", ["جنينة", "حديقة", "park, garden"]),
    ("sehat", WordLang::Indonesian, "adjective", ["صحته كويسة", "صحيح الجسم", "healthy"]),
    ("itu", WordLang::Indonesian, "pronoun", ["ده / دي", "ذلك", "that"]),
    ("penting", WordLang::Indonesian, "adjective", ["مهم", "مهم", "important"]),
    ("apa", WordLang::Indonesian, "pronoun", ["إيه", "ماذا", "what"]),
    ("yang", WordLang::Indonesian, "particle", ["اللي", "الذي", "which, that, who"]),
    ("tidak", WordLang::Indonesian, "adverb", ["لأ / مش", "لا / ليس", "not, no"]),
    ("sudah", WordLang::Indonesian, "adverb", ["خلاص / بالفعل", "قد / بالفعل", "already"]),
    ("dari", WordLang::Indonesian, "preposition", ["من", "من", "from"]),
    ("resmi", WordLang::Indonesian, "adjective", ["رسمي", "رسمي", "official"]),
    ("mulus", WordLang::Indonesian, "adjective", ["زي الفل / من غير خدش", "سليم تمامًا", "flawless, smooth"]),
    ("garansi", WordLang::Indonesian, "noun", ["ضمان", "ضمان", "warranty"]),
    ("jalan", WordLang::Indonesian, "verb", ["ماشي / شغّال", "يسير / ساري", "walk; (warranty) still running"]),
    ("minat", WordLang::Indonesian, "noun", ["مهتم؟", "اهتمام", "interest; interested?"]),
    ("capek", WordLang::Indonesian, "adjective", ["تعبان", "متعب", "tired"]),
    ("banget", WordLang::Indonesian, "adverb", ["قوي / أوي", "جدًا", "very (informal)"]),
    ("gue", WordLang::Indonesian, "pronoun", ["أنا (عامية جاكرتا)", "أنا", "I (Jakarta slang)"]),
    // Arabic Bridge
    ("kabar", WordLang::Indonesian, "noun", ["خبر / أخبار", "خبر", "news"]),
    ("waktu", WordLang::Indonesian, "noun", ["وقت", "وقت", "time"]),
    ("selamat", WordLang::Indonesian, "adjective", ["سلامة / مبروك", "سلامة / تهنئة", "safe; congratulations"]),
    ("lezat", WordLang::Indonesian, "adjective", ["لذيذ", "لذيذ", "delicious"]),
    ("kursi", WordLang::Indonesian, "noun", ["كرسي", "كرسي", "chair"]),
    ("ilmu", WordLang::Indonesian, "noun", ["علم", "علم", "knowledge, science"]),
    ("pikir", WordLang::Indonesian, "verb", ["يفكّر", "يفكّر", "think"]),
    ("paham", WordLang::Indonesian, "verb", ["يفهم", "يفهم", "understand"]),
    ("kalbu", WordLang::Indonesian, "noun", ["قلب (أدبي)", "قلب", "heart (literary)"]),
    ("kalimat", WordLang::Indonesian, "noun", ["جملة", "جملة", "sentence"]),
    ("mungkin", WordLang::Indonesian, "adverb", ["يمكن / جايز", "ربما", "maybe"]),
    // English loans common in chat and listings
    ("iphone", WordLang::English, "noun", ["آيفون", "آيفون", "iPhone"]),
];

/// (word, arabic, root, shift, note [arz, ar, en]).
type BridgeSeed = (&'static str, &'static str, &'static str, Shift, Option<[&'static str; 3]>);

const BRIDGE: &[BridgeSeed] = &[
    ("kabar", "خبر", "خ ب ر", Shift::Same, None),
    ("waktu", "وقت", "و ق ت", Shift::Same, None),
    (
        "selamat",
        "سلامة",
        "س ل م",
        Shift::Broadened,
        Some([
            "بقت كمان «مبروك» وتحية: selamat pagi = صباح الخير",
            "اتسع معناها إلى التهنئة والتحية: selamat pagi = صباح الخير",
            "also 'congratulations' and greetings: selamat pagi = good morning",
        ]),
    ),
    (
        "lezat",
        "لذّة",
        "ل ذ ذ",
        Shift::Shifted,
        Some([
            "من «لذّة» (اسم) بقت صفة: لذيذ",
            "من «لذّة» (اسم) إلى صفة: لذيذ",
            "from the noun 'pleasure' to the adjective 'delicious'",
        ]),
    ),
    ("kursi", "كرسي", "ك ر س", Shift::Same, None),
    ("ilmu", "علم", "ع ل م", Shift::Same, None),
    ("pikir", "فكر", "ف ك ر", Shift::Same, None),
    ("paham", "فهم", "ف ه م", Shift::Same, None),
    (
        "kalbu",
        "قلب",
        "ق ل ب",
        Shift::Narrowed,
        Some([
            "بتتقال في الأدب والدين بس؛ القلب العادي hati",
            "تُستعمل في الأدب والدين؛ القلب في الكلام العادي hati",
            "literary/religious only; everyday 'heart' is hati",
        ]),
    ),
    (
        "kalimat",
        "كلمة",
        "ك ل م",
        Shift::FalseFriend,
        Some([
            "خلي بالك: kalimat يعني «جملة» مش «كلمة». الكلمة = kata",
            "تنبيه: kalimat تعني «جملة» لا «كلمة». الكلمة = kata",
            "careful: kalimat means 'sentence', not 'word'. 'word' = kata",
        ]),
    ),
    (
        "mungkin",
        "ممكن",
        "م ك ن",
        Shift::Shifted,
        Some([
            "معناها «يمكن / جايز» مش «ممكن» بمعنى مُتاح",
            "تعني «ربما» لا «ممكن» بمعنى متاح",
            "means 'maybe', not 'possible'",
        ]),
    ),
];

/// (word, its level, ngoko, krama, krama inggil).
type LevelSeed = (&'static str, SpeechLevel, &'static str, Option<&'static str>, Option<&'static str>);

const LEVELS: &[LevelSeed] = &[
    ("kowe", SpeechLevel::Ngoko, "kowe", Some("sampeyan"), Some("panjenengan")),
    ("sampeyan", SpeechLevel::Krama, "kowe", Some("sampeyan"), Some("panjenengan")),
    ("panjenengan", SpeechLevel::KramaInggil, "kowe", Some("sampeyan"), Some("panjenengan")),
    ("mangan", SpeechLevel::Ngoko, "mangan", Some("nedha"), Some("dhahar")),
    ("nedha", SpeechLevel::Krama, "mangan", Some("nedha"), Some("dhahar")),
    ("dhahar", SpeechLevel::KramaInggil, "mangan", Some("nedha"), Some("dhahar")),
    ("wis", SpeechLevel::Ngoko, "wis", Some("sampun"), Some("sampun")),
    ("sampun", SpeechLevel::Krama, "wis", Some("sampun"), Some("sampun")),
    ("arep", SpeechLevel::Ngoko, "arep", Some("badhe"), Some("kersa")),
    ("pira", SpeechLevel::Ngoko, "pira", Some("pinten"), None),
    ("piye", SpeechLevel::Ngoko, "piye", Some("pripun"), None),
    ("koen", SpeechLevel::Ngoko, "koen", Some("sampeyan"), Some("panjenengan")),
];

impl Default for SeedGlossary {
    fn default() -> Self {
        let mut words: HashMap<&'static str, WordInfo> = SEED
            .iter()
            .map(|&(lemma, lang, pos, [arz, ar, en])| {
                let info = WordInfo {
                    lemma: lemma.into(),
                    lang,
                    pos: Some(pos.into()),
                    gloss: Gloss { arz: arz.into(), ar: ar.into(), en: en.into() },
                    bridge: None,
                    levels: None,
                    note: None,
                    example: None,
                    verified: false,
                };
                (lemma, info)
            })
            .collect();
        for &(word, arabic, root, shift, note) in BRIDGE {
            if let Some(w) = words.get_mut(word) {
                w.bridge = Some(Bridge {
                    arabic: arabic.into(),
                    root: Some(root.into()),
                    shift,
                    note: note.map(|[arz, ar, en]| Gloss { arz: arz.into(), ar: ar.into(), en: en.into() }),
                });
            }
        }
        for &(word, level, ngoko, krama, ki) in LEVELS {
            if let Some(w) = words.get_mut(word) {
                w.levels = Some(Levels {
                    level,
                    ngoko: ngoko.into(),
                    krama: krama.map(Into::into),
                    krama_inggil: ki.map(Into::into),
                });
            }
        }
        Self { words }
    }
}

impl Glossary for SeedGlossary {
    fn lookup(&self, forms: &[&str]) -> Option<&WordInfo> {
        forms.iter().find_map(|f| self.words.get(*f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_and_false_friends() {
        let g = SeedGlossary::default();
        let kabar = g.lookup(&["kabar"]).unwrap();
        assert_eq!(kabar.bridge.as_ref().unwrap().arabic, "خبر");
        let kalimat = g.lookup(&["kalimat"]).unwrap();
        assert_eq!(kalimat.bridge.as_ref().unwrap().shift, Shift::FalseFriend);
        assert!(!kalimat.verified, "seed data starts unverified");
    }

    #[test]
    fn falls_back_through_forms() {
        let g = SeedGlossary::default();
        // "dibersihkan" isn't an entry; its root is.
        assert_eq!(g.lookup(&["dibersihkan", "bersih"]).unwrap().lemma, "bersih");
        assert!(g.lookup(&["zzz"]).is_none());
    }

    #[test]
    fn speech_levels_for_have_you_eaten() {
        let g = SeedGlossary::default();
        let l = g.lookup(&["nedha"]).unwrap().levels.as_ref().unwrap();
        assert_eq!(l.level, SpeechLevel::Krama);
        assert_eq!((l.ngoko.as_str(), l.krama_inggil.as_deref()), ("mangan", Some("dhahar")));
    }

    #[test]
    fn every_bridge_and_level_row_has_a_word() {
        let g = SeedGlossary::default();
        for (w, ..) in BRIDGE {
            assert!(g.lookup(&[w]).unwrap().bridge.is_some(), "{w}");
        }
        for (w, ..) in LEVELS {
            assert!(g.lookup(&[w]).unwrap().levels.is_some(), "{w}");
        }
    }
}
