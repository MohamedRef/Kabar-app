//! Review screens (SPEC §16): classic cards, the Card Wall, and one-minute
//! bursts, on top of `kabar_core::review` (queues, stats) and the FSRS
//! scheduler. Card state is always derived from the review log.

mod outcome;
use outcome::*;

mod burst;
use burst::*;

mod wall;
use wall::*;

mod classic;
use classic::*;

mod labels;
use labels::*;

mod engine;
#[cfg(test)]
use engine::*;
pub(crate) use engine::{deck, due_words, load_deck, note_evidence};

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;
use kabar_core::review::{
    AnswerMatch, BURST_MS, Deck, QueueScope, Session, Span, Summary, cloze, compare_answer, is_quick,
    seed_bridge_deck, seed_words, wall_rating,
};
use kabar_core::srs::{Pile, Scheduler};
use kabar_protocol::coach::ActivityKind;
use kabar_protocol::lesson::Localized;
use kabar_protocol::srs::{Card, CardKind, MeaningShift, Rating, ReviewMode, StudyLanguage};
use serde::Deserialize;

use crate::Screen;
use crate::annotated::Annotated;
use crate::coach::Coach;
use crate::embedded::{Css, css};
use crate::i18n::Msg;
use crate::platform::{self, SpeakError};
use crate::pron::{Pron, Say};
use crate::store::Store;
use crate::{Db, Ui};

const REVIEW_CSS: Css = css!("assets/review.css");
const WALL_JS: &str = include_str!("../assets/wall.js");

/// Classic-review keys, from anywhere on the page: Space reveals, 1–4 rate,
/// Ctrl/⌘+Z undoes. Never while typing an answer. Space is taken over on
/// buttons too, so it can't re-press a focused button by accident.
const KEYS_JS: &str = r#"
if (window.__kabarKeys) window.__kabarKeys();
const typing = (t) =>
  t && (t.tagName === "TEXTAREA" || (t.tagName === "INPUT" && !t.readOnly) || t.isContentEditable);
const down = (e) => {
  if (!document.getElementById("review-classic")) return off();
  if (e.altKey || e.repeat || typing(e.target)) return;
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
    e.preventDefault();
    return dioxus.send("undo");
  }
  if (e.ctrlKey || e.metaKey) return;
  if (e.key === " " || /^[1-4]$/.test(e.key)) {
    e.preventDefault();
    dioxus.send(e.key);
  }
};
const up = (e) => {
  if (e.key === " " && !typing(e.target) && document.getElementById("review-classic")) e.preventDefault();
};
document.addEventListener("keydown", down);
document.addEventListener("keyup", up);
function off() {
  document.removeEventListener("keydown", down);
  document.removeEventListener("keyup", up);
  if (window.__kabarKeys === off) window.__kabarKeys = null;
}
window.__kabarKeys = off;
await new Promise(() => {});
"#;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Home,
    Classic,
    Wall,
    Burst,
    Done,
}

/// Review state shared by the review views.
#[derive(Clone)]
struct Engine {
    db: Db,
    coach: Coach,
    sched: Rc<Scheduler>,
    deck: Signal<Deck>,
    session: Signal<Option<Session>>,
    view: Signal<View>,
    summary: Signal<Option<Summary>>,
    error: Signal<Option<Msg>>,
}

impl PartialEq for Engine {
    fn eq(&self, other: &Self) -> bool {
        self.db == other.db && Rc::ptr_eq(&self.sched, &other.sched)
    }
}

// ---------------------------------------------------------------- text helpers

fn localized<'a>(ui: &Ui, l: &'a Localized) -> &'a str {
    match ui.settings.read().ui_language {
        UiLanguage::ArEg => &l.ar_eg,
        UiLanguage::Ar => &l.ar,
        UiLanguage::En => &l.en,
    }
}

/// Text whose Latin snippets are marked with FSI…PDI (U+2068…U+2069), with
/// each snippet rendered as a real `<bdi>`: not every WebView honors the
/// isolate characters themselves.
#[component]
pub(crate) fn Isolated(text: String) -> Element {
    let mut parts: Vec<(bool, String)> = Vec::new();
    for (i, chunk) in text.split('\u{2068}').enumerate() {
        match chunk.split_once('\u{2069}') {
            Some((latin, rest)) if i > 0 => {
                parts.push((true, latin.to_string()));
                parts.push((false, rest.to_string()));
            }
            _ => parts.push((false, chunk.to_string())),
        }
    }
    rsx! {
        for (i, (latin, part)) in parts.into_iter().enumerate() {
            if latin {
                bdi { key: "{i}", class: "latin", dir: "ltr", "{part}" }
            } else {
                span { key: "{i}", "{part}" }
            }
        }
    }
}

