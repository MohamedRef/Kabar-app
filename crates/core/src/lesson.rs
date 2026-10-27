//! The Lesson Player's logic (SPEC §14): parsing and validating lesson files,
//! deterministic answer grading, and the step machine that walks
//! slides → exercises → retries → dialogue → summary.
//!
//! Pure: the UI feeds answers and elapsed time in, and stores the
//! [`LessonPosition`] this emits on every step change (Lanjut).

use std::collections::HashSet;

use kabar_protocol::lesson::{
    AcceptedMeanings, Direction, Exercise, ExerciseKind, ExerciseResult, LESSON_SCHEMA, LessonCompleted,
    LessonContent, LessonPosition, LessonProgress, LessonStep, Localized, Outcome,
};

use crate::locale::UiLanguage;

// ---------------------------------------------------------------------------
// Parsing and validation
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lesson YAML: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// Parses a `content/lessons/<lang>/<id>.yaml` file. Run [`validate`] next.
pub fn parse_lesson(yaml: &str) -> Result<LessonContent, ParseError> {
    serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))
}

/// A content problem, with a path like `exercises[3] (e04)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Checks what serde can't: the shape rules every lesson must follow so the
/// player never meets an impossible exercise. Empty = valid.
pub fn validate(lesson: &LessonContent) -> Vec<Issue> {
    let mut v = Validator::default();
    v.check_lesson(lesson);
    v.issues
}

#[derive(Default)]
struct Validator {
    issues: Vec<Issue>,
}

impl Validator {
    fn issue(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(Issue { path: path.into(), message: message.into() });
    }

    fn localized(&mut self, path: &str, l: &Localized) {
        for (field, text) in [("ar_eg", &l.ar_eg), ("ar", &l.ar), ("en", &l.en)] {
            if text.trim().is_empty() {
                self.issue(format!("{path}.{field}"), "empty translation");
            }
        }
    }

    fn check_lesson(&mut self, l: &LessonContent) {
        if l.schema != LESSON_SCHEMA {
            self.issue("schema", format!("expected {LESSON_SCHEMA}, found {:?}", l.schema));
        }
        if l.id.trim().is_empty() {
            self.issue("id", "empty");
        }
        if !matches!(l.language.as_str(), "id" | "jv") {
            self.issue("language", format!("must be id or jv, found {:?}", l.language));
        }
        if l.title.trim().is_empty() {
            self.issue("title", "empty");
        }
        self.localized("can_do", &l.can_do);
        let max_slides = pacing::caps("learn").map_or(0, |c| *c.slides.end());
        if l.slides.len() > max_slides {
            self.issue(
                "slides",
                format!("{} slides; a lesson teaches with at most {max_slides}", l.slides.len()),
            );
        }
        for (i, s) in l.slides.iter().enumerate() {
            let p = format!("slides[{i}]");
            self.localized(&format!("{p}.body"), &s.body);
            for (j, e) in s.examples.iter().enumerate() {
                if e.text.trim().is_empty() {
                    self.issue(format!("{p}.examples[{j}].text"), "empty");
                }
                self.localized(&format!("{p}.examples[{j}]"), &e.meaning);
            }
        }
        if !(pacing::MIN_EXERCISES..=pacing::MAX_EXERCISES).contains(&l.exercises.len()) {
            self.issue(
                "exercises",
                format!(
                    "{} exercises; docs/PEDAGOGY.md asks for {}–{}",
                    l.exercises.len(),
                    pacing::MIN_EXERCISES,
                    pacing::MAX_EXERCISES
                ),
            );
        }
        // The 7-minute cap (pacing::estimate_seconds) is an authoring rule:
        // crates/content checks it for every lesson file, the player never refuses one.
        let mut ids = HashSet::new();
        for (i, ex) in l.exercises.iter().enumerate() {
            let p = format!("exercises[{i}] ({})", ex.id);
            if ex.id.trim().is_empty() {
                self.issue(&p, "empty id");
            } else if !ids.insert(ex.id.as_str()) {
                self.issue(&p, "duplicate id");
            }
            if let Some(e) = &ex.explain {
                self.localized(&format!("{p}.explain"), e);
            }
            self.check_exercise(&p, &ex.kind);
        }
        if let Some(d) = &l.dialogue {
            if d.lines.len() < 2 {
                self.issue("dialogue.lines", "a dialogue needs at least 2 lines");
            }
            for (i, line) in d.lines.iter().enumerate() {
                if line.text.trim().is_empty() || line.speaker.trim().is_empty() {
                    self.issue(format!("dialogue.lines[{i}]"), "empty speaker or text");
                }
                self.localized(&format!("dialogue.lines[{i}]"), &line.meaning);
            }
        }
        let mut words = HashSet::new();
        for (i, w) in l.new_words.iter().enumerate() {
            if !words.insert(w.word.as_str()) {
                self.issue(format!("new_words[{i}]"), format!("duplicate word {:?}", w.word));
            }
            self.localized(&format!("new_words[{i}]"), &w.meaning);
        }
        if let Some(s) = &l.souvenir {
            self.localized("souvenir.note", &s.note);
        }
        if let Some(g) = &l.game {
            let known =
                ["uu.", "mp."].iter().any(|p| g.strip_prefix(p).is_some_and(|rest| !rest.trim().is_empty()));
            if !known {
                self.issue(
                    "game",
                    format!("must be an Unggah-Ungguh (uu.*) or Minimal Pairs (mp.*) id, found {g:?}"),
                );
            }
        }
        if !matches!(l.review.status.as_str(), "draft" | "approved" | "needs-changes") {
            self.issue("review.status", format!("unknown status {:?}", l.review.status));
        }
        if l.review.verified && l.review.status != "approved" {
            self.issue("review.verified", "only approved content can be verified");
        }
    }

