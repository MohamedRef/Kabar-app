//! The Lesson Player (SPEC §14): slides → exercises → retries → mini dialogue
//! → summary → souvenir. The step logic lives in `kabar_core::lesson::Player`;
//! this file draws it, plays audio and saves progress for Lanjut.
//!
//! The exercise views below are standalone components (exercise + `Draft`
//! signal in, events out) so level tests can reuse them without the player.

mod text;

use dioxus::prelude::*;
use kabar_core::lesson::{
    Answer, Grade, Player, Step, display_answer, parse_lesson, pick, references, validate,
};
use kabar_core::locale::{UiLanguage, eastern_arabic_digits, format_number};
use kabar_protocol::lesson::{
    Dialogue, Direction, Exercise, ExerciseKind, LessonContent, LessonPosition, LessonStep, Localized,
};

use self::text::{L, lt};
use crate::{Db, Ui};

const LESSON_CSS: &str = include_str!("../assets/lesson.css");

/// Lesson files built into the app. One table, so the lead can point it at
/// `content/lessons/` (or a content pack) later without touching the player.
const LESSONS: &[(&str, &str)] = &[
    ("bipa1.u01.l01", include_str!("../../../fixtures/lessons/bipa1.u01.l01.yaml")),
    ("bipa1.u01.l02", include_str!("../../../fixtures/lessons/bipa1.u01.l02.yaml")),
];

/// The YAML of a lesson, if this build ships it.
pub fn lesson_content(id: &str) -> Option<&'static str> {
    LESSONS.iter().find(|(lesson, _)| *lesson == id).map(|(_, yaml)| *yaml)
}

/// Idle gaps longer than this don't count as lesson time.
const MAX_STEP_MS: u64 = 120_000;

