//! The Lesson Player (SPEC §14): slides → exercises → retries → mini dialogue
//! → summary → souvenir. The step logic lives in `kabar_core::lesson::Player`;
//! this file draws it, plays audio and saves progress for Lanjut.
//!
//! The exercise views below are standalone components (exercise + `Draft`
//! signal in, events out) so level tests can reuse them without the player.

mod notes;
mod test_player;
pub use test_player::PlacementInvite;
pub(crate) use test_player::item_prompt;
pub(crate) mod text;

use dioxus::prelude::*;
use kabar_core::lesson::{
    Answer, Grade, Player, Step, display_answer, parse_lesson, pick, references, validate,
};
use kabar_core::locale::{UiLanguage, format_number, format_percent, localize_digits};
use kabar_protocol::coach::{ActivityKind, Skill};
use kabar_protocol::lesson::{
    Dialogue, Direction, Exercise, ExerciseKind, LessonContent, LessonPosition, LessonStep, Localized,
    SavedResponse, Verification,
};
use kabar_protocol::srs::StudyLanguage;

use self::text::{L, lt};
use crate::annotated::Annotated;
use crate::coach::Coach;
use crate::platform::{self, TtsStatus};
use crate::pron::{Pron, Say};
use crate::{Db, Ui};

const LESSON_CSS: Css = css!("assets/lesson.css");

use crate::embedded::{self, LESSONS, TESTS};
use crate::embedded::{Css, css};

/// The YAML of a lesson, if this build ships it.
pub fn lesson_content(id: &str) -> Option<String> {
    embedded::find(LESSONS, id)
}

/// The YAML of a level or placement test (`bipa1.test`, `placement`), if shipped.
pub fn test_content(id: &str) -> Option<String> {
    embedded::find(TESTS, id)
}

/// Idle gaps longer than this don't count as lesson time.
pub(crate) const MAX_STEP_MS: u64 = 120_000;

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[component]
pub fn LessonScreen() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let id = ui.lesson.read().clone().unwrap_or_default();
    // Unpacked and parsed once per lesson, not on every render.
    let opened = use_memo(use_reactive!(|id| open(&id)));

    let body = match &*opened.read() {
        Opened::Test(yaml) => {
            return rsx! {
                style { {LESSON_CSS.text()} }
                test_player::TestScreen { id: id.clone(), yaml: yaml.clone() }
            };
        }
        Opened::Pending => rsx! {
            Notice { title: lt(lang, L::PendingTitle), body: lt(lang, L::PendingBody).to_string(), id: id.clone() }
        },
        Opened::Broken(problems) => rsx! {
            Notice { title: lt(lang, L::BrokenTitle), body: problems.clone(), id: id.clone() }
        },
        Opened::Lesson(lesson) => rsx! { PlayerView { key: "{id}", lesson: (**lesson).clone() } },
    };
    rsx! {
        style { {LESSON_CSS.text()} }
        {body}
    }
}

/// What the player shows for a lesson id.
#[derive(Clone, PartialEq)]
enum Opened {
    Test(String),
    Lesson(Box<LessonContent>),
    /// Not written yet.
    Pending,
    /// A parse error or validation problems.
    Broken(String),
}

fn open(id: &str) -> Opened {
    if let Some(yaml) = test_content(id) {
        return Opened::Test(yaml);
    }
    match lesson_content(id).map(|yaml| parse_lesson(&yaml)) {
        None => Opened::Pending,
        Some(Err(e)) => Opened::Broken(e.to_string()),
        Some(Ok(lesson)) => {
            let issues = validate(&lesson);
            if issues.is_empty() {
                Opened::Lesson(Box::new(lesson))
            } else {
                Opened::Broken(issues.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n"))
            }
        }
    }
}

/// "Still being written" (or a broken file): friendly, never a crash.
#[component]
fn Notice(title: &'static str, body: String, id: String) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    rsx! {
        div { class: "lp",
            div { class: "lp-top",
                button { class: "lp-close", "data-back": "true", aria_label: lt(lang, L::Close), onclick: move |_| ui.close_lesson(), "×" }
            }
            div { class: "lp-body",
                div { class: "lp-col",
                    div { class: "card lp-notice",
                        h2 { "{title}" }
                        p { class: "pre", "{body}" }
                        p { class: "muted mono", bdi { "{id}" } }
                        button { class: "primary", onclick: move |_| ui.close_lesson(), "{lt(lang, back_label(ui))}" }
                    }
                }
            }
        }
    }
}

/// The answer being built for the current exercise.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub choice: Option<usize>,
    pub text: String,
    /// Build sentence: tile indices, in answer order.
    pub picked: Vec<usize>,
    /// Match pairs: pair indices already matched.
    pub matched: Vec<usize>,
    pub left: Option<usize>,
    pub right: Option<usize>,
    /// Match pairs: target words of wrong taps.
    pub missed: Vec<String>,
    /// Match pairs: the pair (left, right) just tapped wrong, for a short shake.
    pub wrong: Option<(usize, usize)>,
}

/// The answer the Check button would send; None while there's nothing to check.
pub fn draft_answer(ex: &Exercise, d: &Draft, tiles: &[String]) -> Option<Answer> {
    use ExerciseKind as K;
    let typed = || (!d.text.trim().is_empty()).then(|| Answer::Text(d.text.clone()));
    match &ex.kind {
        K::MultipleChoice { .. } => d.choice.map(Answer::Choice),
        K::MatchPairs { .. } => None,
        K::Listen { options, .. } | K::FillBlank { options, .. } if !options.is_empty() => {
            d.choice.map(Answer::Choice)
        }
        K::BuildSentence { .. } => (!d.picked.is_empty())
            .then(|| Answer::Text(d.picked.iter().map(|&i| tiles[i].as_str()).collect::<Vec<_>>().join(" "))),
        K::Speaking { .. } => Some(Answer::Practiced),
        K::Translate { .. } | K::Listen { .. } | K::FillBlank { .. } | K::Dictation { .. } => typed(),
    }
}

/// The stable key of option `i`: its target text, or the English meaning
/// of a meaning choice (meaning options carry no target text).
fn option_key(ex: &Exercise, i: usize) -> Option<String> {
    match &ex.kind {
        ExerciseKind::MultipleChoice { options, options_meaning, .. } if !options_meaning.is_empty() => {
            options_meaning.get(i).map(|m| m.en.clone()).or_else(|| options.get(i).cloned())
        }
        ExerciseKind::MultipleChoice { options, .. }
        | ExerciseKind::Listen { options, .. }
        | ExerciseKind::FillBlank { options, .. } => options.get(i).cloned(),
        _ => None,
    }
}

/// The draft as a saved position keeps it: options, tiles and pairs by their
/// text, so a reshuffle or a content edit can't point it at another one.
pub(crate) fn save_draft(ex: &Exercise, d: &Draft, tiles: &[String]) -> SavedResponse {
    let pair_word = |i: &usize| match &ex.kind {
        ExerciseKind::MatchPairs { pairs } => pairs.get(*i).map(|p| p.text.clone()),
        _ => None,
    };
    SavedResponse {
        choice: d.choice.and_then(|i| option_key(ex, i)),
        text: d.text.clone(),
        tiles: d.picked.iter().filter_map(|&i| tiles.get(i).cloned()).collect(),
        matched: d.matched.iter().filter_map(pair_word).collect(),
        missed: d.missed.clone(),
        checked: None,
    }
}

/// A saved answer back on screen. Pieces the content no longer has are
/// dropped rather than guessed.
pub(crate) fn restore_draft(ex: &Exercise, r: &SavedResponse, tiles: &[String]) -> Draft {
    let count = option_count(ex);
    let choice =
        r.choice.as_ref().and_then(|key| (0..count).find(|&i| option_key(ex, i).as_ref() == Some(key)));
    // Tiles may repeat a word: each saved one takes the first free tile.
    let mut picked: Vec<usize> = Vec::new();
    for t in &r.tiles {
        if let Some(i) = (0..tiles.len()).find(|&i| &tiles[i] == t && !picked.contains(&i)) {
            picked.push(i);
        }
    }
    let matched = match &ex.kind {
        ExerciseKind::MatchPairs { pairs } => {
            r.matched.iter().filter_map(|w| pairs.iter().position(|p| &p.text == w)).collect()
        }
        _ => Vec::new(),
    };
    Draft { choice, text: r.text.clone(), picked, matched, missed: r.missed.clone(), ..Draft::default() }
}

/// What the Check button waits for, said under it while it's disabled.
fn check_hint(ex: &Exercise) -> L {
    use ExerciseKind as K;
    match &ex.kind {
        K::MatchPairs { .. } => L::HintMatch,
        K::BuildSentence { .. } => L::HintTiles,
        K::MultipleChoice { .. } => L::HintChoose,
        K::Listen { options, .. } | K::FillBlank { options, .. } if !options.is_empty() => L::HintChoose,
        _ => L::HintType,
    }
}

/// The label of the button that closes the player: back to where it was
/// opened from.
pub(crate) fn back_label(ui: Ui) -> L {
    match *ui.lesson_from.read() {
        crate::Screen::Journey => L::BackToJourney,
        crate::Screen::Today => L::BackToToday,
        _ => L::Back,
    }
}

/// How many options the number keys can pick from (0: none).
pub(crate) fn option_count(ex: &Exercise) -> usize {
    match &ex.kind {
        ExerciseKind::MultipleChoice { options, options_meaning, .. } => {
            options.len().max(options_meaning.len())
        }
        ExerciseKind::Listen { options, .. } | ExerciseKind::FillBlank { options, .. } => options.len(),
        _ => 0,
    }
}

/// Exercises that can't be done without hearing the target language: with
/// no voice on the device they're skipped like "can't listen now".
pub(crate) fn needs_audio(ex: &Exercise) -> bool {
    matches!(ex.kind, ExerciseKind::Listen { .. } | ExerciseKind::Dictation { .. })
}

/// BCP 47 tag for target-language text (the IME and the voice follow it).
pub(crate) fn target_tag(language: &str) -> &'static str {
    if language == "jv" { "jv" } else { "id" }
}

pub(crate) fn instruction(ex: &Exercise, language: &str) -> L {
    use ExerciseKind as K;
    match &ex.kind {
        K::MultipleChoice { options_meaning, .. } if !options_meaning.is_empty() => L::PickMeaning,
        K::MultipleChoice { .. } => L::PickAnswer,
        K::MatchPairs { .. } => L::MatchPairs,
        K::Translate { direction: Direction::ToTarget, .. } if language == "jv" => L::WriteInJv,
        K::Translate { direction: Direction::ToTarget, .. } => L::WriteInId,
        K::Translate { .. } => L::WriteMeaning,
        K::BuildSentence { .. } => L::BuildSentence,
        K::Listen { options, .. } if options.is_empty() => L::ListenType,
        K::Listen { question: Some(_), .. } => L::ListenAnswer,
        K::Listen { .. } => L::ListenPick,
        K::FillBlank { .. } => L::FillBlank,
        K::Dictation { .. } => L::Dictation,
        K::Speaking { .. } => L::Speaking,
    }
}

