//! Kabar Pagi (spec §14): a 2-minute morning micro-story built from the
//! learner's due words plus a few new ones, with audio.
//!
//! Primary: the `story_episode` AI task through the router (learning text
//! only, never personal data). Offline or when no free engine answers: the
//! best-matching story from the build-time bank in `content/pagi/`.
//!
//! The flow, for callers:
//!
//! 1. [`pick_words`] chooses today's words; [`story_request`] turns them into
//!    the router input (`prompts/story_episode@v1.md`).
//! 2. The router parses the reply as a [`Story`] and runs
//!    [`Story::validate`](AiOutput::validate): shape, length, scripts, and
//!    that every word the story lists in `words` really appears in it. A
//!    failure goes back to the model on the router's one retry, with the
//!    exact problem ("sentence 3: `arz` must be in Arabic script", "`words`
//!    lists `pasar` but no sentence uses it").
//! 3. `validate` cannot see the request, so the caller then checks coverage
//!    with [`uses_words`]. Missing words are not hidden: either show the story
//!    and say which words it skipped, or fall back to [`StoryBank::best`].
//!    Asking again (the router's `regenerate`) is also fine, since the prompt
//!    tells the model to use every word and to list them in `words`.

use std::collections::BTreeSet;

use kabar_protocol::ai::Task;
use kabar_protocol::cloud::{TtsSegment, Voice};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ai::AiOutput;
use crate::lang::morph_id::{IndonesianMorph, segment};
use crate::lang::token::tokenize;

/// Most due words a story is built around, and most new words added.
pub const MAX_DUE: usize = 6;
pub const MAX_NEW: usize = 3;

/// Bounds a story must stay within (see [`Story::validate`]).
pub const MIN_SENTENCES: usize = 6;
pub const MAX_SENTENCES: usize = 14;
/// About two minutes of slow speech (see [`listening_seconds`]).
pub const MAX_WORDS: usize = 220;

/// Speech rate the audio is generated at: Azure's normal Indonesian pace is
/// about 2 words per second; at the learner-friendly -20% rate that is 1.6.
pub const WORDS_PER_SECOND: f32 = 1.6;
/// The rate passed to TTS for Kabar Pagi (`TtsRequest::rate_percent`).
pub const RATE_PERCENT: i8 = -20;
/// The pause the player leaves after each sentence.
pub const PAUSE_SECONDS: f32 = 1.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagiWords {
    pub due: Vec<String>,
    pub new: Vec<String>,
}

impl PagiWords {
    /// Due words first, then new ones.
    pub fn all(&self) -> Vec<String> {
        self.due.iter().chain(&self.new).cloned().collect()
    }
}

/// Picks today's words: the due words the learner is shakiest on first
/// (callers pass them in that order), then new words from `new_pool` (ranked,
/// most useful first) that aren't in `known`.
///
/// Deterministic: the same inputs always give the same words, in order.
/// Words are trimmed and lowercased; duplicates and blanks are dropped. The
/// `seed` is accepted for future variety but does not change the pick today,
/// because the order the caller passes already encodes priority.
pub fn pick_words(due: &[String], known: &[String], new_pool: &[String], seed: u64) -> PagiWords {
    let _ = seed;
    let norm = |w: &String| w.trim().to_lowercase();
    let mut taken = BTreeSet::new();

    let mut picked_due = Vec::new();
    for w in due.iter().map(norm) {
        if picked_due.len() == MAX_DUE {
            break;
        }
        if !w.is_empty() && taken.insert(w.clone()) {
            picked_due.push(w);
        }
    }
    // Due words beyond the cap are still not "new".
    taken.extend(due.iter().map(norm));
    taken.extend(known.iter().map(norm));

    let mut picked_new = Vec::new();
    for w in new_pool.iter().map(norm) {
        if picked_new.len() == MAX_NEW {
            break;
        }
        if !w.is_empty() && taken.insert(w.clone()) {
            picked_new.push(w);
        }
    }
    PagiWords { due: picked_due, new: picked_new }
}

/// A story, as the AI returns it and as the bank stores it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Story {
    /// Indonesian title.
    pub title: String,
    /// e.g. "bipa1".
    pub level: String,
    pub sentences: Vec<StorySentence>,
    /// The target words the story uses, as dictionary forms.
    pub words: Vec<String>,
    /// Sinetron only: 1–3 plain English sentences, passed to the next episode
    /// as `previous_summary`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorySentence {
    /// Indonesian.
    pub id: String,
    /// Egyptian Arabic.
    pub arz: String,
    /// Modern Standard Arabic.
    pub ar: String,
    pub en: String,
}

