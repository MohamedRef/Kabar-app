//! Sound-Shift Detective (spec §12): learn a regular sound change from
//! Arabic (ف → p, خ → k, ق → k, ة → -at), then spot the Arabic origin.

use dioxus::prelude::*;
use kabar_core::games::bridge::{self, DetectiveMode, DetectiveRound};
use kabar_protocol::coach::{ActivityKind, Skill};
use kabar_protocol::content::SoundRule;

use super::logic::{self, meaning};
use super::{ContentProblem, PlayHeader, RoundSummary, content, fresh_seed, seconds_since};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

pub const ROUNDS: usize = 10;

#[derive(Clone, PartialEq)]
struct Session {
    rule: Option<String>,
    rounds: Vec<DetectiveRound>,
    index: usize,
    picked: Option<usize>,
    correct: u32,
    start_ms: u64,
    done: bool,
}

#[component]
pub fn Detective() -> Element {
    if let Err(e) = content::bridge() {
        return rsx! {
            PlayHeader { title: Msg::DetectiveTitle }
            ContentProblem { detail: e.clone() }
        };
    }
    let rule = use_signal(|| None::<String>);
    let session = use_signal(|| None::<Session>);
    let playing = session.read().is_some();

    rsx! {
        PlayHeader { title: Msg::DetectiveTitle }
        if playing {
            Rounds { session }
        } else {
            Intro { rule, session }
        }
    }
}

/// Pick a rule (or all of them) and read how it works.
#[component]
fn Intro(rule: Signal<Option<String>>, session: Signal<Option<Session>>) -> Element {
    let Ok(data) = content::bridge() else { return rsx! {} };
    let ui = use_context::<Ui>();
    let current = rule.read().clone();
    let selected: Option<&SoundRule> = current.as_deref().and_then(|id| data.rule(id));

    rsx! {
        div { class: "card",
            p { class: "muted", "{ui.t(Msg::DetectiveIntro)}" }
            div { class: "row", role: "group",
                button {
                    aria_pressed: if current.is_none() { "true" } else { "false" },
                    onclick: move |_| rule.set(None),
                    "{ui.t(Msg::DetectiveAllRules)}"
                }
                for r in data.rules.iter() {
                    button {
                        key: "{r.id}",
                        aria_pressed: if current.as_deref() == Some(r.id.as_str()) { "true" } else { "false" },
                        onclick: {
                            let id = r.id.clone();
                            move |_| rule.set(Some(id.clone()))
                        },
                        span { lang: "ar", "{r.from}" }
                        span { class: "flip", aria_hidden: "true", " → " }
                        bdi { class: "latin", "{r.to}" }
                    }
                }
            }
        }
        if let Some(r) = selected {
            RuleCard { rule: r.clone() }
        } else {
            div { class: "play-grid",
                for r in data.rules.iter() {
                    RuleCard { key: "{r.id}", rule: r.clone() }
                }
            }
        }
        div { class: "row",
            button {
                class: "primary",
                onclick: move |_| {
                    let rule_id = rule.peek().clone();
                    let rounds = bridge::detective_rounds(data, rule_id.as_deref(), ROUNDS, fresh_seed());
                    let mut session = session;
                    session.set(Some(Session {
                        rule: rule_id,
                        rounds,
                        index: 0,
                        picked: None,
                        correct: 0,
                        start_ms: crate::store::now_ms(),
                        done: false,
                    }));
                },
                "{ui.t(Msg::Start)}"
            }
        }
    }
}

