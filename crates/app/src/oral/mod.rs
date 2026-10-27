//! Oral mode: the structured oral exam, hands-free. The app reads a question
//! aloud, gives Mohamed time to answer out loud, reads the model answer, and
//! he rates himself Again or Good. Again comes back later in the session.
//! No recording and no speech recognition: nothing leaves the device.

mod session;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use batna_core::review::{Grade, Origin, ReviewEvent};
use dioxus::prelude::*;

use crate::content::Library;
use crate::i18n::oral as tx;
use crate::platform::background::{self, Labels, Remote};
use crate::platform::tts;
use session::{Entry, Pick, Session};

const ORAL_CSS: &str = include_str!("oral.css");

/// Think-timer choices, in seconds (the brief's 10–30 s range).
const THINK_MIN: u32 = 10;
const THINK_MAX: u32 = 30;
const SIZES: [usize; 3] = [10, 20, 30];
/// With auto-continue on, how long the answer waits for a rating.
const AUTO_WAIT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, PartialEq)]
struct Settings {
    lecture: Option<String>,
    size: usize,
    think: u32,
    voice: bool,
    /// Index into [`tts::RATES`].
    rate: usize,
    auto_continue: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { lecture: None, size: 10, think: 15, voice: true, rate: 1, auto_continue: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Reading the question.
    Asking,
    /// The think timer: seconds left.
    Thinking(u32),
    /// "And what else?", then a shorter timer.
    WhatElse(u32),
    /// Reading the answer, then waiting for a rating.
    Answer,
}

// Kept across screen changes, for this run of the app. Chunk 3's store keeps
// the review events for good; until it lands they stay here.
static SETTINGS: GlobalSignal<Settings> = Signal::global(Settings::default);
static SESSION: GlobalSignal<Option<Session>> = Signal::global(|| None);
static EVENTS: GlobalSignal<Vec<ReviewEvent>> = Signal::global(Vec::new);

#[component]
pub fn Oral() -> Element {
    use_drop(tts::stop);
    let active = SESSION.read().as_ref().map(|s| s.current.is_some() || s.left() > 0);
    rsx! {
        style { {ORAL_CSS} }
        match active {
            None => rsx! { Setup {} },
            Some(true) => rsx! { Runner {} },
            Some(false) => rsx! { Summary {} },
        }
    }
}

fn start() {
    let settings = SETTINGS.peek().clone();
    let lib = Library::get();
    let pool = session::pool(lib.lectures.values(), settings.lecture.as_deref());
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    let mut s = Session::new(pool, &Pick { lecture: settings.lecture, size: settings.size, seed });
    s.advance();
    *SESSION.write() = Some(s);
}

#[component]
fn Setup() -> Element {
    let settings = SETTINGS.read().clone();
    let lib = Library::get();
    let lectures: Vec<(String, String)> = lib
        .index
        .lectures
        .iter()
        .filter(|e| lib.lectures.get(&e.id).is_some_and(|l| !session::pool([l], None).is_empty()))
        .map(|e| (e.id.clone(), e.title.clone()))
        .collect();
    let available = session::pool(lib.lectures.values(), settings.lecture.as_deref()).len();

    rsx! {
        section { class: "card oral-setup",
            h1 { {tx::TITLE} }
            p { class: "muted", {tx::INTRO} }
            if available > 0 {
                p { class: "muted small", "{available} {tx::AVAILABLE}" }
                button { class: "oral-start", onclick: move |_| start(), {tx::START} }
            }
        }
        if lectures.is_empty() {
            p { class: "note", {tx::EMPTY} }
        } else {
            section { class: "card oral-setup",
                label { class: "oral-field",
                    span { {tx::LECTURE} }
                    select {
                        onchange: move |e| {
                            let v = e.value();
                            SETTINGS.write().lecture = (!v.is_empty()).then_some(v);
                        },
                        option { value: "", selected: settings.lecture.is_none(), {tx::ALL_LECTURES} }
                        for (id, title) in lectures {
                            option {
                                key: "{id}",
                                value: "{id}",
                                selected: settings.lecture.as_deref() == Some(id.as_str()),
                                "{title}"
                            }
                        }
                    }
                }
                div { class: "oral-field",
                    span { {tx::SIZE} }
                    div { class: "segmented", role: "radiogroup",
                        for size in SIZES {
                            button {
                                key: "{size}",
                                role: "radio",
                                "aria-checked": "{settings.size == size}",
                                onclick: move |_| SETTINGS.write().size = size,
                                "{size}"
                            }
                        }
                    }
                }
                label { class: "oral-field",
                    span { "{tx::THINK}: {settings.think} {tx::SECONDS}" }
                    input {
                        r#type: "range",
                        min: "{THINK_MIN}",
                        max: "{THINK_MAX}",
                        step: "5",
                        value: "{settings.think}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                SETTINGS.write().think = v.clamp(THINK_MIN, THINK_MAX);
                            }
                        },
                    }
                }
                label { class: "oral-check",
                    input {
                        r#type: "checkbox",
                        checked: settings.voice,
                        onchange: move |e| SETTINGS.write().voice = e.checked(),
                    }
                    span { {tx::VOICE} }
                }
                if settings.voice {
                    div { class: "oral-field",
                        span { {tx::RATE} }
                        div { class: "segmented", role: "radiogroup",
                            for (i, name) in tx::RATES.iter().enumerate() {
                                button {
                                    key: "{i}",
                                    role: "radio",
                                    "aria-checked": "{settings.rate == i}",
                                    onclick: move |_| SETTINGS.write().rate = i,
                                    {*name}
                                }
                            }
                        }
                    }
                }
                label { class: "oral-check",
                    input {
                        r#type: "checkbox",
                        checked: settings.auto_continue,
                        onchange: move |e| SETTINGS.write().auto_continue = e.checked(),
                    }
                    span { {tx::AUTO} }
                }
                p { class: "muted small", {tx::AUTO_HINT} }
            }
        }
    }
}

