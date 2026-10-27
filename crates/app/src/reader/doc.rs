//! The Reader's view model: a text split into segments (paragraphs or chat
//! bubbles), analyzed by the language engine, with local word data attached.
//! Pure logic, so it's tested without a WebView.

use std::sync::OnceLock;

use kabar_core::grammar::word::{self, WordClass};
use kabar_core::lang::Lang;
use kabar_core::lang::analyze::{AnalyzedToken, Engine, Morph};
use kabar_core::lang::langid::Source;
use kabar_core::lang::morph_jv::{self, JvAnalysis};
use kabar_core::reader::screenshot::{ItemKind, ScreenDoc, Side};

use kabar_core::learn::PersonalDictionary;
use kabar_protocol::content;
use kabar_protocol::learn::TaughtEntry;

use super::glossary::{Glossary, WordInfo, WordLang};

/// Where word data comes from: the content pack and what the learner taught.
#[derive(Clone, Copy)]
pub struct Lexicon<'a> {
    pub glossary: &'a dyn Glossary,
    pub taught: &'a PersonalDictionary,
}

/// The engine loads ~27k roots once per process.
pub fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::default)
}

#[derive(Clone, Debug, PartialEq)]
pub enum SegKind {
    Para,
    Bubble {
        side: Side,
        time: Option<String>,
        speaker: Option<String>,
    },
    /// Screenshot chrome (dates, headers, standalone times): shown, not analyzed.
    Chrome,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: usize,
    pub end: usize,
    pub kind: SegKind,
    /// Index range into `ReadDoc::tokens`.
    pub tokens: std::ops::Range<usize>,
}

/// What the language tag in the popover says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    Indonesian,
    Javanese,
    English,
    Proper,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub analyzed: AnalyzedToken,
    pub tag: Tag,
    /// Written as chat slang or an abbreviation (arp → arep).
    pub slang: bool,
    /// Status key, e.g. "id:baca". None for names, which have no status.
    pub key: Option<String>,
    /// The prebuilt gloss (content pack), never replaced by a taught one.
    pub info: Option<WordInfo>,
    /// What the learner taught Kabar about this word: approved, or a draft.
    pub taught: Option<TaughtEntry>,
    /// Verb, noun, adjective or function word, for its colour; worked out
    /// once here so rendering only reads it. None for names and English.
    pub class: Option<WordClass>,
}

impl Word {
    /// An approved taught entry, shown first and badged "taught by you".
    pub fn taught_approved(&self) -> Option<&TaughtEntry> {
        self.taught.as_ref().filter(|t| t.is_approved())
    }

    /// A draft waiting for review: a chip, not an answer.
    pub fn taught_draft(&self) -> Option<&TaughtEntry> {
        self.taught.as_ref().filter(|t| !t.is_approved())
    }

    /// Kabar has something to say about the word.
    pub fn has_gloss(&self) -> bool {
        self.info.is_some() || self.taught_approved().is_some()
    }

    /// What a one-line view shows: the learner's own entry first.
    pub fn shown(&self) -> Option<WordInfo> {
        self.taught_approved().and_then(super::content::taught_info).or_else(|| self.info.clone())
    }

    /// "di- + bersih + -kan", when the word has affixes.
    pub fn breakdown(&self) -> Option<String> {
        match &self.analyzed.morph {
            Some(Morph::Indonesian(a)) if !a.is_bare() => Some(a.display()),
            Some(Morph::Javanese(a)) if a.prefix.is_some() || !a.suffixes.is_empty() => Some(a.display()),
            _ => None,
        }
    }

    /// English mixed into the text, which the learner already reads.
    pub fn is_english_aside(&self) -> bool {
        self.key.is_none() && self.tag == Tag::English
    }

