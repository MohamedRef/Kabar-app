//! The Lesson Player driven through its real buttons: lessons played from
//! the first slide to the summary, with right, wrong and retried answers,
//! and what that leaves in the store (Lanjut position, completion event,
//! review cards, coach activity).

use super::sim::{self, Node, Sim};
use super::*;
use kabar_core::layout::WidthClass;

thread_local! {
    static WIDTH: std::cell::Cell<WidthClass> = const { std::cell::Cell::new(WidthClass::Compact) };
}

/// The window width of the next test app (set before `Sim::new`).
fn app() -> Element {
    let (ui, _db) = sim::provide(WIDTH.get());
    rsx! {
        if ui.lesson.read().is_some() {
            LessonScreen {}
        }
    }
}

fn sim_at(width: WidthClass) -> Sim {
    WIDTH.set(width);
    Sim::new(app)
}

fn db(sim: &mut Sim) -> Db {
    sim.with(consume_context::<Db>)
}

fn ui(sim: &mut Sim) -> Ui {
    sim.with(consume_context::<Ui>)
}

fn set_lang(sim: &mut Sim, lang: UiLanguage) {
    sim.with(|| {
        let mut ui = consume_context::<Ui>();
        ui.settings.write().ui_language = lang;
    });
}

fn open_lesson(sim: &mut Sim, id: &str, from: crate::Screen) {
    let id = id.to_string();
    sim.with(move || {
        let mut ui = consume_context::<Ui>();
        ui.lesson_from.set(from);
        ui.screen.set(crate::Screen::Lesson);
        ui.lesson.set(Some(id));
    });
}

fn lesson(id: &str) -> LessonContent {
    parse_lesson(&lesson_content(id).unwrap_or_else(|| panic!("{id} not embedded"))).unwrap()
}

/// Text as SSR writes it.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('\'', "&#39;")
}

fn primary(n: &Node) -> bool {
    n.has_class("lp-primary")
}

fn position(sim: &mut Sim) -> Option<LessonPosition> {
    db(sim).0.borrow().position().unwrap()
}

/// Clicks option `i` of the choices on screen.
fn pick_option(sim: &mut Sim, i: usize) {
    let choices: Vec<Node> = sim.nodes().into_iter().filter(|n| n.has_class("lp-choice")).collect();
    let label = choices[i].text.clone();
    sim.click(move |n| n.has_class("lp-choice") && n.text == label);
}

/// What a wrong answer looks like for each kind (None: no wrong answer
/// exists, as for speaking or matching).
fn wrong_option(count: usize, right: usize) -> usize {
    if right == 0 { count - 1 } else { 0 }
}

/// Answers the exercise on screen through the UI: right, or wrong when
/// `wrong` (where a wrong answer exists). Returns whether it was answered
/// wrong.
fn answer(sim: &mut Sim, ex: &Exercise, wrong: bool) -> bool {
    use ExerciseKind as K;
    let lang = ui(sim).settings.peek().ui_language;
    match &ex.kind {
        K::MultipleChoice { answer, .. } => {
            let n = option_count(ex);
            pick_option(sim, if wrong { wrong_option(n, *answer) } else { *answer });
            sim.click(primary);
            wrong
        }
        K::Listen { answer: Some(right), options, .. } if !options.is_empty() => {
            pick_option(sim, if wrong { wrong_option(options.len(), *right) } else { *right });
            sim.click(primary);
            wrong
        }
        K::FillBlank { options, answers, .. } if !options.is_empty() => {
            let right =
                options.iter().position(|o| answers.iter().any(|a| a.eq_ignore_ascii_case(o))).unwrap();
            pick_option(sim, if wrong { wrong_option(options.len(), right) } else { right });
            sim.click(primary);
            wrong
        }
        K::FillBlank { answers, .. } => {
            let text = if wrong { "zzz".to_string() } else { answers[0].clone() };
            sim.input(|n| n.has_class("lp-blank-input"), &text);
            sim.click(primary);
            wrong
        }
        K::Listen { text, .. } | K::Dictation { text, .. } => {
            let text = if wrong { "zzz qqq".to_string() } else { text.clone() };
            sim.input(|n| n.has_class("lp-text"), &text);
            sim.click(primary);
            wrong
        }
        K::Translate { answers, answers_meaning, .. } => {
            let right = answers
                .first()
                .cloned()
                .unwrap_or_else(|| pick_all(answers_meaning.as_ref().unwrap(), lang)[0].clone());
            let text = if wrong { "zzz qqq".to_string() } else { right };
            sim.input(|n| n.has_class("lp-text"), &text);
            sim.click(primary);
            wrong
        }
        K::BuildSentence { answer, .. } => {
            if wrong {
                // One tile only: a sentence missing words.
                sim.click(|n| n.inside("lp-bank") && !n.disabled());
            } else {
                let bare = |w: &str| {
                    w.chars().filter(|c| c.is_alphanumeric() || *c == '-').collect::<String>().to_lowercase()
                };
                for word in answer.split_whitespace() {
                    let w = bare(word);
                    sim.click(move |n| n.inside("lp-bank") && !n.disabled() && bare(&n.text) == w);
                }
            }
            sim.click(primary);
            wrong
        }
        K::MatchPairs { pairs } => {
            for (i, p) in pairs.iter().enumerate() {
                let word = p.text.clone();
                let meaning = pick(&p.meaning, lang).to_string();
                if wrong && i == 0 && pairs.len() > 1 {
                    // A wrong couple first: the word goes on the practice list.
                    let other = pick(&pairs[1].meaning, lang).to_string();
                    let w = word.clone();
                    sim.click(move |n| n.has_class("lp-pair") && n.text == w);
                    sim.click(move |n| n.has_class("lp-pair") && n.text == other);
                    assert!(sim.find(|n| n.has_class("wrong")).is_some(), "a wrong couple shakes");
                }
                sim.click(move |n| n.has_class("lp-pair") && !n.disabled() && n.text == word);
                sim.click(move |n| n.has_class("lp-pair") && !n.disabled() && n.text == meaning);
            }
            wrong && pairs.len() > 1
        }
        K::Speaking { .. } => {
            sim.click(primary);
            false
        }
    }
}

/// The meaning answers in the UI language.
fn pick_all(m: &kabar_protocol::lesson::AcceptedMeanings, lang: UiLanguage) -> Vec<String> {
    match lang {
        UiLanguage::ArEg => m.ar_eg.clone(),
        UiLanguage::Ar => m.ar.clone(),
        UiLanguage::En => m.en.clone(),
    }
}

/// Plays a lesson to its summary. `miss`: which exercises (by index in the
/// lesson) to answer wrong the first time. Returns how many feedback
/// sheets showed each verdict class.
fn play(sim: &mut Sim, l: &LessonContent, miss: &dyn Fn(usize) -> bool) -> Vec<(String, String)> {
    let mut verdicts = Vec::new();
    for _ in 0..400 {
        let html = sim.html();
        if html.contains("class=\"lp-summary\"") {
            return verdicts;
        }
        let Some(pos) = position(sim) else { panic!("no position before the summary:\n{html}") };
        match &pos.step {
            LessonStep::Slide { .. } => sim.click(primary),
            LessonStep::Dialogue => sim.click(primary),
            LessonStep::Summary => unreachable!("the summary isn't saved as a position"),
            LessonStep::Exercise { id, retry } => {
                let i = l.exercises.iter().position(|e| &e.id == id).unwrap();
                // A retry reshuffles its options: answer what is on screen.
                let shown = Player::resume(l.clone(), &pos);
                let ex = shown.exercise().unwrap();
                assert_eq!(&ex.id, id);
                answer(sim, ex, !retry && miss(i));
                let html = sim.html();
                let class = ["good", "almost", "miss", "review"]
                    .into_iter()
                    .find(|c| html.contains(&format!("class=\"lp-actions {c}\"")))
                    .unwrap_or_else(|| panic!("{id}: no feedback after checking:\n{html}"));
                verdicts.push((id.clone(), class.to_string()));
                if class == "review" {
                    // An open answer: the learner compares it with the
                    // references and says it means the same.
                    assert!(sim.find(|n| n.has_class("lp-primary")).unwrap().disabled());
                    sim.click(|n| n.has_class("primary") && !n.has_class("lp-primary") && n.tag == "button");
                }
                sim.click(primary);
            }
        }
    }
    panic!("{} never reached its summary", l.id);
}