/// Speaks a word with the device's own voices: the free fallback until
/// cached Azure audio exists (SPEC §20). Android uses the system
/// TextToSpeech engine (its WebView has no `speechSynthesis`); elsewhere the
/// WebView speaks. Silent where unsupported.
fn speak(text: &str, language: StudyLanguage) {
    match platform::speak(text, language.speech_tag(), false) {
        Err(SpeakError::UseWebView) => {}
        Ok(()) => return,
        Err(e) => {
            crate::diag::report!("no review audio: {e:?}");
            return;
        }
    }
    let payload = serde_json::json!({ "text": text, "lang": language.speech_tag() });
    document::eval(&format!(
        r#"const p = {payload};
        if ("speechSynthesis" in window) {{
          const synth = window.speechSynthesis;
          synth.cancel();
          const u = new SpeechSynthesisUtterance(p.text);
          u.lang = p.lang;
          u.rate = 0.9;
          const voices = synth.getVoices();
          const norm = (l) => l.replace("_", "-").toLowerCase();
          const v = voices.find((v) => norm(v.lang) === p.lang.toLowerCase())
            || voices.find((v) => norm(v.lang).startsWith(p.lang.slice(0, 2)));
          if (v) u.voice = v;
          synth.speak(u);
        }}"#
    ));
}

// ---------------------------------------------------------------- screen

#[component]
pub fn ReviewScreen() -> Element {
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let sched = use_hook(|| crate::schedule::scheduler(&coach, &db.0.borrow()));
    let deck = use_signal({
        let (db, sched) = (db.clone(), sched.clone());
        move || load_deck(&db, &sched)
    });
    let engine = use_context_provider(|| Engine {
        db: db.clone(),
        coach,
        sched: sched.clone(),
        deck,
        session: Signal::new(None),
        view: Signal::new(View::Home),
        summary: Signal::new(None),
        error: Signal::new(None),
    });
    let ui = use_context::<Ui>();
    let view = *engine.view.read();
    let error = *engine.error.read();
    // A running session holds the phone's screen; Finish is its way out.
    crate::focus::use_focus_mode(matches!(view, View::Classic | View::Wall | View::Burst));

    rsx! {
        style { {REVIEW_CSS.text()} }
        div { class: "review",
            if let Some(msg) = error {
                p { class: "review-error", role: "alert", "{ui.t(msg)}" }
            }
            match view {
                View::Home => rsx! { ReviewHome {} },
                View::Classic => rsx! { Classic {} },
                View::Wall => rsx! { CardWall {} },
                View::Burst => rsx! { Burst {} },
                View::Done => rsx! { Done {} },
            }
        }
    }
}

// ---------------------------------------------------------------- home

/// The empty deck's invite: the starter words as chips, each opening its
/// Arabic Bridge card (word, meaning, the Arabic it comes from, and the
/// false-friend warning), so the words teach before the first round.
#[component]
fn SeedInvite() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let mut open = use_signal(|| None::<&'static str>);
    let count = ui.num(seed_words().count() as u64);
    let card = open()
        .and_then(|w| seed_bridge_deck().into_iter().find(|c| c.kind == CardKind::Bridge && c.word == w));
    rsx! {
        crate::ui::DivCard { class: "review-seed",
            h2 { "{ui.t(Msg::RevSeedTitle)}" }
            p { {ui.t(Msg::RevSeedBody).replace("{n}", &count)} }
            ul { class: "seed-words", lang: "id",
                for w in seed_words() {
                    li { key: "{w}",
                        button {
                            class: "seed-word",
                            aria_pressed: if open() == Some(w) { "true" } else { "false" },
                            aria_controls: "seed-card",
                            onclick: move |_| open.set(if open() == Some(w) { None } else { Some(w) }),
                            bdi { "{w}" }
                        }
                    }
                }
            }
            if let Some(card) = card {
                div { id: "seed-card", class: "seed-card", aria_live: "polite",
                    Back { key: "{card.id}", card }
                }
            }
            button { class: "primary", onclick: move |_| engine.seed(), "{ui.t(Msg::RevSeedButton)}" }
        }
    }
}