    fn check_exercise(&mut self, p: &str, kind: &ExerciseKind) {
        match kind {
            ExerciseKind::MultipleChoice { question, question_meaning, options, options_meaning, answer } => {
                if question.trim().is_empty() && question_meaning.is_none() {
                    self.issue(p, "needs a question or question_meaning");
                }
                if let Some(q) = question_meaning {
                    self.localized(&format!("{p}.question_meaning"), q);
                }
                let n = match (options.is_empty(), options_meaning.is_empty()) {
                    (false, true) => {
                        self.unique(p, options);
                        options.len()
                    }
                    (true, false) => {
                        for (i, o) in options_meaning.iter().enumerate() {
                            self.localized(&format!("{p}.options_meaning[{i}]"), o);
                        }
                        options_meaning.len()
                    }
                    _ => {
                        self.issue(p, "exactly one of options / options_meaning must be filled");
                        0
                    }
                };
                if !(2..=4).contains(&n) {
                    self.issue(p, format!("{n} options; use 2–4 (keys 1–4)"));
                }
                if *answer >= n {
                    self.issue(p, format!("answer {answer} is out of range"));
                }
            }
            ExerciseKind::MatchPairs { pairs } => {
                if !(2..=6).contains(&pairs.len()) {
                    self.issue(p, format!("{} pairs; use 2–6", pairs.len()));
                }
                let texts: Vec<String> = pairs.iter().map(|x| x.text.clone()).collect();
                self.unique(p, &texts);
                for (i, pair) in pairs.iter().enumerate() {
                    self.localized(&format!("{p}.pairs[{i}]"), &pair.meaning);
                }
            }
            ExerciseKind::Translate { direction, source_meaning, source_text, answers, answers_meaning } => {
                match direction {
                    Direction::ToTarget => {
                        match source_meaning {
                            Some(m) => self.localized(&format!("{p}.source_meaning"), m),
                            None => self.issue(p, "to_target needs source_meaning"),
                        }
                        if answers.iter().all(|a| a.trim().is_empty()) {
                            self.issue(p, "to_target needs answers");
                        }
                        if source_text.is_some() || answers_meaning.is_some() {
                            self.issue(p, "to_target uses source_meaning + answers only");
                        }
                    }
                    Direction::FromTarget => {
                        if source_text.as_deref().is_none_or(|s| s.trim().is_empty()) {
                            self.issue(p, "from_target needs source_text");
                        }
                        match answers_meaning {
                            Some(a) if !a.ar_eg.is_empty() && !a.ar.is_empty() && !a.en.is_empty() => {}
                            _ => self.issue(p, "from_target needs answers_meaning in ar_eg, ar and en"),
                        }
                        if source_meaning.is_some() || !answers.is_empty() {
                            self.issue(p, "from_target uses source_text + answers_meaning only");
                        }
                    }
                }
            }
            ExerciseKind::BuildSentence { source, answer, alternatives, distractors } => {
                self.localized(&format!("{p}.source"), source);
                let tiles: Vec<String> = answer
                    .split_whitespace()
                    .chain(distractors.iter().map(|s| s.as_str()))
                    .map(normalize)
                    .collect();
                if answer.split_whitespace().count() < 2 {
                    self.issue(p, "answer needs at least 2 tiles");
                }
                for d in distractors {
                    if answer.split_whitespace().any(|w| normalize(w) == normalize(d)) {
                        self.issue(p, format!("distractor {d:?} is also in the answer"));
                    }
                }
                for alt in alternatives {
                    let mut pool = tiles.clone();
                    for w in alt.split_whitespace().map(normalize) {
                        match pool.iter().position(|t| *t == w) {
                            Some(i) => {
                                pool.swap_remove(i);
                            }
                            None => {
                                self.issue(p, format!("alternative {alt:?} uses a word with no tile: {w:?}"))
                            }
                        }
                    }
                }
            }
            ExerciseKind::Listen { text, options, answer, meaning, question, .. } => {
                if text.trim().is_empty() {
                    self.issue(p, "empty text");
                }
                if let Some(m) = meaning {
                    self.localized(&format!("{p}.meaning"), m);
                }
                if let Some(q) = question {
                    self.localized(&format!("{p}.question"), q);
                    if options.is_empty() {
                        self.issue(p, "a comprehension question needs options");
                    }
                }
                match (options.is_empty(), answer) {
                    (true, None) => {}
                    (true, Some(_)) => self.issue(p, "answer without options"),
                    (false, None) => self.issue(p, "options need an answer index"),
                    (false, Some(i)) => {
                        self.unique(p, options);
                        if !(2..=4).contains(&options.len()) {
                            self.issue(p, format!("{} options; use 2–4 (keys 1–4)", options.len()));
                        }
                        match options.get(*i) {
                            // A comprehension answer is about the text, not the text itself.
                            Some(_) if question.is_some() => {}
                            Some(o) if normalize(o) == normalize(text) => {}
                            Some(_) => self.issue(p, "options[answer] must be the heard text"),
                            None => self.issue(p, format!("answer {i} is out of range")),
                        }
                    }
                }
            }
            ExerciseKind::FillBlank { sentence, meaning, options, answers } => {
                if sentence.matches("___").count() != 1 {
                    self.issue(p, "sentence needs exactly one ___");
                }
                self.localized(&format!("{p}.meaning"), meaning);
                if answers.is_empty() {
                    self.issue(p, "no answers");
                }
                if !options.is_empty() {
                    self.unique(p, options);
                    if !answers.iter().any(|a| options.iter().any(|o| normalize(o) == normalize(a))) {
                        self.issue(p, "no option is an accepted answer");
                    }
                }
            }
            ExerciseKind::Dictation { text, meaning, .. } | ExerciseKind::Speaking { text, meaning } => {
                if text.trim().is_empty() {
                    self.issue(p, "empty text");
                }
                self.localized(&format!("{p}.meaning"), meaning);
            }
        }
    }

