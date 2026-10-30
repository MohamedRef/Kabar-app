//! Unggah-Ungguh Simulator (spec §13): short Javanese scenes where each turn
//! offers the same thing at different speech levels. The Respect Meter rises
//! when the choice fits the person and dips when it doesn't, and every pick
//! comes with a kind explanation. The scene logic lives in
//! `kabar_core::games::unggah`; this screen draws it and logs the practice.

use dioxus::prelude::*;
use kabar_core::games::unggah::{
    Answered, Choice, Fit, METER_GREAT, RespectMeter, Run, RunResult, Scenario, Turn, UnggahData, level_rank,
};
use kabar_protocol::coach::{ActivityKind, CoachProfile, Skill};

use super::detective::choice_class;
use super::logic::meaning;
use super::{ContentProblem, PlayHeader, content, fresh_seed, seconds_since};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

/// Activity-log topic for every scene.
pub const TOPIC_UNGGAH: &str = "unggah-ungguh";

/// The learner's Javanese level (`j1`…), if the coach profile names one.
/// Until there is one, every scene is open.
pub fn javanese_level(profile: Option<&CoachProfile>) -> Option<String> {
    let level = profile?.target.as_ref()?.level.trim().to_lowercase();
    level_rank(&level).map(|_| level)
}

/// One scene in progress. The turn on screen and its shuffled choices are
/// kept after a pick so the answer can be shown before moving on.
#[derive(Clone)]
struct Session {
    run: Run,
    turn: Turn,
    /// The turn's choices in display order.
    shown: Vec<Choice>,
    /// 1-based number of the turn on screen.
    turn_no: usize,
    picked: Option<usize>,
    answer: Option<Answered>,
    start_ms: u64,
    done: bool,
}

impl Session {
    /// `None` for a scene without turns (parsing rejects those).
    fn new(scenario: &Scenario, seed: u64, start_ms: u64) -> Option<Self> {
        let run = Run::new(scenario, seed);
        let turn = run.current()?.clone();
        let shown = run.shown().into_iter().cloned().collect();
        Some(Self { run, turn, shown, turn_no: 1, picked: None, answer: None, start_ms, done: false })
    }

    /// Picks a shown choice once per turn.
    fn pick(&mut self, i: usize) -> Option<&Answered> {
        if self.picked.is_some() || self.done {
            return None;
        }
        let answer = self.run.pick(i)?;
        self.picked = Some(i);
        self.answer = Some(answer);
        self.answer.as_ref()
    }

    /// Moves to the next turn; at the end, marks the scene done and returns
    /// its result (once).
    fn next(&mut self) -> Option<RunResult> {
        if self.picked.is_none() || self.done {
            return None;
        }
        if let Some(turn) = self.run.current() {
            self.turn = turn.clone();
            self.shown = self.run.shown().into_iter().cloned().collect();
            self.turn_no += 1;
            self.picked = None;
            self.answer = None;
            None
        } else {
            self.done = true;
            self.run.result()
        }
    }

    /// Where the fitting choice sits among the shown ones.
    fn fitting_index(&self) -> usize {
        self.shown.iter().position(|c| c.fit == Fit::Fits).unwrap_or(0)
    }

    fn is_last(&self) -> bool {
        self.turn_no >= self.run.scenario().turns.len()
    }
}

fn fit_msg(fit: Fit) -> Msg {
    match fit {
        Fit::Fits => Msg::UnggahFits,
        Fit::TooPlain => Msg::UnggahTooPlain,
        Fit::TooStiff => Msg::UnggahTooStiff,
        Fit::SelfRaised => Msg::UnggahSelfRaised,
    }
}

/// The meter's colour band.
fn band(meter: RespectMeter) -> &'static str {
    match meter.0 {
        m if m >= METER_GREAT => "great",
        m if m >= 50 => "mid",
        _ => "low",
    }
}