fn has_arabic(s: &str) -> bool {
    s.chars().any(|c| {
        ('\u{0600}'..='\u{06FF}').contains(&c)
            || ('\u{0750}'..='\u{077F}').contains(&c)
            || ('\u{FB50}'..='\u{FDFF}').contains(&c)
            || ('\u{FE70}'..='\u{FEFF}').contains(&c)
    })
}

impl Story {
    /// Indonesian words across all sentences (what the listener hears).
    pub fn word_count(&self) -> usize {
        self.sentences.iter().map(|s| tokenize(&s.id).iter().filter(|t| t.is_wordlike()).count()).sum()
    }

    /// The level as a number for `bipaN` ids (`bipa1` → 1).
    pub fn level_rank(&self) -> Option<u8> {
        level_rank(&self.level)
    }
}

/// `bipa1`..`bipa7` → 1..7; anything else → `None`.
pub fn level_rank(level: &str) -> Option<u8> {
    let n: u8 = level.trim().to_lowercase().strip_prefix("bipa")?.trim().parse().ok()?;
    (1..=7).contains(&n).then_some(n)
}

impl AiOutput for Story {
    const TASK: Task = Task::StoryEpisode;

    fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("title must not be empty".into());
        }
        if self.level.trim().is_empty() {
            return Err("level must not be empty (e.g. \"bipa1\")".into());
        }
        let n = self.sentences.len();
        if !(MIN_SENTENCES..=MAX_SENTENCES).contains(&n) {
            return Err(format!("the story needs {MIN_SENTENCES}-{MAX_SENTENCES} sentences, got {n}"));
        }
        for (i, s) in self.sentences.iter().enumerate() {
            let k = i + 1;
            for (field, text) in [("id", &s.id), ("arz", &s.arz), ("ar", &s.ar), ("en", &s.en)] {
                if text.trim().is_empty() {
                    return Err(format!("sentence {k}: `{field}` must not be empty"));
                }
            }
            if has_arabic(&s.id) {
                return Err(format!("sentence {k}: `id` must be Indonesian in Latin script"));
            }
            if !has_arabic(&s.arz) {
                return Err(format!("sentence {k}: `arz` must be Egyptian Arabic in Arabic script"));
            }
            if !has_arabic(&s.ar) {
                return Err(format!("sentence {k}: `ar` must be Modern Standard Arabic in Arabic script"));
            }
            if has_arabic(&s.en) {
                return Err(format!("sentence {k}: `en` must be English only, without Arabic script"));
            }
        }
        let words = self.word_count();
        if words > MAX_WORDS {
            return Err(format!(
                "the story is too long: {words} Indonesian words, at most {MAX_WORDS} (about two minutes)"
            ));
        }
        if self.words.is_empty() || self.words.iter().any(|w| w.trim().is_empty()) {
            return Err("`words` must list the target words the story uses, none empty".into());
        }
        let unused = uses_words(self, &self.words);
        if !unused.is_empty() {
            return Err(format!(
                "`words` lists {} but no sentence uses {}",
                unused.iter().map(|w| format!("`{w}`")).collect::<Vec<_>>().join(", "),
                if unused.len() == 1 { "it" } else { "them" }
            ));
        }
        Ok(())
    }

    /// AI stories are learning text; Javanese ones (level `j1`..) stay
    /// unverified until the learner approves them.
    fn needs_review(&self) -> bool {
        self.level.trim().to_lowercase().starts_with('j')
    }
}

/// Indonesian words to aim for: about two minutes at a slow pace.
pub const LENGTH_WORDS: u32 = 90;

/// The router input for `story_episode`, in the golden set's field names.
pub fn story_request(words: &PagiWords, level: &str) -> serde_json::Value {
    json!({
        "series": "kabar_pagi",
        "level": level,
        "target_words": words.new,
        "due_words": words.due,
        "length_words": LENGTH_WORDS,
    })
}

