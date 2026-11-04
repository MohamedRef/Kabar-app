//! The monthly level check, taken from the intro to the outcome the way the
//! learner takes it, and each outcome as it reads.

use kabar_core::coach::voice;
use kabar_core::locale::UiLanguage;
use kabar_protocol::coach::{ActivityKind, CoachProfile, Target};

use super::*;
use crate::store::Store;
use crate::test_screens::{self as td, Setup};

const EN: UiLanguage = UiLanguage::En;

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

fn view(screen: Screen) -> Option<Element> {
    match screen {
        Screen::LevelCheck => Some(rsx! { LevelCheckScreen {} }),
        _ => None,
    }
}

fn learner() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    let today = td::today_now();
    let profile = CoachProfile {
        name: "Hamo".into(),
        target: Some(Target { level: "bipa2".into(), by: today.add_days(200) }),
        ..CoachProfile::default()
    };
    s.save_coach_profile(&profile).unwrap();
    // A steady month: the pace forecast has something to go on.
    for back in 0..28 {
        s.add_activity(ActivityKind::Reviews { cards: 10, correct: 9 }, today.add_days(-back), 15 * 60)
            .unwrap();
    }
    s
}

/// Answers whatever the check shows, like a learner who picks the first
/// option, and returns the pages it went through.
fn take_the_check(dom: &mut crate::test_dom::TestDom) -> Vec<String> {
    answer_until(dom, "st-outcome")
}

/// [`take_the_check`] until the page shows `end`.
fn answer_until(dom: &mut crate::test_dom::TestDom, end: &str) -> Vec<String> {
    let mut pages = Vec::new();
    let (mut pairs_left, mut tries) = (0, 0);
    for _ in 0..80 {
        let html = td::html(dom);
        pages.push(html.clone());
        if html.contains(end) {
            return pages;
        }
        let all = td::listeners(dom);
        let has =
            |class: &str| all.iter().any(|l| l.event == "click" && l.class().split(' ').any(|c| c == class));
        if html.contains("lt-grades") {
            // The honest self-check: "fully" for every open answer.
            let fully = lt(EN, L::GradeFully);
            for l in all
                .iter()
                .filter(|l| l.event == "click" && l.text == fully && l.attr("aria-pressed") == Some("false"))
            {
                td::click_on(dom, l);
            }
        } else if has("lp-skip") {
            td::click_where(dom, |l| l.class() == "lp-skip");
            continue;
        } else if has("lp-choice") {
            let first =
                all.iter().find(|l| l.event == "click" && l.class().starts_with("lp-choice")).unwrap();
            td::click_on(dom, first);
        } else if all.iter().any(|l| l.event == "click" && l.class().starts_with("lp-pair")) {
            // Pair the first unmatched word with each meaning in turn until
            // one sticks (a wrong try is noted as a word to practise).
            let open = |dom: &crate::test_dom::TestDom| -> Vec<td::Listener> {
                td::listeners(dom)
                    .into_iter()
                    .filter(|l| {
                        l.event == "click" && l.class().starts_with("lp-pair") && !l.class().contains("done")
                    })
                    .collect()
            };
            let words = open(dom);
            let m = words.len() / 2;
            if m != pairs_left {
                (pairs_left, tries) = (m, 0);
            }
            td::click_on(dom, &words[0]);
            let meanings: Vec<_> =
                open(dom).into_iter().filter(|l| !l.class().contains("selected")).skip(m - 1).collect();
            td::click_on(dom, &meanings[tries % meanings.len()]);
            tries += 1;
            continue;
        } else if all.iter().any(|l| l.event == "click" && l.class() == "lp-tile") {
            for l in all.iter().filter(|l| l.event == "click" && l.class() == "lp-tile") {
                td::click_on(dom, l);
            }
        }
        if td::listeners(dom).iter().any(|l| l.event == "input") {
            td::input(dom, |_| true, "Saya belajar bahasa Indonesia.");
        }
        let next = td::find(dom, "click", |l| l.class().contains("lp-primary"));
        if next.attr("disabled") == Some("true") {
            panic!("stuck: {}", td::html(dom));
        }
        td::click_on(dom, &next);
    }
    panic!("the check never ended: {}", pages.last().unwrap());
}

