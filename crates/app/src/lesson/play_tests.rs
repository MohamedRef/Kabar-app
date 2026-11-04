//! The Lesson Player driven through its real buttons, and what that leaves
//! in the store (the Lanjut position and the answer on screen).

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

fn primary(n: &Node) -> bool {
    n.has_class("lp-primary")
}

fn position(sim: &mut Sim) -> Option<LessonPosition> {
    db(sim).0.borrow().position().unwrap()
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
