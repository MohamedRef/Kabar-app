//! Watch & listen (PLAN §16 B): the Sinetron, one episode per finished unit,
//! and Kabar Radio, one two-host show per unit, both downloadable for
//! offline. Episodes are build-time content (`content/stories/`,
//! `content/radio/`); the speaker voices come from `kabar_core::story`.

pub(crate) mod player;
mod text;

use std::collections::BTreeSet;
use std::sync::LazyLock;

use dioxus::prelude::*;
use kabar_core::journey::Progress;
use kabar_core::lesson::pick;
use kabar_core::locale::eastern_arabic_digits;
use kabar_core::story::{Program, parse_radio, parse_story, quiz, unlocked};
use kabar_protocol::coach::{ActivityKind, Skill};
use kabar_protocol::story::{RadioEpisode, Story};

use self::player::ProgramPlayer;
use self::text::{T, tt};
use crate::audio::AudioCache;
use crate::coach::Coach;
use crate::lesson::embedded::{RADIO, STORIES};
use crate::lesson::{EpisodeQuiz, Rich, target_tag};
use crate::{Db, Ui};

pub(crate) const LISTEN_CSS: &str = include_str!("../../assets/listen.css");

/// Every episode file, parsed once; a broken file stays listed with its error.
struct Shows {
    sinetron: Vec<Result<Story, (String, String)>>,
    radio: Vec<Result<RadioEpisode, (String, String)>>,
}

static SHOWS: LazyLock<Shows> = LazyLock::new(|| {
    let mut sinetron: Vec<_> = STORIES
        .iter()
        .map(|(id, yaml)| parse_story(yaml).map_err(|e| (id.to_string(), e.to_string())))
        .collect();
    sinetron.sort_by_key(|s| s.as_ref().map_or(u32::MAX, |s| s.episode));
    let radio = RADIO
        .iter()
        .map(|(id, yaml)| parse_radio(yaml).map_err(|e| (id.to_string(), e.to_string())))
        .collect();
    Shows { sinetron, radio }
});

fn story(id: &str) -> Option<&'static Story> {
    SHOWS.sinetron.iter().flatten().find(|s| s.id == id)
}

fn radio(id: &str) -> Option<&'static RadioEpisode> {
    SHOWS.radio.iter().flatten().find(|r| r.id == id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Sinetron,
    Radio,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Open {
    Sinetron(String),
    Quiz(String),
    Radio(String),
}

/// "3.4 MB", in the UI's digits.
fn megabytes(ui: &Ui, bytes: u64) -> String {
    let s = format!("{:.1} MB", bytes as f64 / 1_000_000.0);
    if ui.eastern() { eastern_arabic_digits(&s) } else { s }
}

#[component]
pub fn ListenScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut tab = use_signal(|| Tab::Sinetron);
    let mut open = use_signal(|| None::<Open>);
    // Bumped when an episode is finished or downloads change, so the lists re-read.
    let mut revision = use_signal(|| 0u32);

    let content = match open() {
        Some(Open::Sinetron(id)) => match story(&id) {
            Some(s) => {
                let quiz_id = id.clone();
                rsx! {
                    ProgramPlayer {
                        key: "{id}",
                        program: Program::sinetron(s),
                        title: s.title.target.clone(),
                        on_back: move |_| {
                            open.set(None);
                            revision += 1;
                        },
                        on_end: move |_| {},
                        SinetronEnd { story: s.clone(), on_questions: move |_| open.set(Some(Open::Quiz(quiz_id.clone()))) }
                    }
                }
            }
            None => rsx! {},
        },
        Some(Open::Quiz(id)) => match story(&id) {
            Some(s) => rsx! {
                EpisodeQuiz {
                    key: "quiz-{id}",
                    lesson: quiz(s),
                    on_close: move |_| {
                        open.set(None);
                        revision += 1;
                    },
                }
            },
            None => rsx! {},
        },
        Some(Open::Radio(id)) => match radio(&id) {
            Some(r) => {
                let db = db.clone();
                rsx! {
                    ProgramPlayer {
                        key: "{id}",
                        program: Program::radio(r),
                        title: r.title.target.clone(),
                        on_back: move |_| {
                            open.set(None);
                            revision += 1;
                        },
                        on_end: move |_| {
                            // Listened to the end: synced like a finished lesson.
                            let done = serde_json::json!({ "minutes": r.minutes });
                            if let Err(e) = db.0.borrow_mut().record_lesson_event(&r.id, "completed", &done) {
                                eprintln!("kabar: saving the radio episode failed: {e}");
                            }
                            let listened = ActivityKind::Practice {
                                skill: Skill::Listening,
                                topic: r.title.target.clone(),
                                correct: 0,
                                total: 0,
                            };
                            coach.record(&db, listened, r.minutes * 60);
                        },
                        RadioEnd { episode: r.clone() }
                    }
                }
            }
            None => rsx! {},
        },
        None => rsx! {
            Lists { tab: tab(), revision: revision(), on_open: move |o| open.set(Some(o)), on_changed: move |_| revision += 1 }
        },
    };

    rsx! {
        style { {LISTEN_CSS} }
        div { class: "listen",
            if open().is_none() {
                div { class: "seg", role: "tablist", aria_label: "{ui.t(crate::i18n::Msg::NavListen)}",
                    for (t, label) in [(Tab::Sinetron, "Sinetron"), (Tab::Radio, "Kabar Radio")] {
                        button {
                            key: "{t:?}",
                            role: "tab",
                            aria_selected: if tab() == t { "true" } else { "false" },
                            onclick: move |_| tab.set(t),
                            bdi { class: "latin", "{label}" }
                        }
                    }
                }
            }
            {content}
        }
    }
}

