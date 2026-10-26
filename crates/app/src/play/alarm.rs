//! False Friends Alarm (spec §12): words that look like Arabic an Egyptian
//! knows but mean something else (kalimat is "sentence", not «كلمة»). The
//! alarm is a friendly dawn glow, not a siren.

use dioxus::prelude::*;
use kabar_core::games::bridge::{self, AlarmRound};
use kabar_protocol::coach::{ActivityKind, Skill};

use super::detective::choice_class;
use super::logic::{self, meaning};
use super::{ContentProblem, PlayHeader, RoundSummary, content, fresh_seed, seconds_since};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

pub const ROUNDS: usize = 10;

#[derive(Clone, PartialEq)]
struct Session {
    rounds: Vec<AlarmRound>,
    index: usize,
    picked: Option<usize>,
    correct: u32,
    start_ms: u64,
    done: bool,
}

fn new_session() -> Option<Session> {
    let data = content::bridge().as_ref().ok()?;
    Some(Session {
        rounds: bridge::alarm_rounds(data, ROUNDS, fresh_seed()),
        index: 0,
        picked: None,
        correct: 0,
        start_ms: crate::store::now_ms(),
        done: false,
    })
}

#[component]
pub fn Alarm() -> Element {
    if let Err(e) = content::bridge() {
        return rsx! {
            PlayHeader { title: Msg::AlarmTitle }
            ContentProblem { detail: e.clone() }
        };
    }
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut session = use_signal(|| None::<Session>);
    let Some(s) = session.read().clone() else {
        return rsx! {
            PlayHeader { title: Msg::AlarmTitle }
            div { class: "card alarm-intro",
                p { "{ui.t(Msg::AlarmIntro)}" }
                p { class: "alarm-example",
                    bdi { class: "latin", lang: "id", "kalimat" }
                    " ≠ "
                    span { lang: "ar", "«كلمة»" }
                }
                button { class: "primary", onclick: move |_| session.set(new_session()), "{ui.t(Msg::Start)}" }
            }
        };
    };
    let total = s.rounds.len() as u32;
    if s.rounds.is_empty() {
        return rsx! {
            PlayHeader { title: Msg::AlarmTitle }
            div { class: "card", p { "{ui.t(Msg::DetectiveNoRounds)}" } }
        };
    }
    if s.done {
        return rsx! {
            PlayHeader { title: Msg::AlarmTitle }
            RoundSummary { correct: s.correct, total, onagain: move |_| session.set(new_session()) }
        };
    }

    let round = s.rounds[s.index].clone();
    let picked = s.picked;
    let last = s.index + 1 == s.rounds.len();
    let lang = ui.lang();

    rsx! {
        PlayHeader { title: Msg::AlarmTitle }
        div { class: "card round alarm",
            p { class: "muted small", "{ui.num(s.index as u64 + 1)} / {ui.num(total)}" }
            div { class: "alarm-banner", role: "note",
                span { class: "alarm-dot", aria_hidden: "true" }
                span { "{ui.t(Msg::AlarmLooksLike)} " }
                span { class: "ar", lang: "ar", "«{round.arabic}»" }
            }
            p { class: "prompt latin", lang: "id", bdi { "{round.word}" } }
            p { class: "muted", "{ui.t(Msg::AlarmQuestion)}" }
            div { class: "choices two",
                for (i , choice) in round.choices.iter().enumerate() {
                    button {
                        key: "{i}",
                        class: choice_class(i, picked, round.answer),
                        disabled: picked.is_some(),
                        onclick: move |_| {
                            let mut w = session.write();
                            if let Some(s) = w.as_mut().filter(|s| s.picked.is_none()) {
                                s.picked = Some(i);
                                if i == s.rounds[s.index].answer {
                                    s.correct += 1;
                                }
                            }
                        },
                        "{meaning(lang, choice)}"
                    }
                }
            }
            if let Some(p) = picked {
                div { class: if p == round.answer { "feedback right reveal" } else { "feedback kind reveal" },
                    strong { if p == round.answer { "{ui.t(Msg::AlarmCaught)}" } else { "{ui.t(Msg::AlarmTricky)}" } }
                    if let Some(note) = &round.note {
                        p { "{meaning(lang, note)}" }
                    }
                    div { class: "row end",
                        button {
                            class: "primary",
                            onclick: move |_| {
                                let finished = {
                                    let mut w = session.write();
                                    let Some(s) = w.as_mut() else { return };
                                    if last {
                                        s.done = true;
                                        Some((s.correct, s.rounds.len() as u32, s.start_ms))
                                    } else {
                                        s.index += 1;
                                        s.picked = None;
                                        None
                                    }
                                };
                                if let Some((correct, total, start)) = finished {
                                    coach.record(
                                        &db,
                                        ActivityKind::Practice {
                                            skill: Skill::Vocabulary,
                                            topic: logic::TOPIC_ALARM.to_string(),
                                            correct,
                                            total,
                                        },
                                        seconds_since(start),
                                    );
                                }
                            },
                            if last { "{ui.t(Msg::PlayFinish)}" } else { "{ui.t(Msg::Next)}" }
                        }
                    }
                }
            }
        }
    }
}