/// The forms a story's Indonesian text contains: every word lowercased, the
/// base of reduplications (`anak-anak` → `anak`), the halves of hyphenated
/// words, and the dictionary root (`membaca` → `baca`, `rumahnya` → `rumah`).
/// Also the whole text, lowercased and space-joined, for multiword targets,
/// and the plain tokens for affix segmentation against a target root.
struct StoryForms<'m> {
    morph: &'m IndonesianMorph,
    forms: BTreeSet<String>,
    tokens: BTreeSet<String>,
    joined: String,
}

impl<'m> StoryForms<'m> {
    fn new(story: &Story, morph: &'m IndonesianMorph) -> Self {
        let mut forms = BTreeSet::new();
        let mut tokens = BTreeSet::new();
        let mut joined = String::from(" ");
        for s in &story.sentences {
            for t in tokenize(&s.id).iter().filter(|t| t.is_wordlike()) {
                let lower = t.lower();
                joined.push_str(&lower);
                joined.push(' ');
                tokens.extend(lower.split('-').map(str::to_string));
                let mut add = |w: &str| {
                    if w.is_empty() || !forms.insert(w.to_string()) {
                        return;
                    }
                    if let Some(root) = morph.root(w) {
                        forms.insert(root);
                    }
                };
                add(&lower);
                if let Some((_, base)) = &t.redup {
                    add(&base.to_lowercase());
                }
                if lower.contains('-') {
                    for part in lower.split('-') {
                        add(part);
                    }
                }
            }
        }
        Self { morph, forms, tokens, joined }
    }

    fn uses(&self, word: &str) -> bool {
        let w = word.trim().to_lowercase();
        if w.is_empty() {
            return true;
        }
        if w.contains(char::is_whitespace) {
            let phrase = w.split_whitespace().collect::<Vec<_>>().join(" ");
            return self.joined.contains(&format!(" {phrase} "));
        }
        if self.forms.contains(&w) {
            return true;
        }
        // The stemmer picks one root per word and can pick the wrong one
        // (`memesan` → `mesan`, not `pesan`). For an Indonesian root target,
        // also accept any token that segments as affixes around it.
        w.chars().count() >= 3
            && self.morph.is_root(&w)
            && self.tokens.iter().any(|t| segment(t, &w).is_some_and(|a| !a.is_bare()))
    }
}

/// The words in `words` that the story does NOT use (empty: it uses them
/// all). Affixed forms count (`dibaca` and `membaca` use `baca`), as do
/// reduplications and clitics (`anak-anak`, `rumahnya`). Matching is by
/// exact token, by the root the Indonesian stemmer finds, or by segmenting
/// a token as Indonesian affixes around the word (`memesan` uses `pesan`)
/// when the word is an Indonesian root. Javanese words (not in the
/// Indonesian dictionary) only match exactly. Multiword
/// targets (`terima kasih`) match as a phrase.
pub fn uses_words(story: &Story, words: &[String]) -> Vec<String> {
    uses_words_with(&IndonesianMorph::default(), story, words)
}

fn uses_words_with(morph: &IndonesianMorph, story: &Story, words: &[String]) -> Vec<String> {
    let forms = StoryForms::new(story, morph);
    words.iter().filter(|w| !forms.uses(w)).cloned().collect()
}

/// The build-time story bank (`content/pagi/stories.jsonl`).
#[derive(Clone, Debug, Default)]
pub struct StoryBank {
    pub stories: Vec<Story>,
}

