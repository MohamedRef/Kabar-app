//! Teach Kabar (`docs/SPEC.md` §26, PLAN §15): the learner gives Kabar an
//! explanation of a word it didn't know; the `learn_entry` task rewrites it
//! in Kabar's gloss format; the learner reviews the draft; approved entries
//! live in the personal dictionary above the prebuilt glosses.
//!
//! [`prepare`] cleans the sources and builds the router request,
//! [`LearnEntry`] is the validated model answer, [`draft`] turns it into a
//! [`TaughtEntry`], and [`dictionary`] serves it to popovers.

pub mod dictionary;
pub mod text;

use kabar_protocol::ai::Task;
use kabar_protocol::content::{Example, GlossEntry, Lang, ReviewStatus, Sense, Trilingual};
use kabar_protocol::learn::{
    MAX_EXCERPT_CHARS, SourceKind, SpeechLevels, TAUGHT_SCHEMA_VERSION, TaughtBridge, TaughtEntry,
    TeachSource,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ai::{AiOutput, AiRequest, Answer};
use crate::lang::analyze::{Engine, Morph};

pub use dictionary::{GlossLayer, PersonalDictionary, popover_layers};

/// Longest word or phrase Kabar can be taught (a short phrase, not a text).
pub const MAX_WORD_CHARS: usize = 60;
/// At most this many sources go into one request.
pub const MAX_SOURCES: usize = 4;
/// Longest context sentence sent with the word.
pub const MAX_CONTEXT_CHARS: usize = 300;

/// Something the learner handed Kabar about the word.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceInput {
    /// Typed or pasted text (from Google, a friend, a dictionary app).
    Pasted { text: String },
    /// A web page the app fetched: its HTML, or `None` when the site refused
    /// (then the learner is asked to paste the text instead).
    Link { url: String, html: Option<String> },
    /// OCR text of a screenshot (ML Kit / Apple Vision). Personal.
    Screenshot { ocr_text: String },
    /// Wiktionary's definition API answer.
    Wiktionary { json: Value },
}

/// What the Teach screen collected.
#[derive(Clone, Debug, PartialEq)]
pub struct TeachRequest {
    pub lang: Lang,
    pub word: String,
    /// The sentence the learner met the word in.
    pub context: Option<String>,
    pub sources: Vec<SourceInput>,
}

/// Why a request can't be sent yet; the screen says so warmly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeachError {
    EmptyWord,
    WordTooLong,
    /// Every source was empty after cleaning (or a link couldn't be read).
    NothingToLearnFrom,
    /// A link could not be fetched; ask the learner to paste the text.
    LinkUnreadable {
        url: String,
    },
    /// Wiktionary has no entry for the word in this language.
    NotInWiktionary,
}

/// What the deterministic engine thinks of the word, sent to the model as a
/// hint and compared with its answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affixes: Vec<String>,
}

/// A request ready for the router, plus what the draft keeps.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub lang: Lang,
    /// Lowercase, trimmed.
    pub word: String,
    pub context: Option<String>,
    pub sources: Vec<TeachSource>,
    pub hints: Hints,
    pub request: AiRequest,
}

/// Cleans the word and sources, scrubs personal details and builds the
/// `learn_entry` request. Screenshots make the request personal.
pub fn prepare(req: &TeachRequest, engine: &Engine) -> Result<Prepared, TeachError> {
    let word = req.word.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    if word.is_empty() {
        return Err(TeachError::EmptyWord);
    }
    if word.chars().count() > MAX_WORD_CHARS {
        return Err(TeachError::WordTooLong);
    }
    let context = req
        .context
        .as_deref()
        .map(|c| text::scrub(c.trim()).chars().take(MAX_CONTEXT_CHARS).collect::<String>())
        .filter(|c| !c.is_empty());

    let mut sources = Vec::new();
    let mut personal = false;
    for s in req.sources.iter().take(MAX_SOURCES) {
        let source = match s {
            SourceInput::Pasted { text } => TeachSource {
                kind: SourceKind::Pasted,
                url: None,
                title: None,
                excerpt: text::excerpt(&text::scrub(text), &word, MAX_EXCERPT_CHARS),
            },
            SourceInput::Link { url, html } => {
                let Some(html) = html else { return Err(TeachError::LinkUnreadable { url: url.clone() }) };
                let page = text::html_to_text(html);
                if page.text.trim().is_empty() {
                    return Err(TeachError::LinkUnreadable { url: url.clone() });
                }
                TeachSource {
                    kind: SourceKind::Link,
                    url: Some(url.clone()),
                    title: page.title,
                    excerpt: text::excerpt(&text::scrub(&page.text), &word, MAX_EXCERPT_CHARS),
                }
            }
            SourceInput::Screenshot { ocr_text } => {
                personal = true;
                TeachSource {
                    kind: SourceKind::Screenshot,
                    url: None,
                    title: None,
                    excerpt: text::excerpt(&text::scrub(ocr_text), &word, MAX_EXCERPT_CHARS),
                }
            }
            SourceInput::Wiktionary { json } => {
                let found = text::wiktionary_text(json, req.lang).ok_or(TeachError::NotInWiktionary)?;
                TeachSource {
                    kind: SourceKind::Wiktionary,
                    url: Some(text::wiktionary_page_url(&word)),
                    title: Some(format!("Wiktionary: {word}")),
                    excerpt: text::excerpt(&found, &word, MAX_EXCERPT_CHARS),
                }
            }
        };
        if !source.excerpt.trim().is_empty() {
            sources.push(source);
        }
    }
    if sources.is_empty() {
        return Err(TeachError::NothingToLearnFrom);
    }

    let hints = hints(engine, &word, req.lang);
    let input = json!({
        "lang": req.lang.code(),
        "word": word,
        "context": context,
        "hints": hints,
        "sources": sources.iter().map(|s| json!({
            "kind": s.kind,
            "title": s.title,
            "url": s.url,
            "text": s.excerpt,
        })).collect::<Vec<_>>(),
    });
    let request = AiRequest { input, personal, image_sha256: None };
    Ok(Prepared { lang: req.lang, word, context, sources, hints, request })
}