/// The text an exercise plays on its own when it opens.
pub(crate) fn autoplay_text(ex: &Exercise) -> Option<(String, Option<String>)> {
    match &ex.kind {
        ExerciseKind::Listen { text, audio, .. } | ExerciseKind::Dictation { text, audio, .. } => {
            Some((text.clone(), audio.clone()))
        }
        ExerciseKind::Speaking { text, .. } => Some((text.clone(), None)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

/// Plays target-language audio. A pre-generated `audio` file wins when the
/// content has one; next, the Azure clip from a downloaded offline pack
/// (SPEC §20, `crate::packs`); next, Azure's neural voice through Kabar Cloud
/// (cached on the device, `crate::audio`). Otherwise the device's own voice
/// (id-ID; Javanese asks for jv-ID and falls back to an id-ID voice, which
/// reads Javanese passably): the WebView's `speechSynthesis` on desktop, and
/// the system TextToSpeech engine on Android, whose WebView has no
/// `speechSynthesis`.
pub fn speak(text: &str, language: &str, slow: bool, audio: Option<&str>, missing: Signal<bool>) {
    speak_as(text, language, None, slow, audio, missing);
}

/// A dialogue line in its speaker's pack voice (see [`speak`]).
fn speak_line(dialogue: &Dialogue, index: usize, language: &str, missing: Signal<bool>) {
    if let Some(line) = dialogue.lines.get(index) {
        let voice = kabar_core::packs::speaker_voice(language, dialogue, &line.speaker);
        speak_as(&line.text, language, Some(voice), false, None, missing);
    }
}

fn speak_as(
    text: &str,
    language: &str,
    voice: Option<kabar_protocol::cloud::Voice>,
    slow: bool,
    audio: Option<&str>,
    missing: Signal<bool>,
) {
    if audio.is_some() {
        return speak_with(text, language, slow, audio, missing);
    }
    if let Some(clip) = crate::packs::clip_url(text, language, voice) {
        return speak_with(text, language, slow, Some(&clip), missing);
    }
    if let Some(url) = crate::audio::neural(text, language) {
        let (text, language) = (text.to_owned(), language.to_owned());
        spawn(async move {
            let url = url.await;
            speak_with(&text, &language, slow, url.as_deref(), missing);
        });
        return;
    }
    speak_with(text, language, slow, None, missing);
}

/// A line in one neural voice (a persona's), else the device's voice.
pub(crate) fn speak_in(
    text: &str,
    language: &str,
    voice: kabar_protocol::cloud::Voice,
    missing: Signal<bool>,
) {
    if let Some(url) = crate::audio::neural_in(text, voice) {
        let (text, language) = (text.to_owned(), language.to_owned());
        spawn(async move {
            let url = url.await;
            speak_with(&text, &language, false, url.as_deref(), missing);
        });
        return;
    }
    speak_with(text, language, false, None, missing);
}

pub(crate) fn speak_with(
    text: &str,
    language: &str,
    slow: bool,
    audio: Option<&str>,
    mut missing: Signal<bool>,
) {
    let lang = if language == "jv" { "jv-ID" } else { "id-ID" };
    let mut native = move |text: &str| {
        let ok = platform::speak(text, lang, slow).is_ok();
        if ok == *missing.peek() {
            missing.set(!ok);
        }
    };
    if platform::NATIVE_TTS && audio.is_none() {
        native(text);
        return;
    }
    let js = format!(
        r#"
        const text = {text}, lang = {lang}, rate = {rate}, clipRate = {clip_rate}, file = {file}, web = {web};
        const tts = () => {{
            if (!web) return false;
            const s = window.speechSynthesis;
            if (!s || typeof SpeechSynthesisUtterance === "undefined") return false;
            s.cancel();
            const u = new SpeechSynthesisUtterance(text);
            const voices = s.getVoices();
            const norm = v => v.lang.replace("_", "-").toLowerCase();
            const v = voices.find(v => norm(v) === lang.toLowerCase())
                || voices.find(v => norm(v).startsWith(lang.slice(0, 2).toLowerCase()))
                || voices.find(v => norm(v).startsWith("id"));
            u.lang = v ? v.lang : "id-ID";
            if (v) u.voice = v;
            u.rate = rate;
            s.speak(u);
            return true;
        }};
        if (file) {{
            try {{
                const a = new Audio(file);
                a.playbackRate = clipRate;
                await a.play();
                dioxus.send(true);
            }} catch (e) {{ dioxus.send(tts()); }}
        }} else {{
            dioxus.send(tts());
        }}
        "#,
        text = serde_json::to_string(text).unwrap_or_default(),
        lang = serde_json::to_string(lang).unwrap_or_default(),
        rate = if slow { "0.7" } else { "0.95" },
        // Recorded clips are at natural speed; slow plays them at 3/4.
        clip_rate = if slow { "0.75" } else { "1.0" },
        file = serde_json::to_string(&audio).unwrap_or_else(|_| "null".into()),
        web = !platform::NATIVE_TTS,
    );
    let text = text.to_owned();
    spawn(async move {
        let mut eval = document::eval(&js);
        if let Ok(false) = eval.recv::<bool>().await {
            if platform::NATIVE_TTS {
                // The audio file didn't play: let the system voice read it.
                native(&text);
            } else {
                missing.set(true);
            }
        }
    });
}

/// Tells Kak Nur about the finished lesson: the unit step, the words taught
/// and how the exercises went (the coach's daily plan and reports read these).
fn log_activity(coach: Coach, db: &Db, p: &Player) {
    let lesson = p.lesson();
    let summary = p.summary();
    let seconds = (summary.duration_ms / 1000) as u32;
    let unit_id = lesson.id.rsplit_once('.').map_or(lesson.id.as_str(), |(unit, _)| unit).to_owned();
    coach.record(db, ActivityKind::LessonStep { unit_id, done: true }, seconds);
    let words: Vec<String> = lesson.new_words.iter().map(|w| w.word.clone()).collect();
    if !words.is_empty() {
        coach.record(db, ActivityKind::NewWords { words }, 0);
    }
    if summary.graded > 0 {
        let practice = ActivityKind::Practice {
            skill: Skill::Vocabulary,
            topic: lesson.title.clone(),
            correct: summary.correct,
            total: summary.graded,
        };
        coach.record(db, practice, 0);
    }
}

/// Tells Kak Nur about a finished Sinetron episode: listening practice and
/// its new words.
fn log_episode(coach: Coach, db: &Db, p: &Player) {
    let lesson = p.lesson();
    let summary = p.summary();
    let words: Vec<String> = lesson.new_words.iter().map(|w| w.word.clone()).collect();
    if !words.is_empty() {
        coach.record(db, ActivityKind::NewWords { words }, 0);
    }
    let practice = ActivityKind::Practice {
        skill: Skill::Listening,
        topic: lesson.title.clone(),
        correct: summary.correct,
        total: summary.graded,
    };
    coach.record(db, practice, (summary.duration_ms / 1000) as u32);
}

/// The chime (and phone tap) for a right answer (silent when sounds are off).
fn chime(ui: Ui) {
    crate::sounds::play(ui, crate::sounds::Sound::Correct);
}

// ---------------------------------------------------------------------------
// Bidi helpers
// ---------------------------------------------------------------------------

fn is_latin_letter(c: char) -> bool {
    c.is_alphabetic() && (c as u32) < 0x0250
}

/// Splits mixed text into (is_latin, run) so Latin snippets inside Arabic
/// explanations get their own `<bdi>` (SPEC §9). A run keeps together what
/// reads as one left-to-right phrase: several Latin sentences in a row
/// ("Ni Luh membawa tas. Tasnya dari kain."), affix hyphens ("-nya", "ber-"),
/// chains like "ora ↔ tidak · wis ↔ sudah", and parentheses it opened.
/// A Latin run in a slide body worth a pronunciation: a word or phrase, not
/// a lone letter, a number, an abbreviation or a "X …" pattern.
fn is_target_run(run: &str) -> bool {
    let letters: Vec<char> = run.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2
        && !letters.iter().all(|c| c.is_uppercase())
        && !run.chars().any(|c| c.is_ascii_digit() || c == '…')
}

pub fn latin_runs(s: &str) -> Vec<(bool, String)> {
    let mut out: Vec<(bool, String)> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let latin = |i: usize| chars.get(i).is_some_and(|&c| is_latin_letter(c));
    // The next non-space character after `i` is a capital Latin letter.
    let capital_after = |i: usize| {
        chars[i..]
            .iter()
            .find(|c| !c.is_whitespace())
            .is_some_and(|&c| is_latin_letter(c) && c.is_uppercase())
    };
    // "-ku" after a space or an Arabic letter ("aku و-ku") is a suffix: its
    // hyphen stays inside the Latin run, never mirrored to "ku-".
    let affix_at = |i: usize| {
        chars[i] == '-'
            && latin(i + 1)
            && (i == 0 || !(is_latin_letter(chars[i - 1]) || chars[i - 1].is_ascii_digit()))
    };
    let mut i = 0;
    while i < chars.len() {
        let affix = affix_at(i);
        if latin(i) || affix {
            let start = i;
            let mut end = i;
            if affix {
                i += 1;
            }
            while i < chars.len() {
                let c = chars[i];
                // Letters and digits extend the run; ? ! and a suffix hyphen
                // join it only straight after a letter ("Apa kabar?", "ber-").
                // A trailing … belongs to the pattern it ends ("Menurut X, …").
                if is_latin_letter(c) || c.is_ascii_digit() || c == '…' || ("?!-".contains(c) && end == i) {
                    end = i + 1;
                } else if c == '.' && chars.get(i + 1).is_none_or(|n| n.is_whitespace()) {
                    // A full stop ends the Arabic sentence, unless another
                    // Latin sentence follows it.
                    if !capital_after(i + 1) {
                        // A run made of sentences keeps its last full stop.
                        if run_has_sentence_break(&chars[start..end]) {
                            end = i + 1;
                        }
                        break;
                    }
                } else if !" -'’.,…/↔→·=+()".contains(c) {
                    break;
                }
                i += 1;
            }
            // Close a parenthesis the run opened.
            let open = chars[start..end].iter().filter(|&&c| c == '(').count();
            let close = chars[start..end].iter().filter(|&&c| c == ')').count();
            if open > close && chars.get(end) == Some(&')') {
                end += 1;
            }
            out.push((true, chars[start..end].iter().collect()));
            i = end;
        } else {
            let start = i;
            while i < chars.len() && !latin(i) && !affix_at(i) {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            match out.last_mut() {
                Some((false, prev)) => prev.push_str(&run),
                _ => out.push((false, run)),
            }
        }
    }
    out
}

fn is_arabic_letter(c: char) -> bool {
    matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{08A0}'..='\u{08FF}' | '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// Splits UI-language text of an English line into (is_arabic, run): an
/// Arabic run spans Arabic words and the spaces between them, never the
/// guillemets, brackets or commas around it, which stay with the English.
pub fn arabic_runs(s: &str) -> Vec<(bool, String)> {
    let chars: Vec<char> = s.chars().collect();
    let arabic = |i: usize| chars.get(i).is_some_and(|&c| is_arabic_letter(c));
    let mut out: Vec<(bool, String)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        if arabic(i) {
            let mut end = i;
            while i < chars.len() {
                if arabic(i) || chars[i] == '\u{200C}' {
                    end = i + 1;
                } else if chars[i] != ' ' {
                    break;
                }
                i += 1;
            }
            out.push((true, chars[start..end].iter().collect()));
            i = end;
        } else {
            while i < chars.len() && !arabic(i) {
                i += 1;
            }
            out.push((false, chars[start..i].iter().collect()));
        }
    }
    out
}

/// A run that already holds a sentence break (". " before a capital) is
/// itself made of sentences, so its final full stop belongs to it too.
fn run_has_sentence_break(run: &[char]) -> bool {
    run.windows(3).any(|w| w[0] == '.' && w[1] == ' ' && w[2].is_uppercase())
}

/// Explanation text with Latin runs isolated (explicitly left-to-right: some
/// WebViews don't resolve `dir=auto` on an inline `<bdi>`, which turned
/// «Apa kabar?» into «?Apa kabar»). Digits outside the Latin runs follow the
/// Eastern-digits setting, whichever digits the content was written with.
/// `**words**` in the content are shown in bold.
#[component]
pub(crate) fn Rich(text: String, target: &'static str, #[props(default)] pron: bool) -> Element {
    rsx! {
        for (j, (bold, part)) in bold_parts(&text).into_iter().enumerate() {
            if bold {
                strong { key: "b{j}", RichRuns { text: part, target, pron } }
            } else {
                RichRuns { key: "p{j}", text: part, target, pron }
            }
        }
    }
}

/// Splits `**bold**` markup out of content text: (is bold, text) parts in
/// order. An unpaired `**` stays as written.
pub(crate) fn bold_parts(text: &str) -> Vec<(bool, String)> {
    let pieces: Vec<&str> = text.split("**").collect();
    // An even piece count means one `**` has no partner: the last one.
    let paired = if pieces.len() % 2 == 1 { pieces.len() } else { pieces.len() - 1 };
    let mut parts: Vec<(bool, String)> = pieces[..paired]
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.is_empty())
        .map(|(i, p)| (i % 2 == 1, (*p).to_string()))
        .collect();
    if paired < pieces.len() {
        parts.push((false, format!("**{}", pieces[paired])));
    }
    parts
}

#[component]
fn RichRuns(text: String, target: &'static str, pron: bool) -> Element {
    let ui = use_context::<Ui>();
    let (lang, eastern) = {
        let s = ui.settings.read();
        (s.ui_language, s.eastern_digits)
    };
    let dir = if lang.dir() == kabar_core::locale::Direction::Rtl { "rtl" } else { "ltr" };
    // Slides teach words inside Arabic sentences: say each one right there.
    let say = pron && lang != UiLanguage::En;
    let runs = move |key: String, runs: Vec<(bool, String)>| {
        rsx! {
            for (j , unit) in pron_units(runs, target, say).into_iter().enumerate() {
                match unit {
                    Unit::Plain(t) if dir == "ltr" => rsx! {
                        // Arabic in an English line («إنت») gets its own
                        // right-to-left isolate, the mirror of Latin above, so
                        // it can't pull the quotes and commas around it in.
                        span { key: "{key}.{j}",
                            for (k , (arabic, part)) in arabic_runs(&t).into_iter().enumerate() {
                                if arabic {
                                    bdi { key: "{k}", lang: "ar", dir: "rtl", "{part}" }
                                } else {
                                    span { key: "{k}", "{localize_digits(&part, lang, eastern)}" }
                                }
                            }
                        }
                    },
                    Unit::Plain(t) => rsx! {
                        span { key: "{key}.{j}", "{localize_digits(&in_direction(&t, dir == \"rtl\"), lang, eastern)}" }
                    },
                    Unit::Latin { phrases, close, short } => rsx! {
                        // The phrase, its closing quote and its pronunciation
                        // stay together; a short one stays on one line.
                        span { key: "{key}.{j}", class: if short { "pron-unit nowrap" } else { "pron-unit" },
                            for (k , bit) in layout(&phrases, &close, dir == "rtl").into_iter().enumerate() {
                                match bit {
                                    Bit::Latin(t) => rsx! {
                                        bdi { key: "{k}", lang: target, dir: "ltr", class: "latin", "{t}" }
                                    },
                                    Bit::Close(t) | Bit::Sep(t) => rsx! {
                                        span { key: "{k}", "{t}" }
                                    },
                                    Bit::Say(t) => rsx! {
                                        span { key: "{k}",
                                            Say { text: t.clone(), tag: target }
                                            Pron { text: t, tag: target, inline: true }
                                        }
                                    },
                                }
                            }
                        }
                    },
                }
            }
        }
    };
    rsx! {
        for (i, piece) in bracket_groups(latin_runs(&text)).into_iter().enumerate() {
            match piece {
                Piece::Run(latin, text) => runs(format!("{i}"), vec![(latin, text)]),
                Piece::Group(g) => {
                    let short = g.iter().map(|(_, r)| r.chars().count()).sum::<usize>() <= 28;
                    rsx! {
                        span { key: "{i}", dir, class: if short { "bidi-group nowrap" } else { "bidi-group" },
                            {runs(format!("{i}"), g)}
                        }
                    }
                }
            }
        }
    }
}

/// One phrase of a Latin run ("Selamat pagi" in "Selamat pagi → Sugeng
/// enjing"), whether to say it, and the separator after it.
#[derive(Debug, PartialEq)]
struct Phrase {
    text: String,
    say: bool,
    sep: String,
}

/// What a slide line is drawn from: UI-language text, or a Latin run split
/// into phrases with the closing quote or bracket that ends it.
#[derive(Debug, PartialEq)]
enum Unit {
    Plain(String),
    Latin { phrases: Vec<Phrase>, close: String, short: bool },
}

const CLOSERS: &[char] = &['»', '"', '”', ')'];
const SEPARATORS: &[&str] = &[" → ", " ↔ ", " = ", " / ", " · "];

/// Splits runs into units: each Latin run takes the closing punctuation
/// straight after it (so «selamat jalan» isn't split by its pronunciation),
/// and mapping lines ("A → B") get one pronunciation per side. On Javanese
/// slides the Indonesian side isn't said: the learner knows it from BIPA.
fn pron_units(runs: Vec<(bool, String)>, target: &str, say: bool) -> Vec<Unit> {
    let mut out = Vec::new();
    let mut runs = runs.into_iter().peekable();
    while let Some((latin, text)) = runs.next() {
        if !latin {
            out.push(Unit::Plain(text));
            continue;
        }
        let mut close = String::new();
        if let Some((false, next)) = runs.peek_mut() {
            let n: usize = next.chars().take_while(|c| CLOSERS.contains(c)).map(char::len_utf8).sum();
            close = next[..n].to_string();
            next.replace_range(..n, "");
        }
        let short = text.split_whitespace().count() <= 3;
        let phrases = split_phrases(&text)
            .into_iter()
            .map(|(t, sep)| Phrase {
                say: say && is_target_run(&t) && !(target == "jv" && is_indonesian(&t)),
                text: t,
                sep,
            })
            .collect();
        out.push(Unit::Latin { phrases, close, short });
        if let Some((false, next)) = runs.peek()
            && next.is_empty()
        {
            runs.next();
        }
    }
    out
}

/// UI-language text as drawn in a line of direction `rtl`: an arrow written
/// between an Arabic word and a Latin one ("besar = كبير → lebih besar")
/// points along the reading, so it turns around in a right-to-left line.
fn in_direction(text: &str, rtl: bool) -> std::borrow::Cow<'_, str> {
    if rtl && text.contains('→') { text.replace('→', "←").into() } else { text.into() }
}

/// One piece of a drawn Latin unit, in reading order.
#[derive(Debug, PartialEq)]
enum Bit {
    Latin(String),
    Close(String),
    /// The Arabic-script pronunciation of the phrase before it.
    Say(String),
    Sep(String),
}

/// The order a Latin unit is drawn in: each phrase in its own LTR isolate,
/// then the closing quote, then its pronunciation outside the isolate so in
/// an Arabic line it reads after the word (to its left), never before it.
/// Arrows turn around in a right-to-left line so "A ← B" still points from
/// the Indonesian to the Javanese side.
fn layout(phrases: &[Phrase], close: &str, rtl: bool) -> Vec<Bit> {
    let last = phrases.len().saturating_sub(1);
    let mut out = Vec::new();
    for (k, ph) in phrases.iter().enumerate() {
        out.push(Bit::Latin(ph.text.clone()));
        if k == last && !close.is_empty() {
            out.push(Bit::Close(close.to_string()));
        }
        if ph.say {
            out.push(Bit::Say(ph.text.clone()));
        }
        if !ph.sep.is_empty() {
            out.push(Bit::Sep(if rtl { ph.sep.replace('→', "←") } else { ph.sep.clone() }));
        }
    }
    out
}

/// "Selamat pagi → Sugeng enjing" → [("Selamat pagi", " → "), ("Sugeng enjing", "")].
fn split_phrases(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some((at, sep)) = SEPARATORS.iter().filter_map(|s| rest.find(s).map(|i| (i, *s))).min() {
        out.push((rest[..at].to_string(), sep.to_string()));
        rest = &rest[at + sep.len()..];
    }
    out.push((rest.to_string(), String::new()));
    out
}

/// Every word of the phrase reads as Indonesian (the engine's guess).
fn is_indonesian(phrase: &str) -> bool {
    use kabar_core::lang::Lang;
    let tokens = crate::reader::doc::engine().analyze(phrase);
    let mut words = tokens.iter().filter(|t| t.token.is_wordlike()).peekable();
    words.peek().is_some() && words.all(|t| t.lang.lang == Lang::Indonesian)
}

/// A run of [`latin_runs`] output, or a bracketed group that mixes the UI
/// language with a Latin run ("(هو ngendika)"). The group is isolated in
/// the UI's direction so both brackets pair up around it, and a short one
/// is kept on one line.
#[derive(Debug, PartialEq)]
enum Piece {
    Run(bool, String),
    Group(Vec<(bool, String)>),
}

fn bracket_groups(runs: Vec<(bool, String)>) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut group: Option<Vec<(bool, String)>> = None;
    for (latin, text) in runs {
        if latin {
            match &mut group {
                Some(g) => g.push((true, text)),
                None => out.push(Piece::Run(true, text)),
            }
            continue;
        }
        let mut rest = text.as_str();
        loop {
            match &mut group {
                Some(g) => match rest.find(')') {
                    // The group closes in this run.
                    Some(at) => {
                        let (inside, after) = rest.split_at(at + 1);
                        g.push((false, inside.to_string()));
                        out.push(Piece::Group(group.take().unwrap_or_default()));
                        rest = after;
                    }
                    None => {
                        g.push((false, rest.to_string()));
                        break;
                    }
                },
                None => {
                    // An opening bracket whose partner isn't in this same run
                    // starts a group; brackets closed within the run stay put.
                    let open = rest
                        .char_indices()
                        .filter(|&(_, c)| c == '(')
                        .find(|&(at, _)| !rest[at..].contains(')'));
                    match open {
                        Some((at, _)) => {
                            if at > 0 {
                                out.push(Piece::Run(false, rest[..at].to_string()));
                            }
                            group = Some(Vec::new());
                            rest = &rest[at..];
                        }
                        None => {
                            if !rest.is_empty() {
                                out.push(Piece::Run(false, rest.to_string()));
                            }
                            break;
                        }
                    }
                }
            }
            if rest.is_empty() {
                break;
            }
        }
    }
    // A bracket that never closes: leave its runs as they were.
    if let Some(g) = group {
        out.extend(g.into_iter().map(|(l, t)| Piece::Run(l, t)));
    }
    out
}

/// A slide title: target-language text isolated left to right, or, when it
/// mixes in Arabic ("-nya = الـ / بتاعه"), laid out like [`Rich`] text.
#[component]
fn Title(text: String, target: &'static str) -> Element {
    if text.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)) {
        rsx! { Rich { text, target } }
    } else {
        rsx! { bdi { lang: target, dir: "ltr", "{text}" } }
    }
}