impl StoryBank {
    /// One [`Story`] per line; blank lines are skipped. Every story must pass
    /// [`Story::validate`](AiOutput::validate). Errors name the line.
    pub fn parse(jsonl: &str) -> Result<Self, String> {
        let mut stories = Vec::new();
        for (i, line) in jsonl.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let n = i + 1;
            let story: Story = serde_json::from_str(line).map_err(|e| format!("line {n}: {e}"))?;
            story.validate().map_err(|e| format!("line {n} (`{}`): {e}", story.title))?;
            stories.push(story);
        }
        Ok(Self { stories })
    }

    /// The story that covers the most of today's words at or below `level`;
    /// ties broken by `seed` so mornings vary.
    ///
    /// Coverage counts due words first, then new words (a story with 3 due
    /// words beats one with 2 due and 3 new). Levels are `bipa1`..`bipa7`; a
    /// level outside that scheme only matches stories with the same level id.
    /// Returns `None` only when no story is at or below the level.
    pub fn best(&self, words: &PagiWords, level: &str, seed: u64) -> Option<&Story> {
        let max = level_rank(level);
        let eligible = |s: &&Story| match (max, s.level_rank()) {
            (Some(max), Some(r)) => r <= max,
            _ => s.level.trim().eq_ignore_ascii_case(level.trim()),
        };
        let morph = IndonesianMorph::default();
        let scored: Vec<((usize, usize), &Story)> = self
            .stories
            .iter()
            .filter(eligible)
            .map(|s| {
                let forms = StoryForms::new(s, &morph);
                let due = words.due.iter().filter(|w| forms.uses(w)).count();
                let new = words.new.iter().filter(|w| forms.uses(w)).count();
                ((due, new), s)
            })
            .collect();
        let top = scored.iter().map(|(score, _)| *score).max()?;
        let tied: Vec<&Story> = scored.iter().filter(|(score, _)| *score == top).map(|(_, s)| *s).collect();
        let pick = (seed % tied.len() as u64) as usize;
        Some(tied[pick])
    }
}

/// One TTS segment per sentence (Azure via Kabar Cloud, or system voices).
/// Play them with [`RATE_PERCENT`] and a [`PAUSE_SECONDS`] gap.
pub fn tts_segments(story: &Story, voice: Voice) -> Vec<TtsSegment> {
    story.sentences.iter().map(|s| TtsSegment { voice, text: s.id.trim().to_string() }).collect()
}