#[component]
fn ReviewHome() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let now = now_ms();
    let empty = engine.deck.read().is_empty();

    if empty {
        return rsx! { SeedInvite {} };
    }

    let sched = engine.sched.clone();
    let deck = engine.deck.read();
    let stats = deck.stats(&sched, now);
    let forecast = deck.forecast(&sched, now, 7);
    let today = sched.day.day(now);
    let peak = forecast.iter().copied().max().unwrap_or(0).max(1);
    let next_due = deck.next_due_ms(now).map(|d| when_text(&ui, &sched, now, d));
    drop(deck);
    let width = *ui.width.read();
    let modes = [
        (ReviewMode::Classic, Msg::RevClassic, Msg::RevClassicHint),
        // Say how this device moves cards: pen on the tablet, mouse on the Mac.
        (
            ReviewMode::CardWall,
            Msg::RevWall,
            if *ui.tablet.read() {
                Msg::RevWallHint
            } else if cfg!(target_os = "android") {
                Msg::RevWallHintTouch
            } else {
                Msg::RevWallHintMouse
            },
        ),
        (ReviewMode::Burst, Msg::RevBurst, Msg::RevBurstHint),
    ];
    let nothing_due = stats.due + stats.new_available == 0;

    rsx! {
        crate::ui::DivCard {
            h2 { "{ui.t(Msg::NavReview)}" }
            // Due and new start a classic round; the rest open Stats.
            div { class: "rev-stats",
                Stat {
                    label: Msg::RevDue,
                    value: ui.num(stats.due as u64),
                    accent: true,
                    scope: Some(QueueScope::Due),
                    start: stats.due > 0,
                }
                Stat {
                    label: Msg::RevNew,
                    value: ui.num(stats.new_available as u64),
                    accent: false,
                    scope: Some(QueueScope::New),
                    start: stats.new_available > 0,
                }
                Stat { label: Msg::RevReviewedToday, value: ui.num(stats.reviews_today as u64), accent: false }
                Stat {
                    label: Msg::RevRetention,
                    value: stats.retention.map(|r| ui.percent(r)).unwrap_or_else(|| "–".into()),
                    accent: false,
                }
            }
            if nothing_due {
                crate::misc_art::Picture { art: crate::misc_art::Art::AllReviewed, class: "empty-art" }
                p { class: "rev-calm", "{ui.t(Msg::RevAllDone)}" }
                if let Some(when) = &next_due {
                    p { class: "muted", {ui.t(Msg::RevNextReview).replace("{when}", when)} }
                }
            }
        }
        crate::ui::DivCard {
            h3 { class: "rev-h3", "{ui.t(Msg::RevForecast)}" }
            // The week's bars open Stats, where the forecast runs further.
            button {
                class: "tap tap-plain forecast-tap",
                aria_label: "{ui.t(Msg::RevForecast)}",
                onclick: move |_| crate::stats::open(ui, Screen::Stats),
            ol { class: "forecast",
                for (i, n) in forecast.iter().copied().enumerate() {
                    li {
                        key: "{i}",
                        class: if i == 0 { "today" } else { "" },
                        aria_label: format!(
                            "{}: {}",
                            if i == 0 { ui.t(Msg::RevToday) } else { weekday(&ui, today + i as i64) },
                            ui.num(u64::from(n)),
                        ),
                        span { class: "fc-n", aria_hidden: "true", if n > 0 { "{ui.num(u64::from(n))}" } }
                        span { class: "fc-bar", aria_hidden: "true",
                            span { class: "fc-fill", style: "block-size: {n * 100 / peak}%" }
                        }
                        span { class: "fc-day", aria_hidden: "true",
                            if i == 0 { "{ui.t(Msg::RevToday)}" } else { "{weekday(&ui, today + i as i64)}" }
                        }
                    }
                }
            }
            }
            crate::stats::StatsButton {}
        }
        div { class: "rev-modes",
            for (mode, title, hint) in modes {
                {
                    let count = engine.queue_len(mode, now);
                    let engine = engine.clone();
                    let burst_secs = ui.num(BURST_MS / 1000);
                    rsx! {
                        // The whole card starts the mode when there is something to do;
                        // the Start button's click bubbles up to it.
                        div {
                            key: "{mode:?}",
                            class: match (mode == ReviewMode::Burst, count > 0) {
                                (true, true) => "card card-action rev-mode mode-burst",
                                (true, false) => "card rev-mode mode-burst",
                                (false, true) => "card card-action rev-mode",
                                (false, false) => "card rev-mode",
                            },
                            onclick: move |_| if count > 0 { engine.start(mode) },
                            h3 { class: "rev-h3", Isolated { text: ui.t(title).to_string() } }
                            p { class: "muted", Isolated { text: ui.t(hint).replace("{n}", &burst_secs) } }
                            if mode == ReviewMode::CardWall && width == WidthClass::Compact {
                                p { class: "muted small", "{ui.t(Msg::RevWallPhone)}" }
                            }
                            if count == 0 {
                                p { class: "muted small", "{ui.t(Msg::RevNothingHere)}" }
                            }
                            button {
                                class: "primary",
                                disabled: count == 0,
                                "{ui.t(Msg::RevStart)}"
                                if count > 0 {
                                    span { class: "count-badge", "{ui.num(count as u64)}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A number that opens what it counts: a classic round when `start` is
/// given, else Stats.
#[component]
fn Stat(
    label: Msg,
    value: String,
    accent: bool,
    #[props(default)] start: bool,
    #[props(default)] scope: Option<QueueScope>,
) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    rsx! {
        button {
            class: if accent { "rev-stat accent tap" } else { "rev-stat tap" },
            onclick: move |_| {
                if start {
                    engine.start_scoped(ReviewMode::Classic, scope.unwrap_or(QueueScope::All));
                } else {
                    crate::stats::open(ui, Screen::Stats);
                }
            },
            span { class: "rev-stat-value", "{value}" }
            span { class: "rev-stat-label", "{ui.t(label)}" }
        }
    }
}

#[cfg(test)]
mod tests;