#[test]
fn a_lesson_played_right_through_lands_on_the_summary_and_saves_everything() {
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    let l = lesson("bipa1.u01.l01");
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    // Opening saves Lanjut at the first slide.
    let first = position(&mut sim).unwrap();
    assert_eq!(first.lesson_id, l.id);
    assert_eq!(first.step, LessonStep::Slide { index: 0 });
    let html = sim.html();
    assert!(html.contains("lp-count") && html.contains("1 / "), "{html}");
    assert!(html.contains("Close the lesson (progress is saved)"), "{html}");

    let verdicts = play(&mut sim, &l, &|_| false);
    assert!(!verdicts.is_empty());
    assert!(verdicts.iter().all(|(_, v)| v == "good" || v == "review"), "{verdicts:?}");

    let html = sim.html();
    assert!(html.contains(">Back to the journey<"), "{html}");
    assert!(html.contains("<dt>Score</dt><dd><bdi>100%</bdi>"), "a perfect score: {html}");
    assert!(html.contains("Is the content itself right?"), "{html}");
    // The store: completed event, no Lanjut, the words in the deck.
    let db = db(&mut sim);
    let store = db.0.borrow();
    assert!(store.position().unwrap().is_none(), "a finished lesson leaves nothing to resume");
    let events = store.lesson_events(&l.id).unwrap();
    assert!(events.iter().any(|e| e.kind == "completed"), "{events:?}");
    let cards = store.cards().unwrap();
    for w in &l.new_words {
        assert!(cards.iter().any(|c| c.word == w.word), "{} joined the deck", w.word);
    }
    drop(store);
    // Closing goes back to where the lesson was opened from.
    sim.click(primary);
    let ui = ui(&mut sim);
    assert_eq!(*ui.screen.peek(), crate::Screen::Journey);
    assert!(ui.lesson.peek().is_none());
}

/// What a lesson exercises in the player, so a handful of real lessons
/// covers every exercise shape and screen part.
fn features(l: &LessonContent) -> std::collections::BTreeSet<String> {
    use ExerciseKind as K;
    let mut f = std::collections::BTreeSet::new();
    let mut add = |s: &str| {
        f.insert(s.to_string());
    };
    add(&format!("lang-{}", l.language));
    for ex in &l.exercises {
        add(match &ex.kind {
            K::MultipleChoice { options_meaning, .. } if !options_meaning.is_empty() => "mc-meaning",
            K::MultipleChoice { question_meaning: Some(_), .. } => "mc-question-meaning",
            K::MultipleChoice { .. } => "mc",
            K::MatchPairs { .. } => "match",
            K::Translate { direction: Direction::ToTarget, .. } => "translate-to",
            K::Translate { answers, .. } if answers.is_empty() => "translate-from-meaning",
            K::Translate { .. } => "translate-from",
            K::BuildSentence { .. } => "build",
            K::Listen { options, .. } if options.is_empty() => "listen-type",
            K::Listen { question: Some(_), .. } => "listen-question",
            K::Listen { .. } => "listen-pick",
            K::FillBlank { options, .. } if options.is_empty() => "blank-typed",
            K::FillBlank { .. } => "blank-chips",
            K::Dictation { .. } => "dictation",
            K::Speaking { .. } => "speaking",
        });
        if ex.explain.is_some() {
            add("explain");
        }
    }
    if l.dialogue.as_ref().is_some_and(|d| d.title.is_some()) {
        add("dialogue-title");
    }
    if l.dialogue.is_some() {
        add("dialogue");
    }
    if l.souvenir.is_some() {
        add("souvenir");
    }
    if let Some(g) = l.game.as_deref() {
        add(&format!("game-{}", &g[..2]));
    }
    if l.new_words.iter().any(|w| w.false_friend.is_some()) {
        add("false-friend");
    }
    if l.new_words.iter().any(|w| w.arabic.is_some()) {
        add("bridge");
    }
    if l.slides.iter().any(|s| s.table.is_some()) {
        add("table");
    }
    if l.slides.iter().any(|s| s.title.is_some()) {
        add("slide-title");
    }
    if l.review.verified {
        add("verified");
    }
    f
}

/// Real lessons that between them have every feature any embedded lesson has.
fn covering_lessons() -> Vec<LessonContent> {
    let mut seen = std::collections::BTreeSet::new();
    let mut picked = Vec::new();
    for (id, yaml) in embedded::all(LESSONS) {
        let Ok(l) = parse_lesson(&yaml) else { panic!("{id} doesn't parse") };
        let f = features(&l);
        if !f.is_subset(&seen) {
            seen.extend(f);
            picked.push(l);
        }
    }
    picked
}

/// Every exercise shape in the shipped lessons can be finished through the
/// buttons, in each UI language, including the end-of-lesson retries of
/// missed answers; the summary then lists what to practise tomorrow.
#[test]
fn every_kind_of_exercise_in_the_shipped_lessons_can_be_finished_with_retries() {
    let lessons = covering_lessons();
    assert!(lessons.len() >= 5, "{}", lessons.len());
    let all: std::collections::BTreeSet<String> = lessons.iter().flat_map(features).collect();
    for kind in
        ["mc", "match", "translate-to", "build", "listen-pick", "listen-type", "dictation", "speaking"]
    {
        assert!(all.contains(kind), "no shipped lesson has {kind}: {all:?}");
    }
    let langs = [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En];
    for (n, l) in lessons.iter().enumerate() {
        let lang = langs[n % 3];
        let width = [WidthClass::Compact, WidthClass::Expanded, WidthClass::Medium][n % 3];
        let mut sim = sim_at(width);
        set_lang(&mut sim, lang);
        open_lesson(&mut sim, &l.id, crate::Screen::Today);
        // The first exercise of each kind is missed once.
        let mut kinds = std::collections::HashSet::new();
        let first_of_kind: Vec<bool> =
            l.exercises.iter().map(|e| kinds.insert(std::mem::discriminant(&e.kind))).collect();
        let verdicts = play(&mut sim, l, &|i| first_of_kind[i]);
        let missed: Vec<&Exercise> = verdicts
            .iter()
            .filter(|(_, v)| v == "miss")
            .map(|(id, _)| l.exercises.iter().find(|e| &e.id == id).unwrap())
            .collect();
        let html = sim.html();
        assert!(html.contains("class=\"lp-summary\""), "{}: {html}", l.id);
        if !missed.is_empty() {
            assert!(!html.contains(lt(lang, L::Perfect)), "{}", l.id);
            assert!(!html.contains("<bdi>100%</bdi>") && !html.contains("<bdi>١٠٠٪</bdi>"), "{}", l.id);
        }
        for w in missed.iter().filter_map(|e| e.words.first()) {
            assert!(html.contains(&esc(lt(lang, L::Tomorrow))), "{}: no practice list", l.id);
            assert!(html.contains(&format!(">{w}</bdi>")), "{}: {w} isn't on the practice list", l.id);
        }
        assert!(html.contains(&esc(lt(lang, L::BackToToday))), "{}: closes back to Today", l.id);
        let db = db(&mut sim);
        let events = db.0.borrow().lesson_events(&l.id).unwrap();
        let done: Vec<_> = events.iter().filter(|e| e.kind == "completed").collect();
        assert_eq!(done.len(), 1, "{}: one completion, however many re-renders", l.id);
        assert!(db.0.borrow().position().unwrap().is_none(), "{}", l.id);
    }
}

