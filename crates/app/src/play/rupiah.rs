//! Rupiah Reader (spec §14): read prices the way they're written in shops
//! and listings, fast. Rp26.000.000 → *dua puluh enam juta rupiah*.

use dioxus::prelude::*;
use kabar_core::games::rupiah::{self, Check, Drill, Level};
use kabar_protocol::coach::{ActivityKind, Skill};

use super::speech::{self, Voice};
use super::{PlayHeader, fresh_seed, logic};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

pub const DRILLS: usize = 10;
const LEVELS: [(Level, Msg); 4] = [
    (Level::Hundreds, Msg::RupiahHundreds),
    (Level::Thousands, Msg::RupiahThousands),
    (Level::Millions, Msg::RupiahMillions),
    (Level::Mixed, Msg::RupiahMixed),
];

#[derive(Clone, PartialEq)]
struct Session {
    level: Level,
    drills: Vec<Drill>,
    index: usize,
    input: String,
    check: Option<Check>,
    correct: u32,
    start_ms: u64,
    /// Set when the last drill is answered, shown after its feedback.
    pending: Option<Finished>,
    result: Option<Finished>,
}

/// "Check", then "Next" (or "Finish") on the same button and Enter key.
fn submit(mut session: Signal<Option<Session>>, db: &Db, coach: Coach) {
    let mut w = session.write();
    let Some(s) = w.as_mut() else { return };
    if s.check.is_some() {
        if s.index + 1 < s.drills.len() {
            s.index += 1;
            s.input.clear();
            s.check = None;
        } else {
            s.result = s.pending.take();
        }
        return;
    }
    if s.input.trim().is_empty() {
        return;
    }
    let check = rupiah::check(&s.drills[s.index], &s.input);
    if counts(&check) {
        s.correct += 1;
    }
    s.check = Some(check);
    if s.index + 1 < s.drills.len() {
        return;
    }
    let millis = crate::store::now_ms().saturating_sub(s.start_ms);
    let total = s.drills.len() as u32;
    let score = rupiah::score(s.correct, total, millis);
    let best_before = best(db, s.level);
    if best_before.is_none_or(|b| score > b)
        && let Err(e) = db.0.borrow_mut().set_local_value(&level_key(s.level), &score.to_string())
    {
        crate::diag::report!("saving the Rupiah best failed: {e}");
    }
    s.pending = Some(Finished { millis, score, best_before });
    let kind = ActivityKind::Practice {
        skill: Skill::Reading,
        topic: logic::TOPIC_RUPIAH.to_string(),
        correct: s.correct,
        total,
    };
    drop(w);
    coach.record(db, kind, u32::try_from(millis / 1000).unwrap_or(u32::MAX));
}

#[derive(Clone, Copy, PartialEq)]
struct Finished {
    millis: u64,
    score: u32,
    best_before: Option<u32>,
}

fn level_key(level: Level) -> String {
    logic::best_key(&format!("{level:?}"))
}

fn best(db: &Db, level: Level) -> Option<u32> {
    db.0.borrow().local_value(&level_key(level)).ok().flatten().and_then(|v| v.parse().ok())
}

/// Whether a check counts as right: a small slip still does.
pub fn counts(check: &Check) -> bool {
    matches!(check, Check::Correct | Check::Close { .. })
}

