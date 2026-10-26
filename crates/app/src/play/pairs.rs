//! Minimal Pairs (spec §13): hear or read a Javanese word and pick its
//! spelling, d or dh, t or th. Listen mode is offered only when the device
//! has a real Javanese voice (an Indonesian voice can't make the curled
//! sounds); otherwise Read mode shows the meaning, with the how-to tip in
//! view. The logic lives in `kabar_core::games::pairs`.

use dioxus::prelude::*;
use kabar_core::games::Rng;
use kabar_core::games::pairs::{self, Contrast, PairWord, Round, Side, Tip};
use kabar_protocol::coach::{ActivityKind, Skill};

use super::detective::choice_class;
use super::logic::meaning;
use super::speech::{self, Voice};
use super::{ContentProblem, PlayHeader, RoundSummary, content, fresh_seed, seconds_since};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Hear the word (Javanese voice), pick the spelling.
    Listen,
    /// Read the meaning, pick the spelling.
    Read,
}

impl Mode {
    fn skill(self) -> Skill {
        match self {
            Self::Listen => Skill::Listening,
            Self::Read => Skill::Reading,
        }
    }
}

#[derive(Clone, PartialEq)]
struct Session {
    mode: Mode,
    rounds: Vec<Round>,
    index: usize,
    picked: Option<Side>,
    /// (correct, answered) per contrast, in `Contrast::ALL` order.
    tally: [(u32, u32); 2],
    start_ms: u64,
    done: bool,
}

impl Session {
    fn correct(&self) -> u32 {
        self.tally.iter().map(|t| t.0).sum()
    }
}

fn contrast_index(c: Contrast) -> usize {
    Contrast::ALL.iter().position(|x| *x == c).unwrap_or(0)
}

/// `d / dh` or `t / th`.
fn contrast_label(c: Contrast) -> String {
    format!("{} / {}", c.plain(), c.curled())
}

fn lang_label(lang: &str) -> Msg {
    if lang == "jv" { Msg::PairsJavanese } else { Msg::PairsIndonesian }
}

/// Starts a game; in Listen mode, says the first word.
fn start(contrast: Option<Contrast>, mode: Mode) -> Option<Session> {
    let data = content::pairs().as_ref().ok()?;
    let rounds = pairs::rounds(data, contrast, pairs::ROUND_LEN, &mut Rng::new(fresh_seed()));
    if mode == Mode::Listen
        && let Some(r) = rounds.first()
    {
        speech::speak_javanese(r.target_word().word.clone());
    }
    Some(Session {
        mode,
        rounds,
        index: 0,
        picked: None,
        tally: [(0, 0); 2],
        start_ms: crate::store::now_ms(),
        done: false,
    })
}

#[component]
pub fn Pairs() -> Element {
    if let Err(e) = content::pairs() {
        return rsx! {
            PlayHeader { title: Msg::PairsTitle, latin: true }
            ContentProblem { detail: e.clone() }
        };
    }
    let voice = speech::use_javanese_voice();
    let contrast = use_signal(|| None::<Contrast>);
    let listen = use_signal(|| true);
    let session = use_signal(|| None::<Session>);
    use_drop(speech::stop);
    let playing = session.read().is_some();

    rsx! {
        PlayHeader { title: Msg::PairsTitle, latin: true }
        if playing {
            Rounds { session }
        } else {
            Intro { contrast, listen, session, voice }
        }
    }
}