#[test]
fn a_first_check_runs_from_the_welcome_to_a_saved_outcome() {
    let mut dom = td::mount(Setup::new(learner(), Screen::LevelCheck, view));
    let html = td::html(&dom);
    // The first check is introduced as the starting point, for BIPA 1.
    assert!(td::shows(&html, st(EN, S::CheckTitle)), "{html}");
    assert!(td::shows(&html, &stf(EN, S::CheckIntro, &[("level", &iso("BIPA 1"))])), "{html}");
    td::click(&mut dom, st(EN, S::CheckStart));
    let html = td::html(&dom);
    assert!(html.contains("st-runner") && html.contains("role=\"progressbar\""), "{html}");
    assert!(html.contains("aria-valuenow=\"0\""), "nothing answered yet: {html}");

    let pages = take_the_check(&mut dom);
    assert!(pages.len() > 3, "a check has several items");
    // Progress moved on as items were answered.
    assert!(pages.iter().any(|p| !p.contains("aria-valuenow=\"0\"") && p.contains("st-runner")));
    let html = pages.last().unwrap();
    assert!(td::shows(html, st(EN, S::RadarTitle)), "{html}");
    assert!(td::shows(html, st(EN, S::RadarFirst)), "no earlier check to compare: {html}");

    // The result is saved as a synced lesson event, and each skill tested
    // went into Kak Nur's log.
    let db = td::db(&dom);
    let checks = crate::stats::level_checks(&db);
    assert_eq!(checks.len(), 1);
    let c = &checks[0];
    assert_eq!((c.from_level.as_str(), c.language.as_str()), ("bipa1", "id"));
    assert_eq!(c.date, td::today(&dom));
    assert!(c.score.is_some(), "{c:?}");
    assert!(!c.radar.is_empty());
    let practice = db.0.borrow().activity(None).unwrap();
    assert!(
        practice.iter().any(|e| matches!(&e.kind, ActivityKind::Practice { topic, .. } if topic.starts_with("level_check.bipa1"))),
        "{practice:?}"
    );

    // "Done" goes back to Today.
    td::click(&mut dom, st(EN, S::Done));
    assert_eq!(td::screen(&dom), Screen::Today);
}

#[test]
fn a_check_by_hand_after_last_month_compares_with_it() {
    let mut store = learner();
    let today = td::today_now();
    let prev = LevelCheckResult {
        v: LEVEL_CHECK_VERSION,
        date: today.add_days(-31),
        language: "id".into(),
        from_level: "bipa1".into(),
        level: "bipa1".into(),
        moved: LevelMove::Stay,
        score: Some(0.5),
        radar: vec![RadarPoint { skill: Skill::Reading, value: 0.5 }],
        eta: None,
        missed: Vec::new(),
        duration_ms: 1000,
    };
    store.record_lesson_event(LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_KIND, &prev).unwrap();
    let mut dom = td::mount(Setup::new(store, Screen::LevelCheck, view));
    // Opened by hand: Kak Nur's "whenever you like", never "a month has passed".
    let html = td::html(&dom);
    let anytime: Vec<String> =
        (0..64).map(|seed| voice::say(EN, Line::LevelCheckAnytime, seed, &[("name", "Hamo")])).collect();
    assert!(anytime.iter().any(|l| td::shows(&html, l)), "{html}");
    td::click(&mut dom, st(EN, S::CheckStart));
    let pages = take_the_check(&mut dom);
    let html = pages.last().unwrap();
    assert!(td::shows(html, st(EN, S::RadarBefore)), "last month on the radar: {html}");
    assert_eq!(crate::stats::level_checks(&td::db(&dom)).len(), 2);
}

#[test]
fn closing_the_check_goes_back() {
    let mut dom = td::mount(Setup::new(learner(), Screen::LevelCheck, view));
    td::click_where(&mut dom, |l| l.attr("data-back") == Some("true"));
    assert_eq!(td::screen(&dom), Screen::Today, "where Stats was last opened from");
    assert!(crate::stats::level_checks(&td::db(&dom)).is_empty(), "nothing saved");
}

// ------------------------------------------------------------- outcomes

/// An outcome's props: (result, previous check, pass mark, has test).
type OutcomeProps = (LevelCheckResult, Option<LevelCheckResult>, Option<f32>, bool);

thread_local! {
    static OUTCOME: std::cell::RefCell<Option<OutcomeProps>> = const { std::cell::RefCell::new(None) };
}

fn outcome_view(screen: Screen) -> Option<Element> {
    if screen != Screen::LevelCheck {
        return None;
    }
    let (result, previous, pass_mark, has_test) = OUTCOME.with(|o| o.borrow().clone()).unwrap();
    Some(rsx! { Outcome { result, previous, pass_mark, has_test } })
}

