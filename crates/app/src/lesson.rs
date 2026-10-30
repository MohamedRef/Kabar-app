//! The Lesson Player (SPEC §14): slides → exercises → retries → mini dialogue
//! → summary → souvenir. The step logic lives in `kabar_core::lesson::Player`;
//! this file draws it, plays audio and saves progress for Lanjut.
//!
//! The exercise views below are standalone components (exercise + `Draft`
//! signal in, events out) so level tests can reuse them without the player.

mod exercises;
pub use exercises::ExerciseView;
pub(crate) use exercises::TextAnswer;
use exercises::*;

mod outcome;
use outcome::*;

mod slides;
use slides::*;
pub(crate) use slides::{no_audio_msg, speaker_icon};

mod player;
pub use player::EpisodeQuiz;
use player::*;

mod reading;
use reading::*;
// Preserve the established feature facade, including callers in tests and other stages.
#[allow(unused_imports)]
pub(crate) use reading::{Rich, bold_parts};
#[allow(unused_imports)]
pub use reading::{arabic_runs, latin_runs};

mod decision;
mod notes;
use decision::{ChoiceMark, blank_hugs_next, choice_mark, show_voice_note, voice_note_step};
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
    crate::focus::use_focus_mode(true);
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
                    crate::ui::DivCard { class: "lp-notice",
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

/// The level a lesson id belongs to, as the Journey names it:
/// `bipa1.u01.l01` → "BIPA 1", `j3.u02.l04` → "J3".
fn level_label(id: &str) -> String {
    let level = id.split('.').next().unwrap_or(id);
    match level.strip_prefix("bipa") {
        Some(n) => format!("BIPA {n}"),
        None => level.to_uppercase(),
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

/// Given by a player that already says "no sound on this device" once at
/// the top (the test player, #447): small play buttons then drop out when
/// the sound is missing instead of repeating it beside every prompt.
#[derive(Clone, Copy)]
pub struct NoSoundBanner;

#[cfg(test)]
mod tests;