// ---------------------------------------------------------------------------
// The player
// ---------------------------------------------------------------------------

/// A Sinetron episode's questions and new words, in the Lesson Player.
/// Finishing them marks the episode watched; `on_close` leaves.
#[component]
pub fn EpisodeQuiz(lesson: LessonContent, on_close: EventHandler) -> Element {
    rsx! {
        style { {LESSON_CSS.text()} }
        PlayerView { lesson, on_close }
    }
}

/// Runs a lesson. With `on_close` it runs an episode quiz instead: no
/// Lanjut position, and closing calls it rather than going to the Journey.
#[component]
fn PlayerView(lesson: LessonContent, on_close: Option<EventHandler>) -> Element {
    let ui = use_context::<Ui>();
    let episode = on_close.is_some();
    let close = move || match on_close {
        Some(h) => h.call(()),
        None => ui.close_lesson(),
    };
    let db_ctx = use_context::<Db>();
    let db = use_hook(|| CopyValue::new(db_ctx));
    let coach = use_context::<Coach>();
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;

    // Where this lesson was left, with the answer that was on screen.
    let saved = use_hook(|| {
        let saved = if episode { None } else { db.read().0.borrow().position().ok().flatten() };
        saved.filter(|p| p.lesson_id == lesson.id && p.step != LessonStep::Summary)
    });
    let mut player = use_signal(|| match &saved {
        Some(p) => Player::resume(lesson.clone(), p),
        None => Player::new(lesson.clone()),
    });
    // The answer comes back before Check or Continue is decided from it.
    let draft = use_signal(|| {
        let p = player.peek();
        match (&saved, p.exercise()) {
            (Some(s), Some(ex)) if s.step == p.position().step => {
                s.response.as_ref().map(|r| restore_draft(ex, r, &p.tiles(ex))).unwrap_or_default()
            }
            _ => Draft::default(),
        }
    });
    // Reopened on feedback whose answer wasn't kept (saved by an older
    // build): say so, rather than show feedback for a blank task.
    let mut answered_before = use_signal(|| {
        let p = player.peek();
        p.pending().is_some()
            && *draft.peek() == Draft::default()
            && !matches!(p.exercise().map(|e| &e.kind), Some(ExerciseKind::Speaking { .. }))
    });
    // The draft as last saved, and a counter that cancels a pending save
    // of typed text.
    let mut saved_draft = use_hook(|| CopyValue::new(draft.peek().clone()));
    let mut typing = use_hook(|| CopyValue::new(0u32));
    let last = use_signal(now_ms);
    let revealed = use_signal(|| 1usize);
    let audio_missing = use_signal(|| false);
    let mut finished = use_signal(|| false);
    // Listening items skipped because the device has no voice for them.
    let mut voice_skips = use_signal(|| 0u32);
    // The one step that carries the "no voice" note (see `voice_note_step`).
    let mut voice_note_at = use_signal(|| None::<Step>);
    // The lesson notes pane on wide windows; the learner can tuck it away.
    let mut notes_open = use_signal(|| true);

    // Save where we are on open (Lanjut), with the restored answer so a
    // second reopen still has it, and play the first exercise's audio.
    use_hook(move || {
        let p = player.peek();
        if !episode {
            save_with(db, &p, &draft.peek());
            // Just in time: this unit's and the next unit's audio (SPEC §20).
            crate::packs::prefetch_for(&p.lesson().id);
        }
        if let Some((text, audio)) = p.exercise().and_then(autoplay_text) {
            speak(&text, &p.lesson().language, false, audio.as_deref(), audio_missing);
        }
    });

    // Mutates the player, then: time, Lanjut position, fresh draft, audio,
    // and the `completed` event once the summary is reached.
    let mut commit = move |op: &dyn Fn(&mut Player) -> Option<LessonPosition>| {
        let now = now_ms();
        let mut last = last;
        let delta = now.saturating_sub(*last.peek()).min(MAX_STEP_MS);
        last.set(now);
        let pos = {
            let mut p = player.write();
            p.add_elapsed(delta);
            op(&mut p)
        };
        let Some(pos) = pos else { return };
        let mut draft = draft;
        draft.set(Draft::default());
        saved_draft.set(Draft::default());
        typing += 1;
        answered_before.set(false);
        let mut revealed = revealed;
        revealed.set(1);
        let p = player.peek();
        if p.step() == Step::Summary {
            if !*finished.peek() {
                finished.set(true);
                let db = db.read();
                {
                    let mut store = db.0.borrow_mut();
                    if let Err(e) = store.record_lesson_event(&p.lesson().id, "completed", &p.completed()) {
                        crate::diag::report!("saving the lesson result failed: {e}");
                    }
                    // The new words join the review deck, checked only if the
                    // learner already checked this lesson's content.
                    let approved = store.verified_lessons().is_ok_and(|v| v.contains_key(&p.lesson().id));
                    if !episode && let Err(e) = store.add_lesson_cards(p.lesson(), approved) {
                        crate::diag::report!("adding the lesson's words to review failed: {e}");
                    }
                    if !episode && let Err(e) = store.clear_position() {
                        crate::diag::report!("clearing the lesson position failed: {e}");
                    }
                }
                // First tries on words already in the deck move their reviews.
                let language =
                    if p.lesson().language == "jv" { StudyLanguage::Jv } else { StudyLanguage::Id };
                let signals = kabar_core::review::lesson_signals(p.lesson(), &p.completed().results);
                crate::review::note_evidence(&db, &coach, language, &signals);
                if episode { log_episode(coach, &db, &p) } else { log_activity(coach, &db, &p) }
            }
        } else if !episode {
            save(db, &pos);
        }
        if let Some((text, audio)) = p.exercise().and_then(autoplay_text) {
            speak(&text, &p.lesson().language, false, audio.as_deref(), audio_missing);
        }
        if p.step() == Step::Dialogue
            && let Some(d) = p.lesson().dialogue.as_ref()
        {
            speak_line(d, 0, &p.lesson().language, audio_missing);
        }
    };

    let mut check = move |answer: Answer| {
        let g = player.write().check(&answer, lang);
        if matches!(g, Grade::Correct | Grade::Almost { .. }) {
            chime(ui);
        } else if g == Grade::Wrong {
            crate::sounds::missed(ui);
        }
        if !episode {
            save_with(db, &player.peek(), &draft.peek());
        }
    };

    // Every change to the answer is saved, so closing the lesson (or the
    // app) and coming back shows it again. Typing is saved once it pauses.
    use_effect(move || {
        let d = draft.read().clone();
        if episode || d == *saved_draft.peek() {
            return;
        }
        let typed_only = Draft { text: d.text.clone(), ..saved_draft.peek().clone() } == d;
        saved_draft.set(d);
        typing += 1;
        if !typed_only {
            save_with(db, &player.peek(), &draft.peek());
            return;
        }
        let ticket = *typing.peek();
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            if typing.try_peek().is_ok_and(|t| *t == ticket)
                && let (Ok(p), Ok(d)) = (player.try_peek(), draft.try_peek())
            {
                save_with(db, &p, &d);
            }
        });
    });

    // The one primary action: Enter and the big button both run it.
    let mut primary = move || {
        let step = player.peek().step();
        match step {
            Step::Slide(_) => commit(&|p| Some(p.advance())),
            Step::Exercise { .. } => {
                let pending = player.peek().pending().cloned();
                match pending {
                    Some(Grade::NeedsReview) => {}
                    Some(_) => commit(&|p| Some(p.advance())),
                    None => {
                        let answer = {
                            let p = player.peek();
                            let ex = p.exercise().cloned();
                            ex.and_then(|ex| draft_answer(&ex, &draft.peek(), &p.tiles(&ex)))
                        };
                        if let Some(a) = answer {
                            check(a);
                        }
                    }
                }
            }
            Step::Dialogue => {
                let n = player.peek().lesson().dialogue.as_ref().map_or(0, |d| d.lines.len());
                let shown = *revealed.peek();
                if shown < n {
                    let mut r = revealed;
                    r.set(shown + 1);
                    let p = player.peek();
                    if let Some(d) = p.lesson().dialogue.as_ref() {
                        speak_line(d, shown, &p.lesson().language, audio_missing);
                    }
                    // Keep the newest line in view (after it renders).
                    document::eval(
                        r#"setTimeout(() => document.querySelector(".lp-lines li:last-child")?.scrollIntoView({
                            block: "end",
                            behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
                        }), 60);"#,
                    );
                } else {
                    commit(&|p| Some(p.advance()));
                }
            }
            Step::Summary => close(),
        }
    };

    // No sound for the target language (the `missing` signal): listening
    // items are skipped the "can't listen now" way, with a note, instead of
    // asking for audio that can't play.
    use_effect(move || {
        if !*audio_missing.read() {
            return;
        }
        let (skip, from) = {
            let p = player.read();
            let skip = matches!(p.step(), Step::Exercise { .. })
                && p.pending().is_none()
                && p.exercise().is_some_and(needs_audio);
            (skip, p.step())
        };
        if skip {
            voice_skips += 1;
            commit(&|p| Some(p.skip()));
            let to = player.peek().step();
            let next = voice_note_step(*voice_note_at.peek(), from, to);
            voice_note_at.set(next);
        }
    });

    // Keys are caught on the document, not on the player element: a tapped
    // tile or option can be disabled under the focus, and the next Enter must
    // still check. Enter = check/continue (Shift+Enter is a new line in text
    // answers), 1–9 = pick an option or a fill-in chip. Buttons in the feedback sheet and the
    // close button keep their native Enter.
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            if (window.__kabarLessonKeys) document.removeEventListener("keydown", window.__kabarLessonKeys, true);
            window.__kabarLessonKeys = (e) => {
                if (!document.querySelector(".lp") || e.isComposing) return;
                const t = e.target;
                if (t.closest && t.closest(".lp-feedback, .lp-top")) return;
                const typing = t.tagName === "TEXTAREA" || t.tagName === "INPUT";
                if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    dioxus.send("enter");
                } else if (!typing && !e.ctrlKey && !e.metaKey && !e.altKey && /^[1-9]$/.test(e.key)) {
                    dioxus.send(e.key);
                }
            };
            document.addEventListener("keydown", window.__kabarLessonKeys, true);
            await new Promise(() => {});
            "#,
        );
        while let Ok(key) = eval.recv::<String>().await {
            match key.as_str() {
                "enter" => primary(),
                n => {
                    let Ok(n) = n.parse::<usize>() else { continue };
                    if player.peek().pending().is_some() {
                        continue;
                    }
                    let count = player.peek().exercise().map_or(0, option_count);
                    if n <= count {
                        let mut d = draft;
                        d.write().choice = Some(n - 1);
                    }
                }
            }
        }
    });
    use_drop(|| {
        document::eval(
            r#"if (window.__kabarLessonKeys) { document.removeEventListener("keydown", window.__kabarLessonKeys, true); window.__kabarLessonKeys = null; }"#,
        );
    });

    // Leaving mid-answer keeps what was typed even if it hasn't paused yet.
    let mut leave = move || {
        let unsaved =
            matches!(player.peek().step(), Step::Exercise { .. }) && *draft.peek() != *saved_draft.peek();
        if !episode && unsaved {
            typing += 1;
            save_with(db, &player.peek(), &draft.peek());
        }
        close();
    };

    let p = player.read();
    let l = p.lesson();
    let target = target_tag(&l.language);
    let step = p.step();
    let pct = (p.fraction() * 100.0).round();
    let (count_n, count_total) = p.counter();
    let pending = p.pending().cloned();
    let exercise = p.exercise().cloned();
    let can_check =
        exercise.as_ref().is_some_and(|ex| draft_answer(ex, &draft.read(), &p.tiles(ex)).is_some());

    let (label, enabled) = match (step, &pending) {
        (Step::Exercise { .. }, Some(Grade::NeedsReview)) => (lt(lang, L::Continue), false),
        (Step::Exercise { .. }, Some(_)) => (lt(lang, L::Continue), true),
        (Step::Exercise { .. }, None) => match exercise.as_ref().map(|e| &e.kind) {
            Some(ExerciseKind::Speaking { .. }) => (lt(lang, L::ISaidIt), true),
            Some(ExerciseKind::MatchPairs { .. }) => (lt(lang, L::Check), false),
            _ => (lt(lang, L::Check), can_check),
        },
        (Step::Dialogue, _) => {
            let n = l.dialogue.as_ref().map_or(0, |d| d.lines.len());
            if *revealed.read() < n { (lt(lang, L::NextLine), true) } else { (lt(lang, L::Continue), true) }
        }
        (Step::Summary, _) => (lt(lang, if episode { L::BackToEpisodes } else { back_label(ui) }), true),
        (Step::Slide(_), _) => (lt(lang, L::Continue), true),
    };
    let feedback_class = match &pending {
        Some(Grade::Correct | Grade::Practiced) => "lp-actions good",
        Some(Grade::Almost { .. }) => "lp-actions almost",
        Some(Grade::Wrong) => "lp-actions miss",
        Some(Grade::NeedsReview) => "lp-actions review",
        None => "lp-actions",
    };
    let can_skip = pending.is_none()
        && matches!(
            exercise.as_ref().map(|e| &e.kind),
            Some(
                ExerciseKind::Listen { .. } | ExerciseKind::Dictation { .. } | ExerciseKind::Speaking { .. }
            )
        );
    let tiles = exercise.as_ref().map(|ex| p.tiles(ex)).unwrap_or_default();
    let pair_order = exercise.as_ref().map(|ex| p.pair_order(ex)).unwrap_or_default();
    let language = l.language.clone();
    let summary = p.summary();
    let retry = matches!(step, Step::Exercise { retry: true, .. });
    let lesson = l.clone();
    let wide = *ui.width.read() >= kabar_core::layout::WidthClass::Expanded;
    let marks = if wide { exercise_marks(&p) } else { Vec::new() };
    let answering = matches!(step, Step::Exercise { .. }) && pending.is_none();
    let hint = match (&exercise, answering && !enabled) {
        (Some(ex), true) => Some(lt(lang, check_hint(ex))),
        _ => None,
    };
    let passed = step == Step::Summary && kabar_core::lesson::passes(&p.completed());
    let eastern = settings.eastern_digits;
    let counter = localize_digits(&format!("{count_n} / {count_total}"), lang, eastern);
    let counter_label = lt(lang, L::StepOf)
        .replace("{n}", &format_number(count_n as u64, lang, eastern))
        .replace("{total}", &format_number(count_total as u64, lang, eastern));
    drop(p);

    let with_notes = wide && *notes_open.read();
    rsx! {
        div { class: if with_notes { "lp with-notes" } else { "lp" },
            div { class: "lp-main",
            div { class: "lp-top",
                button {
                    class: "lp-close",
                    "data-back": "true",
                    aria_label: lt(lang, L::Close),
                    title: lt(lang, L::Close),
                    onclick: move |_| leave(),
                    "×"
                }
                // Isolated left to right so "3 / 12" never reads "12 / 3".
                span { class: "lp-count", dir: "ltr", aria_label: "{counter_label}", "{counter}" }
                div {
                    class: "lp-progress",
                    role: "progressbar",
                    aria_valuemin: "0",
                    aria_valuemax: "100",
                    aria_valuenow: "{pct}",
                    div { class: "lp-bar", style: "inline-size: {pct}%" }
                }
                if wide && !with_notes {
                    notes::ShowNotes { on_show: move |_| notes_open.set(true) }
                }
            }
            div { class: "lp-body",
                div { class: "lp-col", key: "{step:?}",
                    if *answered_before.read() {
                        p { class: "muted lp-small lp-voice-note", role: "status", "{lt(lang, L::AnsweredBefore)}" }
                    }
                    if show_voice_note(*voice_note_at.read(), *voice_skips.read(), step) {
                        p { class: "muted lp-small lp-voice-note", role: "status", "{lt(lang, L::NoVoiceSkipped)}" }
                    }
                    match step {
                        Step::Slide(i) => rsx! {
                            SlideView { lesson: lesson.clone(), index: i, missing: audio_missing }
                        },
                        Step::Exercise { .. } => match exercise.clone() {
                            Some(ex) => rsx! {
                                if retry {
                                    span { class: "pill lp-retry", "{lt(lang, L::Retry)}" }
                                }
                                p { class: "lp-instruction", "{lt(lang, instruction(&ex, &language))}" }
                                ExerciseView {
                                    ex,
                                    language: language.clone(),
                                    draft,
                                    locked: pending.is_some(),
                                    tiles: tiles.clone(),
                                    pair_order: pair_order.clone(),
                                    missing: audio_missing,
                                    on_matched: move |missed: Vec<String>| check(Answer::Matched { missed }),
                                }
                            },
                            None => rsx! {},
                        },
                        Step::Dialogue => rsx! {
                            if let Some(d) = lesson.dialogue.clone() {
                                DialogueView { dialogue: d, language: language.clone(), shown: *revealed.read(), missing: audio_missing }
                            }
                        },
                        Step::Summary => rsx! {
                            SummaryView { lesson: lesson.clone(), passed, check_content: !episode, summary_score: summary.score, summary_xp: summary.xp, duration_ms: summary.duration_ms, mistakes: summary.mistakes.clone(), perfect: summary.graded > 0 && summary.correct == summary.graded }
                        },
                    }
                }
            }
            div { class: "{feedback_class}",
                div { class: "lp-col lp-actions-inner",
                    if let (Some(g), Some(ex)) = (pending.clone(), exercise.clone()) {
                        Feedback {
                            ex,
                            grade: g,
                            retry,
                            target,
                            on_assess: move |same: bool| {
                                player.write().self_assess(same);
                                if same {
                                    chime(ui);
                                }
                                if !episode {
                                    save_with(db, &player.peek(), &draft.peek());
                                }
                            },
                        }
                    }
                    if let Some(h) = hint {
                        p { class: "lp-hint", id: "lp-hint", "{h}" }
                    }
                    div { class: "lp-buttons",
                        if matches!(step, Step::Slide(i) if i > 0) {
                            button { onclick: move |_| commit(&|p| p.back()), "{lt(lang, L::Back)}" }
                        }
                        if can_skip {
                            button {
                                class: "lp-skip",
                                onclick: move |_| commit(&|p| Some(p.skip())),
                                "{lt(lang, L::CantListenNow)}"
                            }
                        }
                        button {
                            class: "primary lp-primary",
                            disabled: !enabled,
                            aria_describedby: if hint.is_some() { "lp-hint" } else { "" },
                            onclick: move |_| primary(),
                            "{label}"
                        }
                    }
                }
            }
            }
            if with_notes {
                notes::LessonNotes {
                    lesson: lesson.clone(),
                    step,
                    marks,
                    answering,
                    missing: audio_missing,
                    on_hide: move |_| notes_open.set(false),
                }
            }
        }
    }
}