    fn unique(&mut self, p: &str, items: &[String]) {
        let mut seen = HashSet::new();
        for s in items {
            if !seen.insert(normalize(s)) {
                self.issue(p, format!("duplicate option {s:?}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Localized helpers
// ---------------------------------------------------------------------------

/// The text in the learner's explanation language.
pub fn pick(l: &Localized, lang: UiLanguage) -> &str {
    match lang {
        UiLanguage::ArEg => &l.ar_eg,
        UiLanguage::Ar => &l.ar,
        UiLanguage::En => &l.en,
    }
}

fn pick_answers(a: &AcceptedMeanings, lang: UiLanguage) -> &[String] {
    match lang {
        UiLanguage::ArEg => &a.ar_eg,
        UiLanguage::Ar => &a.ar,
        UiLanguage::En => &a.en,
    }
}

// ---------------------------------------------------------------------------
// Grading
// ---------------------------------------------------------------------------

/// What the learner gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// An option index (multiple choice, listen, fill blank with options).
    Choice(usize),
    /// Typed or handwritten text; build-sentence tiles joined by spaces.
    Text(String),
    /// Match pairs finished; `missed` holds the target words of wrong taps.
    Matched { missed: Vec<String> },
    /// Speaking practice done (no checking yet).
    Practiced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Grade {
    Correct,
    /// Right apart from a small typo; counts as correct, with a gentle note.
    Almost {
        expected: String,
    },
    /// Not yet. The UI shows [`display_answer`] and the explanation.
    Wrong,
    /// An open translation that matched no reference. The UI shows the
    /// references and lets the learner say honestly whether theirs means the
    /// same (later: the AI `grade_answer` task).
    NeedsReview,
    /// Practice that isn't checked (speaking for now).
    Practiced,
}

impl Grade {
    pub fn outcome(&self) -> Outcome {
        match self {
            Grade::Correct => Outcome::Correct,
            Grade::Almost { .. } => Outcome::Almost,
            Grade::Wrong | Grade::NeedsReview => Outcome::Missed,
            Grade::Practiced => Outcome::Practiced,
        }
    }

    pub fn is_correct(&self) -> bool {
        matches!(self, Grade::Correct | Grade::Almost { .. } | Grade::Practiced)
    }
}

/// Folds an answer for comparison: case, punctuation and whitespace, Arabic
/// diacritics, tatweel and letter variants (أ إ آ ٱ → ا, ى → ي, ة → ه), and
/// the Javanese dictionary accents (kowé, dèrèng → kowe, dereng), since Kabar
/// writes plain e.
/// Hyphens inside words stay (anak-anak).
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true;
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = match c {
            '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{0640}'
            | '\u{200C}'..='\u{200F}'
            | '\u{2066}'..='\u{2069}' => {
                continue;
            }
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            'ؤ' => 'و',
            'ئ' => 'ي',
            'é' | 'è' | 'ê' => 'e',
            '’' | '\'' | '`' => continue,
            '-' | '‑' => '-',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        if c == ' ' {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(c);
            space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    // A hyphen standing alone (a dash used as punctuation) is not a word.
    out.split(' ').filter(|w| w.chars().any(char::is_alphanumeric)).collect::<Vec<_>>().join(" ")
}

/// Levenshtein distance over chars.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != *cb)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// One small typo: the same words, each at most one edit off (words of 4+
/// letters only, so "ya"/"yo" never pass), one typo per four words at most;
/// or one missing/extra space.
fn is_close(given: &str, expected: &str) -> bool {
    let (g, e): (Vec<&str>, Vec<&str>) = (given.split(' ').collect(), expected.split(' ').collect());
    if g.len() == e.len() {
        let mut typos = 0;
        for (a, b) in g.iter().zip(&e) {
            if a == b {
                continue;
            }
            if b.chars().count() < 4 || distance(a, b) > 1 {
                return false;
            }
            typos += 1;
        }
        return typos <= (e.len() / 4).max(1);
    }
    given.replace(' ', "") == expected.replace(' ', "")
}

/// Grades text against references: exact (normalized) → Correct, one small
/// typo → Almost (with the reference as written), else None.
/// How text answers are compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compare {
    /// Normalized text as is (Javanese, or answers in the learner's language).
    Plain,
    /// Indonesian: each spoken form counts as its formal twin ("udah" =
    /// "sudah", "nggak" = "tidak"), so either register, or a mix, is right.
    Registers,
}

fn canonical(s: &str, compare: Compare) -> String {
    let n = normalize(s);
    match compare {
        Compare::Plain => n,
        Compare::Registers => n
            .split(' ')
            .map(|w| crate::lang::normalize::formal_twin(w).unwrap_or(w))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn grade_text(given: &str, refs: &[String], allow_almost: bool, compare: Compare) -> Option<Grade> {
    let g = canonical(given, compare);
    if g.is_empty() {
        return None;
    }
    if refs.iter().any(|r| canonical(r, compare) == g) {
        return Some(Grade::Correct);
    }
    refs.iter()
        .find(|r| allow_almost && is_close(&g, &canonical(r, compare)))
        .map(|r| Grade::Almost { expected: r.clone() })
}

/// Grades an answer in an Indonesian lesson. See [`grade_in`].
pub fn grade(ex: &Exercise, answer: &Answer, lang: UiLanguage) -> Grade {
    grade_in(ex, answer, lang, "id")
}

/// Grades an answer. Deterministic, offline, no AI. `lesson_language` is the
/// lesson's (`id` or `jv`): Indonesian answers may use the spoken or the
/// formal form of a word, whichever the reference has.
pub fn grade_in(ex: &Exercise, answer: &Answer, lang: UiLanguage, lesson_language: &str) -> Grade {
    use ExerciseKind as K;
    let target = if lesson_language == "jv" { Compare::Plain } else { Compare::Registers };
    let text = |a: &Answer| match a {
        Answer::Text(t) => Some(t.clone()),
        _ => None,
    };
    match (&ex.kind, answer) {
        (K::MultipleChoice { answer: right, .. }, Answer::Choice(i)) => {
            if i == right {
                Grade::Correct
            } else {
                Grade::Wrong
            }
        }
        (K::MatchPairs { .. }, Answer::Matched { .. }) => Grade::Correct,
        (K::Speaking { .. }, _) | (_, Answer::Practiced) => Grade::Practiced,
        (K::Listen { options, answer: Some(right), .. }, Answer::Choice(i)) if !options.is_empty() => {
            if i == right { Grade::Correct } else { Grade::Wrong }
        }
        (K::FillBlank { options, answers, .. }, Answer::Choice(i)) => match options.get(*i) {
            Some(o) if answers.iter().any(|a| canonical(a, target) == canonical(o, target)) => Grade::Correct,
            _ => Grade::Wrong,
        },
        (K::BuildSentence { answer, alternatives, .. }, a) => {
            let refs: Vec<String> =
                std::iter::once(answer.clone()).chain(alternatives.iter().cloned()).collect();
            // Tiles can't have typos: right or not yet.
            text(a).and_then(|t| grade_text(&t, &refs, false, target)).unwrap_or(Grade::Wrong)
        }
        (K::Listen { text: heard, .. } | K::Dictation { text: heard, .. }, a) => text(a)
            .and_then(|t| grade_text(&t, std::slice::from_ref(heard), true, target))
            .unwrap_or(Grade::Wrong),
        (K::FillBlank { answers, .. }, a) => {
            text(a).and_then(|t| grade_text(&t, answers, true, target)).unwrap_or(Grade::Wrong)
        }
        (K::Translate { direction, .. }, a) => {
            let refs = references(ex, lang);
            let compare = if *direction == Direction::ToTarget { target } else { Compare::Plain };
            match text(a) {
                Some(t) if normalize(&t).is_empty() => Grade::Wrong,
                Some(t) => grade_text(&t, &refs, true, compare)
                    .or_else(|| {
                        // Also accept a reference from another explanation language
                        // (e.g. an English answer while the UI is in Arabic).
                        let all = all_meaning_answers(ex);
                        grade_text(&t, &all, true, compare)
                    })
                    .unwrap_or(Grade::NeedsReview),
                None => Grade::Wrong,
            }
        }
        _ => Grade::Wrong,
    }
}

fn all_meaning_answers(ex: &Exercise) -> Vec<String> {
    match &ex.kind {
        ExerciseKind::Translate { answers_meaning: Some(a), .. } => {
            a.ar_eg.iter().chain(&a.ar).chain(&a.en).cloned().collect()
        }
        _ => Vec::new(),
    }
}

/// Accepted answers as the learner should see them, best first.
pub fn references(ex: &Exercise, lang: UiLanguage) -> Vec<String> {
    use ExerciseKind as K;
    match &ex.kind {
        K::MultipleChoice { options, options_meaning, answer, .. } => match options_meaning.get(*answer) {
            Some(m) => vec![pick(m, lang).to_owned()],
            None => options.get(*answer).cloned().into_iter().collect(),
        },
        K::MatchPairs { pairs } => {
            pairs.iter().map(|p| format!("{} = {}", p.text, pick(&p.meaning, lang))).collect()
        }
        K::Translate { direction: Direction::ToTarget, answers, .. } => answers.clone(),
        K::Translate { answers_meaning, .. } => {
            answers_meaning.as_ref().map(|a| pick_answers(a, lang).to_vec()).unwrap_or_default()
        }
        K::BuildSentence { answer, alternatives, .. } => {
            std::iter::once(answer.clone()).chain(alternatives.iter().cloned()).collect()
        }
        K::Listen { question: Some(_), options, answer: Some(i), .. } => {
            options.get(*i).cloned().into_iter().collect()
        }
        K::Listen { text, .. } | K::Dictation { text, .. } | K::Speaking { text, .. } => vec![text.clone()],
        K::FillBlank { sentence, answers, .. } => {
            answers.iter().map(|a| sentence.replacen("___", a, 1)).collect()
        }
    }
}

/// The one right answer to show under "Not yet".
pub fn display_answer(ex: &Exercise, lang: UiLanguage) -> String {
    references(ex, lang).into_iter().next().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Deterministic shuffling
// ---------------------------------------------------------------------------

/// FNV-1a: a stable seed from a string (same tile order on every device).
pub fn seed_of(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A permutation of `0..n`, never the identity when n > 1, fixed by `seed`.
pub fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    let mut state = seed;
    for i in (1..n).rev() {
        let j = (splitmix(&mut state) % (i as u64 + 1)) as usize;
        idx.swap(i, j);
    }
    if n > 1 && idx.iter().enumerate().all(|(i, &v)| i == v) {
        idx.rotate_left(1);
    }
    idx
}

// ---------------------------------------------------------------------------
// The player
// ---------------------------------------------------------------------------

/// One screen of the lesson.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Slide(usize),
    /// `index` into `lesson.exercises`.
    Exercise {
        index: usize,
        retry: bool,
    },
    Dialogue,
    Summary,
}

/// XP: 10 for finishing, +1 per exercise right the first time, +5 for a
/// lesson with nothing missed.
pub const XP_FINISH: u32 = 10;
pub const XP_PERFECT: u32 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Graded exercises right on the first try (typos count as right).
    pub correct: u32,
    /// Graded exercises (speaking and skipped ones aren't).
    pub graded: u32,
    /// 0–100.
    pub score: u32,
    pub xp: u32,
    pub duration_ms: u64,
    /// Target-language words to practise again, deduplicated.
    pub mistakes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Player {
    lesson: LessonContent,
    seed: u64,
    step: Step,
    progress: LessonProgress,
    /// The grade of the current exercise once checked, until Continue.
    pending: Option<Grade>,
    /// On a retry of a choice item: the exercise with its options in a new
    /// order (see [`retry_options`]), shown and graded in place of the
    /// original so a remembered position isn't the answer.
    retry_view: Option<Exercise>,
}

impl Player {
    /// Starts at the first slide (or exercise). The shuffle seed comes from
    /// the lesson id so every device shows the same tile order.
    pub fn new(lesson: LessonContent) -> Self {
        let seed = seed_of(&lesson.id);
        Self::with_seed(lesson, seed)
    }

    pub fn with_seed(lesson: LessonContent, seed: u64) -> Self {
        let mut p = Self {
            lesson,
            seed,
            step: Step::Summary,
            progress: LessonProgress::default(),
            pending: None,
            retry_view: None,
        };
        p.go(p.first_step());
        p
    }

    /// Moves to `step`, preparing the reshuffled options of a retry.
    fn go(&mut self, step: Step) {
        self.step = step;
        self.retry_view = match step {
            Step::Exercise { index, retry: true } => {
                self.lesson.exercises.get(index).and_then(|ex| retry_options(ex, self.seed))
            }
            _ => None,
        };
    }

    /// Picks up a saved position. Unknown ids (content changed since) fall
    /// back to the start, keeping the results that still match.
    pub fn resume(lesson: LessonContent, position: &LessonPosition) -> Self {
        let mut p = Self::new(lesson);
        if position.lesson_id != p.lesson.id {
            return p;
        }
        if let Some(progress) = &position.progress {
            p.progress = progress.clone();
            p.progress.results.retain(|r| p.lesson.exercises.iter().any(|e| e.id == r.id));
        }
        let step = match &position.step {
            LessonStep::Slide { index } if *index < p.lesson.slides.len() => Some(Step::Slide(*index)),
            LessonStep::Exercise { id, retry } => p
                .lesson
                .exercises
                .iter()
                .position(|e| &e.id == id)
                .map(|index| Step::Exercise { index, retry: *retry }),
            LessonStep::Dialogue if p.lesson.dialogue.is_some() => Some(Step::Dialogue),
            LessonStep::Summary => Some(Step::Summary),
            _ => None,
        };
        if let Some(step) = step {
            p.go(step);
        }
        p
    }

    pub fn lesson(&self) -> &LessonContent {
        &self.lesson
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn pending(&self) -> Option<&Grade> {
        self.pending.as_ref()
    }

    /// The current exercise as shown: on a retry of a choice item, with its
    /// options reshuffled (answers and grading refer to this order).
    pub fn exercise(&self) -> Option<&Exercise> {
        match self.step {
            Step::Exercise { index, .. } => {
                self.retry_view.as_ref().or_else(|| self.lesson.exercises.get(index))
            }
            _ => None,
        }
    }

    pub fn position(&self) -> LessonPosition {
        let step = match self.step {
            Step::Slide(index) => LessonStep::Slide { index },
            Step::Exercise { index, retry } => {
                LessonStep::Exercise { id: self.lesson.exercises[index].id.clone(), retry }
            }
            Step::Dialogue => LessonStep::Dialogue,
            Step::Summary => LessonStep::Summary,
        };
        LessonPosition { lesson_id: self.lesson.id.clone(), step, progress: Some(self.progress.clone()) }
    }

    /// Adds active time (the UI caps idle gaps before calling this).
    pub fn add_elapsed(&mut self, ms: u64) {
        self.progress.elapsed_ms = self.progress.elapsed_ms.saturating_add(ms);
    }

    /// Build-sentence tiles for `ex`, shuffled the same way on every device
    /// for this lesson (see [`tiles`]).
    pub fn tiles(&self, ex: &Exercise) -> Vec<String> {
        tiles(ex, self.seed)
    }

    /// Order of the meanings column in match pairs.
    pub fn pair_order(&self, ex: &Exercise) -> Vec<usize> {
        pair_order(ex, self.seed)
    }

    /// Grades the current exercise and records the attempt. Checking again
    /// before Continue returns the same grade.
    pub fn check(&mut self, answer: &Answer, lang: UiLanguage) -> Grade {
        let Step::Exercise { index, retry } = self.step else { return Grade::Wrong };
        if let Some(g) = &self.pending {
            return g.clone();
        }
        let ex = self.retry_view.as_ref().unwrap_or(&self.lesson.exercises[index]);
        let g = grade_in(ex, answer, lang, &self.lesson.language);
        let mut mistakes = Vec::new();
        if let Answer::Matched { missed } = answer {
            mistakes.extend(missed.iter().cloned());
        }
        if !g.is_correct() {
            mistakes.extend(mistake_words(ex));
        }
        self.record(index, retry, g.outcome(), mistakes);
        self.pending = Some(g.clone());
        g
    }

    /// After [`Grade::NeedsReview`]: the learner says whether their answer
    /// means the same as the references.
    pub fn self_assess(&mut self, means_the_same: bool) {
        let Step::Exercise { index, retry } = self.step else { return };
        if self.pending != Some(Grade::NeedsReview) {
            return;
        }
        let ex = &self.lesson.exercises[index];
        let (outcome, mistakes) = if means_the_same {
            (Outcome::Correct, Vec::new())
        } else {
            (Outcome::Missed, mistake_words(ex))
        };
        self.record(index, retry, outcome, mistakes);
        self.pending = Some(if means_the_same { Grade::Correct } else { Grade::Wrong });
    }

    /// "Can't listen now": no score, no retry.
    pub fn skip(&mut self) -> LessonPosition {
        if let (Step::Exercise { index, retry }, None) = (self.step, &self.pending) {
            self.record(index, retry, Outcome::Skipped, Vec::new());
        }
        self.advance()
    }

    fn record(&mut self, index: usize, retry: bool, outcome: Outcome, mistakes: Vec<String>) {
        let id = &self.lesson.exercises[index].id;
        let results = &mut self.progress.results;
        let pos = results.iter().position(|r| &r.id == id);
        match (pos, retry) {
            (Some(i), true) => {
                results[i].retry = Some(outcome);
                // A right retry clears nothing: the words still go to review.
                for m in mistakes {
                    if !results[i].mistakes.contains(&m) {
                        results[i].mistakes.push(m);
                    }
                }
            }
            (Some(i), false) => {
                results[i] = ExerciseResult { id: id.clone(), first: outcome, retry: None, mistakes };
            }
            (None, _) => {
                results.push(ExerciseResult { id: id.clone(), first: outcome, retry: None, mistakes })
            }
        }
    }

    fn result(&self, index: usize) -> Option<&ExerciseResult> {
        let id = &self.lesson.exercises[index].id;
        self.progress.results.iter().find(|r| &r.id == id)
    }

    fn missed_first(&self, index: usize) -> bool {
        self.result(index).is_some_and(|r| r.first == Outcome::Missed)
    }

    fn first_step(&self) -> Step {
        if !self.lesson.slides.is_empty() {
            Step::Slide(0)
        } else if !self.lesson.exercises.is_empty() {
            Step::Exercise { index: 0, retry: false }
        } else {
            self.after_exercises()
        }
    }

    fn after_exercises(&self) -> Step {
        if self.lesson.dialogue.is_some() { Step::Dialogue } else { Step::Summary }
    }

    fn next_retry(&self, after: Option<usize>) -> Option<Step> {
        let start = after.map_or(0, |i| i + 1);
        (start..self.lesson.exercises.len())
            .find(|&i| self.missed_first(i))
            .map(|index| Step::Exercise { index, retry: true })
    }

    fn next_step(&self) -> Step {
        let n_ex = self.lesson.exercises.len();
        match self.step {
            Step::Slide(i) if i + 1 < self.lesson.slides.len() => Step::Slide(i + 1),
            Step::Slide(_) if n_ex > 0 => Step::Exercise { index: 0, retry: false },
            Step::Slide(_) => self.after_exercises(),
            Step::Exercise { index, retry: false } if index + 1 < n_ex => {
                Step::Exercise { index: index + 1, retry: false }
            }
            Step::Exercise { retry: false, .. } => {
                self.next_retry(None).unwrap_or_else(|| self.after_exercises())
            }
            Step::Exercise { index, retry: true } => {
                self.next_retry(Some(index)).unwrap_or_else(|| self.after_exercises())
            }
            Step::Dialogue | Step::Summary => Step::Summary,
        }
    }

    /// Continue. Returns the position to save for Lanjut.
    pub fn advance(&mut self) -> LessonPosition {
        self.go(self.next_step());
        self.pending = None;
        self.position()
    }

    /// Back one slide (slides only; exercises never go back).
    pub fn back(&mut self) -> Option<LessonPosition> {
        match self.step {
            Step::Slide(i) if i > 0 => {
                self.go(Step::Slide(i - 1));
                Some(self.position())
            }
            _ => None,
        }
    }

    /// 0.0–1.0 for the progress bar. Retries extend the road a little.
    pub fn fraction(&self) -> f32 {
        let slides = self.lesson.slides.len();
        let ex = self.lesson.exercises.len();
        let retries = (0..ex).filter(|&i| self.missed_first(i)).count();
        let dialogue = usize::from(self.lesson.dialogue.is_some());
        let total = slides + ex + retries + dialogue;
        let done = match self.step {
            Step::Slide(i) => i,
            Step::Exercise { index, retry: false } => slides + index,
            Step::Exercise { index, retry: true } => {
                slides + ex + (0..index).filter(|&i| self.missed_first(i)).count()
            }
            Step::Dialogue => slides + ex + retries,
            Step::Summary => return 1.0,
        };
        if total == 0 { 1.0 } else { done as f32 / total as f32 }
    }

    pub fn summary(&self) -> Summary {
        let mut correct = 0u32;
        let mut graded = 0u32;
        let mut mistakes: Vec<String> = Vec::new();
        for r in &self.progress.results {
            match r.first {
                Outcome::Correct | Outcome::Almost => {
                    correct += 1;
                    graded += 1;
                }
                Outcome::Missed => graded += 1,
                Outcome::Practiced | Outcome::Skipped => {}
            }
            for m in &r.mistakes {
                if !mistakes.contains(m) {
                    mistakes.push(m.clone());
                }
            }
        }
        let score = (correct * 100).checked_div(graded).unwrap_or(100);
        let perfect = graded > 0 && correct == graded;
        let xp = XP_FINISH + correct + if perfect { XP_PERFECT } else { 0 };
        Summary { correct, graded, score, xp, duration_ms: self.progress.elapsed_ms, mistakes }
    }

    /// The `completed` event payload.
    pub fn completed(&self) -> LessonCompleted {
        let s = self.summary();
        LessonCompleted {
            score: s.score,
            xp: s.xp,
            duration_ms: s.duration_ms,
            mistakes: s.mistakes,
            results: self.progress.results.clone(),
        }
    }
}

/// First-try score (0–100) at which finishing a lesson approves it.
pub const APPROVE_SCORE: u32 = 80;

/// Whether a finished lesson counts as approved by the learner, so its
/// "not reviewed yet" flag comes off (Javanese included). The learner must
/// score at least [`APPROVE_SCORE`] on the first try, with at least half of
/// the gradable exercises actually answered rather than skipped.
pub fn approves(done: &LessonCompleted) -> bool {
    let count = |o: Outcome| done.results.iter().filter(|r| r.first == o).count();
    let graded = count(Outcome::Correct) + count(Outcome::Almost) + count(Outcome::Missed);
    graded > 0 && graded >= count(Outcome::Skipped) && done.score >= APPROVE_SCORE
}

/// Build-sentence tiles (answer words + distractors), shuffled by `seed` and
/// the exercise id. Edge punctuation is dropped so a tile like "jumpa!"
/// doesn't give away the sentence end (grading ignores it).
pub fn tiles(ex: &Exercise, seed: u64) -> Vec<String> {
    match &ex.kind {
        ExerciseKind::BuildSentence { answer, distractors, .. } => {
            let all: Vec<&str> = answer
                .split_whitespace()
                .chain(distractors.iter().map(String::as_str))
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-'))
                .filter(|w| !w.is_empty())
                .collect();
            shuffled(all.len(), seed ^ seed_of(&ex.id)).into_iter().map(|i| all[i].to_owned()).collect()
        }
        _ => Vec::new(),
    }
}

/// A missed choice item coming back as a retry, with its options in another
/// order (never the same one), fixed by `seed` and the exercise id so a
/// resumed lesson shows the same order. The answer index follows its option,
/// so grading the copy is exactly grading the original. `None` for items
/// without options to pick from.
pub fn retry_options(ex: &Exercise, seed: u64) -> Option<Exercise> {
    use ExerciseKind as K;
    let order = |n: usize| shuffled(n, seed ^ seed_of(&ex.id) ^ 2);
    fn apply<T: Clone>(items: &[T], order: &[usize]) -> Vec<T> {
        order.iter().map(|&i| items[i].clone()).collect()
    }
    let moved = |order: &[usize], answer: usize| order.iter().position(|&i| i == answer).unwrap_or(answer);
    let mut out = ex.clone();
    match &mut out.kind {
        K::MultipleChoice { options, options_meaning, answer, .. } => {
            let n = options.len().max(options_meaning.len());
            if n < 2 {
                return None;
            }
            let o = order(n);
            if options.len() == n {
                *options = apply(options, &o);
            }
            if options_meaning.len() == n {
                *options_meaning = apply(options_meaning, &o);
            }
            *answer = moved(&o, *answer);
        }
        K::Listen { options, answer, .. } if options.len() > 1 => {
            let o = order(options.len());
            *options = apply(options, &o);
            *answer = answer.map(|a| moved(&o, a));
        }
        // Graded by the picked text, so only the order changes.
        K::FillBlank { options, .. } if options.len() > 1 => {
            let o = order(options.len());
            *options = apply(options, &o);
        }
        _ => return None,
    }
    Some(out)
}

/// Order of the meanings column in match pairs, fixed by `seed`.
pub fn pair_order(ex: &Exercise, seed: u64) -> Vec<usize> {
    match &ex.kind {
        ExerciseKind::MatchPairs { pairs } => shuffled(pairs.len(), seed ^ seed_of(&ex.id) ^ 1),
        _ => Vec::new(),
    }
}

/// The words a miss should send to review: the exercise's `words`, else the
/// target-language text it was about.
pub fn mistake_words(ex: &Exercise) -> Vec<String> {
    if !ex.words.is_empty() {
        return ex.words.clone();
    }
    use ExerciseKind as K;
    match &ex.kind {
        K::Listen { text, .. } | K::Dictation { text, .. } | K::Speaking { text, .. } => vec![text.clone()],
        K::BuildSentence { answer, .. } => vec![answer.clone()],
        K::Translate { source_text: Some(t), .. } => vec![t.clone()],
        K::Translate { answers, .. } => answers.first().cloned().into_iter().collect(),
        K::FillBlank { answers, .. } => answers.first().cloned().into_iter().collect(),
        K::MultipleChoice { question, options, answer, .. } => {
            if !question.trim().is_empty() {
                vec![question.clone()]
            } else {
                options.get(*answer).cloned().into_iter().collect()
            }
        }
        K::MatchPairs { .. } => Vec::new(),
    }
}

pub mod level_test;
pub mod pacing;

#[cfg(test)]
mod tests;