#[component]
pub fn RupiahReader() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let mut level = use_signal(|| Level::Thousands);
    let coach = use_context::<Coach>();
    let mut session = use_signal(|| None::<Session>);
    let voice = speech::use_voice();

    let Some(s) = session.read().clone() else {
        let current = *level.read();
        return rsx! {
            PlayHeader { title: Msg::RupiahTitle }
            crate::ui::DivCard {
                p { class: "muted", "{ui.t(Msg::RupiahIntro)}" }
                p { class: "price-example", lang: "id",
                    bdi { class: "latin price", "Rp26.000.000" }
                    span { class: "flip", aria_hidden: "true", " → " }
                    bdi { class: "latin", "dua puluh enam juta rupiah" }
                }
                div { class: "row", role: "group",
                    for (lv , label) in LEVELS {
                        button {
                            key: "{lv:?}",
                            aria_pressed: if lv == current { "true" } else { "false" },
                            onclick: move |_| level.set(lv),
                            span { "{ui.t(label)}" }
                            if let Some(b) = best(&db, lv) {
                                span { class: "small", " · {ui.t(Msg::RupiahBest)} {ui.num(b)}" }
                            }
                        }
                    }
                }
                p {
                    button {
                        class: "primary",
                        onclick: move |_| {
                            let lv = *level.peek();
                            session.set(Some(Session {
                                level: lv,
                                drills: rupiah::drills(lv, DRILLS, fresh_seed()),
                                index: 0,
                                input: String::new(),
                                check: None,
                                correct: 0,
                                start_ms: crate::store::now_ms(),
                                pending: None,
                                result: None,
                            }));
                        },
                        "{ui.t(Msg::Start)}"
                    }
                }
            }
        };
    };

    let total = s.drills.len() as u32;
    if let Some(f) = s.result {
        let new_best = f.best_before.is_none_or(|b| f.score > b);
        return rsx! {
            PlayHeader { title: Msg::RupiahTitle }
            super::RoundSummary {
                correct: s.correct,
                total,
                onagain: move |_| session.set(None),
                // This round's result: nothing further to open, so not a tile.
                div { class: "stats-row",
                    div { class: "stat-static",
                        span { class: "stat-value", "{ui.num(f.score)}" }
                        span { class: "stat-label", "{ui.t(Msg::RupiahScore)}" }
                    }
                    div { class: "stat-static",
                        span { class: "stat-value", "{ui.num(f.millis / 1000)}" }
                        span { class: "stat-label", "{ui.t(Msg::RupiahSeconds)}" }
                    }
                }
                if new_best {
                    p { class: "ok", "{ui.t(Msg::RupiahNewBest)}" }
                }
            }
        };
    }
    let Some(drill) = s.drills.get(s.index).cloned() else {
        return rsx! {
            PlayHeader { title: Msg::RupiahTitle }
            div { class: "card", p { "{ui.t(Msg::DetectiveNoRounds)}" } }
        };
    };
    let last = s.index + 1 == s.drills.len();
    let answered = s.check.clone();

    rsx! {
        PlayHeader { title: Msg::RupiahTitle }
        crate::ui::DivCard { class: "round",
            super::RoundCount { at: s.index as u64 + 1, total: total as u64 }
            p { class: "price big latin", lang: "id", dir: "ltr", bdi { "{drill.shown}" } }
            form {
                class: "row answer-row",
                onsubmit: {
                    let db = db.clone();
                    move |e: FormEvent| {
                        e.prevent_default();
                        submit(session, &db, coach);
                    }
                },
                input {
                    class: "text-input latin",
                    r#type: "text",
                    lang: "id",
                    dir: "ltr",
                    autocomplete: "off",
                    autocapitalize: "off",
                    spellcheck: "false",
                    aria_label: "{ui.t(Msg::RupiahType)}",
                    placeholder: "{ui.t(Msg::RupiahType)}",
                    disabled: answered.is_some(),
                    value: "{s.input}",
                    oninput: move |e: FormEvent| {
                        if let Some(s) = session.write().as_mut() {
                            s.input = e.value();
                        }
                    },
                }
            }
            if let Some(check) = &answered {
                div { class: if counts(check) { "feedback good reveal" } else { "feedback kind reveal" },
                    match check {
                        Check::Correct => rsx! { strong { "{ui.t(Msg::PlayRight)}" } },
                        Check::Close { expected } => rsx! {
                            strong { "{ui.t(Msg::RupiahClose)}" }
                            p { lang: "id", bdi { class: "latin", "{expected}" } }
                        },
                        Check::Wrong { expected, .. } => rsx! {
                            strong { "{ui.t(Msg::RupiahWrong)}" }
                            p { lang: "id", bdi { class: "latin", "{expected}" } }
                        },
                    }
                }
            }
            div { class: "row end",
                if *voice.read() == Voice::Ready {
                    button {
                        onclick: {
                            let text = drill.answer.clone();
                            move |_| speech::speak(vec![text.clone()], 0, |_| {})
                        },
                        "{ui.t(Msg::Listen)}"
                    }
                } else if *voice.read() == Voice::Missing {
                    span { class: "muted small", "{ui.t(Msg::AudioMissing)}" }
                }
                button {
                    class: "primary",
                    onclick: move |_| {
                        submit(session, &db, coach);
                    },
                    if answered.is_none() {
                        "{ui.t(Msg::Check)}"
                    } else if last {
                        "{ui.t(Msg::PlayFinish)}"
                    } else {
                        "{ui.t(Msg::Next)}"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_readings_count() {
        assert!(counts(&Check::Correct));
        assert!(counts(&Check::Close { expected: "dua puluh ribu rupiah".into() }));
        assert!(!counts(&Check::Wrong { expected: "dua puluh ribu rupiah".into(), heard: Some(12_000) }));
    }

    #[test]
    fn best_scores_are_kept_per_level() {
        assert_eq!(level_key(Level::Millions), "play.rupiah.best.Millions");
        assert_ne!(level_key(Level::Hundreds), level_key(Level::Mixed));
    }
}