/// One mark per exercise for the notes pane: how its first try went (or its
/// retry, once that's done), the one on screen, or not reached yet.
fn exercise_marks(p: &Player) -> Vec<notes::Mark> {
    use notes::Mark;
    let results = p.completed().results;
    let current = match p.step() {
        Step::Exercise { index, .. } => Some(index),
        _ => None,
    };
    p.lesson()
        .exercises
        .iter()
        .enumerate()
        .map(|(i, ex)| {
            let done = results.iter().find(|r| r.id == ex.id).map(|r| Mark::of(r.retry.unwrap_or(r.first)));
            if current == Some(i) { Mark::Current } else { done.unwrap_or(Mark::Todo) }
        })
        .collect()
}

/// Saves the position with the answer on screen: what's picked or typed,
/// and once checked, its grade.
fn save_with(db: CopyValue<Db>, p: &Player, d: &Draft) {
    let mut pos = p.position();
    if let Some(ex) = p.exercise() {
        let checked = pos.response.take().and_then(|r| r.checked);
        let r = SavedResponse { checked, ..save_draft(ex, d, &p.tiles(ex)) };
        pos.response = (!r.is_empty()).then_some(r);
    }
    save(db, &pos);
}

fn save(db: CopyValue<Db>, pos: &LessonPosition) {
    if let Err(e) = db.read().0.borrow_mut().save_position(pos) {
        crate::diag::report!("saving the lesson position failed: {e}");
    }
}