/// One sound change: from → to, why, and words that show it.
#[component]
fn RuleCard(rule: SoundRule) -> Element {
    let data = content::bridge().as_ref().ok();
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "card rule-card reveal",
            p { class: "rule-shift",
                span { class: "ar", lang: "ar", "{rule.from}" }
                span { class: "flip", aria_hidden: "true", " → " }
                bdi { class: "latin", lang: "id", "{rule.to}" }
            }
            p { "{meaning(ui.lang(), &rule.explanation)}" }
            ul { class: "examples",
                for ex in rule.examples.iter() {
                    li { key: "{ex}",
                        bdi { class: "bridge-word latin", lang: "id", "{ex}" }
                        if let Some(entry) = data.and_then(|d| d.entry(ex)) {
                            span { aria_hidden: "true", " · " }
                            span { class: "ar gold-text", lang: "ar", "{entry.arabic}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Rounds(session: Signal<Option<Session>>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let Some(s) = session.read().clone() else {
        return rsx! {};
    };
    let total = s.rounds.len() as u32;

    if s.rounds.is_empty() {
        return rsx! {
            div { class: "card",
                p { "{ui.t(Msg::DetectiveNoRounds)}" }
                button { onclick: move |_| session.set(None), "{ui.t(Msg::Back)}" }
            }
        };
    }
    if s.done {
        return rsx! {
            RoundSummary {
                correct: s.correct,
                total,
                onagain: move |_| session.set(None),
            }
        };
    }

    let round = s.rounds[s.index].clone();
    let picked = s.picked;
    let last = s.index + 1 == s.rounds.len();
    let data = content::bridge().as_ref().ok();
    let entry = data.and_then(|d| d.entry(&round.word));
    let rule = data.and_then(|d| round.rule_ids.first().and_then(|id| d.rule(id)));
    let arabic_choices = round.mode == DetectiveMode::GuessArabic;

    rsx! {
        div { class: "card round",
            p { class: "muted small", "{ui.num(s.index as u64 + 1)} / {ui.num(total)}" }
            p { class: "muted",
                if arabic_choices { "{ui.t(Msg::DetectiveGuessArabic)}" } else { "{ui.t(Msg::DetectiveGuessIndonesian)}" }
            }
            if arabic_choices {
                p { class: "prompt latin", lang: "id", bdi { "{round.prompt}" } }
            } else {
                p { class: "prompt ar", lang: "ar", dir: "rtl", "{round.prompt}" }
                if let Some(r) = rule {
                    p { class: "muted small",
                        span { lang: "ar", "{r.from}" }
                        span { class: "flip", aria_hidden: "true", " → " }
                        bdi { class: "latin", "{r.to}" }
                    }
                }
            }
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
                        if arabic_choices {
                            span { class: "ar", lang: "ar", dir: "rtl", "{choice}" }
                        } else {
                            bdi { class: "latin", lang: "id", "{choice}" }
                        }
                    }
                }
            }
            if let Some(p) = picked {
                div { class: if p == round.answer { "feedback right reveal" } else { "feedback kind reveal" },
                    if p == round.answer {
                        strong { "{ui.t(Msg::PlayRight)}" }
                    } else {
                        strong { "{ui.t(Msg::PlayNotQuite)} " }
                        if arabic_choices {
                            span { class: "ar", lang: "ar", "{round.choices[round.answer]}" }
                        } else {
                            bdi { class: "latin", lang: "id", "{round.choices[round.answer]}" }
                        }
                    }
                    if let Some(e) = entry {
                        p { class: "small",
                            bdi { class: "bridge-word latin", lang: "id", "{e.word}" }
                            " · "
                            span { class: "ar", lang: "ar", "{e.arabic}" }
                            " · {meaning(ui.lang(), &e.meaning)}"
                        }
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
                                        Some((s.rule.clone(), s.correct, s.rounds.len() as u32, s.start_ms))
                                    } else {
                                        s.index += 1;
                                        s.picked = None;
                                        None
                                    }
                                };
                                if let Some((rule, correct, total, start)) = finished {
                                    coach.record(
                                        &db,
                                        ActivityKind::Practice {
                                            skill: Skill::Vocabulary,
                                            topic: logic::detective_topic(rule.as_deref()),
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

/// A choice's look once answered: the right one glows Bridge gold.
pub fn choice_class(i: usize, picked: Option<usize>, answer: usize) -> &'static str {
    match picked {
        None => "choice",
        Some(_) if i == answer => "choice right",
        Some(p) if p == i => "choice missed",
        Some(_) => "choice faded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answered_choices_show_the_right_one() {
        assert_eq!(choice_class(1, None, 2), "choice");
        assert_eq!(choice_class(2, Some(2), 2), "choice right");
        assert_eq!(choice_class(2, Some(0), 2), "choice right");
        assert_eq!(choice_class(0, Some(0), 2), "choice missed");
        assert_eq!(choice_class(3, Some(0), 2), "choice faded");
    }
}