/// Rough listening time at a learner-friendly pace: Indonesian words at
/// [`WORDS_PER_SECOND`] (normal Azure pace slowed by 20%), plus a
/// [`PAUSE_SECONDS`] pause after each sentence, rounded up. A 10-sentence,
/// 80-word story is 80 / 1.6 + 10 = 60 seconds.
pub fn listening_seconds(story: &Story) -> u32 {
    let speech = story.word_count() as f32 / WORDS_PER_SECOND;
    let pauses = story.sentences.len() as f32 * PAUSE_SECONDS;
    (speech + pauses).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::json::parse;
    use kabar_protocol::ai::EngineReply;

    const BANK: &str = include_str!("../../../content/pagi/stories.jsonl");

    macro_rules! fixture {
        ($name:literal) => {
            serde_json::from_str::<EngineReply>(include_str!(concat!("../../../fixtures/ai/", $name)))
                .unwrap()
        };
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|w| w.to_string()).collect()
    }

    fn sentence(id: &str) -> StorySentence {
        StorySentence {
            id: id.into(),
            arz: "أنا بقرا كتاب.".into(),
            ar: "أقرأ كتابًا.".into(),
            en: "I am reading a book.".into(),
        }
    }

    /// The §24 morphology test texts as a small story.
    fn morph_story() -> Story {
        Story {
            title: "Hari Minggu".into(),
            level: "bipa1".into(),
            sentences: vec![
                sentence("Apa kabar?"),
                sentence("Saya sedang membaca buku."),
                sentence("Rumahnya dibersihkan kemarin."),
                sentence("Anak-anak bermain di taman."),
                sentence("Kesehatan itu penting."),
                sentence("Bukunya dibaca lagi."),
            ],
            words: s(&["baca", "bersih", "anak", "main", "sehat", "kabar"]),
            summary: None,
        }
    }

    fn bank() -> StoryBank {
        StoryBank::parse(BANK).unwrap()
    }

    // --- pick_words -------------------------------------------------------

    #[test]
    fn pick_words_takes_due_first_then_unknown_new() {
        let due = s(&["Pasar", "beli", "pasar", " ", "murah", "ikan", "harga", "mahal", "senang"]);
        let known = s(&["makan", "minum"]);
        let pool = s(&["makan", "senang", "ikan", "kopi", "Teh", "kopi", "minum", "air", "nasi"]);
        let w = pick_words(&due, &known, &pool, 7);
        assert_eq!(w.due, s(&["pasar", "beli", "murah", "ikan", "harga", "mahal"]));
        // `senang` is due (past the cap), `ikan` due, `makan`/`minum` known.
        assert_eq!(w.new, s(&["kopi", "teh", "air"]));
        assert_eq!(w.all().len(), MAX_DUE + MAX_NEW);
        // Deterministic, whatever the seed.
        assert_eq!(pick_words(&due, &known, &pool, 99), w);
    }

    #[test]
    fn pick_words_handles_short_inputs() {
        let w = pick_words(&[], &[], &[], 0);
        assert!(w.due.is_empty() && w.new.is_empty());
        let w = pick_words(&s(&["kabar"]), &s(&["kabar"]), &s(&["kabar", "waktu"]), 0);
        assert_eq!((w.due, w.new), (s(&["kabar"]), s(&["waktu"])));
    }

    // --- uses_words -------------------------------------------------------

    #[test]
    fn uses_words_counts_affixed_and_reduplicated_forms() {
        let st = morph_story();
        let want = s(&["baca", "bersih", "rumah", "anak", "main", "sehat", "buku", "Kabar", "taman"]);
        assert!(uses_words(&st, &want).is_empty(), "{:?}", uses_words(&st, &want));
        // Exact surface forms count too.
        assert!(uses_words(&st, &s(&["membaca", "dibersihkan", "anak-anak"])).is_empty());
        assert_eq!(uses_words(&st, &s(&["pasar", "baca", "makan"])), s(&["pasar", "makan"]));
        // The stemmer reads `memesan` as `mesan`; segmentation still finds `pesan`.
        let mut st = morph_story();
        st.sentences.push(sentence("Dia memesan ojek, lalu menelepon ibunya."));
        assert!(uses_words(&st, &s(&["pesan", "telepon", "ibu"])).is_empty());
        assert_eq!(uses_words(&st, &s(&["esan", "jek"])), s(&["esan", "jek"]));
    }

    #[test]
    fn uses_words_matches_phrases_and_javanese_exactly() {
        let mut st = morph_story();
        st.sentences.push(sentence("Terima kasih, Bu."));
        st.sentences.push(sentence("Kowe wis mangan?"));
        assert!(uses_words(&st, &s(&["terima kasih", "kowe", "mangan"])).is_empty());
        // Javanese words never match through the Indonesian stemmer.
        assert_eq!(uses_words(&st, &s(&["madang", "kasih terima"])), s(&["madang", "kasih terima"]));
    }

    // --- validate ---------------------------------------------------------

    #[test]
    fn validate_accepts_a_good_story() {
        assert_eq!(morph_story().validate(), Ok(()));
        assert!(!morph_story().needs_review());
    }

    #[test]
    fn validate_explains_each_problem() {
        let check = |f: &dyn Fn(&mut Story), expect: &str| {
            let mut st = morph_story();
            f(&mut st);
            let err = st.validate().unwrap_err();
            assert!(err.contains(expect), "{err:?} should mention {expect:?}");
        };
        check(&|st| st.title = " ".into(), "title");
        check(&|st| st.level.clear(), "level");
        check(&|st| st.sentences.truncate(5), "6-14 sentences, got 5");
        check(&|st| st.sentences.extend((0..9).map(|_| sentence("Saya membaca."))), "got 15");
        check(&|st| st.sentences[2].arz = "Rumahnya".into(), "sentence 3: `arz`");
        check(&|st| st.sentences[0].ar = "".into(), "sentence 1: `ar` must not be empty");
        check(&|st| st.sentences[1].en = "I read كتاب".into(), "sentence 2: `en`");
        check(&|st| st.sentences[1].id = "أنا".into(), "sentence 2: `id`");
        check(&|st| st.words.clear(), "`words`");
        check(&|st| st.words.push("pasar".into()), "`words` lists `pasar` but no sentence uses it");
        check(
            &|st| {
                let long = "Saya membaca buku baru di rumah setiap pagi dan setiap malam bersama teman";
                for x in &mut st.sentences {
                    x.id = format!("{long} {long} {long}.");
                }
            },
            "too long",
        );
    }

    #[test]
    fn javanese_stories_need_review() {
        let mut st = morph_story();
        st.level = "j1".into();
        assert!(st.needs_review());
    }

    // --- request, audio ---------------------------------------------------

    #[test]
    fn story_request_has_the_prompt_shape() {
        let w = PagiWords { due: s(&["pasar", "beli"]), new: s(&["murah"]) };
        assert_eq!(
            story_request(&w, "bipa1"),
            json!({
                "series": "kabar_pagi",
                "level": "bipa1",
                "target_words": ["murah"],
                "due_words": ["pasar", "beli"],
                "length_words": LENGTH_WORDS,
            })
        );
    }

    #[test]
    fn tts_has_one_segment_per_sentence() {
        let st = morph_story();
        let segs = tts_segments(&st, Voice::IdGadis);
        assert_eq!(segs.len(), 6);
        assert_eq!(segs[1], TtsSegment { voice: Voice::IdGadis, text: "Saya sedang membaca buku.".into() });
        assert!(segs.iter().all(|t| t.voice == Voice::IdGadis));
    }

    #[test]
    fn listening_time_is_words_at_slow_pace_plus_pauses() {
        let st = morph_story();
        // 2 + 4 + 3 + 4 + 3 + 3 = 19 words → 11.875 s, + 6 pauses = 17.875 → 18.
        assert_eq!(st.word_count(), 19);
        assert_eq!(listening_seconds(&st), 18);
        // The longest story allowed still fits in about two minutes and a half.
        assert!(MAX_WORDS as f32 / WORDS_PER_SECOND + MAX_SENTENCES as f32 * PAUSE_SECONDS <= 152.0);
    }

    // --- the bank ---------------------------------------------------------

    #[test]
    fn bank_is_complete_and_valid() {
        let b = bank();
        assert_eq!(b.stories.len(), 24);
        let count = |l: &str| b.stories.iter().filter(|s| s.level == l).count();
        assert_eq!((count("bipa1"), count("bipa2")), (16, 8));
        let mut titles = BTreeSet::new();
        for st in &b.stories {
            assert!(titles.insert(st.title.clone()), "duplicate title {}", st.title);
            assert!((6..=10).contains(&st.sentences.len()), "{}", st.title);
            assert!((5..=12).contains(&st.words.len()), "{}: {} words", st.title, st.words.len());
            let secs = listening_seconds(st);
            assert!((20..=120).contains(&secs), "{}: {secs}s", st.title);
            for x in &st.sentences {
                assert!(!x.en.contains('\u{2068}'), "{}: isolates belong in Arabic lines only", st.title);
            }
        }
    }

    #[test]
    fn bank_parse_names_the_bad_line() {
        let good = BANK.lines().next().unwrap();
        let err = StoryBank::parse(&format!("{good}\n\n{{\"title\": 1}}\n")).unwrap_err();
        assert!(err.starts_with("line 3:"), "{err}");

        let mut st: Story = serde_json::from_str(good).unwrap();
        st.sentences[0].arz = "no arabic".into();
        let line = serde_json::to_string(&st).unwrap();
        let err = StoryBank::parse(&format!("{good}\n{line}")).unwrap_err();
        assert!(err.starts_with("line 2 (") && err.contains("`arz`"), "{err}");
        assert!(StoryBank::parse("").unwrap().stories.is_empty());
    }

    #[test]
    fn bank_covers_frequent_words() {
        let b = bank();
        let morph = IndonesianMorph::default();
        let forms: Vec<StoryForms> = b.stories.iter().map(|s| StoryForms::new(s, &morph)).collect();
        let frequent = [
            "makan", "minum", "pergi", "rumah", "pagi", "malam", "sekolah", "kantor", "pasar", "hujan",
            "teman", "ibu", "ayah", "air", "jalan", "beli", "baca", "kerja", "datang", "pulang", "duduk",
            "tidur", "bersih", "senang", "besar", "kecil", "baru", "harus", "sudah", "belum", "mau",
        ];
        let missing: Vec<_> = frequent.iter().filter(|w| !forms.iter().any(|f| f.uses(w))).collect();
        assert!(missing.is_empty(), "no offline story uses {missing:?}");
    }

    #[test]
    fn best_prefers_due_coverage_and_respects_level() {
        let b = bank();
        let w = PagiWords { due: s(&["pasar", "ikan", "mahal", "murah"]), new: s(&[]) };
        let st = b.best(&w, "bipa1", 0).unwrap();
        assert_eq!(st.level, "bipa1");
        assert!(uses_words(st, &w.due).is_empty(), "{}", st.title);

        // A BIPA 2 word: only found when the level allows it.
        let w = PagiWords { due: s(&["wawancara"]), new: s(&[]) };
        assert!(uses_words(b.best(&w, "bipa2", 0).unwrap(), &w.due).is_empty());
        assert!(b.stories.iter().all(|x| x.level == "bipa1" || x.level == "bipa2"));
        assert_eq!(b.best(&w, "bipa1", 0).unwrap().level, "bipa1");
        assert!(b.best(&w, "j1", 0).is_none());
        assert!(StoryBank::default().best(&w, "bipa7", 0).is_none());
    }

    #[test]
    fn best_counts_due_before_new_and_varies_ties_by_seed() {
        let mk = |title: &str, words: &[&str]| Story {
            title: title.into(),
            level: "bipa1".into(),
            sentences: (0..6).map(|i| sentence(words[i % words.len()])).collect(),
            words: words.iter().map(|w| w.to_lowercase()).collect(),
            summary: None,
        };
        let a = mk("A", &["Pasar", "Kopi", "Teh"]);
        let b = mk("B", &["Ikan"]);
        let c = mk("C", &["Ikan"]);
        let bank = StoryBank { stories: vec![a, b, c] };
        let w = PagiWords { due: s(&["ikan"]), new: s(&["pasar", "kopi", "teh"]) };
        // One due word beats three new ones; B and C tie and the seed picks.
        assert_eq!(bank.best(&w, "bipa3", 0).unwrap().title, "B");
        assert_eq!(bank.best(&w, "bipa3", 1).unwrap().title, "C");
        assert_eq!(bank.best(&w, "bipa3", 2).unwrap().title, "B");
        // Nothing matches: still a story, so offline mornings never go empty.
        let none = PagiWords { due: s(&["gunung"]), new: s(&[]) };
        assert!(bank.best(&none, "bipa1", 5).is_some());
    }

    // --- fixtures and the router ------------------------------------------

    #[test]
    fn fixtures_parse_and_validate() {
        let good = fixture!("story_episode.pasar_ikan.workers_ai.json");
        let st: Story = parse(&good.text).unwrap();
        assert_eq!(st.validate(), Ok(()));
        let asked = s(&["pasar", "ikan", "beli", "murah", "segar", "pagi"]);
        assert!(uses_words(&st, &asked).is_empty(), "{:?}", uses_words(&st, &asked));

        let bad = fixture!("story_episode.pasar_ikan.invalid.workers_ai.json");
        let st: Story = parse(&bad.text).unwrap();
        assert!(st.validate().unwrap_err().contains("`arz` must be Egyptian Arabic"));
    }

    #[test]
    fn router_retries_an_invalid_story_with_the_error() {
        use crate::ai::{Engine, EngineFuture, MemoryCache, PromptSet, RouteTable, Router};
        use kabar_protocol::ai::{EngineCall, EngineId};
        use std::cell::RefCell;
        use std::task::{Context, Poll, Waker};

        struct Two(RefCell<Vec<EngineReply>>, RefCell<Vec<EngineCall>>);
        impl Engine for &'static Two {
            fn id(&self) -> EngineId {
                EngineId::WorkersAi
            }
            fn is_free(&self) -> bool {
                true
            }
            fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a> {
                self.1.borrow_mut().push(call.clone());
                let next = self.0.borrow_mut().remove(0);
                Box::pin(async move { Ok(next) })
            }
        }
        let engine: &'static Two = Box::leak(Box::new(Two(
            RefCell::new(vec![
                fixture!("story_episode.pasar_ikan.invalid.workers_ai.json"),
                fixture!("story_episode.pasar_ikan.workers_ai.json"),
            ]),
            RefCell::new(Vec::new()),
        )));
        let r = Router::new(RouteTable::builtin(), PromptSet::builtin(), Box::new(MemoryCache::default()))
            .with_engine(Box::new(engine));
        let words = PagiWords { due: s(&["pasar", "ikan", "beli", "murah"]), new: s(&["segar", "pagi"]) };
        let req = crate::ai::AiRequest::new(story_request(&words, "bipa1"));
        let mut fut = std::pin::pin!(r.run::<Story>(&req));
        let Poll::Ready(answer) = fut.as_mut().poll(&mut Context::from_waker(Waker::noop())) else {
            panic!("test engines never pend");
        };
        let answer = answer.unwrap();
        assert!(uses_words(&answer.value, &words.all()).is_empty());
        let calls = engine.1.borrow();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].user.contains("\"due_words\"") && calls[0].user.contains("pasar"));
        assert!(calls[1].user.contains("`arz` must be Egyptian Arabic"));
    }
}