/// Given by a player that already says "no sound on this device" once at
/// the top (the test player, #447): small play buttons then drop out when
/// the sound is missing instead of repeating it beside every prompt.
#[derive(Clone, Copy)]
pub struct NoSoundBanner;

/// A play button for target-language audio, with a slow option.
#[component]
fn PlayButton(
    text: String,
    language: String,
    audio: Option<String>,
    missing: Signal<bool>,
    big: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let (t1, l1, a1) = (text.clone(), language.clone(), audio.clone());
    let banner = try_use_context::<NoSoundBanner>().is_some();
    if banner && !big && *missing.read() {
        return rsx! {};
    }
    rsx! {
        div { class: "row lp-play",
            button {
                class: if big { "lp-sound big" } else { "lp-sound" },
                aria_label: lt(lang, L::Listen),
                title: lt(lang, L::Listen),
                onclick: move |_| speak(&t1, &l1, false, a1.as_deref(), missing),
                {speaker_icon()}
            }
            if big {
                button {
                    class: "lp-sound slow",
                    aria_label: lt(lang, L::Slowly),
                    title: lt(lang, L::Slowly),
                    onclick: move |_| speak(&text, &language, true, audio.as_deref(), missing),
                    {speaker_icon()}
                    span { class: "lp-slow-label", "{lt(lang, L::Slowly)}" }
                }
            }
            if *missing.read() {
                span { class: "muted lp-small", "{lt(lang, no_audio_msg())}" }
            }
        }
    }
}

/// Why there's no sound: a missing Indonesian voice gets an install hint.
pub(crate) fn no_audio_msg() -> L {
    match platform::tts_status() {
        TtsStatus::MissingVoice | TtsStatus::Unsupported => L::NoVoiceData,
        _ => L::NoAudio,
    }
}

pub(crate) fn speaker_icon() -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: "M4 10v4h4l5 4V6L8 10zM16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11" }
        }
    }
}

// ---------------------------------------------------------------------------
// Slides, dialogue, summary
// ---------------------------------------------------------------------------