/// Steps forward with Continue until the saved position is an exercise.
fn to_first_exercise(sim: &mut Sim) -> (String, LessonPosition) {
    for _ in 0..40 {
        let pos = position(sim).unwrap();
        if let LessonStep::Exercise { id, .. } = &pos.step {
            return (id.clone(), pos);
        }
        sim.click(primary);
    }
    panic!("no exercise");
}

/// Slides go forward and back; Back only shows from the second slide on.
#[test]
fn slides_step_forward_and_back_and_say_where_they_are() {
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    let l = lesson("bipa1.u01.l01");
    assert!(l.slides.len() >= 2);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let back = |n: &Node| n.tag == "button" && n.text == "Back";
    assert!(sim.find(back).is_none(), "no Back on the first slide");
    let html = sim.html();
    assert!(html.contains("aria-label=\"Step 1 of "), "{html}");
    assert!(html.contains(&esc(&l.title)), "the first slide names the lesson");
    sim.click(primary);
    assert_eq!(position(&mut sim).unwrap().step, LessonStep::Slide { index: 1 });
    assert!(sim.html().contains("aria-label=\"Step 2 of "));
    sim.click(back);
    assert_eq!(position(&mut sim).unwrap().step, LessonStep::Slide { index: 0 });
}

/// Closing mid-answer keeps the typed text even before the typing pause
/// saves it, and reopening the lesson shows it again (Lanjut).
#[test]
fn a_typed_answer_survives_closing_and_reopening_the_lesson() {
    let id = embedded::all(LESSONS)
        .find(|(_, y)| {
            parse_lesson(y).is_ok_and(|l| {
                matches!(l.exercises.first().map(|e| &e.kind), Some(ExerciseKind::Translate { .. }))
            })
        })
        .map(|(id, _)| id.to_string())
        .expect("a lesson that opens its practice with a translation");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::ArEg);
    open_lesson(&mut sim, &id, crate::Screen::Journey);
    let (ex, _) = to_first_exercise(&mut sim);
    let check = sim.find(primary).unwrap();
    assert!(check.disabled(), "nothing typed: nothing to check");
    assert!(sim.html().contains(lt(UiLanguage::ArEg, L::HintType)), "the disabled Check says why");
    sim.input(|n| n.has_class("lp-text"), "Saya");
    assert!(!sim.find(primary).unwrap().disabled());
    // Typing is saved once it pauses.
    sim.wait(800);
    let saved = position(&mut sim).unwrap();
    assert_eq!(saved.response.as_ref().map(|r| r.text.as_str()), Some("Saya"));
    // More typing, then straight out: the close saves it without the pause.
    sim.input(|n| n.has_class("lp-text"), "Saya mau");
    sim.click(|n| n.has_class("lp-close"));
    assert!(ui(&mut sim).lesson.peek().is_none());
    let saved = position(&mut sim).unwrap();
    assert_eq!(saved.step, LessonStep::Exercise { id: ex.clone(), retry: false });
    assert_eq!(saved.response.as_ref().unwrap().text, "Saya mau");
    open_lesson(&mut sim, &id, crate::Screen::Journey);
    let html = sim.html();
    assert!(html.contains(">Saya mau</textarea>") || html.contains("value=\"Saya mau\""), "{html}");
}

/// A checked answer comes back checked: reopening shows the feedback and
/// Continue, not a fresh Check.
#[test]
fn a_checked_answer_reopens_with_its_feedback() {
    let l = embedded::all(LESSONS)
        .filter_map(|(_, y)| parse_lesson(&y).ok())
        .find(|l| matches!(l.exercises.first().map(|e| &e.kind), Some(ExerciseKind::MultipleChoice { .. })))
        .expect("a lesson that opens its practice with a choice");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let (_, pos) = to_first_exercise(&mut sim);
    let shown = Player::resume(l.clone(), &pos);
    let ex = shown.exercise().unwrap().clone();
    assert!(answer(&mut sim, &ex, true));
    assert!(sim.html().contains("class=\"lp-actions miss\""));
    sim.click(|n| n.has_class("lp-close"));
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let html = sim.html();
    assert!(html.contains("class=\"lp-actions miss\""), "{html}");
    assert!(html.contains(">Continue<"), "{html}");
    assert!(html.contains(&esc(lt(UiLanguage::En, L::AgainAtEnd))), "a miss comes back at the end");
}

/// "Try again" right after a miss puts the same exercise back, unanswered.
#[test]
fn try_again_clears_the_answer_for_a_second_go() {
    let l = lesson("bipa1.u01.l01");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let (id, pos) = to_first_exercise(&mut sim);
    let ex = Player::resume(l.clone(), &pos).exercise().unwrap().clone();
    answer(&mut sim, &ex, true);
    sim.click(|n| n.has_class("lp-again"));
    let html = sim.html();
    assert!(html.contains("class=\"lp-actions\""), "no feedback any more: {html}");
    assert!(sim.find(primary).unwrap().disabled(), "a fresh, empty answer");
    assert!(matches!(position(&mut sim).unwrap().step, LessonStep::Exercise { id: ref i, .. } if *i == id));
    answer(&mut sim, &ex, false);
    assert!(sim.html().contains("class=\"lp-actions good\""));
}

/// Listening items can be skipped ("can't listen now"); speaking items say
/// "I said it" and count as practised.
#[test]
fn listening_can_be_skipped_and_speaking_is_practised() {
    let l = embedded::all(LESSONS)
        .filter_map(|(_, y)| parse_lesson(&y).ok())
        .find(|l| {
            l.exercises.iter().any(needs_audio)
                && l.exercises.iter().any(|e| matches!(e.kind, ExerciseKind::Speaking { .. }))
        })
        .expect("a lesson with listening and speaking");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let (mut skipped, mut spoke) = (false, false);
    for _ in 0..200 {
        let Some(pos) = position(&mut sim) else { break };
        let LessonStep::Exercise { .. } = &pos.step else {
            sim.click(primary);
            continue;
        };
        let ex = Player::resume(l.clone(), &pos).exercise().unwrap().clone();
        if needs_audio(&ex) && !skipped {
            sim.click(|n| n.has_class("lp-skip"));
            skipped = true;
            assert_ne!(position(&mut sim).unwrap().step, pos.step, "skipping moves on");
            continue;
        }
        if let ExerciseKind::Speaking { .. } = ex.kind {
            assert!(sim.find(primary).unwrap().text.contains("I said it"));
            assert!(sim.find(|n| n.has_class("lp-skip")).is_some(), "speaking can be skipped too");
            sim.click(primary);
            assert!(sim.html().contains(lt(UiLanguage::En, L::Practiced)));
            spoke = true;
            sim.click(primary);
            continue;
        }
        answer(&mut sim, &ex, false);
        if sim.html().contains("class=\"lp-actions review\"") {
            sim.click(|n| n.has_class("primary") && !n.has_class("lp-primary") && n.tag == "button");
        }
        sim.click(primary);
    }
    assert!(skipped && spoke);
}

/// On wide windows the lesson notes sit beside the player and follow it;
/// the learner can tuck them away and bring them back.
#[test]
fn wide_windows_show_lesson_notes_that_can_be_hidden() {
    let l = lesson("bipa1.u01.l01");
    let mut sim = sim_at(WidthClass::Expanded);
    set_lang(&mut sim, UiLanguage::ArEg);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let html = sim.html();
    assert!(html.contains("class=\"lp with-notes\""), "{html}");
    assert!(html.contains("class=\"lp-notes\""));
    for w in &l.new_words {
        assert!(html.contains(&format!(">{}</bdi>", esc(&w.word))), "{} in the notes", w.word);
    }
    let (_, pos) = to_first_exercise(&mut sim);
    let html = sim.html();
    assert!(html.contains("aria-current=\"step\""), "the outline marks the part on screen");
    let ex = Player::resume(l.clone(), &pos).exercise().unwrap().clone();
    answer(&mut sim, &ex, false);
    sim.click(|n| n.has_class("ln-hide"));
    let html = sim.html();
    assert!(!html.contains("class=\"lp-notes\""), "{html}");
    assert!(html.contains("class=\"lp\""));
    sim.click(|n| n.has_class("lp-notes-show"));
    assert!(sim.html().contains("class=\"lp-notes\""));
    // Medium windows have no pane.
    let mut sim = sim_at(WidthClass::Medium);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    assert!(!sim.html().contains("class=\"lp-notes\""));
}