    /// Bridge gold: an Arabic-origin word.
    pub fn is_bridge(&self) -> bool {
        self.info.as_ref().is_some_and(|i| i.bridge.is_some())
            || self.taught_approved().is_some_and(|t| t.bridge.is_some())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadToken {
    /// Absolute byte range in the body.
    pub start: usize,
    pub end: usize,
    pub seg: usize,
    pub word: Option<Word>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadDoc {
    pub body: String,
    pub segments: Vec<Segment>,
    pub tokens: Vec<ReadToken>,
}

impl ReadDoc {
    /// Pasted or shared text: one segment per non-empty line.
    pub fn from_text(body: &str, lex: Lexicon) -> Self {
        let mut b = Builder::new(body);
        let mut offset = 0;
        for line in body.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if !trimmed.trim().is_empty() {
                b.push(offset, offset + trimmed.len(), SegKind::Para, lex);
            }
            offset += line.len();
        }
        b.finish()
    }

    /// A structured screenshot: every item becomes one line of the body.
    pub fn from_screen(screen: &ScreenDoc, lex: Lexicon) -> Self {
        let body = screen.items.iter().map(|i| i.text.replace('\n', " ")).collect::<Vec<_>>().join("\n");
        let mut b = Builder::new(&body);
        let mut offset = 0;
        for item in &screen.items {
            let len = item.text.replace('\n', " ").len();
            let kind = match &item.kind {
                ItemKind::Bubble { side } => {
                    SegKind::Bubble { side: *side, time: item.time.clone(), speaker: item.speaker.clone() }
                }
                ItemKind::Text => SegKind::Para,
                ItemKind::Timestamp | ItemKind::Label => SegKind::Chrome,
            };
            b.push(offset, offset + len, kind, lex);
            offset += len + 1;
        }
        b.finish()
    }

    pub fn text(&self, start: usize, end: usize) -> &str {
        &self.body[start..end]
    }

    /// The sentence around a token, for "meaning in this sentence" and cloze cards.
    pub fn sentence_of(&self, token: usize) -> &str {
        let (start, end) = self.sentence_span(token);
        &self.body[start..end]
    }

    /// Byte range of the sentence around a token, trimmed.
    pub fn sentence_span(&self, token: usize) -> (usize, usize) {
        let t = &self.tokens[token];
        let seg = &self.segments[t.seg];
        let s = &self.body[seg.start..seg.end];
        let rel = t.start - seg.start;
        let is_end = |c: char| matches!(c, '.' | '!' | '?' | '…');
        let from = s[..rel].rfind(is_end).map_or(0, |i| i + 1);
        let to = s[rel..].find(is_end).map_or(s.len(), |i| rel + i + 1);
        let lead = s[from..to].len() - s[from..to].trim_start().len();
        let start = seg.start + from + lead;
        (start, start + s[from..to].trim().len())
    }

    /// The text covered by a set of tokens (lasso, drag-select, highlight).
    pub fn span_of(&self, tokens: &[usize]) -> Option<(usize, usize)> {
        let start = tokens.iter().filter_map(|&i| self.tokens.get(i)).map(|t| t.start).min()?;
        let end = tokens.iter().filter_map(|&i| self.tokens.get(i)).map(|t| t.end).max()?;
        Some((start, end))
    }

    /// Tokens inside a byte range (to paint saved highlights).
    pub fn tokens_in(&self, start: usize, end: usize) -> impl Iterator<Item = usize> + '_ {
        self.tokens.iter().enumerate().filter(move |(_, t)| t.start >= start && t.end <= end).map(|(i, _)| i)
    }

    /// The language of a run of tokens: Javanese when more of its words are
    /// Javanese than Indonesian.
    pub fn lang_of(&self, tokens: impl IntoIterator<Item = usize>) -> Lang {
        let (mut jv, mut id) = (0, 0);
        for t in tokens {
            match self.tokens.get(t).and_then(|t| t.word.as_ref()).map(|w| w.tag) {
                Some(Tag::Javanese) => jv += 1,
                Some(Tag::Indonesian) => id += 1,
                _ => {}
            }
        }
        if jv > id { Lang::Javanese } else { Lang::Indonesian }
    }

    /// Distinct status keys, for one batch status query.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.tokens.iter().filter_map(|t| t.word.as_ref()?.key.as_deref())
    }
}

struct Builder<'a> {
    body: &'a str,
    segments: Vec<Segment>,
    tokens: Vec<ReadToken>,
}

impl<'a> Builder<'a> {
    fn new(body: &'a str) -> Self {
        Self { body, segments: Vec::new(), tokens: Vec::new() }
    }

    fn push(&mut self, start: usize, end: usize, kind: SegKind, lex: Lexicon) {
        let seg = self.segments.len();
        let first = self.tokens.len();
        if kind != SegKind::Chrome {
            for a in engine().analyze(&self.body[start..end]) {
                let (s, e) = (start + a.token.start, start + a.token.end);
                // "512gb" and "17" are numbers, not words to learn.
                let number = a.token.text.starts_with(|c: char| c.is_ascii_digit());
                let word = (a.token.is_wordlike() && !number).then(|| word_of(a, lex));
                self.tokens.push(ReadToken { start: s, end: e, seg, word });
            }
        }
        self.segments.push(Segment { start, end, kind, tokens: first..self.tokens.len() });
    }