fn hints(engine: &Engine, word: &str, lang: Lang) -> Hints {
    if word.contains(' ') {
        return Hints::default();
    }
    let tokens = engine.analyze(word);
    let Some(t) = tokens.iter().find(|t| t.token.is_wordlike()) else { return Hints::default() };
    let normalized = t.normalized.as_ref().map(|n| n.full.to_lowercase()).filter(|n| n != word);
    let (root, display) = match (&t.morph, lang) {
        (Some(Morph::Indonesian(a)), Lang::Id) if !a.is_bare() => (Some(a.root.clone()), a.display()),
        (Some(Morph::Javanese(a)), Lang::Jv) if a.prefix.is_some() || !a.suffixes.is_empty() => {
            (Some(a.root.clone()), a.display())
        }
        _ => (None, String::new()),
    };
    let affixes = if root.is_some() { display.split(" + ").map(String::from).collect() } else { Vec::new() };
    Hints { normalized, root, affixes }
}

/// The model's answer for `learn_entry` (`prompts/learn_entry@v1.md`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearnEntry {
    pub word: String,
    pub lemma: String,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub affixes: Vec<String>,
    pub senses: Vec<Sense>,
    pub examples: Vec<Example>,
    #[serde(default)]
    pub note: Option<Trilingual>,
    #[serde(default)]
    pub arabic_origin: Option<String>,
    #[serde(default)]
    pub false_friend: Option<Trilingual>,
    #[serde(default)]
    pub bridge: Option<TaughtBridge>,
    #[serde(default)]
    pub levels: Option<SpeechLevels>,
    #[serde(default)]
    pub doubts: Vec<Trilingual>,
    pub confident: bool,
}

fn has_arabic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

fn check_tri(t: &Trilingual, what: &str) -> Result<(), String> {
    if t.arz.trim().is_empty() || t.ar.trim().is_empty() || t.en.trim().is_empty() {
        return Err(format!("{what} needs arz, ar and en"));
    }
    if !has_arabic(&t.arz) || !has_arabic(&t.ar) {
        return Err(format!("{what}: arz and ar must be written in Arabic script"));
    }
    Ok(())
}

/// The shape checks a taught gloss must pass, from the model or after the
/// learner's edits.
pub fn check_gloss(
    word: &str,
    lemma: &str,
    root: Option<&str>,
    senses: &[Sense],
    examples: &[Example],
    arabic_origin: Option<&str>,
    false_friend: Option<&Trilingual>,
) -> Result<(), String> {
    if word.trim().is_empty() || lemma.trim().is_empty() {
        return Err("word and lemma must not be empty".into());
    }
    if senses.is_empty() || senses.len() > 4 {
        return Err(format!("give 1-4 senses, got {}", senses.len()));
    }
    for (i, s) in senses.iter().enumerate() {
        check_tri(&s.gloss, &format!("senses[{i}].gloss"))?;
    }
    if examples.is_empty() || examples.len() > 3 {
        return Err(format!("give 2 examples (3 at most), got {}", examples.len()));
    }
    let forms: Vec<String> =
        [Some(word), Some(lemma), root].into_iter().flatten().map(|f| f.trim().to_lowercase()).collect();
    for (i, e) in examples.iter().enumerate() {
        let text = e.text.to_lowercase();
        if !forms.iter().any(|f| !f.is_empty() && text.contains(f.as_str())) {
            return Err(format!("examples[{i}] must contain `{word}` (or its lemma)"));
        }
        if usize::from(e.sense) >= senses.len() {
            return Err(format!("examples[{i}].sense points past the senses list"));
        }
        check_tri(&e.translation, &format!("examples[{i}].translation"))?;
    }
    if let Some(a) = arabic_origin
        && !has_arabic(a)
    {
        return Err("arabic_origin must be in Arabic script".into());
    }
    if false_friend.is_some() && arabic_origin.is_none() {
        return Err("false_friend needs arabic_origin".into());
    }
    if let Some(f) = false_friend {
        check_tri(f, "false_friend")?;
    }
    Ok(())
}