/// What the driver should do next for the current question.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Ask,
    Reveal,
}

#[component]
fn Runner() -> Element {
    let mut phase = use_signal(|| Phase::Asking);
    let no_voice = use_signal(|| false);
    // Bumped by every new step, so a stale timer or voice stops on its own.
    let mut run = use_signal(|| 0u64);
    let mut asked_at = use_signal(Instant::now);
    // Keys work after a click too: a rating button vanishes with its
    // question, so focus goes back to the session for each step.
    let mut root = use_signal(|| None::<std::rc::Rc<MountedData>>);

    let mut drive = move |step: Step| {
        let id = *run.peek() + 1;
        run.set(id);
        tts::stop();
        if let Some(el) = root.peek().clone() {
            spawn(async move {
                let _ = el.set_focus(true).await;
            });
        }
        let Some(entry) = SESSION.peek().as_ref().and_then(|s| s.current.clone()) else { return };
        let Settings { think, voice, rate, auto_continue, .. } = SETTINGS.peek().clone();
        let rate = tts::RATES[rate.min(tts::RATES.len() - 1)];
        let say = move |text: String| speak(text, voice, rate, no_voice);
        spawn(async move {
            let live = move || *run.peek() == id;
            if step == Step::Ask {
                asked_at.set(Instant::now());
                phase.set(Phase::Asking);
                say(spoken_question(&entry)).await;
                if !live() || !countdown(think, Phase::Thinking, phase, live).await {
                    return;
                }
                if entry.prompt.what_else {
                    let more = (think / 2).max(THINK_MIN);
                    phase.set(Phase::WhatElse(more));
                    say("And what else?".into()).await;
                    if !live() || !countdown(more, Phase::WhatElse, phase, live).await {
                        return;
                    }
                }
            }
            phase.set(Phase::Answer);
            say(entry.prompt.spoken_answer()).await;
            if auto_continue && live() {
                tokio::time::sleep(AUTO_WAIT).await;
                if live() {
                    // Moving on changes the session, and the effect below asks the next one.
                    rate_current(None, *asked_at.peek());
                }
            }
        });
    };

    // Ask the first question, and the next one each time the session moves on.
    let asked_now = SESSION.read().as_ref().map_or(0, |s| s.asked);
    let mut asked = use_signal(|| 0u64);
    use_effect(use_reactive!(|asked_now| {
        if asked_now != 0 && *asked.peek() != asked_now {
            asked.set(asked_now);
            drive(Step::Ask);
        }
    }));

    // Screen off (Android): a foreground service keeps the session going,
    // shows it on the lock screen and passes on headset and notification buttons.
    use_hook(|| {
        background::start(&Labels { show: tx::SHOW, again: tx::AGAIN, good: tx::GOOD, end: tx::END });
    });
    use_drop(background::stop);
    let revealed_now = use_memo(move || *phase.read() == Phase::Answer);
    use_effect(move || {
        let revealed = revealed_now();
        let session = SESSION.read();
        let Some(entry) = session.as_ref().and_then(|s| s.current.as_ref()) else { return };
        let title = format!("{} · {} {}", tx::TITLE, tx::LEFT, session.as_ref().map_or(0, Session::left));
        let lines = if revealed { &entry.prompt.answer } else { &entry.prompt.question };
        background::update(&title, &lines.join(" · "), revealed);
    });
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let Some(remote) = background::take() else { continue };
            let revealed = revealed_now();
            match remote {
                Remote::End => end_session(),
                Remote::Again if revealed => rate_current(Some(Grade::Again), *asked_at.peek()),
                Remote::Main | Remote::Good if revealed => rate_current(Some(Grade::Good), *asked_at.peek()),
                Remote::Main | Remote::Good => drive(Step::Reveal),
                Remote::Again => {}
            }
        }
    });

    let session = SESSION.read();
    let Some(s) = session.as_ref() else { return rsx! {} };
    let Some(entry) = s.current.clone() else { return rsx! {} };
    let left = s.left();
    drop(session);
    let phase_now = *phase.read();
    let revealed = phase_now == Phase::Answer;
    let p = &entry.prompt;
    let lecture = p.lecture.rsplit('.').next().unwrap_or(&p.lecture);

    let rate = move |grade: Grade| {
        rate_current(Some(grade), *asked_at.peek());
        tts::stop();
    };

    rsx! {
        div {
            class: "oral-run",
            "data-phase": if revealed { "answer" } else { "ask" },
            tabindex: "0",
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
                root.set(Some(e.data()));
            },
            onkeydown: move |e| match e.key() {
                Key::Character(c) if c == " " && !revealed => drive(Step::Reveal),
                Key::Character(c) if c == "1" && revealed => rate(Grade::Again),
                Key::Character(c) if (c == "2" || c == " ") && revealed => rate(Grade::Good),
                _ => {}
            },
            header { class: "oral-top",
                span { class: "muted small", "{tx::LEFT} " bdi { "{left}" } }
                if entry.follow_up {
                    span { class: "chip accent", {tx::FOLLOW_UP} }
                }
                if entry.again {
                    span { class: "chip", {tx::AGAIN_BADGE} }
                }
                span { class: "muted small", bdi { "{lecture}" } " · {tx::PAGE} " bdi { "{p.page}" } }
                button {
                    class: "oral-end",
                    onclick: move |_| end_session(),
                    {tx::END}
                }
            }
            if *no_voice.read() {
                p { class: "note small", {tx::NO_VOICE} }
            }
            div { class: "oral-panes",
                section { class: "card oral-question",
                    h2 { class: "muted small", {tx::LISTEN} }
                    div { class: "en", dir: "ltr", lang: "en",
                        for (i, line) in p.question.iter().enumerate() {
                            p { key: "{i}", class: if i + 1 == p.question.len() { "oral-ask" } else { "" }, "{line}" }
                        }
                    }
                    if let Some((pass, of)) = p.pass {
                        span { class: "chip", "{tx::PASS} " bdi { "{pass}" } " {tx::OF} " bdi { "{of}" } }
                    }
                }
                match phase_now {
                    Phase::Asking => rsx! {
                        section { class: "oral-timer", "aria-live": "polite",
                            span { class: "oral-status", {tx::LISTEN} }
                        }
                    },
                    Phase::Thinking(n) | Phase::WhatElse(n) => {
                        let what_else = matches!(phase_now, Phase::WhatElse(_));
                        let full = if what_else { (SETTINGS.peek().think / 2).max(THINK_MIN) } else { SETTINGS.peek().think };
                        let pct = 100 * n / full.max(1);
                        rsx! {
                            section { class: "oral-timer", "aria-live": "polite",
                                div { class: "oral-ring", style: "--p: {pct}", span { "{n}" } }
                                span { class: "oral-status",
                                    if what_else { {tx::WHAT_ELSE} } else { {tx::THINKING} }
                                }
                            }
                        }
                    }
                    Phase::Answer => rsx! {
                        section { class: "card oral-answer",
                            h2 { class: "small", {tx::ANSWER} }
                            ul { class: "en", dir: "ltr", lang: "en",
                                for (i, line) in p.answer.iter().enumerate() {
                                    li { key: "{i}", "{line}" }
                                }
                            }
                        }
                    },
                }
            }
            div { class: "oral-actions",
                if revealed {
                    button { class: "oral-big again", onclick: move |_| rate(Grade::Again),
                        span { class: "big-label", {tx::AGAIN} }
                        span { class: "big-hint", {tx::AGAIN_HINT} }
                    }
                    button { class: "oral-big good", onclick: move |_| rate(Grade::Good),
                        span { class: "big-label", {tx::GOOD} }
                        span { class: "big-hint", {tx::GOOD_HINT} }
                    }
                } else {
                    button { class: "oral-small", onclick: move |_| drive(Step::Ask), {tx::REPEAT} }
                    button { class: "oral-big show", onclick: move |_| drive(Step::Reveal),
                        span { class: "big-label", {tx::SHOW} }
                    }
                }
            }
        }
    }
}