    fn finish(self) -> ReadDoc {
        ReadDoc { body: self.body.to_string(), segments: self.segments, tokens: self.tokens }
    }
}

fn word_of(mut analyzed: AnalyzedToken, lex: Lexicon) -> Word {
    let lookup = analyzed.lookup_form();
    let surface = analyzed.token.lower();
    let root = match &analyzed.morph {
        Some(Morph::Indonesian(a)) => Some(a.root.clone()),
        Some(Morph::Javanese(a)) => Some(a.root.clone()),
        None => None,
    };
    let redup = analyzed.token.redup.as_ref().map(|(_, base)| base.to_lowercase());
    // A form normalized in context ("pm" after a phone is "Pro Max") is
    // looked up first, so the gloss follows the reading the headline shows.
    let mut forms: Vec<&str> =
        if analyzed.normalized.is_some() { vec![&lookup, &surface] } else { vec![&surface, &lookup] };
    forms.extend(redup.as_deref());
    forms.extend(root.as_deref());
    let prefer = match analyzed.lang.lang {
        Lang::Javanese => Some(WordLang::Javanese),
        Lang::Indonesian => Some(WordLang::Indonesian),
        Lang::English => Some(WordLang::English),
        _ => None,
    };
    let mut info = lex.glossary.lookup(prefer, &forms).cloned();
    // "ukumane", "bukune": a Javanese ending on a root the engine's small
    // lexicon doesn't know, but the dictionary does.
    if info.is_none()
        && let Some((a, found)) = javanese_ending(&surface, lex.glossary)
    {
        analyzed.lang.lang = Lang::Javanese;
        analyzed.morph = Some(Morph::Javanese(a));
        info = Some(found);
    }
    let taught_langs = match prefer.or(info.as_ref().map(|i| i.lang)) {
        Some(WordLang::Javanese) => [content::Lang::Jv, content::Lang::Id],
        _ => [content::Lang::Id, content::Lang::Jv],
    };
    let taught = taught_langs.into_iter().find_map(|l| lex.taught.lookup_forms(l, &forms)).cloned();
    let taught_lang = taught.as_ref().map(|t| match t.entry.lang {
        content::Lang::Id => WordLang::Indonesian,
        content::Lang::Jv => WordLang::Javanese,
    });

    let tag = match (analyzed.lang.lang, info.as_ref().map(|i| i.lang).or(taught_lang)) {
        (Lang::Proper, _) => Tag::Proper,
        (Lang::Indonesian, _) => Tag::Indonesian,
        (Lang::Javanese, _) => Tag::Javanese,
        (Lang::English, _) => Tag::English,
        (Lang::Unknown, Some(WordLang::Indonesian)) => Tag::Indonesian,
        (Lang::Unknown, Some(WordLang::Javanese)) => Tag::Javanese,
        (Lang::Unknown, Some(WordLang::English)) => Tag::English,
        (Lang::Unknown, None) => Tag::Unknown,
    };
    let slang = analyzed.lang.source == Source::Slang
        || analyzed.normalized.as_ref().is_some_and(|n| n.full.to_lowercase() != surface);

    // English mixed into the chat has no status: Kabar teaches Indonesian
    // and Javanese, and the learner already reads English.
    let english_only = tag == Tag::English && info.is_none() && taught.is_none();
    let key = (tag != Tag::Proper && !english_only).then(|| {
        let prefix = match info.as_ref().map(|i| i.lang).or(taught_lang) {
            Some(WordLang::Javanese) => "jv",
            Some(WordLang::English) => "en",
            Some(WordLang::Indonesian) => "id",
            None => match tag {
                Tag::Javanese => "jv",
                Tag::English => "en",
                _ => "id",
            },
        };
        let lemma = info
            .as_ref()
            .map(|i| i.lemma.clone())
            .or_else(|| taught.as_ref().map(|t| t.entry.lemma.clone()))
            .or(root)
            .or(redup)
            .unwrap_or(lookup);
        format!("{prefix}:{lemma}")
    });
    let class = match tag {
        Tag::Indonesian => Some(Lang::Indonesian),
        Tag::Javanese => Some(Lang::Javanese),
        _ => None,
    }
    .and_then(|lang| word::class_in(&analyzed, lang, info.as_ref().and_then(|i| i.pos.as_deref())));
    Word { analyzed, tag, slang, key, info, taught, class }
}