/// The summary's content check: one tap marks the lesson's content right,
/// and the tap can be taken back.
#[test]
fn the_content_check_can_be_given_and_taken_back() {
    let l = lesson("bipa1.u01.l01");
    assert!(!l.review.verified);
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    play(&mut sim, &l, &|_| false);
    sim.click(|n| n.has_class("lp-content"));
    let db = db(&mut sim);
    assert_eq!(db.0.borrow().verified_lessons().unwrap().get(&l.id), Some(&Verification::Checked));
    let html = sim.html();
    assert!(html.contains(&esc(lt(UiLanguage::En, L::ContentChecked))), "{html}");
    sim.click(|n| n.has_class("lp-content-undo"));
    assert_eq!(db.0.borrow().verified_lessons().unwrap().get(&l.id), None);
    assert!(sim.html().contains(&esc(lt(UiLanguage::En, L::ContentTitle))));
}

/// A lesson already checked adds its words to the deck as checked, and the
/// first slide doesn't call it "not reviewed".
#[test]
fn a_checked_lesson_adds_checked_cards() {
    let l = lesson("bipa1.u01.l01");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    let db = db(&mut sim);
    db.0.borrow_mut().verify_lesson(&l.id).unwrap();
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    play(&mut sim, &l, &|_| false);
    let html = sim.html();
    assert!(html.contains(&esc(lt(UiLanguage::En, L::ContentChecked))), "{html}");
    let cards = db.0.borrow().cards().unwrap();
    let mine: Vec<_> = cards.iter().filter(|c| l.new_words.iter().any(|w| w.word == c.word)).collect();
    assert!(!mine.is_empty() && mine.iter().all(|c| c.verified), "{mine:?}");
}

/// An id the build doesn't ship is "still being written", never a crash;
/// its button goes back where the lesson was opened from.
#[test]
fn a_missing_lesson_says_it_is_being_written() {
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, "bipa9.u99.l99", crate::Screen::Today);
    let html = sim.html();
    assert!(html.contains(&esc(lt(UiLanguage::En, L::PendingTitle))), "{html}");
    assert!(html.contains(">bipa9.u99.l99</bdi>"));
    assert!(html.contains(&esc(lt(UiLanguage::En, L::BackToToday))));
    sim.click(|n| n.tag == "button" && n.has_class("primary"));
    let ui = ui(&mut sim);
    assert_eq!(*ui.screen.peek(), crate::Screen::Today);
    assert!(ui.lesson.peek().is_none());
}

/// The words of `open()`: tests, lessons, missing ids.
#[test]
fn opening_an_id_finds_tests_lessons_or_nothing() {
    assert!(matches!(open("bipa1.test"), Opened::Test(_)));
    assert!(matches!(open("bipa1.u01.l01"), Opened::Lesson(_)));
    assert!(matches!(open("nope"), Opened::Pending));
    assert_eq!(level_label("bipa3.u02.l01"), "BIPA 3");
    assert_eq!(level_label("j2.u01.l01"), "J2");
}

/// A Sinetron episode's quiz runs in the same player without Lanjut, and
/// its last button calls the episode's close handler.
#[test]
fn an_episode_quiz_finishes_without_saving_a_position() {
    use std::cell::Cell;
    thread_local! {
        static CLOSED: Cell<bool> = const { Cell::new(false) };
    }
    /// An episode's questions: the lesson's choices, no dialogue.
    fn quiz() -> LessonContent {
        let mut l = super::play_tests::lesson("bipa1.u01.l01");
        l.exercises.retain(|e| matches!(e.kind, ExerciseKind::MultipleChoice { .. }));
        l.dialogue = None;
        l
    }
    fn quiz_app() -> Element {
        sim::provide(WidthClass::Compact);
        rsx! { EpisodeQuiz { lesson: quiz(), on_close: move |_| CLOSED.set(true) } }
    }
    CLOSED.set(false);
    let l = quiz();
    assert!(!l.exercises.is_empty());
    let mut sim = Sim::new(quiz_app);
    set_lang(&mut sim, UiLanguage::En);
    assert!(position(&mut sim).is_none(), "an episode has no Lanjut");
    let mut answered = 0;
    for _ in 0..100 {
        if sim.html().contains("class=\"lp-summary\"") {
            break;
        }
        if sim.find(|n| n.has_class("lp-choice")).is_some() {
            let ex = &l.exercises[answered];
            let ExerciseKind::MultipleChoice { answer, .. } = ex.kind else { unreachable!() };
            pick_option(&mut sim, answer);
            sim.click(primary);
            assert!(sim.html().contains("class=\"lp-actions good\""));
            answered += 1;
        }
        sim.click(primary);
        assert!(position(&mut sim).is_none(), "still no Lanjut");
    }
    assert_eq!(answered, l.exercises.len());
    let html = sim.html();
    assert!(html.contains("class=\"lp-summary\""), "{html}");
    assert!(
        !html.contains(&esc(lt(UiLanguage::En, L::ContentTitle))),
        "episodes don't ask for a content check"
    );
    assert!(position(&mut sim).is_none());
    sim.click(primary);
    assert!(CLOSED.get());
}

/// Each UI language reads the counter and the score in its own digits.
#[test]
fn eastern_digits_follow_the_setting() {
    let mut sim = sim_at(WidthClass::Compact);
    sim.with(|| {
        let mut ui = consume_context::<Ui>();
        let mut s = ui.settings.write();
        s.ui_language = UiLanguage::ArEg;
        s.eastern_digits = true;
    });
    open_lesson(&mut sim, "bipa1.u01.l01", crate::Screen::Journey);
    let html = sim.html();
    assert!(html.contains(">١ / "), "{html}");
}

// ---------------------------------------------------------------------------
// Level and placement tests
// ---------------------------------------------------------------------------

use kabar_core::lesson::level_test::{TestRunner, TestStep, parse_test};
use kabar_protocol::test::TestItemKind;

/// Takes the test on screen through its buttons, item by item, with a twin
/// runner saying which item is showing. `right`: answer closed items right;
/// open tasks are written and then rated in the self-check as `rate`.
fn take_test(sim: &mut Sim, id: &str, right: bool, rate: L) -> TestRunner {
    let test = parse_test(&test_content(id).unwrap()).unwrap();
    let mut twin = TestRunner::new(test);
    assert!(sim.html().contains("class=\"lp-slide lp-enter lt-intro\""), "the intro first");
    sim.click(primary);
    twin.start();
    let lang = ui(sim).settings.peek().ui_language;
    for _ in 0..400 {
        match twin.step() {
            TestStep::Item { .. } => {
                let item = twin.item().unwrap().clone();
                match &item.kind {
                    TestItemKind::Exercise(_) => {
                        let ex = twin.exercise().unwrap();
                        let wrong = answer(sim, &ex, !right);
                        let given = if wrong { wrong_answer(&ex) } else { super::tests::right_answer(&ex) };
                        twin.answer(&given, lang);
                    }
                    TestItemKind::WritingTask(_) => {
                        sim.input(|n| n.has_class("lp-text"), "Saya belajar bahasa Indonesia.");
                        sim.click(primary);
                        twin.answer_open("Saya belajar bahasa Indonesia.");
                    }
                    TestItemKind::SpeakingTask(_) => {
                        sim.click(primary);
                        twin.answer_open("");
                    }
                }
            }
            TestStep::SelfCheck => {
                let html = sim.html();
                assert!(html.contains(&esc(lt(lang, L::SelfCheckTitle))), "{html}");
                assert!(sim.find(primary).unwrap().disabled(), "every open task needs a rating first");
                let label = lt(lang, rate).to_string();
                while sim
                    .find(|n| {
                        n.has_class("chip") && n.text == label && n.attr("aria-pressed") == Some("false")
                    })
                    .is_some()
                {
                    let l = label.clone();
                    sim.click(move |n| {
                        n.has_class("chip") && n.text == l && n.attr("aria-pressed") == Some("false")
                    });
                }
                sim.click(primary);
                return twin;
            }
            TestStep::Results => return twin,
            TestStep::Intro => unreachable!(),
        }
    }
    panic!("{id} never ended");
}