fn outcome(moved: LevelMove, score: Option<f32>, pass_mark: Option<f32>) -> crate::test_dom::TestDom {
    let today = td::today_now();
    let result = LevelCheckResult {
        v: LEVEL_CHECK_VERSION,
        date: today,
        language: "id".into(),
        from_level: "bipa1".into(),
        level: match moved {
            LevelMove::Up => "bipa2",
            _ => "bipa1",
        }
        .into(),
        moved,
        score,
        radar: vec![
            RadarPoint { skill: Skill::Reading, value: 0.8 },
            RadarPoint { skill: Skill::Listening, value: 0.4 },
        ],
        eta: None,
        missed: Vec::new(),
        duration_ms: 1000,
    };
    let previous = LevelCheckResult {
        date: today.add_days(-30),
        radar: vec![
            RadarPoint { skill: Skill::Reading, value: 0.7 },
            RadarPoint { skill: Skill::Listening, value: 0.5 },
            RadarPoint { skill: Skill::Writing, value: 0.5 },
        ],
        ..result.clone()
    };
    OUTCOME.with(|o| *o.borrow_mut() = Some((result, Some(previous), pass_mark, true)));
    td::mount(Setup::new(learner(), Screen::LevelCheck, outcome_view))
}

#[test]
fn moving_up_celebrates_and_the_score_opens_stats() {
    let mut dom = outcome(LevelMove::Up, Some(0.9), Some(0.7));
    let html = td::html(&dom);
    assert!(td::shows(&html, st(EN, S::ResultUp)), "{html}");
    assert!(html.contains("Kak Nur"), "{html}");
    assert!(html.contains(">90%<"), "{html}");
    // Up: no invitation to the full test (the level already moved).
    assert!(!td::shows(&html, st(EN, S::FlightButton)), "{html}");
    // The radar's change since last month, both ways, and an unmeasured skill.
    assert!(html.contains("st-delta up") && html.contains("+10"), "{html}");
    assert!(html.contains("−10"), "a dip shows in the same calm ink: {html}");
    assert!(td::shows(&html, st(EN, S::RadarNotMeasured)), "{html}");
    // No pace yet: says so instead of guessing a date.
    td::click_where(&mut dom, |l| l.class() == "stat tap");
    assert_eq!(td::screen(&dom), Screen::Stats);
}

#[test]
fn moving_down_is_framed_as_consolidating() {
    let dom = outcome(LevelMove::Down, Some(0.3), Some(0.7));
    let html = td::html(&dom);
    assert!(td::shows(&html, st(EN, S::ResultDown)), "{html}");
    assert!(td::shows(&html, &stf(EN, S::DownBody, &[("level", &iso("BIPA 1"))])), "{html}");
    assert!(!td::shows(&html, st(EN, S::FlightButton)), "{html}");
}

#[test]
fn a_strong_check_that_stays_invites_the_full_level_test() {
    let mut dom = outcome(LevelMove::Stay, Some(0.8), Some(0.7));
    let html = td::html(&dom);
    assert!(td::shows(&html, st(EN, S::ResultStay)), "{html}");
    assert!(td::shows(&html, &stf(EN, S::StayBody, &[("level", &iso("BIPA 1"))])), "{html}");
    assert!(
        td::shows(&html, &stf(EN, S::FlightInvite, &[("mark", "70%"), ("level", &iso("BIPA 1"))])),
        "{html}"
    );
    td::click(&mut dom, st(EN, S::FlightButton));
    assert_eq!(td::screen(&dom), Screen::Lesson);
    assert_eq!(td::lesson(&dom).as_deref(), Some("bipa1.test"));

    // Under the pass mark, or no score at all: no invitation.
    for (score, mark) in [(Some(0.6), Some(0.7)), (None, Some(0.7)), (Some(0.9), None)] {
        let dom = outcome(LevelMove::Stay, score, mark);
        assert!(!td::shows(&td::html(&dom), st(EN, S::FlightButton)), "{score:?} {mark:?}");
    }
}

#[test]
fn the_forecast_reads_the_pace_or_says_there_is_none() {
    // `learner()` studied 15 minutes a day for 28 days: the pace note says so.
    let dom = outcome(LevelMove::Stay, None, None);
    let html = td::html(&dom);
    assert!(td::shows(&html, st(EN, S::ForecastNoPace)), "this result has no date: {html}");
    assert!(
        td::shows(&html, &stf(EN, S::ForecastEstimate, &[("minutes", "15 min"), ("days", "28")])),
        "{html}"
    );
}

// ------------------------------------------------------------- the intro

