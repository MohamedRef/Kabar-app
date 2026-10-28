//! Kabar Pagi (spec §14): a 2-minute morning micro-story built from the
//! learner's due words plus a few new ones, read aloud.
//!
//! Today the story comes from the build-time bank. The AI path
//! (`story_episode` through the router, via Kabar Cloud) plugs in at
//! [`ai_story`] once the app has a Cloud client.

use std::collections::HashSet;

use dioxus::prelude::*;
use kabar_core::coach::onboarding::starting_unit;
use kabar_core::pagi::{self, PagiWords, Story};
use kabar_core::settings::TranslationMode;
use kabar_protocol::coach::{ActivityEvent, ActivityKind, CoachProfile, LocalDate, LocalTime, Skill};

use super::logic::{self, meaning};
use super::speech::{self, Voice};
use super::{ContentProblem, PlayHeader, PlayNav, Route, content};
use crate::annotated::Annotated;
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

/// From when the Today card offers the morning story.
const MORNING_FROM: LocalTime = LocalTime(4 * 60);

/// The seam for the AI-written story (`story_episode` via the router). The
/// app has no Kabar Cloud client yet, so this always falls back to the bank.
pub fn ai_story(words: &PagiWords, level: &str) -> Option<Story> {
    let _ = (words, level);
    None
}

/// The level stories are picked at: the learner's starting unit's level
/// ("bipa1") until the Journey (M8) tracks the current unit.
fn level_for(profile: Option<&CoachProfile>) -> String {
    let unit = starting_unit(profile.map(|p| p.start).unwrap_or_default());
    logic::level_of_unit(unit).to_string()
}

/// Today's words and story: the same all day on every device.
fn todays_story(
    db: &Db,
    sched: &std::rc::Rc<kabar_core::srs::Scheduler>,
    now_ms: u64,
    events: &[ActivityEvent],
    profile: Option<&CoachProfile>,
    date: LocalDate,
) -> Option<(PagiWords, Story)> {
    let bank = content::stories().as_ref().ok()?;
    let seed = logic::day_seed(date);
    // Due review words first; before any cards exist, the words met most recently.
    let mut due = crate::review::due_words(db, sched, now_ms);
    if due.is_empty() {
        due = logic::latest_new_words(events);
    }
    let known = logic::known_words(events);
    let words = pagi::pick_words(&due, &known, content::frequent_words(), seed);
    let level = level_for(profile);
    let story = ai_story(&words, &level).or_else(|| bank.best(&words, &level, seed).cloned())?;
    Some((words, story))
}

/// On Today: the morning story, until it has been read today.
#[component]
pub fn PagiTodayCard() -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let nav = use_context::<PlayNav>();
    let today = coach.today();
    let read = logic::practiced_on(&coach.events.read(), today, logic::TOPIC_PAGI);
    if read || coach.time_of_day() < MORNING_FROM {
        return rsx! {};
    }
    rsx! {
        // A tap anywhere opens the story; the button is the keyboard and
        // screen-reader target, and its click bubbles up to the card.
        div { class: "card card-action pagi-card reveal",
            onclick: move |_| super::open(ui, nav, Route::Pagi),
            h2 { lang: "id", bdi { "Kabar Pagi" } }
            p { class: "muted", "{ui.t(Msg::PagiBlurb)}" }
            button { class: "primary", "{ui.t(Msg::PagiListen)}" }
        }
    }
}

