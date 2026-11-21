use super::sim::{Node, Sim};
use super::*;
use crate::Screen;
use crate::test_dom::TestDom;
use crate::testkit::{self, Setup};
use kabar_core::coach::{history, plan};
use kabar_protocol::coach::{CoachProfile, DailyPlan, LocalDate, PlanSize};
use kabar_protocol::sync::SeqOp;

const MORNING: u64 = 1_793_775_600_000;
const UNIT: &str = "bipa1.u01";

fn view() -> Element {
    let ui = use_context::<Ui>();
    let screen = *ui.screen.read();
    match screen {
        Screen::Lesson => rsx! { LessonScreen {} },
        Screen::Today => rsx! { crate::coach::today::Today {} },
        _ => rsx! {},
    }
}

fn mount(db: Db, screen: Screen) -> Sim {
    let setup =
        Setup { db: Some(db), lang: UiLanguage::En, screen, now_ms: Some(MORNING), ..Setup::default() };
    let mut sim = Sim { dom: TestDom::unsettled(testkit::mount(view, setup)) };
    sim.settle();
    sim
}

fn deep_plan(completed_in_unit: u8) -> (Db, DailyPlan) {
    let db = testkit::memory_db();
    let profile = CoachProfile { daily_size: PlanSize::Deep, ..Default::default() };
    let date = crate::coach::clock::today(MORNING, profile.utc_offset_min);
    let plan = plan::build_plan(&plan::PlanInputs {
        date,
        size: PlanSize::Deep,
        picked: true,
        due_reviews: 2,
        current_unit: Some(UNIT),
        weak_spots: &[],
        last_study_day: None,
    });
    {
        let mut store = db.0.borrow_mut();
        store.save_coach_profile(&profile).unwrap();
        store.save_plan(&plan).unwrap();
        for n in 1..=completed_in_unit {
            store.record_lesson_event(&format!("{UNIT}.l{n:02}"), "completed", &()).unwrap();
        }
        store.add_activity(ActivityKind::Reviews { cards: 2, correct: 1 }, date, 60).unwrap();
    }
    (db, plan)
}

fn daily_stats(db: &Db, date: LocalDate) -> history::DayStats {
    history::day(&db.0.borrow().activity(Some(date)).unwrap(), date)
}

fn position(sim: &mut Sim) -> Option<LessonPosition> {
    sim.with(consume_context::<Db>).0.borrow().position().unwrap()
}

fn primary(node: &Node) -> bool {
    node.has_class("lp-primary")
}

fn finish_lesson(sim: &mut Sim, expected_id: &str) {
    // Start the completion flow at the dialogue checkpoint. The tests
    // already opened the real lesson and can keep its saved attribution.
    let mut saved = position(sim).unwrap();
    assert_eq!(saved.lesson_id, expected_id);
    saved.step = LessonStep::Dialogue;
    sim.with(|| consume_context::<Ui>().close_lesson());
    let db = sim.with(consume_context::<Db>);
    db.0.borrow_mut().save_position(&saved).unwrap();
    sim.with(|| consume_context::<Ui>().open_lesson(expected_id));
    assert_eq!(position(sim).unwrap().step, LessonStep::Dialogue);

    let lesson = parse_lesson(&lesson_content(expected_id).unwrap()).unwrap();
    for _ in 0..lesson.dialogue.unwrap().lines.len().max(1) {
        sim.click(primary);
    }
    assert!(sim.find(|n| n.has_class("lp-summary")).is_some());
    sim.click(primary);
}

