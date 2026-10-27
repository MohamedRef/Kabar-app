//! The test player: level tests ("Take a flight") and the placement test
//! (`kabar.test/v1`). Closed items reuse the Lesson Player's exercise views;
//! there is no per-item feedback, open answers get an honest self-check at
//! the end, then the results: per-skill bars and what to practise. Nothing
//! is ever locked, and a test that isn't passed just says what to practise.

use dioxus::prelude::*;
use kabar_core::lesson::level_test::{
    ItemRecord, Scored, SelfGrade, Tally, TestResults, TestRunner, TestStep, parse_test, validate_test,
};
use kabar_core::lesson::{Answer, XP_FINISH, mistake_words, pick, references};
use kabar_core::locale::{UiLanguage, format_number, localize_digits};
use kabar_protocol::coach::{ActivityKind, Skill};
use kabar_protocol::coach::{CoachProfile, StartingPoint};
use kabar_protocol::lesson::{Direction, ExerciseKind, LessonCompleted};
use kabar_protocol::test::{TestContent, TestItem, TestItemKind, TestKind, TestSkill};

use super::text::{L, lt};
use super::{
    Draft, ExerciseView, MAX_STEP_MS, Notice, Rich, TextAnswer, autoplay_text, back_label, draft_answer,
    instruction, needs_audio, now_ms, option_count, speak, target_tag,
};
use crate::annotated::Annotated;
use crate::coach::Coach;
use crate::{Db, Screen, Ui};

const TEST_CSS: &str = include_str!("../../assets/test.css");

#[component]
pub fn TestScreen(id: String, yaml: &'static str) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let body = match parse_test(yaml) {
        Err(e) => rsx! { Notice { title: lt(lang, L::BrokenTitle), body: e.to_string(), id: id.clone() } },
        Ok(test) => {
            let issues = validate_test(&test);
            if issues.is_empty() {
                rsx! { TestView { key: "{id}", test } }
            } else {
                let list = issues.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
                rsx! { Notice { title: lt(lang, L::BrokenTitle), body: list, id: id.clone() } }
            }
        }
    };
    rsx! {
        style { {TEST_CSS} }
        {body}
    }
}

/// Kak Nur's offer of the placement test (spec §6), shown on Today until the
/// learner has a placement result or has finished a lesson. Optional: the
/// onboarding self-assessment already gives a start.
#[component]
pub fn PlacementInvite() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.settings.read().ui_language;
    let eastern = ui.settings.read().eastern_digits;
    let show = use_hook(|| {
        let store = db.0.borrow();
        let placed = store.placement().map(|p| p.is_some()).unwrap_or(true);
        let started = store.completed_lessons().map(|c| !c.is_empty()).unwrap_or(true);
        !placed && !started
    });
    if !show {
        return rsx! {};
    }
    rsx! {
        div { class: "card placement-invite", style: "margin-block-start: 1rem",
            h3 { "{lt(lang, L::InviteTitle)}" }
            p { class: "muted", "{localize_digits(lt(lang, L::InviteBody), lang, eastern)}" }
            button { class: "primary", onclick: move |_| ui.open_lesson("placement"), "{lt(lang, L::InviteButton)}" }
        }
    }
}

/// `bipa2` → `BIPA 2`, `j1` → `J1`.
pub fn level_label(level: &str) -> String {
    match level.strip_prefix("bipa") {
        Some(n) => format!("BIPA {n}"),
        None => level.to_uppercase(),
    }
}

fn part_label(ui: &Ui, i: usize, n: usize) -> String {
    lt(ui.lang(), L::TestPart).replace("{i}", &ui.num(i as u64 + 1)).replace("{n}", &ui.num(n as u64))
}

fn skill_label(s: TestSkill) -> L {
    match s {
        TestSkill::Listening => L::SkillListening,
        TestSkill::Reading => L::SkillReading,
        TestSkill::Writing => L::SkillWriting,
        TestSkill::Speaking => L::SkillSpeaking,
        TestSkill::Vocabulary => L::SkillVocabulary,
        TestSkill::Grammar => L::SkillGrammar,
    }
}