#[component]
pub fn PagiPlayer() -> Element {
    if let Err(e) = content::stories() {
        return rsx! {
            PlayHeader { title: Msg::PagiTitle, latin: true }
            ContentProblem { detail: e.clone() }
        };
    }
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let today = coach.today();
    let story = use_hook(|| {
        let sched = crate::schedule::scheduler(&coach, &db.0.borrow());
        todays_story(&db, &sched, coach.now(), &coach.events.peek(), coach.profile.peek().as_ref(), today)
    });
    let voice = speech::use_voice();
    let speaking = use_signal(|| None::<usize>);
    let mut step_mode = use_signal(|| false);
    let mut shown = use_signal(|| 1usize);
    // The translation setting decides whether the story starts translated.
    let mut all_translations = use_signal(|| ui.settings.peek().translation == TranslationMode::On);
    let revealed = use_signal(HashSet::<usize>::new);
    use_drop(speech::stop);

    let Some((words, story)) = story else {
        return rsx! {
            PlayHeader { title: Msg::PagiTitle, latin: true }
            div { class: "card", p { "{ui.t(Msg::PagiNoStory)}" } }
        };
    };
    let read = logic::practiced_on(&coach.events.read(), today, logic::TOPIC_PAGI);
    let targets: Vec<String> = story.words.clone();
    let new_words: HashSet<String> = words.new.iter().cloned().collect();
    let texts: Vec<String> = story.sentences.iter().map(|s| s.id.clone()).collect();
    let count = story.sentences.len();
    let visible = if *step_mode.read() { (*shown.read()).min(count) } else { count };
    let now = *speaking.read();

    rsx! {
        PlayHeader { title: Msg::PagiTitle, latin: true }
        div { class: "pagi-layout",
            div { class: "card",
                h2 { class: "story-title", lang: "id", bdi { "{story.title}" } }
                // Who's in today's story, by the names the sentences use.
                ul { class: "pagi-cast",
                    for (name, src) in crate::portraits::mentioned("pagi", &texts.join(" ")) {
                        li { key: "{name}",
                            crate::portraits::Face { src, class: "pagi-face" }
                            bdi { class: "latin small", "{name}" }
                        }
                    }
                }
                div { class: "words", lang: "id",
                    for w in targets.iter() {
                        bdi {
                            key: "{w}",
                            class: if new_words.contains(w) { "word new-word" } else { "word" },
                            "{w}"
                        }
                    }
                }
                p { class: "muted small", "{ui.t(Msg::PagiWordsNote)}" }
                div { class: "row",
                    if *voice.read() == Voice::Ready {
                        if now.is_some() {
                            button { onclick: move |_| speech::stop(), "{ui.t(Msg::Stop)}" }
                        } else {
                            button {
                                class: "primary",
                                onclick: {
                                    let texts = texts.clone();
                                    move |_| {
                                        let mut speaking = speaking;
                                        let mut shown = shown;
                                        let n = texts.len();
                                        speech::speak(texts.clone(), 0, move |i| {
                                            speaking.set(i);
                                            if let Some(i) = i {
                                                let next = (i + 1).max(*shown.peek()).min(n);
                                                shown.set(next);
                                            }
                                        });
                                    }
                                },
                                "{ui.t(Msg::PagiPlayAll)}"
                            }
                        }
                    }
                    button {
                        aria_pressed: if *step_mode.read() { "true" } else { "false" },
                        onclick: move |_| {
                            let on = !*step_mode.peek();
                            step_mode.set(on);
                            shown.set(1);
                        },
                        "{ui.t(Msg::PagiOneByOne)}"
                    }
                    button {
                        aria_pressed: if *all_translations.read() { "true" } else { "false" },
                        onclick: move |_| {
                            let on = !*all_translations.peek();
                            all_translations.set(on);
                        },
                        "{ui.t(Msg::PagiAllTranslations)}"
                    }
                }
                if *voice.read() == Voice::Missing {
                    p { class: "muted small audio-note", "{ui.t(Msg::AudioMissing)}" }
                }
            }
            div { class: "card",
                ol { class: "sentences",
                    for (i , s) in story.sentences.iter().take(visible).enumerate() {
                        li {
                            key: "{i}",
                            class: if now == Some(i) { "sentence speaking" } else { "sentence" },
                            div { class: "sentence-row",
                                p { class: "id-text",
                                    Annotated { text: s.id.clone(), tag: "id", marked: targets.clone() }
                                }
                                if *voice.read() == Voice::Ready {
                                    button {
                                        class: "icon-button",
                                        aria_label: "{ui.t(Msg::Listen)}",
                                        onclick: {
                                            let text = s.id.clone();
                                            move |_| {
                                                let mut speaking = speaking;
                                                speech::speak(vec![text.clone()], i, move |at| speaking.set(at));
                                            }
                                        },
                                        "▶"
                                    }
                                }
                            }
                            if *all_translations.read() || revealed.read().contains(&i) {
                                p { class: "translation muted reveal", "{meaning(ui.lang(), &trilingual(s))}" }
                            } else {
                                button {
                                    class: "quiet small",
                                    onclick: move |_| {
                                        let mut revealed = revealed;
                                        revealed.write().insert(i);
                                    },
                                    "{ui.t(Msg::ShowMeaning)}"
                                }
                            }
                        }
                    }
                }
                if visible < count {
                    button { onclick: move |_| shown += 1, "{ui.t(Msg::Next)}" }
                }
            }
            div { class: "card",
                if read {
                    p { class: "ok", "✓ {ui.t(Msg::PagiReadToday)}" }
                } else {
                    button {
                        class: "primary",
                        onclick: {
                            let new = words.new.clone();
                            let story = story.clone();
                            move |_| {
                                let logged = logic::logged_words(&coach.events.peek());
                                let fresh = logic::unlogged(&new, &logged);
                                if !fresh.is_empty() {
                                    coach.record(&db, ActivityKind::NewWords { words: fresh }, 0);
                                }
                                let n = story.sentences.len() as u32;
                                coach.record(
                                    &db,
                                    ActivityKind::Practice {
                                        skill: Skill::Listening,
                                        topic: logic::TOPIC_PAGI.to_string(),
                                        correct: n,
                                        total: n,
                                    },
                                    pagi::listening_seconds(&story),
                                );
                            }
                        },
                        "{ui.t(Msg::PagiDone)}"
                    }
                }
            }
        }
    }
}

fn trilingual(s: &pagi::StorySentence) -> kabar_protocol::content::Trilingual {
    kabar_protocol::content::Trilingual { arz: s.arz.clone(), ar: s.ar.clone(), en: s.en.clone() }
}