/// The warm line at the end of a scene.
fn end_line(meter: RespectMeter) -> Msg {
    match band(meter) {
        "great" => Msg::UnggahEndGreat,
        "mid" => Msg::UnggahEndGood,
        _ => Msg::UnggahEndGentle,
    }
}

/// "J3" for `j3`.
fn level_label(level: &str) -> String {
    level.to_uppercase()
}

#[component]
pub fn Unggah() -> Element {
    let data = match content::unggah() {
        Ok(d) => d,
        Err(e) => {
            return rsx! {
                PlayHeader { title: Msg::UnggahTitle, latin: true }
                ContentProblem { detail: e.clone() }
            };
        }
    };
    let session = use_signal(|| None::<Session>);
    if session.read().is_some() {
        return rsx! {
            PlayHeader { title: Msg::UnggahTitle, latin: true }
            SceneView { session }
        };
    }
    rsx! {
        PlayHeader { title: Msg::UnggahTitle, latin: true }
        Picker { session, data_reviewed: data.reviewed }
    }
}

/// The scene cards. Scenes above the learner's Javanese level are shown
/// greyed with the level that opens them.
#[component]
fn Picker(session: Signal<Option<Session>>, data_reviewed: bool) -> Element {
    let Ok(data): Result<&UnggahData, _> = content::unggah().as_ref() else { return rsx! {} };
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let level = javanese_level(coach.profile.read().as_ref());
    let open: Vec<String> = data.open_at(level.as_deref()).iter().map(|s| s.id.clone()).collect();

    rsx! {
        crate::ui::DivCard {
            p { class: "muted", "{ui.t(Msg::UnggahIntro)}" }
        }
        if data.scenarios.is_empty() {
            crate::ui::Card { element: crate::ui::CardElement::Div, class: "", p { "{ui.t(Msg::UnggahNoScenes)}" } }
        }
        div { class: "play-grid",
            for sc in data.scenarios.iter() {
                {
                    let unlocked = open.contains(&sc.id);
                    rsx! {
                        button {
                            key: "{sc.id}",
                            class: if unlocked { "game-card uu-scene" } else { "game-card uu-scene locked" },
                            disabled: !unlocked,
                            onclick: {
                                let sc = sc.clone();
                                move |_| {
                                    let mut session = session;
                                    session.set(Session::new(&sc, fresh_seed(), crate::store::now_ms()));
                                }
                            },
                            span { class: "game-text",
                                strong { "{meaning(ui.lang(), &sc.title)}" }
                                span { class: "small",
                                    bdi { class: "latin", lang: "jv", "{sc.partner.name}" }
                                    " · {meaning(ui.lang(), &sc.partner.role)}"
                                }
                                span { class: "row",
                                    span { class: "pill uu-level", bdi { class: "latin", "{level_label(&sc.level)}" } }
                                    if !unlocked {
                                        span { class: "muted small",
                                            "{ui.t(Msg::UnggahOpensAt)} "
                                            bdi { class: "latin", "{level_label(&sc.level)}" }
                                        }
                                    }
                                    if !data_reviewed {
                                        span { class: "pill unverified small", "{ui.t(Msg::Unverified)} · " bdi { "Jawa" } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The animated Respect Meter.
#[component]
fn Meter(value: u8, delta: Option<i8>) -> Element {
    let ui = use_context::<Ui>();
    let value = value.min(100);
    let b = band(RespectMeter(value));
    rsx! {
        div { class: "uu-meter",
            div { class: "uu-meter-head",
                span { class: "uu-meter-label", "{ui.t(Msg::UnggahMeter)}" }
                span { class: "uu-meter-value",
                    if let Some(d) = delta.filter(|d| *d != 0) {
                        span {
                            key: "{value}",
                            class: if d > 0 { "uu-delta up" } else { "uu-delta down" },
                            bdi { dir: "ltr", if d > 0 { "+{ui.num(d.unsigned_abs())}" } else { "−{ui.num(d.unsigned_abs())}" } }
                        }
                    }
                    strong { "{ui.num(value)}" }
                }
            }
            div {
                class: "uu-meter-track",
                role: "meter",
                aria_label: "{ui.t(Msg::UnggahMeter)}",
                aria_valuemin: "0",
                aria_valuemax: "100",
                aria_valuenow: "{value}",
                div { class: "uu-meter-fill {b}", style: "inline-size: {value}%" }
            }
        }
    }
}

#[component]
fn SceneView(session: Signal<Option<Session>>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let reviewed = content::unggah().as_ref().map(|d| d.reviewed).unwrap_or(false);
    let Some(s) = session.read().clone() else {
        return rsx! {};
    };
    let sc = s.run.scenario().clone();
    let meter = s.run.meter();
    let total = sc.turns.len();

    let partner_card = rsx! {
        crate::ui::DivCard { class: "uu-partner",
            div { class: "row",
                strong { class: "uu-partner-name", bdi { class: "latin", lang: "jv", "{sc.partner.name}" } }
                span { class: "muted small", "{meaning(ui.lang(), &sc.partner.role)}" }
                if !reviewed {
                    span { class: "pill unverified small", "{ui.t(Msg::Unverified)}" }
                }
            }
            p { class: "small", "{meaning(ui.lang(), &sc.setting)}" }
        }
    };

    if s.done {
        let result = s.run.result();
        let (fitting, turns) = result.map_or((0, total as u32), |r| (r.fitting, r.turns));
        return rsx! {
            {partner_card}
            crate::ui::DivCard { class: "summary-card uu-end reveal",
                Meter { value: meter.0, delta: None }
                p { class: "score",
                    strong { "{ui.num(fitting)}" }
                    span { class: "muted", " / {ui.num(turns)} " }
                    span { class: "muted small", "{ui.t(Msg::UnggahFitting)}" }
                }
                p { "{ui.t(end_line(meter))}" }
                div { class: "row",
                    button {
                        class: "primary",
                        onclick: {
                            let sc = sc.clone();
                            move |_| session.set(Session::new(&sc, fresh_seed(), crate::store::now_ms()))
                        },
                        "{ui.t(Msg::PlayAgain)}"
                    }
                    button { onclick: move |_| session.set(None), "{ui.t(Msg::UnggahScenes)}" }
                }
            }
        };
    }

    let picked = s.picked;
    let fitting = s.fitting_index();
    let last = s.is_last();
    let delta = s.answer.as_ref().map(|a| a.delta);

    rsx! {
        div { class: "uu-layout",
            div { class: "uu-side",
                {partner_card}
                crate::ui::Card { element: crate::ui::CardElement::Div, class: "", Meter { value: meter.0, delta } }
            }
            crate::ui::DivCard { class: "round uu-turn",
                super::RoundCount { at: s.turn_no as u64, total: total as u64, label: Msg::UnggahTurn }
                p { "{meaning(ui.lang(), &s.turn.situation)}" }
                if let Some(said) = &s.turn.partner_says {
                    div { class: "bubble seller uu-says",
                        p { lang: "jv", bdi { class: "latin", "{said.text}" } }
                        p { class: "muted small", "{meaning(ui.lang(), &said.meaning)}" }
                    }
                }
                p { class: "muted", "{ui.t(Msg::UnggahYourTurn)}" }
                div { class: "choices uu-choices",
                    for (i , choice) in s.shown.iter().enumerate() {
                        button {
                            key: "{s.turn_no}-{i}",
                            class: choice_class(i, picked, fitting),
                            disabled: picked.is_some(),
                            onclick: move |_| {
                                if let Some(s) = session.write().as_mut() {
                                    s.pick(i);
                                }
                            },
                            bdi { class: "latin", lang: "jv", "{choice.said.text}" }
                            if picked.is_some() {
                                span { class: "hint", "{meaning(ui.lang(), &choice.said.meaning)}" }
                            }
                        }
                    }
                }
                if let Some(a) = s.answer.clone() {
                    div {
                        class: if a.fit == Fit::Fits { "feedback right reveal" } else { "feedback kind reveal" },
                        role: "status",
                        p { strong { "{ui.t(fit_msg(a.fit))}" } }
                        p { "{meaning(ui.lang(), &a.why)}" }
                        if let Some(better) = &a.better {
                            p { class: "small",
                                "{ui.t(Msg::UnggahBetter)} "
                                bdi { class: "latin", lang: "jv", "{better.said.text}" }
                                " · {meaning(ui.lang(), &better.said.meaning)}"
                            }
                        }
                        div { class: "row end",
                            button {
                                class: "primary",
                                onclick: move |_| {
                                    let finished = {
                                        let mut w = session.write();
                                        let Some(s) = w.as_mut() else { return };
                                        s.next().map(|r| (r, s.start_ms))
                                    };
                                    if let Some((r, start)) = finished {
                                        coach.record(
                                            &db,
                                            ActivityKind::Practice {
                                                skill: Skill::Speaking,
                                                topic: TOPIC_UNGGAH.to_string(),
                                                correct: r.fitting,
                                                total: r.turns,
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
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::{LocalDate, Target};

    use super::*;

    fn data() -> &'static UnggahData {
        content::unggah().as_ref().expect("the shipped scenes load")
    }

    #[test]
    fn level_comes_from_a_javanese_target() {
        assert_eq!(javanese_level(None), None);
        let mut p = CoachProfile::default();
        assert_eq!(javanese_level(Some(&p)), None, "no target: every scene open");
        p.target = Some(Target { level: "bipa-2".into(), by: LocalDate(0) });
        assert_eq!(javanese_level(Some(&p)), None);
        p.target = Some(Target { level: "J2".into(), by: LocalDate(0) });
        assert_eq!(javanese_level(Some(&p)).as_deref(), Some("j2"));
        let open = data().open_at(javanese_level(Some(&p)).as_deref());
        assert_eq!(open.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["uu.friend", "uu.market"]);
    }

    #[test]
    fn labels_and_bands() {
        assert_eq!(fit_msg(Fit::Fits), Msg::UnggahFits);
        assert_eq!(fit_msg(Fit::SelfRaised), Msg::UnggahSelfRaised);
        assert_eq!(band(RespectMeter(100)), "great");
        assert_eq!(band(RespectMeter(METER_GREAT)), "great");
        assert_eq!(band(RespectMeter(60)), "mid");
        assert_eq!(band(RespectMeter(10)), "low");
        assert_eq!(end_line(RespectMeter(90)), Msg::UnggahEndGreat);
        assert_eq!(end_line(RespectMeter(55)), Msg::UnggahEndGood);
        assert_eq!(end_line(RespectMeter(0)), Msg::UnggahEndGentle);
        assert_eq!(level_label("j3"), "J3");
    }

    #[test]
    fn a_session_shows_each_answer_before_moving_on() {
        let sc = data().scenario("uu.grandmother").expect("grandmother scene");
        let mut s = Session::new(sc, 11, 0).expect("has turns");
        assert_eq!((s.turn_no, s.shown.len()), (1, 3));
        assert!(s.next().is_none(), "can't skip an unanswered turn");
        let first_turn = s.turn.id.clone();
        let mut turns = 0;
        let result = loop {
            turns += 1;
            let slip = s.shown.iter().position(|c| c.fit != Fit::Fits).unwrap();
            let a = s.pick(slip).cloned().expect("first pick counts");
            assert!(a.better.is_some(), "a missed fit shows the one that fits");
            assert!(s.pick(slip).is_none(), "one pick per turn");
            assert_eq!(s.turn.id == first_turn, turns == 1, "the answered turn stays on screen");
            if let Some(r) = s.next() {
                break r;
            }
            assert!(!s.done && s.picked.is_none());
        };
        assert!(s.done);
        assert_eq!(turns, sc.turns.len());
        assert_eq!(result.fitting, 0);
        assert!(s.next().is_none(), "the result is logged once");
        assert_eq!(end_line(result.meter), Msg::UnggahEndGentle);
    }
}