#[test]
fn deep_plan_credits_the_lesson_it_opens_after_a_unit_boundary() {
    let (db, plan) = deep_plan(6);
    assert_eq!(plan.items[1].id, "lesson:bipa1.u01");
    assert_eq!(plan.items[2].id, "lesson:bipa1.u01:2");
    let mut sim = mount(db.clone(), Screen::Today);
    sim.click(|n| n.has_class("hero-start"));
    finish_lesson(&mut sim, "bipa1.u01.l07");
    assert_eq!(plan::progress(&plan, &daily_stats(&db, plan.date)).done, [true, true, false, false]);

    sim.click(|n| n.has_class("hero-start"));
    finish_lesson(&mut sim, "bipa1.u02.l01");
    let stats = daily_stats(&db, plan.date);
    assert_eq!(plan::progress(&plan, &stats).done, [true, true, true, false]);
    assert_eq!(stats.lessons_finished[UNIT], 1);
    assert_eq!(stats.lessons_finished["bipa1.u02"], 1);
    assert_eq!(stats.reviews, 2);
    assert_eq!(stats.reviews_correct, 1);
    assert!(stats.items_done.is_empty());

    drop(sim);
    let reopened = mount(db, Screen::Today);
    let hero = reopened.find(|n| n.attr("id") == Some("hero-title")).unwrap();
    assert_eq!(hero.text, crate::i18n::t(UiLanguage::En, crate::i18n::Msg::ItemReading));
}

#[test]
fn deep_plan_still_credits_two_ordinary_lessons_in_the_same_unit() {
    let (db, plan) = deep_plan(0);
    let mut sim = mount(db.clone(), Screen::Today);
    sim.click(|n| n.has_class("hero-start"));
    finish_lesson(&mut sim, "bipa1.u01.l01");
    assert_eq!(plan::progress(&plan, &daily_stats(&db, plan.date)).done, [true, true, false, false]);

    sim.click(|n| n.has_class("hero-start"));
    finish_lesson(&mut sim, "bipa1.u01.l02");
    let stats = daily_stats(&db, plan.date);
    assert_eq!(plan::progress(&plan, &stats).done, [true, true, true, false]);
    assert_eq!(stats.lessons_finished[UNIT], 2);

    sim.with(|| consume_context::<Ui>().open_lesson("bipa1.u01.l01"));
    finish_lesson(&mut sim, "bipa1.u01.l01");
    let stats = daily_stats(&db, plan.date);
    assert_eq!(stats.lessons_finished[UNIT], 3);
    assert_eq!(plan::progress(&plan, &stats).done, [true, true, true, false]);
}

fn relay(from: &Db, to: &Db) {
    let ops = from.0.borrow().outbox(500).unwrap();
    let mut to = to.0.borrow_mut();
    let first_seq = to.last_seq().unwrap() + 1;
    let ops: Vec<_> = ops.into_iter().zip(first_seq..).map(|(op, seq)| SeqOp { seq, op }).collect();
    to.apply_remote(&ops, ops.last().unwrap().seq).unwrap();
}

#[test]
fn only_the_launched_fallback_credits_the_step_and_it_survives_sync_and_resume() {
    let (phone, plan) = deep_plan(6);
    let tablet = testkit::memory_db();
    let mut sim = mount(phone.clone(), Screen::Today);
    sim.click(|n| n.has_class("hero-start"));
    finish_lesson(&mut sim, "bipa1.u01.l07");

    // Taking another lesson in the next unit independently gives no plan credit.
    sim.with(|| consume_context::<Ui>().open_lesson("bipa1.u02.l02"));
    finish_lesson(&mut sim, "bipa1.u02.l02");
    assert_eq!(plan::progress(&plan, &daily_stats(&phone, plan.date)).done, [true, true, false, false]);

    sim.click(|n| n.has_class("hero-start"));
    assert_eq!(position(&mut sim).unwrap().lesson_id, "bipa1.u02.l01");
    sim.click(primary);
    let saved = position(&mut sim).unwrap();
    assert_eq!(plan::progress(&plan, &daily_stats(&phone, plan.date)).done, [true, true, false, false]);
    relay(&phone, &tablet);

    // Resume without a fresh plan launch on the tablet.
    let mut resumed = mount(tablet.clone(), Screen::Journey);
    resumed.with(|| consume_context::<Ui>().open_lesson(saved.lesson_id.clone()));
    assert_eq!(position(&mut resumed).unwrap(), saved);
    finish_lesson(&mut resumed, "bipa1.u02.l01");
    relay(&tablet, &phone);
    let stats = daily_stats(&phone, plan.date);
    assert_eq!(stats, daily_stats(&tablet, plan.date));
    assert_eq!(plan::progress(&plan, &stats).done, [true, true, true, false]);
    assert_eq!(stats.lessons_finished["bipa1.u02"], 2);
    assert!(phone.0.borrow().position().unwrap().is_none());
}
