//! The monthly level check (SPEC §6): about ten minutes of the current
//! level's test items, then the skill radar against last month, Kak Nur's
//! decision (`kabar_core::coach::level::decide`, never re-implemented here)
//! and the pace forecast. Works offline: no AI is needed for any of it.
//!
//! A down move is framed as consolidating the base; nothing is taken away
//! and the journey stays open. A strong mini check doesn't move the level by
//! itself (the golden rule: the full level test decides), so it invites the
//! full test instead.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use kabar_core::coach::level::{Moved, decide};
use kabar_core::coach::voice::Line;
use kabar_core::lesson::level_test::{Scored, SelfGrade, TestResults, TestRunner, TestStep, parse_test};
use kabar_core::lesson::{Answer, pick, references};
use kabar_core::stats::level_check::{self as lc, mini_check, month_seed};
use kabar_core::stats::{self as core_stats, pace};
use kabar_protocol::coach::{LocalDate, Skill};
use kabar_protocol::lesson::ExerciseKind;
use kabar_protocol::level_check::{
    LEVEL_CHECK_KIND, LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_VERSION, LevelCheckResult, LevelEta, LevelMove,
    RadarPoint,
};
use kabar_protocol::test::{TestContent, TestItemKind, TestSkill};

use super::text::{S, st};
use super::{fill, iso, level_label, level_tests, load_logs, pct, rtl};
use crate::annotated::Annotated;
use crate::coach::{Coach, labels, say, widgets::Nur};
use crate::lesson::text::{L, lt};
use crate::lesson::{
    Draft, ExerciseView, MAX_STEP_MS, Rich, TextAnswer, autoplay_text, draft_answer, instruction,
    item_prompt, needs_audio, now_ms, option_count, speak, target_tag,
};
use crate::{Db, Screen, Ui};

const LESSON_CSS: &str = include_str!("../../assets/lesson.css");
const TEST_CSS: &str = include_str!("../../assets/test.css");
/// Only the Indonesian route has level tests so far.
const LANGUAGE: &str = "id";

/// An open answer to self-assess: (id, prompt, answer, references, grade).
type SelfRow = (String, String, String, Vec<String>, Option<SelfGrade>);

/// What the check starts from, worked out once when the screen opens.
#[derive(Clone, PartialEq)]
struct Setup {
    today: LocalDate,
    level: String,
    /// The full test's pass mark, when the level has a test.
    pass_mark: Option<f32>,
    /// `None` when Kabar has no test items for this level yet.
    mini: Option<TestContent>,
    previous: Option<LevelCheckResult>,
    /// Kak Nur's welcome on the intro, worded for how the check was reached.
    welcome: Line,
}

/// Kak Nur's welcome: a first check is introduced as the starting point; one
/// opened from the monthly invitation says a month has passed; one opened by
/// hand is simply welcome, whenever the learner likes.
fn welcome_line(first: bool, invited: bool) -> Line {
    match (first, invited) {
        (true, _) => Line::LevelCheckFirst,
        (false, true) => Line::LevelCheckInvite,
        (false, false) => Line::LevelCheckAnytime,
    }
}

fn setup(db: &Db, coach: &Coach) -> Setup {
    let today = coach.today();
    let checks = super::level_checks(db);
    let journey = super::journey_position(db).map(|(l, _)| l).unwrap_or_else(|| "bipa1".into());
    let (_, last_pass) = level_tests(db, &journey, coach.offset());
    let previous = checks.last().cloned();
    let last = previous.as_ref().map(|c| (c.level.as_str(), c.moved, c.date));
    let level = lc::current_level(&journey, last, last_pass);
    let full = crate::lesson::test_content(&format!("{level}.test")).and_then(|y| parse_test(&y).ok());
    Setup {
        today,
        pass_mark: full.as_ref().and_then(|t| t.pass_mark),
        mini: full.as_ref().map(|t| mini_check(t, month_seed(today))),
        level,
        welcome: welcome_line(previous.is_none(), *super::CHECK_INVITED.peek()),
        previous,
    }
}