/// Counts down `secs`, showing each second. `false` if the step was replaced.
async fn countdown(
    secs: u32,
    show: fn(u32) -> Phase,
    mut phase: Signal<Phase>,
    live: impl Fn() -> bool,
) -> bool {
    for n in (1..=secs).rev() {
        phase.set(show(n));
        tokio::time::sleep(Duration::from_secs(1)).await;
        if !live() {
            return false;
        }
    }
    true
}

/// Says `text` when the voice is on and working; on a missing voice, turns
/// it off for the session and shows why (the text is on screen anyway).
async fn speak(text: String, voice: bool, rate: f32, mut no_voice: Signal<bool>) {
    if !voice || *no_voice.peek() {
        return;
    }
    if let Err(e) = tts::say(&text, rate).await {
        if !matches!(e, tts::SpeakError::Failed(_)) {
            no_voice.set(true);
        }
        eprintln!("oral: speech failed: {e:?}");
    }
}

/// The question as the examiner says it.
fn spoken_question(entry: &Entry) -> String {
    let lead = if entry.follow_up {
        "Following on from that. "
    } else if entry.again {
        "Once more. "
    } else {
        ""
    };
    format!("{lead}{}", entry.prompt.spoken_question())
}

/// Stops early; the summary shows what was done.
fn end_session() {
    tts::stop();
    if let Some(s) = SESSION.write().as_mut() {
        s.end();
    }
}