/// The answer `answer(.., true)` gives, for the twin runner.
fn wrong_answer(ex: &Exercise) -> Answer {
    use ExerciseKind as K;
    match &ex.kind {
        K::MultipleChoice { answer, .. } => Answer::Choice(wrong_option(option_count(ex), *answer)),
        K::Listen { answer: Some(a), options, .. } if !options.is_empty() => {
            Answer::Choice(wrong_option(options.len(), *a))
        }
        K::FillBlank { options, answers, .. } if !options.is_empty() => {
            let right =
                options.iter().position(|o| answers.iter().any(|a| a.eq_ignore_ascii_case(o))).unwrap();
            Answer::Choice(wrong_option(options.len(), right))
        }
        K::FillBlank { .. } => Answer::Text("zzz".into()),
        K::BuildSentence { .. } => Answer::Text("wrong".into()),
        K::MatchPairs { pairs } => Answer::Matched { missed: vec![pairs[0].text.clone()] },
        K::Speaking { .. } => Answer::Practiced,
        _ => Answer::Text("zzz qqq".into()),
    }
}

fn open_test_sim(id: &str, lang: UiLanguage) -> Sim {
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, lang);
    open_lesson(&mut sim, id, crate::Screen::Journey);
    sim
}

/// A level test taken well passes: the result says so, and the level is
/// recorded as finished and completed.
#[test]
fn a_level_test_taken_well_passes_and_is_recorded() {
    let mut sim = open_test_sim("bipa1.test", UiLanguage::ArEg);
    let twin = take_test(&mut sim, "bipa1.test", true, L::GradeFully);
    assert_eq!(twin.results().passed, Some(true));
    let html = sim.html();
    assert!(html.contains("lt-results"), "{html}");
    assert!(html.contains(&esc(lt(UiLanguage::ArEg, L::PassedTitle))), "{html}");
    assert!(!html.contains("class=\"lt-retry\""), "nothing to retry");
    let db = db(&mut sim);
    let kinds: Vec<String> =
        db.0.borrow().lesson_events("bipa1.test").unwrap().into_iter().map(|e| e.kind).collect();
    assert!(
        kinds.contains(&"test_finished".to_string()) && kinds.contains(&"completed".to_string()),
        "{kinds:?}"
    );
    // Its button goes back where the test was opened from.
    sim.click(primary);
    assert!(ui(&mut sim).lesson.peek().is_none());
}

/// A level test taken badly doesn't pass: it lists what to practise and
/// can be taken again from the start.
#[test]
fn a_level_test_not_passed_lists_what_to_practise_and_can_be_retaken() {
    let mut sim = open_test_sim("bipa1.test", UiLanguage::En);
    let twin = take_test(&mut sim, "bipa1.test", false, L::GradeNone);
    assert_eq!(twin.results().passed, Some(false));
    let html = sim.html();
    assert!(html.contains(&esc(lt(UiLanguage::En, L::NotPassedTitle))), "{html}");
    assert!(html.contains(&esc(lt(UiLanguage::En, L::ToPractise))), "{html}");
    let db = db(&mut sim);
    let kinds: Vec<String> =
        db.0.borrow().lesson_events("bipa1.test").unwrap().into_iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec!["test_finished".to_string()], "not passed: not completed");
    sim.click(|n| n.has_class("lt-retry"));
    assert!(sim.html().contains("lt-intro"), "back to the intro");
}

/// The placement test places the learner and sends them to the Journey.
#[test]
fn the_placement_test_places_the_learner() {
    let mut sim = open_test_sim("placement", UiLanguage::Ar);
    let twin = take_test(&mut sim, "placement", true, L::GradeFully);
    assert_eq!(twin.results().start_level.as_deref(), Some("bipa5"));
    let html = sim.html();
    assert!(html.contains(&esc(lt(UiLanguage::Ar, L::PlacementTitle))), "{html}");
    let db = db(&mut sim);
    assert!(db.0.borrow().placement().unwrap().is_some(), "the placement is saved");
    sim.click(primary);
    let ui = ui(&mut sim);
    assert_eq!(*ui.screen.peek(), crate::Screen::Journey);
    assert!(ui.lesson.peek().is_none());
}

/// Closing a test midway asks first; staying keeps the place, leaving goes.
#[test]
fn leaving_a_test_midway_asks_first() {
    let mut sim = open_test_sim("bipa2.test", UiLanguage::En);
    // On the intro the close just closes; reopen and start.
    sim.click(|n| n.has_class("lp-close"));
    assert!(ui(&mut sim).lesson.peek().is_none());
    open_lesson(&mut sim, "bipa2.test", crate::Screen::Journey);
    let html = sim.html();
    assert!(html.contains("lt-intro"), "reopened: {html}");
    sim.click(primary);
    assert!(sim.html().contains("lt-section"), "started");
    // Nothing answered yet: nothing to lose, so no question.
    sim.click(|n| n.has_class("lp-close"));
    assert!(ui(&mut sim).lesson.peek().is_none());
    open_lesson(&mut sim, "bipa2.test", crate::Screen::Journey);
    sim.click(primary);
    let mut twin = TestRunner::new(parse_test(&test_content("bipa2.test").unwrap()).unwrap());
    twin.start();
    let ex = twin.exercise().expect("bipa2.test opens with an exercise");
    answer(&mut sim, &ex, false);
    sim.click(|n| n.has_class("lp-close"));
    let html = sim.html();
    assert!(html.contains("lt-confirm"), "{html}");
    assert!(html.contains(&esc(lt(UiLanguage::En, L::LeaveTestTitle))));
    sim.click(|n| n.tag == "button" && n.text == lt(UiLanguage::En, L::StayInTest));
    assert!(!sim.html().contains("lt-confirm-backdrop"));
    assert!(ui(&mut sim).lesson.peek().is_some());
    sim.click(|n| n.has_class("lp-close"));
    sim.click(|n| n.tag == "button" && n.text == lt(UiLanguage::En, L::LeaveTest));
    assert!(ui(&mut sim).lesson.peek().is_none());
}

/// Listening items in a test can be skipped; the results say how many.
#[test]
fn test_listening_items_can_be_skipped() {
    let id = embedded::all(TESTS)
        .find(|(_, y)| {
            parse_test(y).is_ok_and(|t| {
                t.sections
                    .first()
                    .and_then(|s| s.items.first())
                    .and_then(|i| i.exercise())
                    .is_some_and(|e| needs_audio(&e))
            })
        })
        .map(|(id, _)| id.to_string());
    let Some(id) = id else { return };
    let mut sim = open_test_sim(&id, UiLanguage::En);
    sim.click(primary);
    sim.click(|n| n.has_class("lp-skip"));
    assert!(sim.html().contains("lt-section"), "the next item shows");
}