/// Works out and saves the outcome. `results` is `None` when there was no
/// mini check (history only).
fn conclude(db: &Db, coach: Coach, s: &Setup, runner: Option<&TestRunner>) -> LevelCheckResult {
    let today = coach.today();
    let now = coach.now();
    let logs = load_logs(db);
    let events = coach.events.peek().clone();
    let totals = core_stats::day_totals(&events, &logs.log, coach.offset());
    let res: Option<TestResults> = runner.map(TestRunner::results);

    // The radar: this check's skill shares blended with the last month.
    let check_skills: Vec<(TestSkill, f32)> =
        res.iter().flat_map(|r| r.skills.iter().filter_map(|(sk, t)| Some((*sk, t.share()?)))).collect();
    let recent = lc::recent_skills(&events, today, lc::LOOKBACK_DAYS);
    let radar = lc::radar(&check_skills, &recent);

    // Kak Nur's rule, fed with the full level test (if fresh) and the month.
    let sched = crate::schedule::scheduler(&coach, &db.0.borrow());
    let retention = core_stats::retention(&sched, &logs.log, now, lc::LOOKBACK_DAYS as u64);
    let active = core_stats::window(&totals, today, lc::LOOKBACK_DAYS).days;
    let (tests, _) = level_tests(db, &s.level, coach.offset());
    let level_test = s.pass_mark.and_then(|mark| lc::fresh_level_test(&tests, mark, today));
    let stats = lc::check_stats(&events, active, retention.all().share(), level_test, today);
    let decision = decide(&s.level, &stats);

    // Pace forecast for the learner's goal from the level they're now in.
    let share = if decision.moved == Moved::Stay { super::level_share(db, &decision.level) } else { 0.0 };
    let target = coach.profile.peek().as_ref().and_then(|p| p.target.as_ref().map(|t| t.level.clone()));
    let goal = pace::goal_level(&decision.level, target.as_deref()).to_string();
    let eta = pace::pace(&totals, today)
        .and_then(|p| pace::eta(&decision.level, share, &goal, p, today))
        .map(|date| LevelEta { level: goal.clone(), date });

    let result = LevelCheckResult {
        v: LEVEL_CHECK_VERSION,
        date: today,
        language: LANGUAGE.into(),
        from_level: s.level.clone(),
        level: decision.level.clone(),
        moved: lc::level_move(decision.moved),
        score: res.as_ref().and_then(|r| r.total.share()),
        radar,
        eta,
        missed: res.as_ref().map(|r| r.missed.iter().map(|m| m.id.clone()).collect()).unwrap_or_default(),
        duration_ms: res.as_ref().map_or(0, |r| r.duration_ms),
    };
    if let Err(e) = db.0.borrow_mut().record_lesson_event(LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_KIND, &result) {
        crate::diag::report!("saving the level check failed: {e}");
    }
    // Kak Nur's log: one practice entry per skill (reports, weak spots).
    if let Some(r) = runner {
        let mut by_skill: BTreeMap<TestSkill, (u32, u32)> = BTreeMap::new();
        for rec in r.records().iter().filter(|x| matches!(x.scored, Scored::Earned(_))) {
            let t = by_skill.entry(rec.skill).or_default();
            t.0 += u32::from(!rec.missed());
            t.1 += 1;
        }
        let mut seconds = (result.duration_ms / 1000) as u32;
        for kind in lc::practice_entries(&by_skill, &s.level) {
            coach.record(db, kind, std::mem::take(&mut seconds));
        }
    }
    result
}

#[derive(Clone, PartialEq)]
enum Phase {
    Intro,
    Running,
    Outcome(LevelCheckResult),
}

#[component]
pub fn LevelCheckScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let s = use_hook(|| setup(&db, &coach));
    let mut phase = use_signal(|| Phase::Intro);
    let runner = use_signal(|| s.mini.clone().map(TestRunner::new));
    let lang = ui.lang();

    let body = match phase() {
        Phase::Intro => {
            let s2 = s.clone();
            let db2 = db.clone();
            rsx! {
                Intro {
                    level: s.level.clone(),
                    welcome: s.welcome,
                    has_test: s.mini.is_some(),
                    on_start: move |_| {
                        if s2.mini.is_some() {
                            let mut r = runner;
                            if let Some(r) = r.write().as_mut() {
                                r.start();
                            }
                            phase.set(Phase::Running);
                        } else {
                            phase.set(Phase::Outcome(conclude(&db2, coach, &s2, None)));
                        }
                    },
                }
            }
        }
        Phase::Running => {
            let s2 = s.clone();
            let db2 = db.clone();
            rsx! {
                Runner {
                    runner,
                    on_done: move |_| {
                        let r = runner.peek();
                        let result = conclude(&db2, coach, &s2, r.as_ref());
                        drop(r);
                        phase.set(Phase::Outcome(result));
                    },
                }
            }
        }
        Phase::Outcome(result) => rsx! {
            Outcome { result, previous: s.previous.clone(), pass_mark: s.pass_mark, has_test: s.mini.is_some() }
        },
    };
    rsx! {
        style { {LESSON_CSS} }
        style { {TEST_CSS} }
        style { {super::STATS_CSS} }
        div { class: "st st-check",
            div { class: "st-head",
                button {
                    class: "st-back",
                    "data-back": "true",
                    aria_label: st(lang, S::CheckClose),
                    title: st(lang, S::CheckClose),
                    onclick: move |_| super::back(ui),
                    "{st(lang, S::Back)}"
                }
                h2 { "{st(lang, S::CheckTitle)}" }
            }
            {body}
        }
    }
}