/// Records the rating (none when heard without one) and moves on.
fn rate_current(grade: Option<Grade>, asked_at: Instant) {
    let mut guard = SESSION.write();
    let Some(s) = guard.as_mut() else { return };
    let Some(entry) = &s.current else { return };
    if let Some(grade) = grade {
        let at_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        EVENTS.write().push(ReviewEvent {
            item_id: entry.prompt.item_id.clone(),
            grade,
            origin: Origin::Oral,
            at_ms,
            took_ms: asked_at.elapsed().as_millis().min(u32::MAX as u128) as u32,
        });
    }
    s.rate(grade);
    s.advance();
}

#[component]
fn Summary() -> Element {
    let session = SESSION.read();
    let Some(s) = session.as_ref() else { return rsx! {} };
    let t = s.tally.clone();
    drop(session);
    rsx! {
        section { class: "card oral-summary",
            h1 { {tx::DONE} }
            dl { class: "oral-tally",
                div { dt { {tx::DONE_GOOD} } dd { "{t.good}" } }
                div { dt { {tx::DONE_AGAIN} } dd { "{t.again}" } }
                if t.heard > 0 {
                    div { dt { {tx::DONE_HEARD} } dd { "{t.heard}" } }
                }
            }
        }
        section { class: "card",
            h2 { {tx::MISSED} }
            if t.missed.is_empty() {
                p { class: "muted", {tx::NONE_MISSED} }
            } else {
                ul { class: "en oral-missed", dir: "ltr", lang: "en",
                    for p in t.missed {
                        li { key: "{p.item_id}",
                            span { "{p.question.last().cloned().unwrap_or_default()}" }
                            span { class: "muted small", " (p. {p.page})" }
                        }
                    }
                }
            }
        }
        div { class: "oral-actions",
            button { class: "oral-small", onclick: move |_| *SESSION.write() = None, {tx::BACK} }
            button { class: "oral-big good", onclick: move |_| start(),
                span { class: "big-label", {tx::ANOTHER} }
            }
        }
    }
}