/// Pick a contrast and a mode, and read how to make the sounds.
#[component]
fn Intro(
    contrast: Signal<Option<Contrast>>,
    listen: Signal<bool>,
    session: Signal<Option<Session>>,
    voice: Signal<Voice>,
) -> Element {
    let Ok(data) = content::pairs() else { return rsx! {} };
    let ui = use_context::<Ui>();
    let current = *contrast.read();
    let voice_now = *voice.read();
    let mode = if voice_now == Voice::Ready && *listen.read() { Mode::Listen } else { Mode::Read };
    let tips: Vec<Tip> =
        data.tips.iter().filter(|t| current.is_none_or(|c| c == t.contrast)).cloned().collect();

    rsx! {
        div { class: "card pairs-intro",
            p { class: "muted", "{ui.t(Msg::PairsIntro)}" }
            if !data.reviewed {
                p {
                    span { class: "pill unverified small", "{ui.t(Msg::Unverified)} · " bdi { "Jawa" } }
                }
            }
            div { class: "row", role: "group",
                for c in Contrast::ALL {
                    button {
                        key: "{c:?}",
                        aria_pressed: if current == Some(c) { "true" } else { "false" },
                        onclick: move |_| contrast.set(Some(c)),
                        bdi { class: "latin", lang: "jv", "{contrast_label(c)}" }
                    }
                }
                button {
                    aria_pressed: if current.is_none() { "true" } else { "false" },
                    onclick: move |_| contrast.set(None),
                    "{ui.t(Msg::PairsBoth)}"
                }
            }
            if voice_now == Voice::Ready {
                div { class: "row", role: "group",
                    button {
                        aria_pressed: if mode == Mode::Listen { "true" } else { "false" },
                        onclick: move |_| listen.set(true),
                        "{ui.t(Msg::PairsListenMode)}"
                    }
                    button {
                        aria_pressed: if mode == Mode::Read { "true" } else { "false" },
                        onclick: move |_| listen.set(false),
                        "{ui.t(Msg::PairsReadMode)}"
                    }
                }
            } else if voice_now == Voice::Missing {
                p { class: "muted small", "{ui.t(Msg::PairsNoJavaneseVoice)}" }
            }
        }
        div { class: "pairs-tips",
            for tip in tips {
                TipCard { key: "{tip.contrast:?}", tip }
            }
        }
        div { class: "row",
            button {
                class: "primary",
                onclick: move |_| {
                    let mut session = session;
                    session.set(start(*contrast.peek(), mode));
                },
                "{ui.t(Msg::Start)}"
            }
        }
    }
}

/// How to make one contrast's sounds, for an Egyptian Arabic speaker.
#[component]
fn TipCard(tip: Tip) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "card pairs-tip",
            p { class: "pairs-tip-title",
                strong { "{ui.t(Msg::PairsTipTitle)} " }
                bdi { class: "latin", lang: "jv", "{contrast_label(tip.contrast)}" }
            }
            p { "{meaning(ui.lang(), &tip.text)}" }
        }
    }
}