/// Kak Nur's placement invite opens the placement test, until the learner
/// has finished a lesson.
#[test]
fn the_placement_invite_opens_the_test_until_a_lesson_is_done() {
    fn invite_app() -> Element {
        sim::provide(WidthClass::Compact);
        rsx! { PlacementInvite {} }
    }
    let mut sim = Sim::new(invite_app);
    sim.click(|n| n.has_class("placement-invite"));
    assert_eq!(ui(&mut sim).lesson.peek().as_deref(), Some("placement"));
    let html = sim.html();
    assert!(html.contains("placement-invite"), "{html}");
}

#[test]
fn test_labels_name_levels_parts_and_skills() {
    assert_eq!(test_player::level_label("bipa2"), "BIPA 2");
    assert_eq!(test_player::level_label("j1"), "J1");
}

// ---------------------------------------------------------------------------
// With a page that answers: keys, and a device without a voice
// ---------------------------------------------------------------------------

/// Enter continues and the number keys pick options, from anywhere on the
/// page.
#[test]
fn keys_continue_pick_and_check() {
    let l = embedded::all(LESSONS)
        .filter_map(|(_, y)| parse_lesson(&y).ok())
        .find(|l| {
            l.slides.len() >= 2
                && matches!(l.exercises.first().map(|e| &e.kind), Some(ExerciseKind::MultipleChoice { .. }))
        })
        .expect("a lesson with slides and a choice first");
    sim::live("__kabarLessonKeys");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    sim.page_sends("__kabarLessonKeys", serde_json::json!("enter"));
    assert_eq!(position(&mut sim).unwrap().step, LessonStep::Slide { index: 1 });
    while !matches!(position(&mut sim).unwrap().step, LessonStep::Exercise { .. }) {
        sim.page_sends("__kabarLessonKeys", serde_json::json!("enter"));
    }
    let ExerciseKind::MultipleChoice { answer, .. } = l.exercises[0].kind else { unreachable!() };
    // A number beyond the options does nothing; a real one picks.
    sim.page_sends("__kabarLessonKeys", serde_json::json!("9"));
    assert!(sim.find(|n| n.has_class("lp-choice") && n.attr("aria-pressed") == Some("true")).is_none());
    sim.page_sends("__kabarLessonKeys", serde_json::json!((answer + 1).to_string()));
    let chosen = sim
        .nodes()
        .into_iter()
        .filter(|n| n.has_class("lp-choice"))
        .position(|n| n.attr("aria-pressed") == Some("true"));
    assert_eq!(chosen, Some(answer));
    sim.page_sends("__kabarLessonKeys", serde_json::json!("enter"));
    assert!(sim.html().contains("class=\"lp-actions good\""));
    // Picked keys are ignored once checked.
    sim.page_sends("__kabarLessonKeys", serde_json::json!("1"));
    assert!(sim.html().contains("class=\"lp-actions good\""));
    assert!(sim::scripts().iter().any(|s| s.contains("scrollTo(0, 0)")), "a new step scrolls to the top");
}

/// With no voice on the device, listening items are skipped the "can't
/// listen now" way and the player says so once.
#[test]
fn without_a_voice_listening_items_are_skipped_with_a_note() {
    let l = embedded::all(LESSONS)
        .filter_map(|(_, y)| parse_lesson(&y).ok())
        .find(|l| l.exercises.iter().any(needs_audio) && l.exercises.iter().any(|e| !needs_audio(e)))
        .expect("a lesson with listening");
    sim::on_eval(|js| if js.contains("speechSynthesis") { vec![serde_json::json!(false)] } else { vec![] });
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let mut noted = false;
    let mut answered = Vec::new();
    for _ in 0..300 {
        let html = sim.html();
        if html.contains("class=\"lp-summary\"") {
            break;
        }
        noted |= html.contains(&esc(lt(UiLanguage::En, L::NoVoiceSkipped)));
        let pos = position(&mut sim).unwrap();
        match &pos.step {
            LessonStep::Exercise { .. } => {
                let ex = Player::resume(l.clone(), &pos).exercise().unwrap().clone();
                assert!(!needs_audio(&ex), "{}: a listening item stayed on screen", ex.id);
                answer(&mut sim, &ex, false);
                answered.push(ex.id.clone());
                if sim.html().contains("class=\"lp-actions review\"") {
                    sim.click(|n| n.has_class("primary") && !n.has_class("lp-primary") && n.tag == "button");
                }
                sim.click(primary);
            }
            _ => {
                // The play buttons on slides say there's no sound.
                if sim.find(|n| n.has_class("lp-example")).is_some() {
                    sim.click(|n| n.has_class("lp-example"));
                }
                sim.click(primary);
            }
        }
    }
    assert!(noted, "the skip is explained");
    let listening = l.exercises.iter().filter(|e| needs_audio(e)).count();
    assert_eq!(answered.len(), l.exercises.len() - listening);
    assert!(sim.html().contains(&esc(lt(UiLanguage::En, L::NoVoiceSkipped))), "and again in the summary");
}

/// Every play button reads its text aloud through the page's voice.
#[test]
fn play_buttons_speak_the_target_language() {
    let l = embedded::all(LESSONS)
        .filter_map(|(_, y)| parse_lesson(&y).ok())
        .find(|l| l.dialogue.is_some() && l.slides.iter().any(|s| !s.examples.is_empty()))
        .expect("a lesson with examples and a dialogue");
    let mut sim = sim_at(WidthClass::Expanded);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let spoken = |_: &mut Sim| sim::scripts().iter().filter(|s| s.contains("speechSynthesis")).count();
    while sim.find(|n| n.has_class("lp-example")).is_none() {
        sim.click(primary);
    }
    let before = spoken(&mut sim);
    sim.click(|n| n.has_class("lp-example"));
    sim.click(|n| n.has_class("ln-word"));
    assert_eq!(spoken(&mut sim), before + 2);
    let last = sim::scripts().into_iter().rev().find(|s| s.contains("speechSynthesis")).unwrap();
    assert!(last.contains("\"id-ID\"") || last.contains("\"jv-ID\""), "{last}");
    // On to the dialogue: Next line reveals and speaks one line at a time.
    for _ in 0..200 {
        if position(&mut sim).is_some_and(|p| p.step == LessonStep::Dialogue) {
            break;
        }
        let pos = position(&mut sim).unwrap();
        if let LessonStep::Exercise { .. } = pos.step {
            let ex = Player::resume(l.clone(), &pos).exercise().unwrap().clone();
            if needs_audio(&ex) {
                sim.click(|n| n.has_class("lp-skip"));
                continue;
            }
            answer(&mut sim, &ex, false);
            if sim.html().contains("class=\"lp-actions review\"") {
                sim.click(|n| n.has_class("primary") && !n.has_class("lp-primary") && n.tag == "button");
            }
        } else if sim.find(|n| n.has_class("lp-sound")).is_some() {
            sim.click(|n| n.has_class("lp-sound") && !n.has_class("slow"));
        }
        sim.click(primary);
    }
    let lines = l.dialogue.as_ref().unwrap().lines.len();
    assert_eq!(sim.count(|n| n.has_class("lp-bubble")), 1, "one line at a time");
    sim.click(|n| n.has_class("lp-bubble"));
    if lines > 1 {
        assert!(sim.find(primary).unwrap().text.contains(lt(UiLanguage::En, L::NextLine)));
        sim.click(primary);
        assert_eq!(sim.count(|n| n.has_class("lp-bubble")), 2);
    }
}