thread_local! {
    static INTRO: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

fn intro_view(screen: Screen) -> Option<Element> {
    (screen == Screen::LevelCheck).then(|| {
        let has_test = INTRO.with(|i| i.get());
        rsx! { Intro { level: "bipa4".to_string(), welcome: Line::LevelCheckInvite, has_test, on_start: move |_| {} } }
    })
}

#[test]
fn without_test_items_the_intro_offers_the_result_from_history() {
    INTRO.with(|i| i.set(false));
    let dom = td::mount(Setup::new(learner(), Screen::LevelCheck, intro_view));
    let html = td::html(&dom);
    assert!(td::shows(&html, &stf(EN, S::CheckNoTest, &[("level", &iso("BIPA 4"))])), "{html}");
    assert!(td::shows(&html, st(EN, S::CheckSeeResult)), "{html}");
    INTRO.with(|i| i.set(true));
    let dom = td::mount(Setup::new(learner(), Screen::LevelCheck, intro_view));
    assert!(td::shows(&td::html(&dom), st(EN, S::CheckStart)));
}

#[test]
fn concluding_from_history_alone_still_decides_and_forecasts() {
    let mut dom = td::mount(Setup::new(learner(), Screen::LevelCheck, |_| Some(rsx! {})));
    let result = dom.peek_app(|| {
        let db = consume_context::<Db>();
        let coach = consume_context::<Coach>();
        let mut s = setup(&db, &coach);
        assert_eq!(s.level, "bipa1");
        assert_eq!(s.welcome, Line::LevelCheckFirst);
        assert!(s.mini.is_some() && s.pass_mark.is_some());
        s.mini = None;
        conclude(&db, coach, &s, None)
    });
    dom.render();
    assert_eq!(result.score, None, "no items, no score");
    assert!(result.missed.is_empty() && result.duration_ms == 0);
    assert_eq!(result.moved, LevelMove::Stay);
    // A steady month gives a pace, so a date for BIPA 2 (the learner's goal).
    let eta = result.eta.clone().expect("a month of study gives a pace");
    assert_eq!(eta.level, "bipa2");
    assert!(eta.date > td::today(&dom));
    assert_eq!(crate::stats::level_checks(&td::db(&dom)), vec![result]);
}

// ------------------------------------------------------------- every kind of item

thread_local! {
    static RESULTS: std::cell::RefCell<Option<kabar_core::lesson::level_test::TestResults>> =
        const { std::cell::RefCell::new(None) };
}

/// BIPA 1's test cut down to its first item of each kind: choices, blanks,
/// tiles, pairs, listening, dictation, translation, speaking, and the open
/// writing and speaking tasks.
fn one_of_each() -> TestContent {
    let yaml = crate::lesson::test_content("bipa1.test").unwrap();
    let mut test = parse_test(&yaml).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for section in &mut test.sections {
        section.items.retain(|it| {
            let kind = match &it.kind {
                TestItemKind::Exercise(e) => format!("exercise {:?}", std::mem::discriminant(e)),
                other => format!("item {:?}", std::mem::discriminant(other)),
            };
            seen.insert(kind)
        });
    }
    test.sections.retain(|s| !s.items.is_empty());
    test
}

#[component]
fn RunnerHost() -> Element {
    let runner = use_signal(|| {
        let mut r = TestRunner::new(one_of_each());
        r.start();
        Some(r)
    });
    let mut done = use_signal(|| false);
    rsx! {
        if done() {
            p { class: "finished", "finished" }
        } else {
            Runner {
                runner,
                on_done: move |_| {
                    RESULTS.with(|r| *r.borrow_mut() = runner.peek().as_ref().map(TestRunner::results));
                    done.set(true);
                },
            }
        }
    }
}

#[test]
fn every_kind_of_item_can_be_answered_and_open_answers_are_self_checked() {
    let mut dom = td::mount(Setup::new(learner(), Screen::LevelCheck, |_| Some(rsx! { RunnerHost {} })));
    let pages = answer_until(&mut dom, "class=\"finished\"");
    let all = pages.join("\n");
    // The writing task asks for Indonesian; the speaking task says it isn't
    // recorded; then the honest self-check of both open answers.
    for line in [L::WriteInId, L::SpeakTask, L::SpeakTaskNote, L::SelfCheckTitle, L::SelfCheckBody, L::Sample]
    {
        assert!(td::shows(&all, lt(EN, line)), "{line:?} never showed");
    }
    assert!(all.contains("lp-pairs") && all.contains("lp-text"), "pairs and a text answer were asked");
    assert!(td::shows(&all, "Saya belajar bahasa Indonesia."), "the written answer is shown back");
    assert!(td::shows(&all, lt(EN, L::Spoken)), "a spoken answer reads as spoken");
    assert!(td::shows(&all, lt(EN, L::ISpoke)), "{all}");
    // Every scored item counted; the self-check graded both open answers.
    let results = RESULTS.with(|r| r.borrow_mut().take()).expect("the runner finished");
    assert!(results.total.available > 0.0, "{results:?}");
    assert!(!results.skills.is_empty());
}

#[test]
fn the_check_button_waits_for_an_answer_and_for_real_writing() {
    let mut dom = td::mount(Setup::new(learner(), Screen::LevelCheck, |_| Some(rsx! { RunnerHost {} })));
    // The first item, nothing picked: pressing the button (Enter does the
    // same) leaves the learner on it.
    let before = td::html(&dom);
    let next = td::find(&dom, "click", |l| l.class().contains("lp-primary"));
    td::click_on(&mut dom, &next);
    assert_eq!(td::html(&dom), before, "an unanswered item is not marked");

    // At the writing task (BIPA 1's message to Raka), an empty or blank
    // answer isn't sent either.
    let write = "Write a short message";
    answer_until(&mut dom, write);
    for blank in ["", "   "] {
        td::input(&mut dom, |_| true, blank);
        let next = td::find(&dom, "click", |l| l.class().contains("lp-primary"));
        td::click_on(&mut dom, &next);
        assert!(td::html(&dom).contains(write), "{blank:?} was taken as an answer");
    }
    td::input(&mut dom, |_| true, "Saya suka kopi.");
    let next = td::find(&dom, "click", |l| l.class().contains("lp-primary"));
    td::click_on(&mut dom, &next);
    let html = td::html(&dom);
    assert!(!html.contains(write) && html.contains("st-runner"), "a real answer moves on: {html}");
}

#[component]
fn EmptyRunner() -> Element {
    let runner = use_signal(|| None::<TestRunner>);
    rsx! { div { class: "host", Runner { runner, on_done: move |_| {} } } }
}

#[test]
fn a_runner_without_a_test_shows_nothing() {
    let dom = td::mount(Setup::new(learner(), Screen::LevelCheck, |_| Some(rsx! { EmptyRunner {} })));
    let html = td::html(&dom);
    assert!(html.contains("class=\"host\""), "{html}");
    assert!(!html.contains("st-runner") && !html.contains("lp-primary"), "{html}");
}

#[test]
fn past_the_last_test_the_check_concludes_from_history_alone() {
    // A new learner (no study yet) whose last check moved past BIPA 7:
    // there are no BIPA 8 items, so the check reads the month instead.
    let mut store = Store::open_in_memory().unwrap();
    let today = td::today_now();
    let prev = LevelCheckResult {
        v: LEVEL_CHECK_VERSION,
        date: today.add_days(-31),
        language: "id".into(),
        from_level: "bipa7".into(),
        level: "bipa8".into(),
        moved: LevelMove::Up,
        score: Some(0.9),
        radar: Vec::new(),
        eta: None,
        missed: Vec::new(),
        duration_ms: 1000,
    };
    store.record_lesson_event(LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_KIND, &prev).unwrap();
    let mut dom = td::mount(Setup::new(store, Screen::LevelCheck, view));
    let html = td::html(&dom);
    assert!(td::shows(&html, &stf(EN, S::CheckNoTest, &[("level", &iso("BIPA 8"))])), "{html}");
    td::click(&mut dom, st(EN, S::CheckSeeResult));
    let html = td::html(&dom);
    assert!(html.contains("st-outcome"), "{html}");
    assert!(!html.contains("st-runner"), "no items were asked: {html}");
    // No study yet: no pace, so no "based on N minutes a day" note.
    assert!(!td::shows(&html, td::lead(st(EN, S::ForecastEstimate))), "{html}");
    let checks = crate::stats::level_checks(&td::db(&dom));
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[1].from_level, "bipa8");
    assert_eq!(checks[1].score, None);
}

#[test]
fn a_check_that_cannot_be_saved_still_shows_its_outcome() {
    let mut dom = td::mount(Setup::new(learner(), Screen::LevelCheck, |_| Some(rsx! {})));
    let result = dom.peek_app(|| {
        let db = consume_context::<Db>();
        let coach = consume_context::<Coach>();
        let mut s = setup(&db, &coach);
        s.mini = None;
        db.0.borrow().refuse_writes();
        conclude(&db, coach, &s, None)
    });
    dom.render();
    assert_eq!((result.level.as_str(), result.moved), ("bipa1", LevelMove::Stay));
    assert!(result.eta.is_some(), "the outcome is worked out all the same");
    assert!(crate::stats::level_checks(&td::db(&dom)).is_empty(), "nothing was written");
}