/// A word with its contrast letters marked.
#[component]
fn Spelling(word: PairWord, segments: Vec<(String, bool)>) -> Element {
    rsx! {
        bdi { class: "latin pair-word", lang: "{word.lang}",
            for (i , (text , marked)) in segments.into_iter().enumerate() {
                if marked {
                    mark { key: "{i}", class: "pair-mark", "{text}" }
                } else {
                    span { key: "{i}", "{text}" }
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
    let data = content::pairs().as_ref().ok();
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
            RoundSummary { correct: s.correct(), total, onagain: move |_| session.set(None) }
        };
    }

    let round = s.rounds[s.index].clone();
    let picked = s.picked;
    let last = s.index + 1 == s.rounds.len();
    let listen = s.mode == Mode::Listen;
    let lang = ui.lang();
    let answer = round.options.iter().position(|o| *o == round.target).unwrap_or(0);
    let picked_index = picked.and_then(|p| round.options.iter().position(|o| *o == p));
    let target = round.target_word().clone();
    let reviewed = data.is_some_and(|d| d.reviewed);
    let tip = data.and_then(|d| d.tip(round.pair.contrast)).cloned();
    let in_lang = if target.lang == "jv" { Msg::PairsInJavanese } else { Msg::PairsInIndonesian };

    rsx! {
        div { class: "card round pairs-round",
            div { class: "row pairs-meta",
                span { class: "muted small", "{ui.num(s.index as u64 + 1)} / {ui.num(total)}" }
                span { class: "pill small", bdi { class: "latin", lang: "jv", "{contrast_label(round.pair.contrast)}" } }
                if !reviewed {
                    span { class: "pill unverified small", "{ui.t(Msg::Unverified)} · " bdi { "Jawa" } }
                }
            }
            if listen {
                p { class: "muted", "{ui.t(Msg::PairsWhichHeard)}" }
                div { class: "row center",
                    button {
                        class: "pairs-hear",
                        onclick: {
                            let word = target.word.clone();
                            move |_| speech::speak_javanese(word.clone())
                        },
                        span { aria_hidden: "true", "🔊 " }
                        "{ui.t(Msg::PairsHear)}"
                    }
                }
            } else {
                p { class: "muted", "{ui.t(Msg::PairsWhichSpelling)}" }
                p { class: "pairs-prompt",
                    span { class: "muted small", "{ui.t(in_lang)} " }
                    strong { "{meaning(lang, &target.meaning)}" }
                }
            }
            div { class: "choices two",
                for (i , side) in round.options.into_iter().enumerate() {
                    button {
                        key: "{i}",
                        class: choice_class(i, picked_index, answer),
                        disabled: picked.is_some(),
                        onclick: move |_| {
                            let mut w = session.write();
                            if let Some(s) = w.as_mut().filter(|s| s.picked.is_none()) {
                                s.picked = Some(side);
                                let r = &s.rounds[s.index];
                                let t = &mut s.tally[contrast_index(r.pair.contrast)];
                                t.1 += 1;
                                if r.check(side) {
                                    t.0 += 1;
                                }
                            }
                        },
                        bdi { class: "latin", lang: "{round.word(side).lang}", "{round.word(side).word}" }
                    }
                }
            }
            if let Some(p) = picked {
                div { class: if round.check(p) { "feedback right reveal" } else { "feedback kind reveal" },
                    if round.check(p) {
                        strong { "{ui.t(Msg::PlayRight)}" }
                    } else {
                        strong { "{ui.t(Msg::PlayNotQuite)} " }
                        bdi { class: "latin", lang: "{target.lang}", "{target.word}" }
                    }
                    p { class: "muted small", "{ui.t(Msg::PairsTheTwoWords)}" }
                    ul { class: "pairs-compare",
                        for side in [Side::Plain, Side::Curled] {
                            li {
                                key: "{side:?}",
                                class: if side == round.target { "pairs-word target" } else { "pairs-word" },
                                Spelling { word: round.word(side).clone(), segments: round.pair.segments(side) }
                                span { class: "pill small", "{ui.t(lang_label(&round.word(side).lang))}" }
                                span { class: "pairs-meaning", "{meaning(lang, &round.word(side).meaning)}" }
                                if listen {
                                    button {
                                        class: "quiet icon-button",
                                        aria_label: ui.t(Msg::PairsHear),
                                        title: ui.t(Msg::PairsHear),
                                        onclick: {
                                            let word = round.word(side).word.clone();
                                            move |_| speech::speak_javanese(word.clone())
                                        },
                                        span { aria_hidden: "true", "🔊" }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(note) = &round.pair.note {
                        p { class: "small", "{meaning(lang, note)}" }
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
                                        Some((s.mode, s.tally, s.start_ms))
                                    } else {
                                        s.index += 1;
                                        s.picked = None;
                                        if s.mode == Mode::Listen {
                                            speech::speak_javanese(s.rounds[s.index].target_word().word.clone());
                                        }
                                        None
                                    }
                                };
                                if let Some((mode, tally, start)) = finished {
                                    // The time counts once, with the first entry.
                                    let mut seconds = Some(seconds_since(start));
                                    for (c, (correct, total)) in Contrast::ALL.into_iter().zip(tally) {
                                        if total > 0 {
                                            coach.record(
                                                &db,
                                                ActivityKind::Practice {
                                                    skill: mode.skill(),
                                                    topic: c.topic().to_string(),
                                                    correct,
                                                    total,
                                                },
                                                seconds.take().unwrap_or(0),
                                            );
                                        }
                                    }
                                }
                            },
                            if last { "{ui.t(Msg::PlayFinish)}" } else { "{ui.t(Msg::Next)}" }
                        }
                    }
                }
            }
        }
        if !listen && let Some(tip) = tip {
            TipCard { tip }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_log_their_skill() {
        assert_eq!(Mode::Listen.skill(), Skill::Listening);
        assert_eq!(Mode::Read.skill(), Skill::Reading);
    }

    #[test]
    fn contrasts_have_labels_and_slots() {
        assert_eq!(contrast_label(Contrast::DDh), "d / dh");
        assert_eq!(contrast_label(Contrast::TTh), "t / th");
        assert_eq!(contrast_index(Contrast::DDh), 0);
        assert_eq!(contrast_index(Contrast::TTh), 1);
        assert_eq!(lang_label("jv"), Msg::PairsJavanese);
        assert_eq!(lang_label("id"), Msg::PairsIndonesian);
    }

    #[test]
    fn the_shipped_pairs_load() {
        let data = content::pairs().as_ref().expect("minimal-pairs.toml loads");
        assert!(data.tip(Contrast::DDh).is_some() && data.tip(Contrast::TTh).is_some());
    }
}