/// The language of a table column: an "Indonesia" column in a Javanese
/// lesson holds Indonesian words; every other column the lesson's own.
fn column_tag(head: Option<&str>, target: &'static str) -> &'static str {
    let indonesian = head.is_some_and(|h| {
        let h = h.trim().to_lowercase();
        h == "indonesia" || h == "indonesian" || h == "bahasa indonesia"
    });
    if indonesian { "id" } else { target }
}

/// One table cell. A cell that is a word or phrase always shows how it's
/// said, in its column's language, so a table follows one rule (#381):
/// Javanese "awan" is said even though Indonesian spells it the same.
#[component]
fn TableCell(text: String, tag: &'static str) -> Element {
    let ui = use_context::<Ui>();
    let say = ui.settings.read().ui_language != UiLanguage::En;
    let word = latin_runs(&text).len() == 1 && is_target_run(&text) && !text.contains("**");
    if !word {
        return rsx! { Rich { text, target: tag, pron: true } };
    }
    rsx! {
        // Word over its pronunciation, so a four-column table fits a phone.
        span { class: "lp-cell-word",
            bdi { lang: tag, dir: "ltr", class: "latin", "{text}" }
            Say { text: text.clone(), tag }
            if say {
                Pron { text: text.clone(), tag, inline: true }
            }
        }
    }
}

#[component]
fn SlideView(lesson: LessonContent, index: usize, missing: Signal<bool>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&lesson.language);
    // Passed earlier with a reasonable score: the learner approved it.
    let db = use_context::<Db>();
    let approved = use_hook(|| db.0.borrow().approved_lessons().is_ok_and(|ids| ids.contains(&lesson.id)));
    let Some(slide) = lesson.slides.get(index).cloned() else { return rsx! {} };
    let language = lesson.language.clone();
    rsx! {
        article { class: "lp-slide lp-enter",
            if index == 0 {
                p { class: "lp-lesson-title",
                    bdi { lang: target, dir: "ltr", "{lesson.title}" }
                    if !lesson.review.verified && !approved {
                        span { class: "pill lp-draft", "{lt(lang, L::NotReviewed)}" }
                    }
                }
                p { class: "muted", Rich { text: pick(&lesson.can_do, lang).to_string(), target } }
            }
            if let Some(title) = &slide.title {
                h2 { class: "lp-slide-title", Title { text: title.clone(), target } }
            }
            p { class: "lp-slide-body pre", Rich { text: pick(&slide.body, lang).trim().to_string(), target, pron: true } }
            if let Some(table) = &slide.table {
                div { class: "lp-table-wrap",
                    table { class: "lp-table",
                        thead {
                            tr {
                                for (i, h) in table.head.iter().enumerate() {
                                    th { key: "{i}", bdi { lang: target, dir: "ltr", "{h}" } }
                                }
                                th { "{lt(lang, L::TableMeaning)}" }
                            }
                        }
                        tbody {
                            for (i, row) in table.rows.iter().enumerate() {
                                tr { key: "{i}",
                                    for (j, cell) in row.cells.iter().enumerate() {
                                        {
                                            let tag = column_tag(table.head.get(j).map(String::as_str), target);
                                            rsx! {
                                                td { key: "{j}", lang: tag, TableCell { text: cell.clone(), tag } }
                                            }
                                        }
                                    }
                                    td { class: "muted", "{pick(&row.meaning, lang)}" }
                                }
                            }
                        }
                    }
                }
            }
            if !slide.examples.is_empty() {
                p { class: "muted lp-small", "{lt(lang, L::Examples)}" }
                ul { class: "lp-examples",
                    for (i, ex) in slide.examples.iter().enumerate() {
                        li { key: "{i}",
                            button {
                                class: "lp-example",
                                onclick: {
                                    let (t, l) = (ex.text.clone(), language.clone());
                                    move |_| speak(&t, &l, false, None, missing)
                                },
                                span { class: "lp-example-icon", {speaker_icon()} }
                                span { class: "lp-example-text",
                                    Annotated { text: ex.text.clone(), tag: target, meaning: pick(&ex.meaning, lang).to_string(), class: "lp-target", say: false }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DialogueView(dialogue: Dialogue, language: String, shown: usize, missing: Signal<bool>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&language);
    let first = dialogue.lines.first().map(|l| l.speaker.clone()).unwrap_or_default();
    rsx! {
        article { class: "lp-dialogue",
            p { class: "lp-instruction", "{lt(lang, L::Dialogue)}" }
            if let Some(t) = &dialogue.title {
                h2 { class: "lp-slide-title", bdi { lang: target, dir: "ltr", "{t}" } }
            }
            p { class: "muted lp-small", "{lt(lang, L::TapToHear)}" }
            ol { class: "lp-lines",
                for (i, line) in dialogue.lines.iter().take(shown).enumerate() {
                    li {
                        key: "{i}",
                        class: if line.speaker == first { "lp-line a lp-enter" } else { "lp-line b lp-enter" },
                        button {
                            class: "lp-bubble",
                            onclick: {
                                let (d, l) = (dialogue.clone(), language.clone());
                                move |_| speak_line(&d, i, &l, missing)
                            },
                            span { class: "lp-speaker",
                                crate::portraits::SpeakerFace { speaker: line.speaker.clone(), class: "lp-face" }
                                bdi { "{line.speaker}" }
                            }
                            Annotated { text: line.text.clone(), tag: target, meaning: pick(&line.meaning, lang).to_string(), class: "lp-target", say: false }
                        }
                    }
                }
            }
        }
    }
}

fn duration_text(ms: u64, lang: UiLanguage, eastern: bool) -> String {
    let s = ms / 1000;
    localize_digits(&format!("{}:{:02}", s / 60, s % 60), lang, eastern)
}

#[component]
fn SummaryView(
    lesson: LessonContent,
    passed: bool,
    check_content: bool,
    summary_score: u32,
    summary_xp: u32,
    duration_ms: u64,
    mistakes: Vec<String>,
    perfect: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<crate::play::PlayNav>();
    // The gong rings once when the summary appears.
    use_hook(move || crate::sounds::play(ui, crate::sounds::Sound::LessonEnd));
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;
    let eastern = settings.eastern_digits;
    let target = target_tag(&lesson.language);
    let num = |n: u32| format_number(u64::from(n), lang, eastern);
    rsx! {
        article { class: "lp-summary",
            div { class: "lp-gong reveal", aria_hidden: "true",
                span { class: "lp-gong-ring" }
                crate::misc_art::Picture { art: crate::misc_art::Art::Gong, class: "" }
            }
            h2 { class: "lp-done reveal", "{lt(lang, L::Done)}" }
            p { class: "lp-lesson-title reveal", bdi { lang: target, dir: "ltr", "{lesson.title}" } }
            // Finishing (and passing) is the learner's progress; whether the
            // content is right is asked separately below.
            p { class: "lp-finished reveal",
                span { class: "pill ok", "✓ {lt(lang, L::Finished)}" }
                if passed {
                    span { class: "pill ok", "{lt(lang, L::Passed)}" }
                }
            }
            if perfect {
                p { class: "ok reveal", "{lt(lang, L::Perfect)}" }
            }
            dl { class: "lp-stats reveal d2",
                div { dt { "{lt(lang, L::Score)}" } dd { bdi { "{format_percent(u64::from(summary_score), lang, eastern)}" } } }
                div { dt { "{lt(lang, L::Xp)}" } dd { bdi { "+{num(summary_xp)}" } } }
                div { dt { "{lt(lang, L::Time)}" } dd { bdi { "{duration_text(duration_ms, lang, eastern)}" } } }
            }
            if check_content && !lesson.review.verified {
                ContentCheck { lesson_id: lesson.id.clone() }
            }
            if !lesson.new_words.is_empty() {
                section { class: "card reveal d2",
                    h3 { "{lt(lang, L::NewWords)}" }
                    ul { class: "lp-words",
                        for w in lesson.new_words.iter() {
                            li { key: "{w.word}", class: if w.arabic.is_some() { "bridge-row" } else { "" },
                                Annotated { text: w.word.clone(), tag: target, class: if w.arabic.is_some() { "bridge-word" } else { "" } }
                                span { class: "muted", Rich { text: pick(&w.meaning, lang).to_string(), target } }
                                if let Some(ff) = &w.false_friend {
                                    span { class: "lp-false-friend", title: lt(lang, L::FalseFriend),
                                        span { "≠ {lt(lang, L::FalseFriend)}" }
                                        bdi { lang: "id", dir: "ltr", class: "latin", "{ff}" }
                                    }
                                }
                                if let Some(ar) = &w.arabic {
                                    span { class: "lp-bridge", title: lt(lang, L::FromArabic),
                                        span { class: "lp-bridge-label", "{lt(lang, L::FromArabic)}" }
                                        span { lang: "ar", class: "ar", "{ar}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !mistakes.is_empty() {
                section { class: "card reveal d3",
                    h3 { "{lt(lang, L::Tomorrow)}" }
                    p { class: "row",
                        for m in mistakes.iter() {
                            bdi { key: "{m}", lang: target, dir: "ltr", class: "pill latin", "{m}" }
                        }
                    }
                }
            }
            if let Some(s) = &lesson.souvenir {
                section { class: "card lp-souvenir reveal d3",
                    crate::souvenir_art::SouvenirArt {
                        unit_id: crate::souvenir_art::unit_of(&s.id).to_string(),
                        size: crate::souvenir_art::Size::Card,
                    }
                    p { class: "muted lp-small", "{lt(lang, L::Souvenir)}" }
                    h3 { bdi { lang: target, dir: "ltr", "{s.title}" } }
                    p { Rich { text: pick(&s.note, lang).to_string(), target } }
                }
            }
            if let Some(route) = lesson.game.as_deref().and_then(game_route) {
                div { class: "row center reveal d3",
                    button {
                        onclick: move |_| {
                            ui.close_lesson();
                            crate::play::open(ui, nav, route);
                        },
                        "{lt(lang, L::PlayGame)}"
                    }
                }
            }
        }
    }
}

/// The learner's check of the lesson's content, apart from their score:
/// one tap marks it right (its "not reviewed" flag and its cards' come off),
/// and the tap can be taken back. A lesson counted as checked under the old
/// pass rule says so.
#[component]
fn ContentCheck(lesson_id: String) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.settings.read().ui_language;
    let eastern = ui.settings.read().eastern_digits;
    let read = {
        let (db, id) = (db.clone(), lesson_id.clone());
        move || {
            db.0.borrow().verified_lessons().map(|v| v.get(&id).copied()).unwrap_or_else(|e| {
                crate::diag::report!("reading checked lessons failed: {e}");
                None
            })
        }
    };
    let mut state = use_signal(read.clone());
    let set = {
        let (db, id) = (db.clone(), lesson_id.clone());
        move |ok: bool| {
            let mut store = db.0.borrow_mut();
            let done = if ok { store.verify_lesson(&id) } else { store.unverify_lesson(&id) };
            if let Err(e) = done {
                crate::diag::report!("saving the content check failed: {e}");
            }
            drop(store);
            state.set(read());
        }
    };
    match *state.read() {
        None => {
            let mut set = set.clone();
            rsx! {
                section {
                    class: "card card-action lp-content reveal d2",
                    onclick: move |_| set(true),
                    h3 { "{lt(lang, L::ContentTitle)}" }
                    p { class: "muted lp-small", "{lt(lang, L::ContentHint)}" }
                    button { class: "tonal lp-content-ok", "{lt(lang, L::ContentOk)}" }
                }
            }
        }
        Some(Verification::Checked) => {
            let mut set = set.clone();
            rsx! {
                div { class: "lp-content-done reveal d2", role: "status",
                    span { "✓ {lt(lang, L::ContentChecked)}" }
                    button { class: "ghost lp-content-undo", onclick: move |_| set(false), "{lt(lang, L::ContentUndo)}" }
                }
            }
        }
        Some(Verification::PassedEarlier { score }) => rsx! {
            div { class: "lp-content-done reveal d2", role: "status",
                span {
                    "✓ "
                    {lt(lang, L::ContentPassedEarlier).replace("{score}", &format_percent(u64::from(score), lang, eastern))}
                }
            }
        },
    }
}

/// The Play screen a lesson's `game` id opens: `uu.*` is an Unggah-Ungguh
/// scene, `mp.*` a Minimal Pairs pair. Opens the game, not the exact item.
fn game_route(id: &str) -> Option<crate::play::Route> {
    if id.starts_with("uu.") {
        Some(crate::play::Route::Unggah)
    } else if id.starts_with("mp.") {
        Some(crate::play::Route::Pairs)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Feedback
// ---------------------------------------------------------------------------

/// Spoken-text exercises also teach the meaning in the feedback.
fn meaning_of(ex: &Exercise) -> Option<&Localized> {
    match &ex.kind {
        ExerciseKind::Listen { meaning, .. } => meaning.as_ref(),
        ExerciseKind::Dictation { meaning, .. }
        | ExerciseKind::Speaking { meaning, .. }
        | ExerciseKind::FillBlank { meaning, .. } => Some(meaning),
        _ => None,
    }
}

#[component]
fn Feedback(
    ex: Exercise,
    grade: Grade,
    retry: bool,
    target: &'static str,
    on_assess: EventHandler<bool>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    // Vary the praise, but the same exercise always gets the same words.
    let praise = [L::Correct1, L::Correct2, L::Correct3][ex.id.bytes().map(usize::from).sum::<usize>() % 3];
    let word = ex.words.first().cloned();
    let meaning = meaning_of(&ex).map(|m| pick(m, lang).to_string());
    // Comprehension: the answer is about the audio, so also show what was said.
    let transcript = match &ex.kind {
        ExerciseKind::Listen { question: Some(_), text, .. } => Some(text.clone()),
        _ => None,
    };
    let explain = ex.explain.as_ref().map(|e| pick(e, lang).to_string());
    let answer = display_answer(&ex, lang);
    let answer_tag = match &ex.kind {
        ExerciseKind::MultipleChoice { options_meaning, .. } if !options_meaning.is_empty() => lang.tag(),
        ExerciseKind::Translate { direction: Direction::FromTarget, .. } => lang.tag(),
        _ => target,
    };
    rsx! {
        div { class: "lp-feedback lp-enter", role: "status", aria_live: "polite",
            match &grade {
                Grade::Correct => rsx! {
                    p { class: "lp-verdict", "{lt(lang, praise)}" }
                    if let Some(w) = &word {
                        p {
                            Rich { text: lt(lang, L::CorrectWord).replace("{w}", w), target }
                            Say { text: w.clone(), tag: target }
                        }
                        Pron { text: w.clone(), tag: target }
                    }
                },
                Grade::Practiced => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::Practiced)}" }
                },
                Grade::Almost { expected } => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::Almost)}" }
                    p { Annotated { text: expected.clone(), tag: target, class: "lp-answer" } }
                },
                Grade::Wrong => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::NotYet)}" }
                    if answer_tag == target {
                        p { Annotated { text: answer.clone(), tag: target, class: "lp-answer" } }
                    } else {
                        p { bdi { lang: answer_tag, dir: "auto", class: "lp-answer", "{answer}" } }
                    }
                    if !retry {
                        p { class: "muted lp-small", "{lt(lang, L::AgainAtEnd)}" }
                    }
                },
                Grade::NeedsReview => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::NeedsReview)}" }
                    ul { class: "lp-refs",
                        for r in references(&ex, lang).into_iter().take(3) {
                            li { key: "{r}", bdi { lang: answer_tag, dir: if answer_tag == target { "ltr" } else { "auto" }, class: "lp-answer", "{r}" } }
                        }
                    }
                    p { "{lt(lang, L::SameMeaning)}" }
                    div { class: "row",
                        button { class: "primary", onclick: move |_| on_assess.call(true), "{lt(lang, L::YesSame)}" }
                        button { onclick: move |_| on_assess.call(false), "{lt(lang, L::NotQuite)}" }
                    }
                },
            }
            if let Some(t) = transcript {
                p { class: "lp-small",
                    span { class: "muted", "{lt(lang, L::Transcript)} " }
                    Annotated { text: t.clone(), tag: target, class: "lp-target" }
                }
            }
            if let Some(m) = meaning {
                p { class: "muted", Rich { text: m, target } }
            }
            if !matches!(grade, Grade::Correct | Grade::Practiced) {
                if let Some(e) = explain {
                    details { class: "lp-why", open: true,
                        summary { "{lt(lang, L::Why)}" }
                        p { Rich { text: e, target } }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Exercises (standalone: also usable by level tests)
// ---------------------------------------------------------------------------

#[component]
pub fn ExerciseView(
    ex: Exercise,
    language: String,
    draft: Signal<Draft>,
    locked: bool,
    tiles: Vec<String>,
    pair_order: Vec<usize>,
    missing: Signal<bool>,
    on_matched: EventHandler<Vec<String>>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&language);
    use ExerciseKind as K;
    match ex.kind.clone() {
        K::MultipleChoice { question, question_meaning, options, options_meaning, answer } => {
            let (labels, tag) = if options_meaning.is_empty() {
                (options, target)
            } else {
                (options_meaning.iter().map(|m| pick(m, lang).to_string()).collect(), lang.tag())
            };
            rsx! {
                div { class: "lp-prompt",
                    if !question.trim().is_empty() {
                        div { class: "row",
                            PlayButton { text: question.clone(), language: language.clone(), audio: None, missing, big: false }
                            p { class: "lp-question pre", bdi { lang: target, dir: "ltr", class: "latin", "{question}" } }
                        }
                    }
                    if let Some(q) = question_meaning {
                        p { class: "lp-question-meaning", Rich { text: pick(&q, lang).to_string(), target } }
                    }
                }
                Choices { labels, tag, draft, locked, answer }
            }
        }
        K::MatchPairs { pairs } => rsx! {
            MatchPairs { pairs: pairs.iter().map(|p| (p.text.clone(), pick(&p.meaning, lang).to_string())).collect::<Vec<_>>(), order: pair_order, target, draft, locked, on_matched }
        },
        K::Translate { direction, source_meaning, source_text, .. } => {
            let (answer_tag, prompt) = match direction {
                Direction::ToTarget => (target, None),
                Direction::FromTarget => (lang.tag(), source_text.clone()),
            };
            rsx! {
                div { class: "lp-prompt",
                    if let Some(m) = &source_meaning {
                        p { class: "lp-question-meaning", Rich { text: pick(m, lang).to_string(), target } }
                    }
                    if let Some(t) = prompt {
                        div { class: "row",
                            PlayButton { text: t.clone(), language: language.clone(), audio: None, missing, big: false }
                            p { class: "lp-question", bdi { lang: target, dir: "ltr", class: "latin", "{t}" } }
                        }
                    }
                }
                TextAnswer { tag: answer_tag, draft, locked }
            }
        }
        K::BuildSentence { source, .. } => rsx! {
            p { class: "lp-question-meaning", Rich { text: pick(&source, lang).to_string(), target } }
            Tiles { tiles, target, draft, locked }
        },
        K::Listen { text, audio, options, answer, question, .. } => rsx! {
            div { class: "lp-prompt center",
                PlayButton { text, language: language.clone(), audio, missing, big: true }
            }
            if let Some(q) = question {
                p { class: "lp-question-meaning", Rich { text: pick(&q, lang).to_string(), target } }
            }
            if options.is_empty() {
                TextAnswer { tag: target, draft, locked }
            } else {
                Choices { labels: options, tag: target, draft, locked, answer: answer.unwrap_or(usize::MAX) }
            }
        },
        K::FillBlank { sentence, meaning, options, answers } => {
            let (before, after) = sentence.split_once("___").unwrap_or((&sentence, ""));
            let (before, after) = (before.to_string(), after.to_string());
            let chosen = draft.read().choice.and_then(|i| options.get(i).cloned());
            let right = options.iter().position(|o| answers.iter().any(|a| a.eq_ignore_ascii_case(o)));
            // "Selamat ___, Pak!": the blank hugs the comma, no gap before it.
            let hug = if blank_hugs_next(&after) { " hug-end" } else { "" };
            rsx! {
                p { class: "lp-question lp-blank-line", lang: target, dir: "ltr",
                    "{before}"
                    if options.is_empty() {
                        input {
                            class: "lp-blank-input{hug}",
                            r#type: "text",
                            lang: target,
                            autocomplete: "off",
                            autocapitalize: "off",
                            spellcheck: "false",
                            disabled: locked,
                            size: "8",
                            value: "{draft.read().text}",
                            oninput: move |e: FormEvent| draft.write().text = e.value(),
                        }
                    } else {
                        span { class: if chosen.is_some() { "lp-blank filled{hug}" } else { "lp-blank{hug}" },
                            "{chosen.clone().unwrap_or_default()}"
                        }
                    }
                    "{after}"
                }
                p { class: "muted", Rich { text: pick(&meaning, lang).to_string(), target } }
                if !options.is_empty() {
                    Choices { labels: options.clone(), tag: target, draft, locked, answer: right.unwrap_or(usize::MAX), chips: true }
                }
            }
        }
        K::Dictation { text, audio, .. } => rsx! {
            div { class: "lp-prompt center",
                PlayButton { text, language: language.clone(), audio, missing, big: true }
            }
            TextAnswer { tag: target, draft, locked }
        },
        K::Speaking { text, meaning } => rsx! {
            div { class: "lp-prompt center",
                p { class: "lp-question big", bdi { lang: target, dir: "ltr", class: "latin", "{text}" } }
                p { class: "muted", Rich { text: pick(&meaning, lang).to_string(), target } }
                PlayButton { text: text.clone(), language: language.clone(), audio: None, missing, big: true }
                p { class: "muted lp-small", "{lt(lang, L::SpeakingLater)}" }
            }
        },
    }
}

/// Option buttons and fill-in chips (keys 1–9 pick them too). After checking, the right one is
/// marked and a wrong pick is shown gently.
#[component]
fn Choices(
    labels: Vec<String>,
    tag: &'static str,
    draft: Signal<Draft>,
    locked: bool,
    answer: usize,
    #[props(default)] chips: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let chosen = draft.read().choice;
    rsx! {
        div { class: if chips { "lp-choices chips" } else { "lp-choices" }, role: "group",
            for (i, label) in labels.iter().enumerate() {
                {
                    let mark = choice_mark(locked, i == answer, chosen == Some(i));
                    let aria = mark.map(|m| format!("{label} — {}", lt(lang, m.says())));
                    rsx! {
                        button {
                            key: "{i}",
                            class: match mark {
                                Some(ChoiceMark::Right | ChoiceMark::YoursRight) => "lp-choice right",
                                Some(ChoiceMark::YoursWrong) => "lp-choice wrong",
                                None => "lp-choice",
                            },
                            aria_pressed: if chosen == Some(i) { "true" } else { "false" },
                            aria_label: aria,
                            disabled: locked,
                            onclick: move |_| draft.write().choice = Some(i),
                            if let Some(m) = mark {
                                span { class: "lp-key lp-mark", aria_hidden: "true", "{m.glyph()}" }
                            } else if i < 9 {
                                span { class: "lp-key", aria_hidden: "true", "{i + 1}" }
                            }
                            bdi { lang: tag, dir: "auto", "{label}" }
                        }
                    }
                }
            }
        }
    }
}

/// How a graded choice is marked, so right and wrong never rely on colour
/// alone (WCAG 1.4.1): a glyph in the key-badge slot plus an aria label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChoiceMark {
    /// The right answer, which the learner didn't pick.
    Right,
    /// The learner's pick, and it's right.
    YoursRight,
    /// The learner's pick, and it's wrong.
    YoursWrong,
}

impl ChoiceMark {
    fn glyph(self) -> &'static str {
        match self {
            ChoiceMark::Right | ChoiceMark::YoursRight => "✓",
            ChoiceMark::YoursWrong => "✗",
        }
    }

    fn says(self) -> L {
        match self {
            ChoiceMark::Right => L::ChoiceCorrect,
            ChoiceMark::YoursRight => L::ChoiceYoursCorrect,
            ChoiceMark::YoursWrong => L::ChoiceYours,
        }
    }
}

/// Where the "no voice, listening skipped" note goes after a skip from `from`
/// to `to`: the first step the learner actually sees after the first skip.
/// A run of back-to-back skips carries it forward; later skips leave it put,
/// so the note shows once per lesson instead of on every step.
fn voice_note_step(note: Option<Step>, from: Step, to: Step) -> Option<Step> {
    match note {
        None => Some(to),
        Some(at) if at == from => Some(to),
        kept => kept,
    }
}

/// Whether the text after a blank starts with punctuation that should sit
/// right against the filled word.
fn blank_hugs_next(after: &str) -> bool {
    after.starts_with([',', '.', '!', '?', ';', ':', ')', '…'])
}

/// The note shows on its one step, and again in the summary.
fn show_voice_note(note: Option<Step>, skips: u32, step: Step) -> bool {
    skips > 0 && (note == Some(step) || step == Step::Summary)
}

/// The mark for one option. Nothing is marked until the answer is checked;
/// other options stay unmarked.
fn choice_mark(locked: bool, is_answer: bool, is_chosen: bool) -> Option<ChoiceMark> {
    match (locked, is_answer, is_chosen) {
        (false, _, _) => None,
        (true, true, true) => Some(ChoiceMark::YoursRight),
        (true, true, false) => Some(ChoiceMark::Right),
        (true, false, true) => Some(ChoiceMark::YoursWrong),
        (true, false, false) => None,
    }
}

/// Free text: typed, Samsung handwriting keyboard, or DeX keyboard. `lang`
/// tells the IME which language to expect. Enter checks (the player's key
/// listener); Shift+Enter is a
/// new line.
#[component]
pub(crate) fn TextAnswer(tag: &'static str, draft: Signal<Draft>, locked: bool) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let dir = if tag.starts_with("ar") { "rtl" } else { "ltr" };
    rsx! {
        textarea {
            class: "lp-text",
            lang: tag,
            dir,
            rows: "3",
            autocomplete: "off",
            autocapitalize: "sentences",
            spellcheck: "false",
            placeholder: lt(lang, L::AnswerPlaceholder),
            disabled: locked,
            value: "{draft.read().text}",
            oninput: move |e: FormEvent| draft.write().text = e.value(),
        }
    }
}

/// Build a sentence: tap a tile in the bank to add it, tap it in the answer
/// to send it back.
#[component]
fn Tiles(tiles: Vec<String>, target: &'static str, draft: Signal<Draft>, locked: bool) -> Element {
    let picked = draft.read().picked.clone();
    rsx! {
        div { class: "lp-answer-line", lang: target, dir: "auto",
            for (pos, &i) in picked.iter().enumerate() {
                button {
                    key: "a{i}",
                    class: "lp-tile",
                    disabled: locked,
                    onclick: move |_| {
                        draft.write().picked.remove(pos);
                    },
                    "{tiles[i]}"
                }
            }
        }
        div { class: "lp-bank", lang: target, dir: "auto",
            for (i, t) in tiles.iter().enumerate() {
                button {
                    key: "b{i}",
                    class: if picked.contains(&i) { "lp-tile ghost" } else { "lp-tile" },
                    disabled: locked || picked.contains(&i),
                    onclick: move |_| draft.write().picked.push(i),
                    "{t}"
                }
            }
        }
    }
}

/// Tap a word, then its meaning (either order). Works the same with the pen.
#[component]
fn MatchPairs(
    pairs: Vec<(String, String)>,
    order: Vec<usize>,
    target: &'static str,
    draft: Signal<Draft>,
    locked: bool,
    on_matched: EventHandler<Vec<String>>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let d = draft.read().clone();
    let words: Vec<String> = pairs.iter().map(|p| p.0.clone()).collect();
    let state = |sel: Option<usize>, i: usize, wrong: Option<usize>| pair_class(&d.matched, sel, i, wrong);
    let done_label = |i: usize, text: &str| {
        d.matched.contains(&i).then(|| format!("{text} — {}", lt(lang, L::PairMatched)))
    };
    rsx! {
        div { class: "lp-pairs",
            div { class: "lp-pair-col",
                for (i, (word, _)) in pairs.iter().enumerate() {
                    button {
                        key: "l{i}",
                        class: state(d.left, i, d.wrong.map(|w| w.0)),
                        disabled: locked || d.matched.contains(&i),
                        aria_pressed: if d.left == Some(i) { "true" } else { "false" },
                        aria_label: done_label(i, word),
                        onclick: {
                            let words = words.clone();
                            move |_| {
                                draft.write().left = Some(i);
                                try_match(draft, &words, on_matched);
                            }
                        },
                        if let Some(n) = couple_number(&d.matched, i) {
                            span { class: "lp-pair-tick", aria_hidden: "true", "✓{n}" }
                        }
                        bdi { lang: target, dir: "ltr", class: "latin", "{word}" }
                    }
                }
            }
            div { class: "lp-pair-col",
                for &i in order.iter() {
                    button {
                        key: "r{i}",
                        class: state(d.right, i, d.wrong.map(|w| w.1)),
                        disabled: locked || d.matched.contains(&i),
                        aria_pressed: if d.right == Some(i) { "true" } else { "false" },
                        aria_label: done_label(i, &pairs[i].1),
                        onclick: {
                            let words = words.clone();
                            move |_| {
                                draft.write().right = Some(i);
                                try_match(draft, &words, on_matched);
                            }
                        },
                        if let Some(n) = couple_number(&d.matched, i) {
                            span { class: "lp-pair-tick", aria_hidden: "true", "✓{n}" }
                        }
                        Rich { text: pairs[i].1.clone(), target }
                    }
                }
            }
        }
    }
}

/// Couple tints in lesson.css; a board has at most six pairs.
const COUPLE_TINTS: usize = 6;

/// The class of one pair button. A matched couple reads as done, not
/// disabled: both halves share a tint (by the order they were matched), so
/// the learner can see which word went with which meaning.
fn pair_class(matched: &[usize], sel: Option<usize>, i: usize, wrong: Option<usize>) -> String {
    if let Some(k) = matched.iter().position(|&m| m == i) {
        format!("lp-pair done couple-{}", k % COUPLE_TINTS)
    } else if wrong == Some(i) {
        "lp-pair wrong".into()
    } else if sel == Some(i) {
        "lp-pair selected".into()
    } else {
        "lp-pair".into()
    }
}

/// The number both halves of a matched couple wear (1 for the first match),
/// so colour is never the only cue (#311).
fn couple_number(matched: &[usize], i: usize) -> Option<usize> {
    matched.iter().position(|&m| m == i).map(|k| k + 1)
}

/// Once both sides are selected: a match sticks, a mismatch is remembered as
/// a word to practise. All matched → `on_matched`.
fn try_match(mut draft: Signal<Draft>, words: &[String], on_matched: EventHandler<Vec<String>>) {
    let (Some(l), Some(r)) = ({
        let d = draft.peek();
        (d.left, d.right)
    }) else {
        return;
    };
    let done = {
        let mut d = draft.write();
        d.left = None;
        d.right = None;
        if l == r {
            d.matched.push(l);
            d.wrong = None;
        } else {
            d.wrong = Some((l, r));
            if !d.missed.contains(&words[l]) {
                d.missed.push(words[l].clone());
            }
        }
        (d.matched.len() == words.len()).then(|| d.missed.clone())
    };
    if let Some(missed) = done {
        on_matched.call(missed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../fixtures/lessons/bipa1.u01.l01.yaml");

    use kabar_core::lesson::level_test::{SelfGrade, TestRunner, TestStep, parse_test, validate_test};

    /// Graded choices carry a glyph and a spoken label, not just a colour (#149).
    #[test]
    fn graded_choices_are_marked_beyond_colour() {
        assert_eq!(choice_mark(false, true, true), None);
        assert_eq!(choice_mark(true, false, false), None);
        assert_eq!(choice_mark(true, true, false), Some(ChoiceMark::Right));
        assert_eq!(choice_mark(true, true, true), Some(ChoiceMark::YoursRight));
        assert_eq!(choice_mark(true, false, true), Some(ChoiceMark::YoursWrong));
        assert_eq!(ChoiceMark::Right.glyph(), "✓");
        assert_eq!(ChoiceMark::YoursWrong.glyph(), "✗");
        for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
            for m in [ChoiceMark::Right, ChoiceMark::YoursRight, ChoiceMark::YoursWrong] {
                assert!(!lt(lang, m.says()).is_empty());
            }
        }
    }

    /// The "no voice" note shows once: on the first step after the first run
    /// of skipped listening items, and in the summary (#156).
    #[test]
    fn the_no_voice_note_shows_once_per_lesson() {
        let ex = |index| Step::Exercise { index, retry: false };
        // e2 is skipped, landing on e3: the note goes on e3.
        let note = voice_note_step(None, ex(2), ex(3));
        assert_eq!(note, Some(ex(3)));
        // e3 is a listening item too: the note moves on with the skip.
        let note = voice_note_step(note, ex(3), ex(4));
        assert_eq!(note, Some(ex(4)));
        // A later skip (e7 -> e8) leaves it where the learner saw it.
        let note = voice_note_step(note, ex(7), ex(8));
        assert_eq!(note, Some(ex(4)));
        assert!(show_voice_note(note, 3, ex(4)));
        assert!(!show_voice_note(note, 3, ex(5)));
        assert!(!show_voice_note(note, 3, ex(8)));
        assert!(show_voice_note(note, 3, Step::Summary));
        assert!(!show_voice_note(None, 0, Step::Summary));
    }

    /// A filled blank sits right against the punctuation after it (#156).
    #[test]
    fn a_blank_hugs_following_punctuation() {
        let sentence = "Selamat ___, Pak!";
        let (_, after) = sentence.split_once("___").unwrap();
        assert!(blank_hugs_next(after));
        assert!(blank_hugs_next("."));
        assert!(!blank_hugs_next(" pagi."));
        assert!(!blank_hugs_next(""));
    }

    /// Both halves of a matched couple share a tint; unmatched ones keep their state.
    #[test]
    fn matched_pairs_share_a_couple_tint() {
        let matched = [2, 0, 4, 1, 3];
        assert_eq!(pair_class(&matched, None, 2, None), "lp-pair done couple-0");
        assert_eq!(pair_class(&matched, None, 0, None), "lp-pair done couple-1");
        assert_eq!(pair_class(&matched, None, 3, None), "lp-pair done couple-4");
        // Six couples, six tints, and the number repeats nothing.
        let six = [5, 4, 3, 2, 1, 0];
        let tints: std::collections::BTreeSet<String> =
            (0..6).map(|i| pair_class(&six, None, i, None)).collect();
        assert_eq!(tints.len(), 6);
        assert_eq!((couple_number(&six, 5), couple_number(&six, 0)), (Some(1), Some(6)));
        assert_eq!(couple_number(&six[..2], 0), None);
        assert_eq!(pair_class(&[], Some(1), 1, None), "lp-pair selected");
        assert_eq!(pair_class(&[], Some(1), 1, Some(1)), "lp-pair wrong");
        assert_eq!(pair_class(&[], None, 1, None), "lp-pair");
    }

    /// The reference answer for a closed exercise.
    fn right_answer(ex: &Exercise) -> Answer {
        use ExerciseKind as K;
        match &ex.kind {
            K::MultipleChoice { answer, .. } => Answer::Choice(*answer),
            K::Listen { answer: Some(i), options, .. } if !options.is_empty() => Answer::Choice(*i),
            K::Listen { text, .. } | K::Dictation { text, .. } => Answer::Text(text.clone()),
            K::FillBlank { options, answers, .. } if !options.is_empty() => {
                Answer::Choice(options.iter().position(|o| answers.contains(o)).unwrap())
            }
            K::FillBlank { answers, .. } => Answer::Text(answers[0].clone()),
            K::Translate { answers, answers_meaning, .. } => Answer::Text(
                answers
                    .first()
                    .cloned()
                    .unwrap_or_else(|| answers_meaning.as_ref().unwrap().ar_eg[0].clone()),
            ),
            K::BuildSentence { answer, .. } => Answer::Text(answer.clone()),
            K::MatchPairs { .. } => Answer::Matched { missed: vec![] },
            K::Speaking { .. } => Answer::Practiced,
        }
    }

    /// Takes a test answering everything right (open tasks self-rated fully).
    fn take_perfectly(r: &mut TestRunner) {
        r.start();
        while let TestStep::Item { .. } = r.step() {
            match r.exercise() {
                Some(ex) => r.answer(&right_answer(&ex), UiLanguage::ArEg),
                None => r.answer_open("Hai!"),
            }
        }
        let ids: Vec<String> = r.open_answers().map(|o| o.id.clone()).collect();
        for id in ids {
            r.self_assess(&id, SelfGrade::Fully);
        }
        r.finish_self_check();
        assert_eq!(r.step(), TestStep::Results);
    }

    #[test]
    fn every_embedded_test_parses_validates_and_can_be_passed() {
        assert!(TESTS.rows.len() >= 3, "only {} tests embedded", TESTS.rows.len());
        for (id, yaml) in embedded::all(TESTS) {
            let test = parse_test(&yaml).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(&test.id, id);
            let issues: Vec<String> = validate_test(&test).iter().map(|i| i.to_string()).collect();
            assert!(issues.is_empty(), "{id}:\n{}", issues.join("\n"));
            let mut r = TestRunner::new(test.clone());
            take_perfectly(&mut r);
            let res = r.results();
            assert!(res.missed.is_empty(), "{id}: {:?}", res.missed);
            match test.kind {
                kabar_protocol::test::TestKind::Level => assert_eq!(res.passed, Some(true), "{id}"),
                kabar_protocol::test::TestKind::Placement => {
                    assert_eq!(res.start_level.as_deref(), Some("bipa5"), "{id}")
                }
            }
        }
        assert!(test_content("bipa1.test").is_some());
        assert!(test_content("placement").is_some());
        assert!(test_content("bipa7.test").is_some());
        assert!(lesson_content("bipa1.test").is_none());
    }

    #[test]
    fn every_embedded_lesson_parses_and_validates() {
        assert!(LESSONS.rows.len() >= 100, "only {} lessons embedded", LESSONS.rows.len());
        let mut problems = Vec::new();
        for (id, yaml) in embedded::all(LESSONS) {
            match parse_lesson(&yaml) {
                Ok(lesson) => {
                    if lesson.id != *id {
                        problems.push(format!("{id}: id is {:?}", lesson.id));
                    }
                    problems.extend(validate(&lesson).iter().map(|i| format!("{id}: {i}")));
                }
                Err(e) => problems.push(format!("{id}: {e}")),
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
        assert!(lesson_content("bipa1.u01.l01").is_some());
        assert!(lesson_content("bipa2.u10.l05").is_some());
        assert!(lesson_content("bipa7.u10.l14").is_some());
        assert!(lesson_content("bipa8.u01.l01").is_none());
    }

    #[test]
    fn a_lesson_game_opens_its_play_screen() {
        use crate::play::Route;
        assert_eq!(game_route("uu.grandmother"), Some(Route::Unggah));
        assert_eq!(game_route("mp.wedi-wedhi"), Some(Route::Pairs));
        assert_eq!(game_route("pasar"), None);
    }

    #[test]
    fn bold_markup_is_split_out() {
        let parts = bold_parts("فيه كلمات **مشتركة** بين اللغتين");
        assert_eq!(
            parts,
            vec![(false, "فيه كلمات ".into()), (true, "مشتركة".into()), (false, " بين اللغتين".into())]
        );
        assert_eq!(bold_parts("**Aku**"), vec![(true, "Aku".into())]);
        assert_eq!(bold_parts("no markup"), vec![(false, "no markup".into())]);
        assert_eq!(bold_parts("a ** b"), vec![(false, "a ".into()), (false, "** b".into())]);
    }

    #[test]
    fn arabic_in_an_english_line_gets_its_own_isolate() {
        // #403: j1.u01.l01, English UI.
        let text = "ngoko between friends (like «إنت»), krama when being polite (like «حضرتك»).";
        let plain: Vec<String> = pron_units(latin_runs(text), "jv", false)
            .into_iter()
            .filter_map(|u| if let Unit::Plain(t) = u { Some(t) } else { None })
            .collect();
        let arabic: Vec<String> =
            plain.iter().flat_map(|t| arabic_runs(t)).filter(|(a, _)| *a).map(|(_, r)| r).collect();
        assert_eq!(arabic, ["إنت", "حضرتك"]);
        // Every character survives the split, quotes and commas outside.
        for t in &plain {
            assert_eq!(arabic_runs(t).into_iter().map(|(_, r)| r).collect::<String>(), *t);
        }
        assert_eq!(
            arabic_runs("«إنت»), "),
            [(false, "«".to_string()), (true, "إنت".to_string()), (false, "»), ".to_string())]
        );
        assert_eq!(arabic_runs("like «مع السلامة» here")[1], (true, "مع السلامة".to_string()));
        assert_eq!(arabic_runs("only English"), [(false, "only English".to_string())]);
    }

    #[test]
    fn latin_runs_isolate_target_text() {
        let runs = latin_runs("«Apa kabar?» يعني «إزيك؟»");
        assert_eq!(
            runs,
            vec![(false, "«".into()), (true, "Apa kabar?".into()), (false, "» يعني «إزيك؟»".into())]
        );
        let runs = latin_runs("selamat + وقت");
        assert_eq!(runs, vec![(true, "selamat".into()), (false, " + وقت".into())]);
        assert_eq!(latin_runs("سلام"), vec![(false, "سلام".into())]);
        let runs = latin_runs("بنقول sore. malam بتبدأ");
        assert_eq!(runs[1], (true, "sore".into()));
        assert_eq!(runs[3], (true, "malam".into()));
        assert_eq!(
            latin_runs("قول Selamat jalan, sampai jumpa!")[1],
            (true, "Selamat jalan, sampai jumpa!".into())
        );
        let runs = latin_runs("- Menurut X, … = على حسب X …");
        assert_eq!(runs[1], (true, "Menurut X, …".into()));
        assert_eq!(runs[3], (true, "X …".into()));
    }

    #[test]
    fn suffixes_keep_their_hyphen_and_arrows_follow_the_line() {
        let runs = latin_runs("aku و-ku = saya");
        assert_eq!(runs, vec![(true, "aku".into()), (false, " و".into()), (true, "-ku = saya".into()),]);
        // A hyphen inside a Latin word is still a hyphen, not a new run.
        assert_eq!(latin_runs("anak-anak"), vec![(true, "anak-anak".into())]);
        let runs = latin_runs("besar = كبير → lebih besar = أكبر");
        let plain: String =
            runs.iter().filter(|(l, _)| !l).map(|(_, t)| in_direction(t, true).into_owned()).collect();
        assert!(plain.contains('←') && !plain.contains('→'), "{plain}");
        assert_eq!(in_direction("a → b", false), "a → b");
    }

    #[test]
    fn pronunciation_follows_the_closing_quote() {
        let units = pron_units(latin_runs("«selamat jalan» لما تمشي"), "id", true);
        assert_eq!(units[0], Unit::Plain("«".into()));
        let Unit::Latin { phrases, close, short } = &units[1] else { panic!("{units:?}") };
        assert_eq!((phrases[0].text.as_str(), close.as_str(), *short), ("selamat jalan", "»", true));
        assert!(phrases[0].say);
        assert_eq!(units[2], Unit::Plain(" لما تمشي".into()));
    }

    #[test]
    fn mapping_lines_say_each_side_and_skip_indonesian_on_javanese_slides() {
        let units = pron_units(latin_runs("- Selamat pagi → Sugeng enjing (صباح الخير)"), "jv", true);
        let latin = units.iter().find_map(|u| match u {
            Unit::Latin { phrases, .. } => Some(phrases),
            Unit::Plain(_) => None,
        });
        let phrases = latin.unwrap();
        let said: Vec<(&str, bool)> = phrases.iter().map(|p| (p.text.as_str(), p.say)).collect();
        assert_eq!(said, [("Selamat pagi", false), ("Sugeng enjing", true)]);
        assert_eq!(phrases[0].sep, " → ");
        // On Indonesian slides both sides are said.
        let units = pron_units(vec![(true, "ora ↔ tidak".into())], "id", true);
        let Unit::Latin { phrases, .. } = &units[0] else { panic!() };
        assert!(phrases.iter().all(|p| p.say));
    }

    #[test]
    fn pronunciation_is_drawn_after_its_word_and_quote_outside_the_latin_isolate() {
        let units = pron_units(latin_runs("«selamat jalan» لما تمشي"), "id", true);
        let Unit::Latin { phrases, close, .. } = &units[1] else { panic!("{units:?}") };
        assert_eq!(
            layout(phrases, close, true),
            [Bit::Latin("selamat jalan".into()), Bit::Close("»".into()), Bit::Say("selamat jalan".into())]
        );
        let units = pron_units(latin_runs("- Selamat pagi → Sugeng enjing (صباح الخير)"), "jv", true);
        let Some(Unit::Latin { phrases, close, .. }) = units.iter().find(|u| matches!(u, Unit::Latin { .. }))
        else {
            panic!("{units:?}")
        };
        assert_eq!(
            layout(phrases, close, true),
            [
                Bit::Latin("Selamat pagi".into()),
                Bit::Sep(" ← ".into()),
                Bit::Latin("Sugeng enjing".into()),
                Bit::Say("Sugeng enjing".into()),
            ]
        );
        assert_eq!(layout(phrases, close, false)[1], Bit::Sep(" → ".into()));
    }

    #[test]
    fn brackets_around_mixed_text_form_one_group() {
        let pieces = bracket_groups(latin_runs("كلامك إنت للكبير (هو ngendika)"));
        assert_eq!(
            pieces,
            vec![
                Piece::Run(false, "كلامك إنت للكبير ".into()),
                Piece::Group(vec![(false, "(هو ".into()), (true, "ngendika".into()), (false, ")".into())]),
            ]
        );
        // Text after the group, and brackets that close inside one run, stay plain.
        let pieces = bracket_groups(latin_runs("(شنطة) tas (يعني bag) وخلاص"));
        assert_eq!(pieces[0], Piece::Run(false, "(شنطة) ".into()));
        assert_eq!(pieces[1], Piece::Run(true, "tas".into()));
        assert!(matches!(&pieces[3], Piece::Group(g) if g.len() == 3));
        assert_eq!(pieces[4], Piece::Run(false, " وخلاص".into()));
        // An unclosed bracket changes nothing.
        let pieces = bracket_groups(latin_runs("قول (apa kabar"));
        assert!(pieces.iter().all(|p| matches!(p, Piece::Run(..))));
        let text: String =
            pieces.iter().map(|p| if let Piece::Run(_, t) = p { t.as_str() } else { "" }).collect();
        assert_eq!(text, "قول (apa kabar");
    }

    #[test]
    fn latin_runs_keep_phrases_whole() {
        let latin = |s: &str| -> Vec<String> {
            latin_runs(s).into_iter().filter(|(l, _)| *l).map(|(_, r)| r).collect()
        };
        // bipa3.u02.l01: two Latin sentences inside an Arabic line.
        assert_eq!(
            latin("Ni Luh membawa tas. Tasnya dari kain = الشنطة (اللي اتكلمنا عنها) من القماش."),
            ["Ni Luh membawa tas. Tasnya dari kain"]
        );
        assert_eq!(
            latin("قول Ni Luh membawa tas. Tasnya dari kain. يعني"),
            ["Ni Luh membawa tas. Tasnya dari kain."]
        );
        // Affix hyphens stay with the word.
        assert_eq!(latin("بتاعه / الـ = -nya"), ["-nya"]);
        assert_eq!(latin("البادئة ber- بتعمل فعل"), ["ber-"]);
        assert_eq!(latin("- Menurut X"), ["Menurut X"]);
        // Javanese ↔ Indonesian chains read left to right as one run.
        assert_eq!(latin("قارن: ora ↔ tidak · wis ↔ sudah"), ["ora ↔ tidak · wis ↔ sudah"]);
        // A parenthesis opened inside the run closes inside it.
        assert_eq!(latin("pantainya (pantai + nya) يعني"), ["pantainya (pantai + nya)"]);
        // The Arabic sentence's own full stop stays Arabic.
        assert_eq!(latin("بنقول sore. malam بتبدأ"), ["sore", "malam"]);
    }

    #[test]
    fn draft_answers_follow_the_exercise_kind() {
        let lesson = parse_lesson(FIXTURE).unwrap();
        let ex = |id: &str| lesson.exercises.iter().find(|e| e.id == id).unwrap().clone();
        let d = Draft::default();
        assert_eq!(draft_answer(&ex("e01"), &d, &[]), None);
        assert_eq!(
            draft_answer(&ex("e01"), &Draft { choice: Some(1), ..d.clone() }, &[]),
            Some(Answer::Choice(1))
        );
        assert_eq!(draft_answer(&ex("e06"), &Draft { text: "  ".into(), ..d.clone() }, &[]), None);
        let tiles = vec!["jalan,".to_string(), "Selamat".to_string()];
        assert_eq!(
            draft_answer(&ex("e07"), &Draft { picked: vec![1, 0], ..d.clone() }, &tiles),
            Some(Answer::Text("Selamat jalan,".into()))
        );
        assert_eq!(draft_answer(&ex("e11"), &d, &[]), Some(Answer::Practiced));
        assert_eq!(draft_answer(&ex("e02"), &d, &[]), None, "match pairs check themselves");
    }

    /// Every kind's answer survives a save and a reopen, by content: the
    /// same option, tiles (repeats included), pairs and text come back.
    #[test]
    fn drafts_survive_saving_for_every_kind() {
        let lesson = parse_lesson(FIXTURE).unwrap();
        let player = Player::new(lesson.clone());
        let mut kinds = std::collections::HashSet::new();
        for ex in &lesson.exercises {
            let tiles = player.tiles(ex);
            let d = match &ex.kind {
                ExerciseKind::MatchPairs { .. } => {
                    Draft { matched: vec![1, 0], missed: vec!["pagi".into()], ..Draft::default() }
                }
                ExerciseKind::BuildSentence { .. } => {
                    Draft { picked: vec![tiles.len() - 1, 0], ..Draft::default() }
                }
                _ if option_count(ex) > 0 => Draft { choice: Some(option_count(ex) - 1), ..Draft::default() },
                _ => Draft { text: "Selamat sore".into(), ..Draft::default() },
            };
            kinds.insert(std::mem::discriminant(&ex.kind));
            let saved = save_draft(ex, &d, &tiles);
            let json = serde_json::to_string(&saved).unwrap();
            let back = restore_draft(ex, &serde_json::from_str(&json).unwrap(), &tiles);
            assert_eq!(back, d, "{}", ex.id);
        }
        assert!(kinds.len() >= 6, "the fixture covers {} kinds", kinds.len());
        // Repeated tiles each find their own slot; unknown pieces drop out.
        let ex =
            lesson.exercises.iter().find(|e| matches!(e.kind, ExerciseKind::BuildSentence { .. })).unwrap();
        let tiles = vec!["a".to_string(), "b".into(), "a".into()];
        let r = SavedResponse { tiles: vec!["a".into(), "a".into(), "zzz".into()], ..Default::default() };
        assert_eq!(restore_draft(ex, &r, &tiles).picked, vec![0, 2]);
    }

    /// A choice is kept by its text, so a reshuffled retry restores the
    /// same option, not the same slot.
    #[test]
    fn a_choice_is_kept_by_its_text() {
        let lesson = parse_lesson(FIXTURE).unwrap();
        let ex =
            lesson.exercises.iter().find(|e| matches!(e.kind, ExerciseKind::MultipleChoice { .. })).unwrap();
        let saved = save_draft(ex, &Draft { choice: Some(0), ..Draft::default() }, &[]);
        let view = kabar_core::lesson::retry_options(ex, 7).unwrap();
        let back = restore_draft(&view, &saved, &[]).choice.unwrap();
        assert_eq!(option_key(&view, back), option_key(ex, 0));
    }

    #[test]
    fn a_disabled_check_says_what_it_waits_for() {
        let lesson = parse_lesson(FIXTURE).unwrap();
        for ex in &lesson.exercises {
            for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
                assert!(!lt(lang, check_hint(ex)).is_empty());
            }
        }
    }
}