/// The engine's Javanese analysis, checked against the dictionary instead of
/// the engine's own root list.
struct DictionaryRoots<'a> {
    glossary: &'a dyn Glossary,
    /// Also accept Indonesian roots: "bukune" is buku + Javanese -ne.
    any_lang: bool,
}

impl kabar_core::lang::Lexicon for DictionaryRoots<'_> {
    fn is_id_root(&self, _: &str) -> bool {
        false
    }

    fn is_jv_root(&self, word: &str) -> bool {
        self.glossary
            .lookup(Some(WordLang::Javanese), &[word])
            .is_some_and(|i| self.any_lang || i.lang == WordLang::Javanese)
    }
}

fn javanese_ending(surface: &str, glossary: &dyn Glossary) -> Option<(JvAnalysis, WordInfo)> {
    let jv = DictionaryRoots { glossary, any_lang: false };
    let found =
        morph_jv::analyze(surface, &jv).filter(|a| a.verified && !a.suffixes.is_empty()).or_else(|| {
            // An Indonesian root only takes the possessive ending, nothing else.
            let any = DictionaryRoots { glossary, any_lang: true };
            morph_jv::analyze(surface, &any).filter(|a| {
                a.verified
                    && a.prefix.is_none()
                    && a.suffixes.len() == 1
                    && matches!(a.suffixes[0].label.as_str(), "-e" | "-ne")
            })
        })?;
    let info = glossary.lookup(Some(WordLang::Javanese), &[&found.root])?.clone();
    Some((found, info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::glossary::SeedGlossary;

    fn doc(text: &str) -> ReadDoc {
        doc_with(text, &PersonalDictionary::default())
    }

    fn doc_with(text: &str, taught: &PersonalDictionary) -> ReadDoc {
        ReadDoc::from_text(text, Lexicon { glossary: &SeedGlossary::default(), taught })
    }

    fn word<'a>(d: &'a ReadDoc, surface: &str) -> &'a Word {
        d.tokens
            .iter()
            .find(|t| d.text(t.start, t.end) == surface)
            .and_then(|t| t.word.as_ref())
            .unwrap_or_else(|| panic!("no word {surface}"))
    }

    #[test]
    fn javanese_chat_slang_resolves_to_glosses() {
        let d = doc("Sek tak lapor pusat\nng prabowo?\npiye dhe\narp dicul piro");
        assert_eq!(d.segments.len(), 4);
        let arp = word(&d, "arp");
        assert!(arp.slang);
        assert_eq!(arp.tag, Tag::Javanese);
        assert_eq!(arp.info.as_ref().unwrap().lemma, "arep");
        assert_eq!(arp.key.as_deref(), Some("jv:arep"));
        let ng = word(&d, "ng");
        assert_eq!(ng.info.as_ref().unwrap().lemma, "nang");
        let dicul = word(&d, "dicul");
        assert_eq!(dicul.key.as_deref(), Some("jv:cul"));
        assert!(dicul.breakdown().unwrap().contains("di-"));
        // Lowercase "prabowo" can't be told from a word without AI; it just has no gloss.
        assert!(word(&d, "prabowo").info.is_none());
        let named = doc("Sek tak lapor ng Prabowo");
        assert_eq!(word(&named, "Prabowo").key, None, "names have no status");
    }

    #[test]
    fn indonesian_morphology_and_bridge() {
        let d = doc("Rumahnya dibersihkan kemarin.\nApa kabar?");
        let w = word(&d, "dibersihkan");
        assert_eq!(w.breakdown().as_deref(), Some("di- + bersih + -kan"));
        assert_eq!(w.key.as_deref(), Some("id:bersih"));
        assert!(word(&d, "kabar").is_bridge());
        assert!(!word(&d, "kemarin").is_bridge());
    }

    #[test]
    fn reduplication_shares_the_base_lemma() {
        let d = doc("Anak-anak bermain di taman.");
        assert_eq!(word(&d, "Anak-anak").key.as_deref(), Some("id:anak"));
        assert_eq!(word(&d, "bermain").key.as_deref(), Some("id:main"));
    }

    #[test]
    fn sentences_spans_and_offsets() {
        let d = doc("Saya sedang membaca buku. Kesehatan itu penting.");
        let i = d.tokens.iter().position(|t| d.text(t.start, t.end) == "itu").unwrap();
        assert_eq!(d.sentence_of(i), "Kesehatan itu penting.");
        let (s, e) = d.span_of(&[i, i + 1]).unwrap();
        assert_eq!(d.text(s, e), "itu penting");
        assert_eq!(d.tokens_in(s, e).count(), 2);
    }

    #[test]
    fn code_mixed_slang() {
        let d = doc("Which is, gue literally capek banget.");
        assert_eq!(word(&d, "capek").info.as_ref().unwrap().gloss.en, "tired");
        assert_eq!(word(&d, "literally").tag, Tag::English);
        // English the learner already reads has no status to track.
        assert_eq!(word(&d, "literally").key, None);
        assert!(word(&d, "gue").key.is_some());
    }

    #[test]
    fn marketplace_listing_follows_the_normalized_reading() {
        let d = doc("iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa");
        // "pm" after a model number is Pro Max, and the gloss says so too.
        assert_eq!(word(&d, "pm").info.as_ref().unwrap().gloss.en, "Pro Max (the largest model)");
        assert_eq!(word(&d, "nomin").info.as_ref().unwrap().gloss.en, "no flaws at all");
        // Numbers with units aren't words to learn.
        let gb = d.tokens.iter().find(|t| d.text(t.start, t.end) == "512gb").unwrap();
        assert!(gb.word.is_none());
    }

    #[test]
    fn javanese_endings_on_dictionary_roots() {
        let g = crate::reader::content::LocalGlossary::load();
        let d = ReadDoc::from_text(
            "Ukumane abot. Bukune ana ngendi?",
            Lexicon { glossary: &g, taught: &PersonalDictionary::default() },
        );
        let w = word(&d, "Ukumane");
        assert_eq!(w.info.as_ref().map(|i| i.lemma.as_str()), Some("ukuman"));
        assert_eq!(w.tag, Tag::Javanese);
        assert_eq!(w.breakdown().as_deref(), Some("ukuman + -e"));
        let w = word(&d, "Bukune");
        assert_eq!(w.info.as_ref().map(|i| i.lemma.as_str()), Some("buku"));
        assert_eq!(w.breakdown().as_deref(), Some("buku + -ne"));
    }

    #[test]
    fn taught_words_come_first_and_drafts_stay_chips() {
        let mut taught: TaughtEntry =
            serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
        let kw = taught.entry.word.clone();
        let text = format!("Ini {kw} baru.");
        // A draft: no answer, only the chip.
        let d = doc_with(&text, &PersonalDictionary::new([taught.clone()]));
        let w = word(&d, &kw);
        assert!(w.taught_draft().is_some());
        assert!(w.taught_approved().is_none());
        // Approved: shown first, and the prebuilt gloss is kept.
        taught.entry.status = kabar_protocol::content::ReviewStatus::Approved;
        let d = doc_with(&text, &PersonalDictionary::new([taught]));
        let w = word(&d, &kw);
        assert!(w.taught_approved().is_some());
        assert!(w.has_gloss());
    }
}

/// Timings for the performance pass (SPEC §25: popover under 100 ms on the
/// S24 Ultra). Not a gate: run it with
/// `cargo test --release -p kabar-app popover_timing -- --ignored --nocapture`.
#[cfg(test)]
mod perf {
    use super::*;
    use crate::reader::content::LocalGlossary;

    #[test]
    #[ignore = "timing probe, run by hand with --release"]
    fn popover_timing() {
        let t = std::time::Instant::now();
        let g = LocalGlossary::load();
        eprintln!("glossary load: {:?}", t.elapsed());
        // The §24 test texts.
        let texts = [
            "Sek tak lapor pusat\nng prabowo?\npiye dhe\narp dicul piro",
            "iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa",
            "Saya sedang membaca buku. Rumahnya dibersihkan kemarin. Anak-anak bermain di taman.",
        ];
        let taught = PersonalDictionary::default();
        let t = std::time::Instant::now();
        for text in texts {
            let _ = ReadDoc::from_text(text, Lexicon { glossary: &g, taught: &taught });
        }
        eprintln!("analyse the §24 texts: {:?}", t.elapsed());
    }
}