/// Every player string exists in all three languages, the Arabic ones are
/// Arabic, and each keeps the same {placeholders} as its English.
#[test]
fn every_player_string_is_translated_with_its_placeholders() {
    let all = [
        L::Close,
        L::Check,
        L::Continue,
        L::Back,
        L::Listen,
        L::Slowly,
        L::CantListenNow,
        L::Examples,
        L::PickMeaning,
        L::PickAnswer,
        L::MatchPairs,
        L::WriteInId,
        L::WriteInJv,
        L::WriteMeaning,
        L::BuildSentence,
        L::ListenPick,
        L::ListenType,
        L::FillBlank,
        L::Dictation,
        L::Speaking,
        L::SpeakingLater,
        L::ISaidIt,
        L::AnswerPlaceholder,
        L::Correct1,
        L::Correct2,
        L::Correct3,
        L::CorrectWord,
        L::Almost,
        L::NotYet,
        L::AgainAtEnd,
        L::NeedsReview,
        L::SameMeaning,
        L::YesSame,
        L::NotQuite,
        L::Why,
        L::Practiced,
        L::Dialogue,
        L::NextLine,
        L::TapToHear,
        L::Done,
        L::Perfect,
        L::Score,
        L::Xp,
        L::Time,
        L::NewWords,
        L::FromArabic,
        L::FalseFriend,
        L::TableMeaning,
        L::Tomorrow,
        L::Souvenir,
        L::BackToJourney,
        L::BackToEpisodes,
        L::PlayGame,
        L::PendingTitle,
        L::PendingBody,
        L::BrokenTitle,
        L::NotReviewed,
        L::HintChoose,
        L::HintType,
        L::HintTiles,
        L::HintMatch,
        L::Finished,
        L::Passed,
        L::ContentTitle,
        L::ContentOk,
        L::ContentChecked,
        L::ContentPassedEarlier,
        L::ContentUndo,
        L::StepOf,
        L::Retry,
        L::NoAudio,
        L::NoVoiceData,
        L::ListenAnswer,
        L::Transcript,
        L::CloseTest,
        L::TestStart,
        L::TestMinutes,
        L::TestRoute,
        L::TestNoFeedback,
        L::PlacementNoFeedback,
        L::TestPart,
        L::TestNext,
        L::SpeakTask,
        L::SpeakTaskNote,
        L::ISpoke,
        L::SelfCheckTitle,
        L::SelfCheckBody,
        L::SelfRated,
        L::YourAnswer,
        L::Spoken,
        L::Rubric,
        L::Sample,
        L::Accepted,
        L::GradeNone,
        L::GradePartly,
        L::GradeFully,
        L::SeeResults,
        L::PassedTitle,
        L::PassedBody,
        L::NotPassedTitle,
        L::NotPassedBody,
        L::BySkill,
        L::ToPractise,
        L::Skipped,
        L::TryAgain,
        L::PlacementTitle,
        L::PlacementBody,
        L::PlacementUnit,
        L::UnitsToRevisit,
        L::Revisit,
        L::GoToJourney,
        L::InviteTitle,
        L::InviteBody,
        L::InviteButton,
        L::SkillListening,
        L::SkillReading,
        L::SkillWriting,
        L::SkillSpeaking,
        L::SkillVocabulary,
        L::SkillGrammar,
        L::NoVoiceSkipped,
        L::LeaveTestTitle,
        L::LeaveTestBody,
        L::StayInTest,
        L::LeaveTest,
        L::BackToToday,
        L::ChoiceCorrect,
        L::ChoiceYours,
        L::ChoiceYoursCorrect,
        L::PairMatched,
    ];
    let holes = |s: &str| {
        let mut h: Vec<String> =
            s.split('{').skip(1).filter_map(|p| p.split_once('}')).map(|(n, _)| n.to_string()).collect();
        h.sort();
        h
    };
    for m in all {
        let [arz, ar, en] = [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En].map(|l| lt(l, m));
        assert!(!en.trim().is_empty(), "{m:?}");
        for a in [arz, ar] {
            assert!(a.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)), "{m:?} is Arabic: {a}");
            assert_eq!(holes(a), holes(en), "{m:?}");
        }
    }
}

/// A test file that doesn't parse shows the "broken" notice, never a crash.
#[test]
fn a_broken_test_file_says_so() {
    fn broken_app() -> Element {
        sim::provide(WidthClass::Compact);
        rsx! { test_player::TestScreen { id: "bipa9.test", yaml: "id: [unclosed" } }
    }
    let mut sim = Sim::new(broken_app);
    sim.with(|| consume_context::<Ui>().settings.write().ui_language = UiLanguage::En);
    let html = sim.html();
    assert!(html.contains(&esc(lt(UiLanguage::En, L::BrokenTitle))), "{html}");
    assert!(html.contains(">bipa9.test</bdi>"));
    // Its close button works too.
    sim.with(|| consume_context::<Ui>().lesson.set(Some("bipa9.test".into())));
    sim.click(|n| n.has_class("lp-close"));
    assert!(sim.with(consume_context::<Ui>).lesson.peek().is_none());
}

/// Test keys: Enter starts and goes on, numbers pick, both wait while the
/// "leave?" question shows.
#[test]
fn test_keys_start_pick_and_wait_for_the_leave_question() {
    sim::live("__kabarTestKeys");
    let test = parse_test(&test_content("bipa1.test").unwrap()).unwrap();
    let mut twin = TestRunner::new(test);
    twin.start();
    let ex = twin.exercise().expect("bipa1.test opens with an exercise");
    let mut sim = open_test_sim("bipa1.test", UiLanguage::En);
    let key = |sim: &mut Sim, k: &str| sim.page_sends("__kabarTestKeys", serde_json::json!(k));
    key(&mut sim, "enter");
    assert!(sim.html().contains("lt-section"), "started");
    if option_count(&ex) > 0 {
        key(&mut sim, "1");
        assert!(sim.find(|n| n.has_class("lp-choice") && n.attr("aria-pressed") == Some("true")).is_some());
        sim.click(|n| n.has_class("lp-close"));
        assert!(sim.html().contains("lt-confirm"));
        key(&mut sim, "enter");
        key(&mut sim, "2");
        assert!(sim.html().contains("lt-confirm"), "keys wait for the answer to the question");
        sim.click(|n| n.tag == "button" && n.text == lt(UiLanguage::En, L::StayInTest));
        key(&mut sim, "enter");
        assert_eq!(
            sim.count(|n| n.has_class("lp-choice") && n.attr("aria-pressed") == Some("true")),
            0,
            "next item"
        );
    }
    key(&mut sim, "x");
}

/// With no voice, a test's listening items are skipped and it says so.
#[test]
fn without_a_voice_test_listening_items_are_skipped() {
    let id = embedded::all(TESTS)
        .find(|(_, y)| {
            parse_test(y).is_ok_and(|t| {
                t.sections
                    .iter()
                    .flat_map(|s| &s.items)
                    .any(|i| i.exercise().is_some_and(|e| needs_audio(&e)))
            })
        })
        .map(|(id, _)| id.to_string())
        .expect("a test with listening");
    sim::on_eval(|js| if js.contains("speechSynthesis") { vec![serde_json::json!(false)] } else { vec![] });
    let mut sim = open_test_sim(&id, UiLanguage::En);
    sim.click(primary);
    let test = parse_test(&test_content(&id).unwrap()).unwrap();
    let mut twin = TestRunner::new(test);
    twin.start();
    for _ in 0..400 {
        let TestStep::Item { .. } = twin.step() else { break };
        let item = twin.item().unwrap().clone();
        match &item.kind {
            TestItemKind::Exercise(_) => {
                let ex = twin.exercise().unwrap();
                if needs_audio(&ex) {
                    twin.skip();
                    continue;
                }
                answer(&mut sim, &ex, false);
                twin.answer(&super::tests::right_answer(&ex), UiLanguage::En);
            }
            TestItemKind::WritingTask(_) => {
                sim.input(|n| n.has_class("lp-text"), "Saya belajar.");
                sim.click(primary);
                twin.answer_open("Saya belajar.");
            }
            TestItemKind::SpeakingTask(_) => {
                sim.click(primary);
                twin.answer_open("");
            }
        }
    }
    let html = sim.html();
    assert!(
        html.contains(&esc(lt(UiLanguage::En, L::NoVoiceSkipped)))
            || html.contains("lt-results")
            || html.contains("lt-selfcheck"),
        "{html}"
    );
    assert!(twin.results().skipped > 0);
}