impl AiOutput for LearnEntry {
    const TASK: Task = Task::LearnEntry;

    fn validate(&self) -> Result<(), String> {
        check_gloss(
            &self.word,
            &self.lemma,
            self.root.as_deref(),
            &self.senses,
            &self.examples,
            self.arabic_origin.as_deref(),
            self.false_friend.as_ref(),
        )?;
        if let Some(n) = &self.note {
            check_tri(n, "note")?;
        }
        if let Some(b) = &self.bridge {
            if !has_arabic(&b.arabic) {
                return Err("bridge.arabic must be in Arabic script".into());
            }
            if self.arabic_origin.is_none() {
                return Err("bridge needs arabic_origin".into());
            }
        }
        if let Some(root) = &self.root
            && !self.affixes.is_empty()
            && !self.affixes.iter().any(|a| a == root)
        {
            return Err(format!("affixes must include the root `{root}`"));
        }
        Ok(())
    }

    /// Every taught entry is reviewed before it is final.
    fn needs_review(&self) -> bool {
        true
    }
}

/// The draft the review screen shows.
pub fn draft(prepared: &Prepared, answer: &Answer<LearnEntry>) -> TaughtEntry {
    let a = &answer.value;
    let mut doubts = a.doubts.clone();
    if let (Some(hint), Some(root)) = (&prepared.hints.root, &a.root)
        && hint != root
    {
        doubts.push(Trilingual {
            arz: format!("محرّك كبر قرا الجذر «{hint}»، والمسودة بتقول «{root}». بُص عليها."),
            ar: format!("حلّل محرّك كبر الجذر «{hint}»، والمسودة تقول «{root}». تستحق المراجعة."),
            en: format!("Kabar's engine reads the root as “{hint}”; the draft says “{root}”. Worth a look."),
        });
    }
    if !a.confident && a.doubts.is_empty() {
        doubts.push(Trilingual {
            arz: "كبر مش متأكد من المعنى ده. راجعه كويس قبل ما توافق.".into(),
            ar: "كبر غير متأكد من هذا المعنى. راجعه جيدًا قبل الموافقة.".into(),
            en: "Kabar isn't sure about this one. Check it before you approve.".into(),
        });
    }
    TaughtEntry {
        schema: TAUGHT_SCHEMA_VERSION,
        entry: GlossEntry {
            lang: prepared.lang,
            word: prepared.word.clone(),
            rank: 0,
            lemma: a.lemma.trim().to_lowercase(),
            root: a.root.clone().filter(|r| !r.trim().is_empty()),
            affixes: a.affixes.clone(),
            senses: a.senses.clone(),
            examples: a.examples.clone(),
            note: a.note.clone(),
            false_friend: a.false_friend.clone(),
            arabic_origin: a.arabic_origin.clone(),
            status: ReviewStatus::Unverified,
            provenance: TaughtEntry::provenance(&format!("learn_entry@v{}", answer.prompt_version)),
        },
        bridge: a.bridge.clone(),
        levels: if prepared.lang == Lang::Jv { a.levels.clone() } else { None },
        context: prepared.context.clone(),
        sources: prepared.sources.clone(),
        doubts,
        engine: answer.engine.as_str().into(),
        edited: false,
    }
}

/// The learner changed the draft by hand. Checks the result still has a
/// valid gloss shape; on success the entry is marked edited.
pub fn edit(entry: &TaughtEntry, f: impl FnOnce(&mut TaughtEntry)) -> Result<TaughtEntry, String> {
    let mut next = entry.clone();
    f(&mut next);
    next.entry.word = entry.entry.word.clone();
    next.entry.lang = entry.entry.lang;
    let g = &next.entry;
    check_gloss(
        &g.word,
        &g.lemma,
        g.root.as_deref(),
        &g.senses,
        &g.examples,
        g.arabic_origin.as_deref(),
        g.false_friend.as_ref(),
    )?;
    next.edited = next != *entry || entry.edited;
    Ok(next)
}

/// The learner approved the entry: it becomes an answer in popovers.
pub fn approve(entry: &TaughtEntry) -> TaughtEntry {
    let mut next = entry.clone();
    next.entry.status = ReviewStatus::Approved;
    next
}

#[cfg(test)]
mod tests;