#[component]
fn Lists(tab: Tab, revision: u32, on_open: EventHandler<Open>, on_changed: EventHandler) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let db = use_context::<Db>();
    let cache = try_use_context::<AudioCache>();
    let _ = revision; // A new value re-renders the lists.
    let completed: BTreeSet<String> = db.0.borrow().completed_lessons().unwrap_or_else(|e| {
        eprintln!("kabar: reading finished lessons failed: {e}");
        BTreeSet::new()
    });
    let start = db.0.borrow().placement().ok().flatten().map(|p| p.start_level);
    let has_track = |id: &str| cache.as_ref().is_some_and(|c| c.track(id).is_some());
    let usage = cache.as_ref().map(AudioCache::downloads).unwrap_or_default();

    let list = match tab {
        Tab::Sinetron => rsx! {
            p { class: "muted", "{tt(lang, T::SinetronIntro)}" }
            ol { class: "lx-list",
                for entry in SHOWS.sinetron.iter() {
                    match entry {
                        Ok(s) => {
                            let target = target_tag(&s.language);
                            let open = crate::journey::curriculum(&s.language).is_some_and(|c| {
                                unlocked(c, &Progress::new(c, &completed), start.as_deref(), &s.unlock_after)
                            });
                            let unit = crate::journey::curriculum(&s.language)
                                .and_then(|c| c.levels.iter().flat_map(|l| &l.units).find(|u| u.id == s.unlock_after))
                                .map(|u| crate::journey::local(lang, &u.title).to_string())
                                .unwrap_or_else(|| s.unlock_after.clone());
                            let id = s.id.clone();
                            rsx! {
                                li { key: "{s.id}",
                                    button {
                                        class: if open { "card lx-card" } else { "card lx-card locked" },
                                        disabled: !open,
                                        onclick: move |_| on_open.call(Open::Sinetron(id.clone())),
                                        span { class: "muted small", "{tt(lang, T::Episode)} {ui.num(s.episode)}" }
                                        strong { bdi { lang: target, class: "latin", "{s.title.target}" } }
                                        span { Rich { text: pick(&s.title.meaning, lang).to_string(), target } }
                                        if open {
                                            span { class: "muted small", Rich { text: pick(&s.summary, lang).to_string(), target } }
                                        } else {
                                            span { class: "muted small", "🔒 {tt(lang, T::OpensAfter)}: " Rich { text: unit, target } }
                                        }
                                        span { class: "row",
                                            if completed.contains(&s.id) {
                                                span { class: "pill", "✓ {tt(lang, T::Watched)}" }
                                            }
                                            if has_track(&s.id) {
                                                span { class: "pill lx-offline", "⤓ {tt(lang, T::Downloaded)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err((id, e)) => rsx! { Broken { key: "{id}", id: id.clone(), error: e.clone() } },
                    }
                }
            }
        },
        Tab::Radio => rsx! {
            p { class: "muted", "{tt(lang, T::RadioIntro)}" }
            ol { class: "lx-list",
                for entry in SHOWS.radio.iter() {
                    match entry {
                        Ok(r) => {
                            let target = target_tag(&r.language);
                            let id = r.id.clone();
                            rsx! {
                                li { key: "{r.id}",
                                    button {
                                        class: "card lx-card",
                                        onclick: move |_| on_open.call(Open::Radio(id.clone())),
                                        span { class: "muted small", "{crate::coach::labels::minutes(&ui, r.minutes)}" }
                                        strong { bdi { lang: target, class: "latin", "{r.title.target}" } }
                                        span { Rich { text: pick(&r.title.meaning, lang).to_string(), target } }
                                        span { class: "row",
                                            if completed.contains(&r.id) {
                                                span { class: "pill", "✓ {tt(lang, T::Listened)}" }
                                            }
                                            if has_track(&r.id) {
                                                span { class: "pill lx-offline", "⤓ {tt(lang, T::Downloaded)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err((id, e)) => rsx! { Broken { key: "{id}", id: id.clone(), error: e.clone() } },
                    }
                }
            }
        },
    };

    rsx! {
        {list}
        section { class: "card lx-space",
            if usage.files == 0 {
                p { class: "muted", "{tt(lang, T::NoDownloads)}" }
            } else {
                p { "{tt(lang, T::DownloadsUse)} " bdi { "{megabytes(&ui, usage.bytes)}" } }
                button {
                    onclick: move |_| {
                        if let Some(c) = cache.as_ref() {
                            c.remove_downloads();
                        }
                        on_changed.call(());
                    },
                    "{tt(lang, T::FreeUpSpace)}"
                }
            }
        }
    }
}

#[component]
fn Broken(id: String, error: String) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        li { class: "card lx-card locked",
            strong { bdi { class: "mono", "{id}" } }
            span { class: "muted small", "{tt(ui.lang(), T::Broken)}" }
            span { class: "mono small", dir: "ltr", "{error}" }
        }
    }
}

/// After the last line: the teaser for next time, then the questions.
#[component]
fn SinetronEnd(story: Story, on_questions: EventHandler) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let target = target_tag(&story.language);
    rsx! {
        if let Some(next) = &story.next {
            p { class: "lx-next",
                strong { "{tt(lang, T::NextTime)}: " }
                Rich { text: pick(next, lang).to_string(), target }
            }
        }
        button { class: "primary", onclick: move |_| on_questions.call(()), "{tt(lang, T::Questions)}" }
    }
}

/// After the show: the unit words it spotlighted.
#[component]
fn RadioEnd(episode: RadioEpisode) -> Element {
    let ui = use_context::<Ui>();
    let target = target_tag(&episode.language);
    rsx! {
        h3 { "{tt(ui.lang(), T::Words)}" }
        p { class: "row",
            for w in episode.words.iter() {
                bdi { key: "{w}", lang: target, class: "pill latin", "{w}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lesson::embedded::PERSONAS;

    #[test]
    fn every_shipped_episode_and_persona_parses() {
        assert!(SHOWS.sinetron.len() >= 10 && SHOWS.radio.len() >= 5);
        for s in &SHOWS.sinetron {
            let s = s.as_ref().unwrap();
            let p = Program::sinetron(s);
            assert!(!p.cues.is_empty(), "{}", s.id);
            assert!(crate::journey::curriculum(&s.language).is_some());
            let q = quiz(s);
            // A quiz is shorter than a lesson (3-6 questions); the rest must hold.
            let issues: Vec<_> =
                kabar_core::lesson::validate(&q).into_iter().filter(|i| i.path != "exercises").collect();
            assert!(
                issues.is_empty(),
                "{}: {}",
                s.id,
                issues.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("; ")
            );
        }
        for r in &SHOWS.radio {
            let r = r.as_ref().unwrap();
            assert_eq!(
                player::requests(&Program::radio(r)).len(),
                r.segments.iter().map(|s| s.lines.len()).sum::<usize>()
            );
        }
        for (id, yaml) in PERSONAS {
            let p = kabar_core::story::parse_persona(yaml).unwrap();
            assert_eq!(&p.id, id);
        }
        // Episodes come in order.
        let numbers: Vec<u32> = SHOWS.sinetron.iter().flatten().map(|s| s.episode).collect();
        assert!(numbers.windows(2).all(|w| w[0] < w[1]));
    }
}