/// A placement above the planned target lifts the target, and the start
/// follows the level placed into.
#[test]
fn a_placement_refines_the_coach_profile() {
    use kabar_protocol::coach::{CoachProfile, StartingPoint, Target};
    let mut sim = open_test_sim("placement", UiLanguage::ArEg);
    sim.with(|| {
        let coach = consume_context::<crate::coach::Coach>();
        let db = consume_context::<Db>();
        let profile = CoachProfile {
            target: Some(Target { level: "bipa2".into(), by: kabar_protocol::coach::LocalDate(20_800) }),
            start: StartingPoint::BrandNew,
            ..Default::default()
        };
        coach.save_profile(&db, profile);
    });
    take_test(&mut sim, "placement", true, L::GradeFully);
    let profile = sim.with(|| consume_context::<crate::coach::Coach>().profile.peek().clone()).unwrap();
    assert_eq!(profile.start, StartingPoint::CanChat);
    assert_eq!(profile.target.unwrap().level, "bipa5");
}

/// Draft keys survive content edits only where they still point somewhere.
#[test]
fn saved_answers_come_back_by_content_not_position() {
    let l = lesson("bipa1.u01.l01");
    for ex in &l.exercises {
        let tiles = Player::new(l.clone()).tiles(ex);
        let d = Draft {
            choice: (option_count(ex) > 0).then(|| option_count(ex) - 1),
            text: "Selamat".into(),
            picked: (0..tiles.len().min(2)).collect(),
            matched: match &ex.kind {
                ExerciseKind::MatchPairs { pairs } => (0..pairs.len().min(2)).collect(),
                _ => vec![],
            },
            missed: vec!["pagi".into()],
            ..Draft::default()
        };
        let saved = save_draft(ex, &d, &tiles);
        let back = restore_draft(ex, &saved, &tiles);
        assert_eq!(
            (back.choice, &back.text, &back.picked, &back.matched, &back.missed),
            (d.choice, &d.text, &d.picked, &d.matched, &d.missed),
            "{}",
            ex.id
        );
        // A reshuffle keeps the same tiles by text.
        let mut shuffled = tiles.clone();
        shuffled.reverse();
        let moved = restore_draft(ex, &saved, &shuffled);
        let words: Vec<&String> = moved.picked.iter().map(|&i| &shuffled[i]).collect();
        let before: Vec<&String> = d.picked.iter().map(|&i| &tiles[i]).collect();
        assert_eq!(words, before, "{}", ex.id);
        // Pieces the content no longer has are dropped.
        let gone = SavedResponse {
            choice: Some("not an option".into()),
            tiles: vec!["zzz".into()],
            matched: vec!["zzz".into()],
            ..saved.clone()
        };
        let back = restore_draft(ex, &gone, &tiles);
        assert!(back.choice.is_none() && back.picked.is_empty() && back.matched.is_empty(), "{}", ex.id);
        assert!(check_hint(ex) != L::Close);
        assert!(!lt(UiLanguage::En, instruction(ex, &l.language)).is_empty());
    }
    assert_eq!(target_tag("jv"), "jv");
    assert_eq!(target_tag("id"), "id");
}

/// The close button names where it goes back to.
#[test]
fn the_back_label_names_the_screen_it_returns_to() {
    let mut sim = sim_at(WidthClass::Compact);
    for (from, label) in [
        (crate::Screen::Journey, L::BackToJourney),
        (crate::Screen::Today, L::BackToToday),
        (crate::Screen::Review, L::Back),
    ] {
        let ui = ui(&mut sim);
        let mut f = ui.lesson_from;
        sim.with(move || f.set(from));
        assert_eq!(sim.with(move || back_label(ui)), label);
    }
}

/// A clip that ships with the content plays as a file; a persona's voice
/// falls back to the device voice when there is no Kabar Cloud.
#[test]
fn recorded_clips_and_persona_voices_go_to_the_page() {
    let mut sim = sim_at(WidthClass::Compact);
    sim.with(|| {
        let missing = Signal::new(false);
        speak("Selamat pagi", "id", true, Some("audio/pagi.mp3"), missing);
        speak_in("Sugeng enjing", "jv", kabar_protocol::cloud::Voice::JvSiti, missing);
    });
    let scripts = sim::scripts();
    let clip = scripts.iter().find(|s| s.contains("audio/pagi.mp3")).expect("the clip plays");
    assert!(clip.contains("clipRate = 0.75"), "slow clips play at three quarters");
    let jv = scripts.iter().find(|s| s.contains("Sugeng enjing")).expect("the line is read");
    assert!(jv.contains("\"jv-ID\""));
}

/// A lesson with a game offers it at the end; the button opens Play there.
#[test]
fn a_lessons_game_opens_from_its_summary() {
    let l = embedded::all(LESSONS)
        .filter_map(|(_, y)| parse_lesson(&y).ok())
        .find(|l| l.game.as_deref().and_then(outcome::game_route).is_some())
        .expect("a lesson with a game");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    play(&mut sim, &l, &|_| false);
    sim.click_text(lt(UiLanguage::En, L::PlayGame));
    let ui = ui(&mut sim);
    assert_eq!(*ui.screen.peek(), crate::Screen::Play);
    assert!(ui.lesson.peek().is_none());
    let nav = sim.with(consume_context::<crate::play::PlayNav>);
    assert_eq!(*nav.0.peek(), outcome::game_route(l.game.as_deref().unwrap()).unwrap());
    assert!(outcome::game_route("xx.1").is_none());
}

/// Tiles go back to the bank when tapped in the answer; "not quite" on an
/// open answer counts it as a miss; the big play buttons speak, slowly too.
#[test]
fn answers_can_be_taken_apart_and_honestly_self_rated() {
    let l = embedded::all(LESSONS)
        .filter_map(|(_, y)| parse_lesson(&y).ok())
        .find(|l| l.exercises.iter().any(|e| matches!(e.kind, ExerciseKind::BuildSentence { .. })))
        .expect("a lesson that builds a sentence");
    let mut sim = sim_at(WidthClass::Compact);
    set_lang(&mut sim, UiLanguage::En);
    open_lesson(&mut sim, &l.id, crate::Screen::Journey);
    let mut built = false;
    let mut self_rated = false;
    for _ in 0..300 {
        if sim.html().contains("class=\"lp-summary\"") {
            break;
        }
        let pos = position(&mut sim).unwrap();
        let LessonStep::Exercise { .. } = &pos.step else {
            sim.click(primary);
            continue;
        };
        let ex = Player::resume(l.clone(), &pos).exercise().unwrap().clone();
        if sim.find(|n| n.has_class("lp-sound") && n.has_class("big")).is_some() {
            let before = sim::scripts().len();
            sim.click(|n| n.has_class("lp-sound") && n.has_class("slow"));
            sim.click(|n| n.has_class("lp-sound") && n.has_class("big"));
            assert!(sim::scripts().len() >= before + 2, "both play buttons speak");
        }
        if let (ExerciseKind::BuildSentence { .. }, false) = (&ex.kind, built) {
            sim.click(|n| n.inside("lp-bank") && !n.disabled());
            assert_eq!(sim.count(|n| n.inside("lp-answer-line")), 1);
            sim.click(|n| n.inside("lp-answer-line"));
            assert_eq!(sim.count(|n| n.inside("lp-answer-line")), 0, "the tile went back");
            assert!(sim.find(primary).unwrap().disabled());
            built = true;
        }
        answer(&mut sim, &ex, false);
        if sim.html().contains("class=\"lp-actions review\"") {
            if self_rated {
                sim.click(|n| n.has_class("primary") && !n.has_class("lp-primary") && n.tag == "button");
            } else {
                sim.click_text(lt(UiLanguage::En, L::NotQuite));
                assert!(sim.html().contains("class=\"lp-actions miss\""), "not the same: a miss");
                self_rated = true;
            }
        }
        sim.click(primary);
    }
    assert!(built);
}