fn now_ms() -> u64 {
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

    let body = match lesson_content(&id).map(parse_lesson) {
        None => rsx! {
            Notice { title: lt(lang, L::PendingTitle), body: lt(lang, L::PendingBody).to_string(), id: id.clone() }
        },
        Some(Err(e)) => rsx! {
            Notice { title: lt(lang, L::BrokenTitle), body: e.to_string(), id: id.clone() }
        },
        Some(Ok(lesson)) => {
            let issues = validate(&lesson);
            if issues.is_empty() {
                rsx! { PlayerView { key: "{id}", lesson } }
            } else {
                let list = issues.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
                rsx! { Notice { title: lt(lang, L::BrokenTitle), body: list, id: id.clone() } }
            }
        }
    };
    rsx! {
        style { {LESSON_CSS} }
        {body}
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
                button { class: "lp-close", aria_label: lt(lang, L::Close), onclick: move |_| ui.close_lesson(), "×" }
            }
            div { class: "lp-body",
                div { class: "lp-col",
                    div { class: "card lp-notice",
                        h2 { "{title}" }
                        p { class: "pre", "{body}" }
                        p { class: "muted mono", bdi { "{id}" } }
                        button { class: "primary", onclick: move |_| ui.close_lesson(), "{lt(lang, L::BackToJourney)}" }
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

/// BCP 47 tag for target-language text (the IME and the voice follow it).
fn target_tag(language: &str) -> &'static str {
    if language == "jv" { "jv" } else { "id" }
}

fn instruction(ex: &Exercise, language: &str) -> L {
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
        K::Listen { .. } => L::ListenPick,
        K::FillBlank { .. } => L::FillBlank,
        K::Dictation { .. } => L::Dictation,
        K::Speaking { .. } => L::Speaking,
    }
}

/// The text an exercise plays on its own when it opens.
fn autoplay_text(ex: &Exercise) -> Option<(String, Option<String>)> {
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
/// content has one (Azure voices, SPEC §20). Until the TTS pipeline lands we
/// fall back to the WebView's `speechSynthesis` (id-ID; Javanese asks for
/// jv-ID and falls back to an id-ID voice, which reads Javanese passably).
/// Android's system WebView has no `speechSynthesis`, so there it reports
/// "no audio" until the Kotlin TextToSpeech bridge or packaged audio exists.
pub fn speak(text: &str, language: &str, slow: bool, audio: Option<&str>, mut missing: Signal<bool>) {
    let lang = if language == "jv" { "jv-ID" } else { "id-ID" };
    let js = format!(
        r#"
        const text = {text}, lang = {lang}, rate = {rate}, file = {file};
        const tts = () => {{
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
                a.playbackRate = rate;
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
        file = serde_json::to_string(&audio).unwrap_or_else(|_| "null".into()),
    );
    spawn(async move {
        let mut eval = document::eval(&js);
        if let Ok(false) = eval.recv::<bool>().await {
            missing.set(true);
        }
    });
}

/// A soft two-note chime for a right answer (synthesized; no sound files).
fn chime() {
    document::eval(
        r#"
        try {
            const C = window.AudioContext || window.webkitAudioContext;
            const c = new C(), t = c.currentTime;
            [[659.25, 0], [987.77, 0.08]].forEach(([f, d]) => {
                const o = c.createOscillator(), g = c.createGain();
                o.type = "sine"; o.frequency.value = f;
                g.gain.setValueAtTime(0.0001, t + d);
                g.gain.exponentialRampToValueAtTime(0.08, t + d + 0.015);
                g.gain.exponentialRampToValueAtTime(0.0001, t + d + 0.3);
                o.connect(g).connect(c.destination);
                o.start(t + d); o.stop(t + d + 0.35);
            });
            setTimeout(() => c.close(), 700);
        } catch (e) {}
        "#,
    );
}

// ---------------------------------------------------------------------------
// Bidi helpers
// ---------------------------------------------------------------------------

fn is_latin_letter(c: char) -> bool {
    c.is_alphabetic() && (c as u32) < 0x0250
}

/// Splits mixed text into (is_latin, run) so Latin snippets inside Arabic
/// explanations get their own `<bdi>` (SPEC §9).
pub fn latin_runs(s: &str) -> Vec<(bool, String)> {
    let mut out: Vec<(bool, String)> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if is_latin_letter(chars[i]) {
            let start = i;
            let mut end = i;
            while i < chars.len() {
                let c = chars[i];
                // Letters and digits extend the run; ? and ! join it only
                // straight after a letter ("Apa kabar?").
                if is_latin_letter(c) || c.is_ascii_digit() || ("?!".contains(c) && end == i) {
                    end = i + 1;
                } else if c == '.' && chars.get(i + 1).is_none_or(|n| n.is_whitespace()) {
                    // A full stop ends the Arabic sentence, not the Latin run.
                    break;
                } else if !" -'’.,…/".contains(c) {
                    break;
                }
                i += 1;
            }
            out.push((true, chars[start..end].iter().collect()));
            i = end;
        } else {
            let start = i;
            while i < chars.len() && !is_latin_letter(chars[i]) {
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

/// Explanation text with Latin runs isolated.
#[component]
fn Rich(text: String, target: &'static str) -> Element {
    rsx! {
        for (i, (latin, run)) in latin_runs(&text).into_iter().enumerate() {
            if latin {
                bdi { key: "{i}", lang: target, class: "latin", "{run}" }
            } else {
                span { key: "{i}", "{run}" }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The player
// ---------------------------------------------------------------------------

#[component]
fn PlayerView(lesson: LessonContent) -> Element {
    let ui = use_context::<Ui>();
    let db_ctx = use_context::<Db>();
    let db = use_hook(|| CopyValue::new(db_ctx));
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;

    let mut player = use_signal(|| {
        let saved = db.read().0.borrow().position().ok().flatten();
        match saved {
            Some(p) if p.lesson_id == lesson.id && p.step != LessonStep::Summary => {
                Player::resume(lesson.clone(), &p)
            }
            _ => Player::new(lesson.clone()),
        }
    });
    let draft = use_signal(Draft::default);
    let last = use_signal(now_ms);
    let revealed = use_signal(|| 1usize);
    let audio_missing = use_signal(|| false);
    let mut finished = use_signal(|| false);

    // Save where we are on open (Lanjut) and play the first exercise's audio.
    use_hook(move || {
        let p = player.peek();
        save(db, &p.position());
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
        let mut revealed = revealed;
        revealed.set(1);
        let p = player.peek();
        if p.step() == Step::Summary {
            if !*finished.peek() {
                finished.set(true);
                let db = db.read();
                let mut store = db.0.borrow_mut();
                if let Err(e) = store.record_lesson_event(&p.lesson().id, "completed", &p.completed()) {
                    eprintln!("kabar: saving the lesson result failed: {e}");
                }
                if let Err(e) = store.clear_position() {
                    eprintln!("kabar: clearing the lesson position failed: {e}");
                }
            }
        } else {
            save(db, &pos);
        }
        if let Some((text, audio)) = p.exercise().and_then(autoplay_text) {
            speak(&text, &p.lesson().language, false, audio.as_deref(), audio_missing);
        }
        if p.step() == Step::Dialogue
            && let Some(line) = p.lesson().dialogue.as_ref().and_then(|d| d.lines.first())
        {
            speak(&line.text, &p.lesson().language, false, None, audio_missing);
        }
    };

    let mut check = move |answer: Answer| {
        let g = player.write().check(&answer, lang);
        if matches!(g, Grade::Correct | Grade::Almost { .. }) {
            chime();
        }
        let p = player.peek();
        save(db, &p.position());
    };

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
                    if let Some(line) = p.lesson().dialogue.as_ref().and_then(|d| d.lines.get(shown)) {
                        speak(&line.text, &p.lesson().language, false, None, audio_missing);
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
            Step::Summary => ui.close_lesson(),
        }
    };

    // Keys are caught on the document, not on the player element: a tapped
    // tile or option can be disabled under the focus, and the next Enter must
    // still check. Enter = check/continue (Shift+Enter is a new line in text
    // answers), 1–4 = pick an option. Buttons in the feedback sheet and the
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
                } else if (!typing && /^[1-4]$/.test(e.key)) {
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
                    let count = player.peek().exercise().map_or(0, |ex| match &ex.kind {
                        ExerciseKind::MultipleChoice { options, options_meaning, .. } => {
                            options.len().max(options_meaning.len())
                        }
                        ExerciseKind::Listen { options, .. } | ExerciseKind::FillBlank { options, .. } => {
                            options.len()
                        }
                        _ => 0,
                    });
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

    let p = player.read();
    let l = p.lesson();
    let target = target_tag(&l.language);
    let step = p.step();
    let pct = (p.fraction() * 100.0).round();
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
        (Step::Summary, _) => (lt(lang, L::BackToJourney), true),
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
    drop(p);

    rsx! {
        div { class: "lp",
            div { class: "lp-top",
                button {
                    class: "lp-close",
                    aria_label: lt(lang, L::Close),
                    title: lt(lang, L::Close),
                    onclick: move |_| ui.close_lesson(),
                    "×"
                }
                div {
                    class: "lp-progress",
                    role: "progressbar",
                    aria_valuemin: "0",
                    aria_valuemax: "100",
                    aria_valuenow: "{pct}",
                    div { class: "lp-bar", style: "inline-size: {pct}%" }
                }
            }
            div { class: "lp-body",
                div { class: "lp-col", key: "{step:?}",
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
                            SummaryView { lesson: lesson.clone(), summary_score: summary.score, summary_xp: summary.xp, duration_ms: summary.duration_ms, mistakes: summary.mistakes.clone(), perfect: summary.graded > 0 && summary.correct == summary.graded }
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
                                    chime();
                                }
                                save(db, &player.peek().position());
                            },
                        }
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
                            onclick: move |_| primary(),
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}

fn save(db: CopyValue<Db>, pos: &LessonPosition) {
    if let Err(e) = db.read().0.borrow_mut().save_position(pos) {
        eprintln!("kabar: saving the lesson position failed: {e}");
    }
}

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
                span { class: "muted lp-small", "{lt(lang, L::NoAudio)}" }
            }
        }
    }
}

fn speaker_icon() -> Element {
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

#[component]
fn SlideView(lesson: LessonContent, index: usize, missing: Signal<bool>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&lesson.language);
    let Some(slide) = lesson.slides.get(index).cloned() else { return rsx! {} };
    let language = lesson.language.clone();
    rsx! {
        article { class: "lp-slide lp-enter",
            if index == 0 {
                p { class: "lp-lesson-title",
                    bdi { lang: target, "{lesson.title}" }
                    if !lesson.review.verified {
                        span { class: "pill lp-draft", "{lt(lang, L::NotReviewed)}" }
                    }
                }
                p { class: "muted", Rich { text: pick(&lesson.can_do, lang).to_string(), target } }
            }
            if let Some(title) = &slide.title {
                h2 { class: "lp-slide-title", bdi { lang: target, "{title}" } }
            }
            p { class: "lp-slide-body pre", Rich { text: pick(&slide.body, lang).trim().to_string(), target } }
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
                                    bdi { lang: target, class: "latin lp-target", "{ex.text}" }
                                    span { class: "muted", Rich { text: pick(&ex.meaning, lang).to_string(), target } }
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
                h2 { class: "lp-slide-title", bdi { lang: target, "{t}" } }
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
                                let (t, l) = (line.text.clone(), language.clone());
                                move |_| speak(&t, &l, false, None, missing)
                            },
                            span { class: "lp-speaker", bdi { "{line.speaker}" } }
                            bdi { lang: target, class: "latin lp-target", "{line.text}" }
                            span { class: "muted", Rich { text: pick(&line.meaning, lang).to_string(), target } }
                        }
                    }
                }
            }
        }
    }
}

fn duration_text(ms: u64, lang: UiLanguage, eastern: bool) -> String {
    let s = ms / 1000;
    let text = format!("{}:{:02}", s / 60, s % 60);
    if eastern && lang != UiLanguage::En { eastern_arabic_digits(&text) } else { text }
}

#[component]
fn SummaryView(
    lesson: LessonContent,
    summary_score: u32,
    summary_xp: u32,
    duration_ms: u64,
    mistakes: Vec<String>,
    perfect: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;
    let eastern = settings.eastern_digits;
    let target = target_tag(&lesson.language);
    let num = |n: u32| format_number(u64::from(n), lang, eastern);
    rsx! {
        article { class: "lp-summary",
            h2 { class: "lp-done reveal", "{lt(lang, L::Done)}" }
            p { class: "lp-lesson-title reveal", bdi { lang: target, "{lesson.title}" } }
            if perfect {
                p { class: "ok reveal", "{lt(lang, L::Perfect)}" }
            }
            dl { class: "lp-stats reveal d2",
                div { dt { "{lt(lang, L::Score)}" } dd { bdi { "{num(summary_score)}%" } } }
                div { dt { "{lt(lang, L::Xp)}" } dd { bdi { "+{num(summary_xp)}" } } }
                div { dt { "{lt(lang, L::Time)}" } dd { bdi { "{duration_text(duration_ms, lang, eastern)}" } } }
            }
            if !lesson.new_words.is_empty() {
                section { class: "card reveal d2",
                    h3 { "{lt(lang, L::NewWords)}" }
                    ul { class: "lp-words",
                        for w in lesson.new_words.iter() {
                            li { key: "{w.word}", class: if w.arabic.is_some() { "bridge-row" } else { "" },
                                bdi { lang: target, class: if w.arabic.is_some() { "latin bridge-word" } else { "latin" }, "{w.word}" }
                                span { class: "muted", Rich { text: pick(&w.meaning, lang).to_string(), target } }
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
                            bdi { key: "{m}", lang: target, class: "pill latin", "{m}" }
                        }
                    }
                }
            }
            if let Some(s) = &lesson.souvenir {
                section { class: "card lp-souvenir reveal d3",
                    p { class: "muted lp-small", "{lt(lang, L::Souvenir)}" }
                    h3 { bdi { lang: target, "{s.title}" } }
                    p { Rich { text: pick(&s.note, lang).to_string(), target } }
                }
            }
        }
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
                        p { Rich { text: lt(lang, L::CorrectWord).replace("{w}", w), target } }
                    }
                },
                Grade::Practiced => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::Practiced)}" }
                },
                Grade::Almost { expected } => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::Almost)}" }
                    p { bdi { lang: target, class: "lp-answer latin", "{expected}" } }
                },
                Grade::Wrong => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::NotYet)}" }
                    p { bdi { lang: answer_tag, class: "lp-answer", "{answer}" } }
                    if !retry {
                        p { class: "muted lp-small", "{lt(lang, L::AgainAtEnd)}" }
                    }
                },
                Grade::NeedsReview => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::NeedsReview)}" }
                    ul { class: "lp-refs",
                        for r in references(&ex, lang).into_iter().take(3) {
                            li { key: "{r}", bdi { lang: answer_tag, class: "lp-answer", "{r}" } }
                        }
                    }
                    p { "{lt(lang, L::SameMeaning)}" }
                    div { class: "row",
                        button { class: "primary", onclick: move |_| on_assess.call(true), "{lt(lang, L::YesSame)}" }
                        button { onclick: move |_| on_assess.call(false), "{lt(lang, L::NotQuite)}" }
                    }
                },
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
                            p { class: "lp-question", bdi { lang: target, class: "latin", "{question}" } }
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
                            p { class: "lp-question", bdi { lang: target, class: "latin", "{t}" } }
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
        K::Listen { text, audio, options, answer, .. } => rsx! {
            div { class: "lp-prompt center",
                PlayButton { text, language: language.clone(), audio, missing, big: true }
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
            rsx! {
                p { class: "lp-question lp-blank-line", lang: target, dir: "ltr",
                    "{before}"
                    if options.is_empty() {
                        input {
                            class: "lp-blank-input",
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
                        span { class: if chosen.is_some() { "lp-blank filled" } else { "lp-blank" },
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
                p { class: "lp-question big", bdi { lang: target, class: "latin", "{text}" } }
                p { class: "muted", Rich { text: pick(&meaning, lang).to_string(), target } }
                PlayButton { text: text.clone(), language: language.clone(), audio: None, missing, big: true }
                p { class: "muted lp-small", "{lt(lang, L::SpeakingLater)}" }
            }
        },
    }
}

/// Option buttons (keys 1–4 pick them too). After checking, the right one is
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
    let chosen = draft.read().choice;
    rsx! {
        div { class: if chips { "lp-choices chips" } else { "lp-choices" }, role: "group",
            for (i, label) in labels.iter().enumerate() {
                button {
                    key: "{i}",
                    class: match (locked, i == answer, chosen == Some(i)) {
                        (true, true, _) => "lp-choice right",
                        (true, false, true) => "lp-choice wrong",
                        _ => "lp-choice",
                    },
                    aria_pressed: if chosen == Some(i) { "true" } else { "false" },
                    disabled: locked,
                    onclick: move |_| draft.write().choice = Some(i),
                    if !chips {
                        span { class: "lp-key", aria_hidden: "true", "{i + 1}" }
                    }
                    bdi { lang: tag, "{label}" }
                }
            }
        }
    }
}

/// Free text: typed, Samsung handwriting keyboard, or DeX keyboard. `lang`
/// tells the IME which language to expect. Enter checks (the player's key
/// listener); Shift+Enter is a
/// new line.
#[component]
fn TextAnswer(tag: &'static str, draft: Signal<Draft>, locked: bool) -> Element {
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
        div { class: "lp-answer-line", lang: target, dir: "ltr",
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
        div { class: "lp-bank", lang: target, dir: "ltr",
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
    let d = draft.read().clone();
    let words: Vec<String> = pairs.iter().map(|p| p.0.clone()).collect();
    let state = |sel: Option<usize>, i: usize, wrong: Option<usize>| {
        if d.matched.contains(&i) {
            "lp-pair done"
        } else if wrong == Some(i) {
            "lp-pair wrong"
        } else if sel == Some(i) {
            "lp-pair selected"
        } else {
            "lp-pair"
        }
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
                        onclick: {
                            let words = words.clone();
                            move |_| {
                                draft.write().left = Some(i);
                                try_match(draft, &words, on_matched);
                            }
                        },
                        bdi { lang: target, class: "latin", "{word}" }
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
                        onclick: {
                            let words = words.clone();
                            move |_| {
                                draft.write().right = Some(i);
                                try_match(draft, &words, on_matched);
                            }
                        },
                        "{pairs[i].1}"
                    }
                }
            }
        }
    }
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

    #[test]
    fn every_embedded_lesson_parses_and_validates() {
        for (id, yaml) in LESSONS {
            let lesson = parse_lesson(yaml).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(&lesson.id, id);
            assert_eq!(validate(&lesson), vec![], "{id}");
        }
        assert!(lesson_content("bipa1.u01.l01").is_some());
        assert!(lesson_content("bipa7.u10.l05").is_none());
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
    }

    #[test]
    fn draft_answers_follow_the_exercise_kind() {
        let lesson = parse_lesson(LESSONS[0].1).unwrap();
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
}