/// The skill-radar axis a test skill feeds (grammar counts as vocabulary,
/// the radar's "language knowledge" axis).
fn radar_skill(s: TestSkill) -> Skill {
    match s {
        TestSkill::Listening => Skill::Listening,
        TestSkill::Reading => Skill::Reading,
        TestSkill::Writing => Skill::Writing,
        TestSkill::Speaking => Skill::Speaking,
        TestSkill::Vocabulary | TestSkill::Grammar => Skill::Vocabulary,
    }
}

/// The self-assessment the onboarding start choice maps to (Kak Nur's plan
/// starts at BIPA 1, 2 or 3 from it).
fn starting_point(level: &str) -> StartingPoint {
    match level {
        "bipa1" => StartingPoint::BrandNew,
        "bipa2" => StartingPoint::KnowsBasics,
        _ => StartingPoint::CanChat,
    }
}

/// 1–7 for `bipa1`…`bipa7` (also `bipa-2`); 0 for anything else.
fn level_number(level: &str) -> u32 {
    level.strip_prefix("bipa").map(|n| n.trim_start_matches('-')).and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn percent(share: f32) -> u32 {
    (share * 100.0).round() as u32
}

/// Saves the outcome once Results is reached: the test events, the coach's
/// practice log and, for placement, where the journey starts.
fn record(db: &Db, coach: Coach, r: &TestRunner) {
    let test = r.test();
    let res = r.results();
    {
        let mut store = db.0.borrow_mut();
        match test.kind {
            TestKind::Level => {
                if let Err(e) = store.record_lesson_event(&test.id, "test_finished", &r.finished()) {
                    crate::diag::report!("saving the test result failed: {e}");
                }
                if res.passed == Some(true) {
                    let mut mistakes: Vec<String> = Vec::new();
                    for m in &res.missed {
                        let item = &test.sections[m.section].items[m.item];
                        for w in item.exercise().map(|ex| mistake_words(&ex)).unwrap_or_default() {
                            if !mistakes.contains(&w) {
                                mistakes.push(w);
                            }
                        }
                    }
                    let done = LessonCompleted {
                        score: percent(res.total.share().unwrap_or(0.0)),
                        xp: XP_FINISH,
                        duration_ms: res.duration_ms,
                        mistakes,
                        results: Vec::new(),
                    };
                    if let Err(e) = store.record_lesson_event(&test.id, "completed", &done) {
                        crate::diag::report!("marking the level done failed: {e}");
                    }
                }
            }
            TestKind::Placement => {
                if let Some(p) = r.placement()
                    && let Err(e) = store.record_lesson_event(&test.id, "placed", &p)
                {
                    crate::diag::report!("saving the placement failed: {e}");
                }
            }
        }
    }
    // Kak Nur: one practice entry per skill (feeds the radar and reports).
    let mut seconds = (res.duration_ms / 1000) as u32;
    for (skill, _) in &res.skills {
        let scored: Vec<&ItemRecord> = r
            .records()
            .iter()
            .filter(|x| x.skill == *skill && matches!(x.scored, Scored::Earned(_)))
            .collect();
        let total = scored.len() as u32;
        let correct = scored.iter().filter(|x| !x.missed()).count() as u32;
        let topic = match skill {
            TestSkill::Grammar => format!("{} · grammar", test.id),
            _ => test.id.clone(),
        };
        let kind = ActivityKind::Practice { skill: radar_skill(*skill), topic, correct, total };
        coach.record(db, kind, std::mem::take(&mut seconds));
    }
    // The placement refines the onboarding start (spec §6).
    if let Some(level) = &res.start_level {
        let profile: Option<CoachProfile> = coach.profile.peek().clone();
        if let Some(mut profile) = profile {
            profile.start = starting_point(level);
            // `suggest_target` stops at BIPA 3; a learner placed above that
            // aims at the level they placed into, never below it.
            if let Some(target) = profile.target.as_mut()
                && level_number(level) > level_number(&target.level)
            {
                target.level = level.clone();
            }
            coach.save_profile(db, profile);
        }
    }
}

#[cfg(test)]
mod placement_tests {
    use super::level_number;

    #[test]
    fn level_numbers() {
        assert_eq!(level_number("bipa4"), 4);
        assert_eq!(level_number("bipa-2"), 2);
        assert_eq!(level_number("j1"), 0);
    }
}

#[component]
fn TestView(test: TestContent) -> Element {
    let ui = use_context::<Ui>();
    let db_ctx = use_context::<Db>();
    let db = use_hook(|| CopyValue::new(db_ctx));
    let coach = use_context::<Coach>();
    let lang = ui.settings.read().ui_language;

    let mut runner = use_signal(|| TestRunner::new(test.clone()));
    let draft = use_signal(Draft::default);
    let last = use_signal(now_ms);
    let audio_missing = use_signal(|| false);
    let mut recorded = use_signal(|| false);
    // Listening items skipped because the device has no voice for them.
    let mut voice_skips = use_signal(|| 0u32);
    // "Leave the test?" is showing.
    let mut confirm_close = use_signal(|| false);

    // Runs an operation on the runner, then: time, fresh draft, audio, and
    // the saved outcome once the results are reached.
    let mut commit = move |op: &dyn Fn(&mut TestRunner)| {
        let now = now_ms();
        let mut last = last;
        let delta = now.saturating_sub(*last.peek()).min(MAX_STEP_MS);
        last.set(now);
        {
            let mut r = runner.write();
            if matches!(r.step(), TestStep::Item { .. }) {
                r.add_elapsed(delta);
            }
            op(&mut r);
        }
        let mut draft = draft;
        draft.set(Draft::default());
        let r = runner.peek();
        if r.step() == TestStep::Results && !*recorded.peek() {
            recorded.set(true);
            record(&db.read(), coach, &r);
        }
        if let Some((text, audio)) = r.exercise().and_then(|ex| autoplay_text(&ex)) {
            speak(&text, &r.test().language, false, audio.as_deref(), audio_missing);
        }
        document::eval("document.querySelector('.lp-body')?.scrollTo(0, 0);");
    };

    // The one primary action (Enter and the big button).
    let mut primary = move || {
        let step = runner.peek().step();
        match step {
            TestStep::Intro => commit(&|r| r.start()),
            TestStep::Item { .. } => {
                let (item, answer) = {
                    let r = runner.peek();
                    let item = r.item().cloned();
                    let answer = r.exercise().and_then(|ex| draft_answer(&ex, &draft.peek(), &r.tiles(&ex)));
                    (item, answer)
                };
                let Some(item) = item else { return };
                match &item.kind {
                    TestItemKind::Exercise(_) => {
                        if let Some(a) = answer {
                            commit(&|r| r.answer(&a, lang));
                        }
                    }
                    TestItemKind::WritingTask(_) => {
                        let text = draft.peek().text.clone();
                        if !text.trim().is_empty() {
                            commit(&|r| r.answer_open(&text));
                        }
                    }
                    TestItemKind::SpeakingTask(_) => commit(&|r| r.answer_open("")),
                }
            }
            TestStep::SelfCheck => commit(&|r| r.finish_self_check()),
            TestStep::Results => ui.close_lesson(),
        }
    };

    // No sound for the target language: listening items are skipped the
    // "can't listen now" way (unscored), with a note, so the placement never
    // opens on audio that can't play.
    use_effect(move || {
        if !*audio_missing.read() {
            return;
        }
        let skip = {
            let r = runner.read();
            matches!(r.step(), TestStep::Item { .. }) && r.exercise().is_some_and(|ex| needs_audio(&ex))
        };
        if skip {
            voice_skips += 1;
            commit(&|r| r.skip());
        }
    });

    // Enter = next, 1–9 = pick an option (same rules as the Lesson Player).
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            if (window.__kabarTestKeys) document.removeEventListener("keydown", window.__kabarTestKeys, true);
            window.__kabarTestKeys = (e) => {
                if (!document.querySelector(".lt") || e.isComposing) return;
                const t = e.target;
                if (t.closest && t.closest(".lp-top, .lt-selfcheck, .lt-results, .lt-confirm")) return;
                const typing = t.tagName === "TEXTAREA" || t.tagName === "INPUT";
                if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    dioxus.send("enter");
                } else if (!typing && !e.ctrlKey && !e.metaKey && !e.altKey && /^[1-9]$/.test(e.key)) {
                    dioxus.send(e.key);
                }
            };
            document.addEventListener("keydown", window.__kabarTestKeys, true);
            await new Promise(() => {});
            "#,
        );
        while let Ok(key) = eval.recv::<String>().await {
            match key.as_str() {
                "enter" if *confirm_close.peek() => {}
                "enter" => primary(),
                n => {
                    let Ok(n) = n.parse::<usize>() else { continue };
                    if *confirm_close.peek() {
                        continue;
                    }
                    let count = runner.peek().exercise().map_or(0, |ex| option_count(&ex));
                    if n <= count {
                        let mut d = draft;
                        d.write().choice = Some(n - 1);
                    }
                }
            }
        }
    });
    use_drop(|| {
        document::eval(
            r#"if (window.__kabarTestKeys) { document.removeEventListener("keydown", window.__kabarTestKeys, true); window.__kabarTestKeys = null; }"#,
        );
    });

    let r = runner.read();
    let t = r.test().clone();
    let step = r.step();
    let pct = (r.fraction() * 100.0).round();
    let target = target_tag(&t.language);
    let item = r.item().cloned();
    let exercise = r.exercise();
    let section = r.section().cloned();
    let tiles = exercise.as_ref().map(|ex| r.tiles(ex)).unwrap_or_default();
    let pair_order = exercise.as_ref().map(|ex| r.pair_order(ex)).unwrap_or_default();
    let self_check_done = r.self_check_done();
    let results = (step == TestStep::Results).then(|| r.results());
    drop(r);

    let can_next = match item.as_ref().map(|i| &i.kind) {
        Some(TestItemKind::Exercise(ExerciseKind::MatchPairs { .. })) => false,
        Some(TestItemKind::Exercise(_)) => {
            exercise.as_ref().is_some_and(|ex| draft_answer(ex, &draft.read(), &tiles).is_some())
        }
        Some(TestItemKind::WritingTask(_)) => !draft.read().text.trim().is_empty(),
        Some(TestItemKind::SpeakingTask(_)) => true,
        None => true,
    };
    let (label, enabled) = match step {
        TestStep::Intro => (lt(lang, L::TestStart), true),
        TestStep::Item { .. } => match item.as_ref().map(|i| &i.kind) {
            Some(TestItemKind::SpeakingTask(_)) => (lt(lang, L::ISpoke), true),
            Some(TestItemKind::Exercise(ExerciseKind::Speaking { .. })) => (lt(lang, L::ISaidIt), true),
            _ => (lt(lang, L::TestNext), can_next),
        },
        TestStep::SelfCheck => (lt(lang, L::SeeResults), self_check_done),
        TestStep::Results => (lt(lang, back_label(ui)), true),
    };
    let can_skip = matches!(
        exercise.as_ref().map(|e| &e.kind),
        Some(ExerciseKind::Listen { .. } | ExerciseKind::Dictation { .. } | ExerciseKind::Speaking { .. })
    );
    let language = t.language.clone();
    let n_sections = t.sections.len();
    // Answers would be lost: ask before leaving mid-test.
    let midway = matches!(step, TestStep::Item { .. } | TestStep::SelfCheck)
        && (!runner.read().records().is_empty() || *draft.read() != Draft::default());

    rsx! {
        div { class: "lp lt",
            div { class: "lp-top",
                button {
                    class: "lp-close",
                    "data-back": "true",
                    aria_label: lt(lang, L::CloseTest),
                    title: lt(lang, L::CloseTest),
                    onclick: move |_| if midway { confirm_close.set(true) } else { ui.close_lesson() },
                    "×"
                }
                div {
                    class: "lp-progress",
                    role: "progressbar",
                    aria_valuemin: "0",
                    aria_valuemax: "100",
                    aria_valuenow: "{pct}",
                    div { class: "lp-bar", style: "inline-size: {pct}%" }
                }
            }
            if *confirm_close.read() {
                div { class: "lt-confirm-backdrop",
                    div {
                        class: "card lt-confirm",
                        role: "alertdialog",
                        aria_modal: "true",
                        aria_labelledby: "lt-confirm-title",
                        h2 { id: "lt-confirm-title", "{lt(lang, L::LeaveTestTitle)}" }
                        p { "{lt(lang, L::LeaveTestBody)}" }
                        div { class: "row lt-confirm-buttons",
                            button {
                                class: "primary",
                                "data-back": "true",
                                autofocus: true,
                                onclick: move |_| confirm_close.set(false),
                                "{lt(lang, L::StayInTest)}"
                            }
                            button { onclick: move |_| ui.close_lesson(), "{lt(lang, L::LeaveTest)}" }
                        }
                    }
                }
            }
            div { class: "lp-body",
                div { class: "lp-col", key: "{step:?}",
                    if *voice_skips.read() > 0 && matches!(step, TestStep::Item { .. }) {
                        p { class: "muted lp-small lp-voice-note", role: "status", "{lt(lang, L::NoVoiceSkipped)}" }
                    }
                    match step {
                        TestStep::Intro => rsx! { Intro { test: t.clone() } },
                        TestStep::Item { section: si, .. } => rsx! {
                            if let Some(s) = &section {
                                header { class: "lt-section",
                                    span { class: "pill", "{part_label(&ui, si, n_sections)}" }
                                    " "
                                    strong { Rich { text: pick(&s.title, lang).to_string(), target } }
                                    if let Some(ins) = &s.instructions {
                                        p { class: "muted lp-small", Rich { text: pick(ins, lang).to_string(), target } }
                                    }
                                }
                            }
                            if let Some(it) = item.clone() {
                                ItemView {
                                    item: it,
                                    exercise: exercise.clone(),
                                    language: language.clone(),
                                    draft,
                                    tiles: tiles.clone(),
                                    pair_order: pair_order.clone(),
                                    missing: audio_missing,
                                    on_matched: move |missed: Vec<String>| commit(&|r| r.answer(&Answer::Matched { missed: missed.clone() }, lang)),
                                }
                            }
                        },
                        TestStep::SelfCheck => rsx! { SelfCheck { runner } },
                        TestStep::Results => rsx! {
                            if let Some(res) = results.clone() {
                                Results { test: t.clone(), results: res, on_retry: move |_| {
                                    recorded.set(false);
                                    let test = runner.peek().test().clone();
                                    runner.set(TestRunner::new(test));
                                } }
                            }
                        },
                    }
                }
            }
            div { class: "lp-actions",
                div { class: "lp-col lp-actions-inner",
                    div { class: "lp-buttons",
                        if can_skip && matches!(step, TestStep::Item { .. }) {
                            button {
                                class: "lp-skip",
                                onclick: move |_| commit(&|r| r.skip()),
                                "{lt(lang, L::CantListenNow)}"
                            }
                        }
                        if step == TestStep::Results && t.kind == TestKind::Placement {
                            button {
                                class: "primary lp-primary",
                                onclick: move |_| {
                                    let mut screen = ui.screen;
                                    let mut lesson = ui.lesson;
                                    lesson.set(None);
                                    screen.set(Screen::Journey);
                                },
                                "{lt(lang, L::GoToJourney)}"
                            }
                        } else {
                            button {
                                class: "primary lp-primary",
                                disabled: !enabled,
                                onclick: move |_| primary(),
                                "{label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Intro(test: TestContent) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&test.language);
    let minutes_line =
        lt(lang, L::TestMinutes).replace("{n}", &crate::coach::labels::minutes(&ui, test.minutes));
    let note = match test.kind {
        TestKind::Level => L::TestNoFeedback,
        TestKind::Placement => L::PlacementNoFeedback,
    };
    rsx! {
        article { class: "lp-slide lp-enter lt-intro",
            h2 { class: "lp-slide-title",
                Rich { text: pick(&test.title, lang).to_string(), target }
            }
            if !test.review.verified {
                span { class: "pill lp-draft", "{lt(lang, L::NotReviewed)}" }
            }
            if let Some(intro) = &test.intro {
                p { class: "lp-slide-body pre", Rich { text: pick(intro, lang).trim().to_string(), target } }
            }
            p { class: "muted", "{minutes_line}" }
            p { class: "muted", "{lt(lang, note)}" }
            ol { class: if ui.eastern() { "lt-parts eastern" } else { "lt-parts" },
                for s in test.sections.iter() {
                    li { key: "{s.id}", Rich { text: pick(&s.title, lang).to_string(), target } }
                }
            }
        }
    }
}

/// One item: a lesson exercise (unlocked, no feedback) or an open task.
#[component]
fn ItemView(
    item: TestItem,
    exercise: Option<kabar_protocol::lesson::Exercise>,
    language: String,
    draft: Signal<Draft>,
    tiles: Vec<String>,
    pair_order: Vec<usize>,
    missing: Signal<bool>,
    on_matched: EventHandler<Vec<String>>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&language);
    let write_in = if language == "jv" { L::WriteInJv } else { L::WriteInId };
    match (&item.kind, exercise) {
        (TestItemKind::Exercise(_), Some(ex)) => rsx! {
            p { class: "lp-instruction", "{lt(lang, instruction(&ex, &language))}" }
            ExerciseView {
                ex,
                language: language.clone(),
                draft,
                locked: false,
                tiles,
                pair_order,
                missing,
                on_matched,
            }
        },
        (TestItemKind::WritingTask(task), _) => rsx! {
            p { class: "lp-instruction", "{lt(lang, write_in)}" }
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

/// What an item asked, in a line (for the self-check and the results).
pub(crate) fn item_prompt(item: &TestItem, lang: UiLanguage) -> String {
    use ExerciseKind as K;
    match &item.kind {
        TestItemKind::WritingTask(t) | TestItemKind::SpeakingTask(t) => {
            pick(&t.prompt, lang).trim().to_string()
        }
        TestItemKind::Exercise(kind) => match kind {
            K::MultipleChoice { question, question_meaning, .. } => {
                let q = question.trim();
                match question_meaning {
                    Some(m) if q.is_empty() || q.contains('\n') => pick(m, lang).to_string(),
                    Some(m) => format!("{q} — {}", pick(m, lang)),
                    None => q.to_string(),
                }
            }
            K::Listen { question: Some(q), text, .. } => format!("{text} — {}", pick(q, lang)),
            K::Listen { text, .. } | K::Dictation { text, .. } | K::Speaking { text, .. } => text.clone(),
            K::FillBlank { sentence, .. } => sentence.clone(),
            K::Translate { direction: Direction::ToTarget, source_meaning: Some(m), .. } => {
                pick(m, lang).to_string()
            }
            K::Translate { source_text, .. } => source_text.clone().unwrap_or_default(),
            K::BuildSentence { source, .. } => pick(source, lang).to_string(),
            K::MatchPairs { pairs } => pairs.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join(" · "),
        },
    }
}

/// The right answer(s) to show for a missed item.
fn item_answer(item: &TestItem, lang: UiLanguage) -> String {
    match (&item.kind, item.exercise()) {
        (TestItemKind::WritingTask(t) | TestItemKind::SpeakingTask(t), _) => t.sample.trim().to_string(),
        (TestItemKind::Exercise(ExerciseKind::MatchPairs { .. }), Some(ex)) => {
            references(&ex, lang).join(" · ")
        }
        (_, Some(ex)) => references(&ex, lang).into_iter().next().unwrap_or_default(),
        _ => String::new(),
    }
}

#[component]
fn SelfCheck(runner: Signal<TestRunner>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let r = runner.read();
    let target = target_tag(&r.test().language);
    let rows: Vec<(TestItem, String, Option<SelfGrade>)> = r
        .open_answers()
        .map(|rec| {
            let item = r.test().sections[rec.section].items[rec.item].clone();
            (item, r.pending_answer(&rec.id).unwrap_or_default().to_string(), rec.self_grade)
        })
        .collect();
    drop(r);
    rsx! {
        article { class: "lt-selfcheck lp-enter",
            h2 { class: "lp-slide-title", "{lt(lang, L::SelfCheckTitle)}" }
            p { class: "muted", "{lt(lang, L::SelfCheckBody)}" }
            for (item, answer, chosen) in rows {
                section { key: "{item.id}", class: "card lt-open",
                    p { class: "lp-question-meaning pre", Rich { text: item_prompt(&item, lang), target } }
                    p { class: "muted lp-small", "{lt(lang, L::YourAnswer)}" }
                    if answer.trim().is_empty() {
                        p { class: "muted", "{lt(lang, L::Spoken)}" }
                    } else {
                        p { class: "lt-answer pre", bdi { dir: "auto", "{answer}" } }
                    }
                    match item.open_task() {
                        Some(task) => rsx! {
                            p { class: "muted lp-small", "{lt(lang, L::Rubric)}" }
                            ul { class: "lt-rubric",
                                for (i, c) in task.rubric.iter().enumerate() {
                                    li { key: "{i}", bdi { lang: "en", dir: "ltr", "{c}" } }
                                }
                            }
                            p { class: "muted lp-small", "{lt(lang, L::Sample)}" }
                            p { class: "lt-sample pre", Annotated { text: task.sample.trim().to_string(), tag: target } }
                        },
                        None => rsx! {
                            p { class: "muted lp-small", "{lt(lang, L::Accepted)}" }
                            ul { class: "lp-refs",
                                for a in item.exercise().map(|ex| references(&ex, lang)).unwrap_or_default().into_iter().take(3) {
                                    li { key: "{a}", bdi { dir: "auto", class: "lp-answer", "{a}" } }
                                }
                            }
                        },
                    }
                    div { class: "row lt-grades", role: "group", aria_label: lt(lang, L::SelfRated),
                        for (g, label) in [(SelfGrade::NotYet, L::GradeNone), (SelfGrade::Partly, L::GradePartly), (SelfGrade::Fully, L::GradeFully)] {
                            button {
                                key: "{label:?}",
                                class: "chip",
                                aria_pressed: if chosen == Some(g) { "true" } else { "false" },
                                onclick: {
                                    let id = item.id.clone();
                                    move |_| runner.write().self_assess(&id, g)
                                },
                                "{lt(lang, label)}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SkillBar(label: &'static str, tally: Tally) -> Element {
    let ui = use_context::<Ui>();
    let share = tally.share().unwrap_or(0.0);
    let pct = percent(share);
    let shown = ui.num(pct);
    rsx! {
        li { class: "lt-skill",
            span { class: "lt-skill-name", "{label}" }
            div {
                class: "lt-skill-bar",
                role: "progressbar",
                aria_label: "{label}",
                aria_valuemin: "0",
                aria_valuemax: "100",
                aria_valuenow: "{pct}",
                div { class: "lt-skill-fill", style: "inline-size: {pct}%" }
            }
            span { class: "lt-skill-pct", bdi { "{shown}%" } }
        }
    }
}

#[component]
fn Results(test: TestContent, results: TestResults, on_retry: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;
    let target = target_tag(&test.language);
    let score = percent(results.total.share().unwrap_or(0.0));
    let score_text = format_number(u64::from(score), lang, settings.eastern_digits);
    let level = test.id.strip_suffix(".test").map(level_label).unwrap_or_default();
    let mark = test
        .pass_mark
        .map(|m| format!("{}%", format_number(u64::from(percent(m)), lang, settings.eastern_digits)))
        .unwrap_or_default();
    let not_passed = lt(lang, L::NotPassedBody).replace("{mark}", &mark);
    let skipped_line = lt(lang, L::Skipped).replace("{n}", &ui.num(results.skipped as u64));
    let missed: Vec<(TestItem, String, String)> = results
        .missed
        .iter()
        .map(|m| {
            let item = test.sections[m.section].items[m.item].clone();
            let (q, a) = (item_prompt(&item, lang), item_answer(&item, lang));
            (item, q, a)
        })
        .collect();
    let answer_tag = |item: &TestItem| match &item.kind {
        TestItemKind::Exercise(ExerciseKind::MultipleChoice { options_meaning, .. })
            if !options_meaning.is_empty() =>
        {
            lang.tag()
        }
        TestItemKind::Exercise(ExerciseKind::Translate { direction: Direction::FromTarget, .. }) => {
            lang.tag()
        }
        _ => target,
    };
    rsx! {
        article { class: "lp-summary lt-results",
            match (test.kind, results.passed) {
                (TestKind::Placement, _) => rsx! {
                    h2 { class: "lp-done reveal", "{lt(lang, L::PlacementTitle)}" }
                    if let Some(start) = &results.start_level {
                        p { class: "reveal",
                            Rich { text: lt(lang, L::PlacementBody).replace("{level}", &level_label(start)), target }
                        }
                    }
                },
                (_, Some(true)) => rsx! {
                    h2 { class: "lp-done reveal", "{lt(lang, L::PassedTitle)}" }
                    p { class: "reveal", Rich { text: lt(lang, L::PassedBody).replace("{level}", &level), target } }
                },
                _ => rsx! {
                    h2 { class: "lp-done reveal", "{lt(lang, L::NotPassedTitle)}" }
                    p { class: "reveal", "{not_passed}" }
                },
            }
            if test.kind == TestKind::Level {
                dl { class: "lp-stats reveal d2",
                    div { dt { "{lt(lang, L::Score)}" } dd { bdi { "{score_text}%" } } }
                }
            }
            if !results.skills.is_empty() {
                section { class: "card reveal d2",
                    h3 { "{lt(lang, L::BySkill)}" }
                    ul { class: "lt-skills",
                        for (skill, tally) in results.skills.iter() {
                            SkillBar { key: "{skill:?}", label: lt(lang, skill_label(*skill)), tally: tally.clone() }
                        }
                    }
                }
            }
            if results.skipped > 0 {
                p { class: "muted lp-small", "{skipped_line}" }
            }
            if !missed.is_empty() && test.kind == TestKind::Level {
                section { class: "card reveal d3",
                    h3 { "{lt(lang, L::ToPractise)}" }
                    ul { class: "lt-missed",
                        for (item, q, a) in missed.iter() {
                            li { key: "{item.id}",
                                p { class: "pre", Rich { text: q.clone(), target } }
                                p {
                                    span { class: "muted", "{lt(lang, L::NotYet)} " }
                                    bdi { lang: answer_tag(item), dir: "auto", class: "lp-answer pre", "{a}" }
                                }
                            }
                        }
                    }
                }
            }
            if test.kind == TestKind::Level && results.passed != Some(true) {
                button { class: "lt-retry", onclick: move |_| on_retry.call(()), "{lt(lang, L::TryAgain)}" }
            }
        }
    }
}