#[component]
fn Intro(level: String, welcome: Line, has_test: bool, on_start: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let lang = ui.lang();
    let label = iso(&level_label(&level));
    let name = coach.profile.read().as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let invite = say(&ui, welcome, coach.today(), &[("name", &name)]);
    let (text, button) = if has_test {
        (fill(st(lang, S::CheckIntro), &[("level", &label)]), S::CheckStart)
    } else {
        (fill(st(lang, S::CheckNoTest), &[("level", &label)]), S::CheckSeeResult)
    };
    rsx! {
        section { class: "card st-card",
            Nur { text: invite }
            p { "{text}" }
            button { class: "primary", onclick: move |_| on_start.call(()), "{st(lang, button)}" }
        }
    }
}

/// The items, one at a time, no feedback; then the honest self-check of open
/// answers. Reuses the Lesson Player's exercise views.
#[component]
fn Runner(runner: Signal<Option<TestRunner>>, on_done: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let draft = use_signal(Draft::default);
    let last = use_signal(now_ms);
    let missing = use_signal(|| false);

    let commit = move |op: &dyn Fn(&mut TestRunner)| {
        let now = now_ms();
        let mut last = last;
        let delta = now.saturating_sub(*last.peek()).min(MAX_STEP_MS);
        last.set(now);
        let mut runner = runner;
        let step = {
            let mut w = runner.write();
            let Some(r) = w.as_mut() else { return };
            if matches!(r.step(), TestStep::Item { .. }) {
                r.add_elapsed(delta);
            }
            op(r);
            r.step()
        };
        let mut draft = draft;
        draft.set(Draft::default());
        if step == TestStep::Results {
            on_done.call(());
            return;
        }
        let r = runner.peek();
        if let Some((text, audio)) = r.as_ref().and_then(|r| r.exercise()).and_then(|ex| autoplay_text(&ex)) {
            speak(&text, LANGUAGE, false, audio.as_deref(), missing);
        }
        document::eval("document.querySelector('.st-check')?.scrollIntoView({block: 'start'});");
    };

    // No sound for Indonesian: listening items are skipped the "can't
    // listen now" way (unscored), with a note, as in the other tests.
    let mut voice_skips = use_signal(|| 0u32);
    use_effect(move || {
        if !*missing.read() {
            return;
        }
        let skip = {
            let r = runner.read();
            r.as_ref().is_some_and(|r| {
                matches!(r.step(), TestStep::Item { .. }) && r.exercise().is_some_and(|ex| needs_audio(&ex))
            })
        };
        if skip {
            voice_skips += 1;
            commit(&|r| r.skip());
        }
    });
    // Play the first item's audio as it opens, as the Lesson Player does
    // (later items play from `commit`). Without a voice this is what sets
    // `missing`, so a listening item is skipped on sight, not after Play.
    use_hook(move || {
        let r = runner.peek();
        let first =
            r.as_ref().filter(|r| matches!(r.step(), TestStep::Item { .. })).and_then(|r| r.exercise());
        if let Some((text, audio)) = first.and_then(|ex| autoplay_text(&ex)) {
            speak(&text, LANGUAGE, false, audio.as_deref(), missing);
        }
    });

    // Enter = next, 1–9 = pick an option (same keys as the Lesson Player and
    // the level tests).
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            if (window.__kabarCheckKeys) document.removeEventListener("keydown", window.__kabarCheckKeys, true);
            window.__kabarCheckKeys = (e) => {
                const root = document.querySelector(".st-runner");
                if (!root || e.isComposing) return;
                const t = e.target;
                if (t.closest && t.closest(".lt-selfcheck")) return;
                const typing = t.tagName === "TEXTAREA" || t.tagName === "INPUT";
                if (e.key === "Enter" && !e.shiftKey) {
                    const next = root.querySelector(".lp-primary");
                    if (next && !next.disabled && (!t.closest || !t.closest("button"))) {
                        e.preventDefault();
                        next.click();
                    }
                } else if (!typing && !e.ctrlKey && !e.metaKey && !e.altKey && /^[1-9]$/.test(e.key)) {
                    dioxus.send(e.key);
                }
            };
            document.addEventListener("keydown", window.__kabarCheckKeys, true);
            await new Promise(() => {});
            "#,
        );
        while let Ok(key) = eval.recv::<String>().await {
            let Ok(n) = key.parse::<usize>() else { continue };
            let count = runner.peek().as_ref().and_then(|r| r.exercise()).map_or(0, |ex| option_count(&ex));
            if (1..=count).contains(&n) {
                let mut d = draft;
                d.write().choice = Some(n - 1);
            }
        }
    });
    use_drop(|| {
        document::eval(
            r#"if (window.__kabarCheckKeys) { document.removeEventListener("keydown", window.__kabarCheckKeys, true); window.__kabarCheckKeys = null; }"#,
        );
    });

    let guard = runner.read();
    let Some(r) = guard.as_ref() else { return rsx! {} };
    let step = r.step();
    let pct_done = (r.fraction() * 100.0).round();
    let item = r.item().cloned();
    let exercise = r.exercise();
    let section = r.section().cloned();
    let tiles = exercise.as_ref().map(|ex| r.tiles(ex)).unwrap_or_default();
    let pair_order = exercise.as_ref().map(|ex| r.pair_order(ex)).unwrap_or_default();
    let self_rows: Vec<SelfRow> = r
        .open_answers()
        .map(|rec| {
            let it = &r.test().sections[rec.section].items[rec.item];
            // The same "what was asked" line as the level tests' self-check,
            // so a translate item shows the sentence it asked about.
            let refs = match (&it.kind, it.exercise()) {
                (TestItemKind::WritingTask(t) | TestItemKind::SpeakingTask(t), _) => {
                    vec![t.sample.trim().to_string()]
                }
                (_, Some(ex)) => references(&ex, lang).into_iter().take(3).collect(),
                _ => Vec::new(),
            };
            (
                rec.id.clone(),
                item_prompt(it, lang).trim().to_string(),
                r.pending_answer(&rec.id).unwrap_or_default().to_string(),
                refs,
                rec.self_grade,
            )
        })
        .collect();
    let self_done = r.self_check_done();
    let target = target_tag(LANGUAGE);
    drop(guard);

    let can_next = match item.as_ref().map(|i| &i.kind) {
        Some(TestItemKind::Exercise(ExerciseKind::MatchPairs { .. })) => false,
        Some(TestItemKind::Exercise(_)) => {
            exercise.as_ref().is_some_and(|ex| draft_answer(ex, &draft.read(), &tiles).is_some())
        }
        Some(TestItemKind::WritingTask(_)) => !draft.read().text.trim().is_empty(),
        _ => true,
    };
    let can_skip = matches!(
        exercise.as_ref().map(|e| &e.kind),
        Some(ExerciseKind::Listen { .. } | ExerciseKind::Dictation { .. } | ExerciseKind::Speaking { .. })
    );
    let primary = move || {
        let step = runner.peek().as_ref().map(TestRunner::step);
        match step {
            Some(TestStep::Item { .. }) => {
                let (kind, answer) = {
                    let r = runner.peek();
                    let Some(r) = r.as_ref() else { return };
                    let kind = r.item().map(|i| i.kind.clone());
                    let answer = r.exercise().and_then(|ex| draft_answer(&ex, &draft.peek(), &r.tiles(&ex)));
                    (kind, answer)
                };
                match kind {
                    Some(TestItemKind::Exercise(_)) => {
                        if let Some(a) = answer {
                            commit(&|r| r.answer(&a, lang));
                        }
                    }
                    Some(TestItemKind::WritingTask(_)) => {
                        let text = draft.peek().text.clone();
                        if !text.trim().is_empty() {
                            commit(&|r| r.answer_open(&text));
                        }
                    }
                    Some(TestItemKind::SpeakingTask(_)) => commit(&|r| r.answer_open("")),
                    None => {}
                }
            }
            Some(TestStep::SelfCheck) => commit(&|r| r.finish_self_check()),
            _ => {}
        }
    };
    let label = match (step, item.as_ref().map(|i| &i.kind)) {
        (TestStep::SelfCheck, _) => lt(lang, L::SeeResults),
        (_, Some(TestItemKind::SpeakingTask(_))) => lt(lang, L::ISpoke),
        (_, Some(TestItemKind::Exercise(ExerciseKind::Speaking { .. }))) => lt(lang, L::ISaidIt),
        _ => st(lang, S::CheckNext),
    };
    let enabled = if step == TestStep::SelfCheck { self_done } else { can_next };

    rsx! {
        div { class: "lp lt st-runner",
            div {
                class: "lp-progress",
                role: "progressbar",
                aria_valuemin: "0",
                aria_valuemax: "100",
                aria_valuenow: "{pct_done}",
                div { class: "lp-bar", style: "inline-size: {pct_done}%" }
            }
            div { class: "lp-col", key: "{step:?}",
                if *voice_skips.read() > 0 && matches!(step, TestStep::Item { .. }) {
                    p { class: "muted lp-small lp-voice-note", role: "status", "{lt(lang, L::NoVoiceSkipped)}" }
                }
                if let (TestStep::Item { .. }, Some(it)) = (step, item.clone()) {
                    if let Some(sec) = &section {
                        header { class: "lt-section",
                            strong { Rich { text: pick(&sec.title, lang).to_string(), target } }
                        }
                    }
                    match (&it.kind, exercise.clone()) {
                        (TestItemKind::Exercise(_), Some(ex)) => rsx! {
                            p { class: "lp-instruction", "{lt(lang, instruction(&ex, LANGUAGE))}" }
                            ExerciseView {
                                ex,
                                language: LANGUAGE.to_string(),
                                draft,
                                locked: false,
                                tiles: tiles.clone(),
                                pair_order: pair_order.clone(),
                                missing,
                                on_matched: move |missed: Vec<String>| commit(&|r| r.answer(&Answer::Matched { missed: missed.clone() }, lang)),
                            }
                        },
                        (TestItemKind::WritingTask(task), _) => rsx! {
                            p { class: "lp-instruction", "{lt(lang, L::WriteInId)}" }
                            p { class: "lp-question-meaning pre", Rich { text: pick(&task.prompt, lang).trim().to_string(), target } }
                            TextAnswer { tag: target, draft, locked: false }
                        },
                        (TestItemKind::SpeakingTask(task), _) => rsx! {
                            p { class: "lp-instruction", "{lt(lang, L::SpeakTask)}" }
                            p { class: "lp-question-meaning pre", Rich { text: pick(&task.prompt, lang).trim().to_string(), target } }
                            p { class: "muted lp-small", "{lt(lang, L::SpeakTaskNote)}" }
                        },
                        _ => rsx! {},
                    }
                }
                if step == TestStep::SelfCheck {
                    h3 { "{lt(lang, L::SelfCheckTitle)}" }
                    p { class: "muted", "{lt(lang, L::SelfCheckBody)}" }
                    for (id, prompt, answer, refs, chosen) in self_rows {
                        section { key: "{id}", class: "card lt-open",
                            if !prompt.is_empty() {
                                p { class: "lp-question-meaning pre", Rich { text: prompt.clone(), target } }
                            }
                            p { class: "muted lp-small", "{lt(lang, L::YourAnswer)}" }
                            if answer.trim().is_empty() {
                                p { class: "muted", "{lt(lang, L::Spoken)}" }
                            } else {
                                p { class: "lt-answer pre", bdi { dir: "auto", "{answer}" } }
                            }
                            p { class: "muted lp-small", "{lt(lang, L::Sample)}" }
                            for (i, a) in refs.iter().enumerate() {
                                p { key: "{i}", class: "lt-sample pre", Annotated { text: a.clone(), tag: target } }
                            }
                            div { class: "row lt-grades", role: "group", aria_label: lt(lang, L::SelfRated),
                                for (g, gl) in [(SelfGrade::NotYet, L::GradeNone), (SelfGrade::Partly, L::GradePartly), (SelfGrade::Fully, L::GradeFully)] {
                                    button {
                                        key: "{gl:?}",
                                        class: "chip",
                                        aria_pressed: if chosen == Some(g) { "true" } else { "false" },
                                        onclick: {
                                            let id = id.clone();
                                            move |_| {
                                                let mut runner = runner;
                                                if let Some(r) = runner.write().as_mut() {
                                                    r.self_assess(&id, g);
                                                }
                                            }
                                        },
                                        "{lt(lang, gl)}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "lp-buttons st-runner-actions",
                if can_skip && matches!(step, TestStep::Item { .. }) {
                    button { class: "lp-skip", onclick: move |_| commit(&|r| r.skip()), "{lt(lang, L::CantListenNow)}" }
                }
                button { class: "primary lp-primary", disabled: !enabled, onclick: move |_| primary(), "{label}" }
            }
        }
    }
}

/// "BIPA 3 around 1/5/2027", in Kak Nur's words, and the pace behind it.
#[component]
pub fn EtaLine(level: String, date: LocalDate, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let date_text = labels::date(&ui, date, today);
    let line = say(&ui, Line::LevelForecast, today, &[("level", &level_label(&level)), ("date", &date_text)]);
    rsx! {
        div { class: "st-eta",
            h4 { "{st(ui.lang(), S::ForecastLevelTitle)}" }
            p { "{line}" }
        }
    }
}

fn skill_name(ui: &Ui, s: Skill) -> &'static str {
    ui.t(labels::skill(s))
}

/// The radar, its legend, and the same numbers as text (with the change
/// since last month).
#[component]
pub fn RadarView(now: Vec<RadarPoint>, before: Option<Vec<RadarPoint>>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let axis = Skill::ALL.map(|s| skill_name(&ui, s).to_string());
    let svg =
        super::charts::radar(&now, before.as_deref(), rtl(&ui), st(lang, S::RadarTitle), &axis, |s, v| {
            fill(st(lang, S::RadarSummary), &[("skill", skill_name(&ui, s)), ("value", &pct(&ui, v))])
        });
    let change = lc::radar_change(&now, before.as_deref().unwrap_or(&[]));
    let rows: Vec<(Skill, Option<f32>, Option<f32>)> = Skill::ALL
        .iter()
        .zip(change)
        .map(|(&s, (_, d))| (s, now.iter().find(|p| p.skill == s).map(|p| p.value), d))
        .collect();
    let has_before = before.as_ref().is_some_and(|b| !b.is_empty());
    rsx! {
        div { class: "st-radar",
            h4 { "{st(lang, S::RadarTitle)}" }
            div { class: "st-chart st-radar-chart", dangerous_inner_html: svg }
            ul { class: "st-keys",
                li { span { class: "st-key now", aria_hidden: "true" } "{st(lang, S::RadarNow)}" }
                if has_before {
                    li { span { class: "st-key before", aria_hidden: "true" } "{st(lang, S::RadarBefore)}" }
                }
            }
            ul { class: "st-radar-list",
                for (s, v, d) in rows {
                    li { key: "{s:?}",
                        span { "{skill_name(&ui, s)}" }
                        match v {
                            Some(v) => rsx! {
                                span { class: "st-big", bdi { "{pct(&ui, v)}" } }
                                if let Some(d) = d {
                                    DeltaBadge { delta: d }
                                }
                            },
                            None => rsx! { span { class: "muted small", "{st(lang, S::RadarNotMeasured)}" } },
                        }
                    }
                }
            }
            if !has_before {
                p { class: "muted small", "{st(lang, S::RadarFirst)}" }
            }
        }
    }
}

/// "+6" / "−4" points; a dip is information, shown in the same calm ink.
#[component]
fn DeltaBadge(delta: f32) -> Element {
    let ui = use_context::<Ui>();
    let points = (delta * 100.0).round() as i64;
    let sign = match points.signum() {
        1 => "+",
        -1 => "−",
        _ => "±",
    };
    let text = format!("{sign}{}", ui.num(points.unsigned_abs()));
    rsx! {
        span { class: if points > 0 { "st-delta up" } else { "st-delta" }, bdi { dir: "ltr", "{text}" } }
    }
}

#[component]
fn Outcome(
    result: LevelCheckResult,
    previous: Option<LevelCheckResult>,
    pass_mark: Option<f32>,
    has_test: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let lang = ui.lang();
    let today = result.date;
    let name = coach.profile.read().as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let level = level_label(&result.level);
    let (title, nur, body) = match result.moved {
        LevelMove::Up => (S::ResultUp, Some(Line::LevelUp), None),
        LevelMove::Down => (S::ResultDown, Some(Line::LevelRebuild), Some(S::DownBody)),
        LevelMove::Stay => (S::ResultStay, None, Some(S::StayBody)),
    };
    let nur_text = nur.map(|l| say(&ui, l, today, &[("name", &name), ("level", &level)]));
    let body_text = body.map(|b| fill(st(lang, b), &[("level", &iso(&level))]));
    let flight = match (result.moved, result.score, pass_mark) {
        (LevelMove::Stay, Some(score), Some(mark)) if score + 1e-4 >= mark => Some(fill(
            st(lang, S::FlightInvite),
            &[("mark", &pct(&ui, mark)), ("level", &iso(&level_label(&result.from_level)))],
        )),
        _ => None,
    };
    let flight_test = format!("{}.test", result.from_level);
    let before = previous.map(|p| p.radar);
    let _ = has_test;
    rsx! {
        section { class: "card st-card st-outcome",
            h3 { class: "lp-done reveal", "{st(lang, title)}" }
            if let Some(t) = nur_text {
                Nur { text: t }
            }
            if let Some(b) = body_text {
                p { class: "reveal d2", "{b}" }
            }
            if let Some(score) = result.score {
                div { class: "stats-row",
                    div { class: "stat",
                        span { class: "stat-value", bdi { "{pct(&ui, score)}" } }
                        span { class: "stat-label", "{st(lang, S::CheckScore)}" }
                    }
                }
            }
            if let Some(f) = flight {
                div { class: "st-flight",
                    p { "{f}" }
                    button { onclick: move |_| ui.open_lesson(flight_test.clone()), "{st(lang, S::FlightButton)}" }
                }
            }
        }
        section { class: "card st-card",
            RadarView { now: result.radar.clone(), before }
        }
        section { class: "card st-card",
            match &result.eta {
                Some(eta) => rsx! { EtaLine { level: eta.level.clone(), date: eta.date, today } },
                None => rsx! {
                    h4 { "{st(lang, S::ForecastLevelTitle)}" }
                    p { class: "muted", "{st(lang, S::ForecastNoPace)}" }
                },
            }
            PaceNote {}
        }
        div { class: "lp-buttons",
            button {
                class: "primary lp-primary",
                onclick: move |_| {
                    let mut s = ui.screen;
                    s.set(Screen::Today);
                },
                "{st(lang, S::Done)}"
            }
        }
    }
}

/// "Based on 14 minutes a day over the last 28 days."
#[component]
fn PaceNote() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let log = db.0.borrow().review_log().unwrap_or_default();
    let totals = core_stats::day_totals(&coach.events.read(), &log, coach.offset());
    let Some(p) = pace::pace(&totals, coach.today()) else { return rsx! {} };
    let text = fill(
        st(ui.lang(), S::ForecastEstimate),
        &[("minutes", &labels::minutes(&ui, p.minutes_per_day.round() as u32)), ("days", &ui.num(p.days))],
    );
    rsx! { p { class: "muted small", "{text}" } }
}

#[cfg(test)]
mod tests {
    use kabar_core::coach::voice;
    use kabar_core::locale::UiLanguage;

    use super::*;

    #[test]
    fn the_welcome_fits_how_the_check_was_reached() {
        assert_eq!(welcome_line(true, false), Line::LevelCheckFirst);
        assert_eq!(welcome_line(true, true), Line::LevelCheckFirst);
        assert_eq!(welcome_line(false, true), Line::LevelCheckInvite);
        assert_eq!(welcome_line(false, false), Line::LevelCheckAnytime);
        // Only the monthly invitation says a month has passed.
        for seed in 0..4 {
            for line in [Line::LevelCheckFirst, Line::LevelCheckAnytime] {
                let ar = voice::say(UiLanguage::ArEg, line, seed, &[("name", "Mohamed")]);
                assert!(!ar.contains("شهر"), "{ar}");
                let en = voice::say(UiLanguage::En, line, seed, &[("name", "Mohamed")]);
                assert!(!en.contains("month"), "{en}");
            }
        }
    }
}
