//! The reset card driven by taps (`test_dom`): pick, confirm, backup, undo.

use std::cell::RefCell;
use std::rc::Rc;

use kabar_core::locale::UiLanguage::En;
use kabar_protocol::srs::{Rating, ReviewMode};

use super::*;
use crate::store::Store;
use crate::test_dom::TestDom;

thread_local! {
    static DB: RefCell<Option<Db>> = const { RefCell::new(None) };
}

const CARD: &str = "lesson.bipa1.u01.l01.pagi.recognition";

fn db() -> Db {
    DB.with(|d| d.borrow().clone().unwrap())
}

fn ui() -> Ui {
    use kabar_core::layout::WidthClass;
    Ui {
        settings: Signal::new(kabar_core::settings::Settings { ui_language: En, ..Default::default() }),
        width: Signal::new(WidthClass::Compact),
        tablet: Signal::new(false),
        screen: Signal::new(crate::Screen::Settings),
        lesson: Signal::new(None),
        lesson_from: Signal::new(crate::Screen::Today),
        focus: Signal::new(false),
    }
}

fn host() -> Element {
    let db = use_context_provider(db);
    use_context_provider(ui);
    use_context_provider(|| Coach::load(&db));
    rsx! { ResetCard {} }
}

fn level_host() -> Element {
    let db = use_context_provider(db);
    use_context_provider(ui);
    use_context_provider(|| Coach::load(&db));
    rsx! { LevelReset { level: "bipa1", code: "BIPA 1" } }
}

/// A store with one BIPA 1 answer and one finished lesson; backups go to a
/// fresh folder (or `Err` to make them fail).
fn setup(backups: Result<(), ()>) -> std::path::PathBuf {
    let mut s = Store::open_in_memory().unwrap();
    s.record_review(CARD, Rating::Good, crate::store::now_ms(), 2000, ReviewMode::Classic).unwrap();
    s.record_lesson_event("bipa1.u01.l01", "completed", &serde_json::json!({})).unwrap();
    DB.with(|d| *d.borrow_mut() = Some(Db(Rc::new(RefCell::new(s)))));
    let dir = tempfile::tempdir().unwrap().keep();
    let target = match backups {
        Ok(()) => Ok(dir.clone()),
        Err(()) => Err("no Downloads folder".to_string()),
    };
    crate::data_export::tests::EXPORT_DIR.with(|d| *d.borrow_mut() = Some(target));
    dir
}

fn studied() -> bool {
    db().0.borrow().progress_log().unwrap().iter().any(|e| e.card_id == CARD)
}

#[test]
fn one_level_takes_a_second_tap_saves_a_backup_and_undoes() {
    let dir = setup(Ok(()));
    let mut t = TestDom::new(host);
    t.click_on("button", "BIPA 1");
    let html = t.html();
    assert!(html.contains("Cards you studied") || html.contains("cards you studied: 1"), "{html}");
    assert!(html.contains("lessons you finished: 1"), "{html}");
    t.click_on("button", tr(En, R::Go));
    assert!(db().0.borrow().resets().unwrap().is_empty(), "the first tap only arms it");
    t.click_on("button", tr(En, R::Confirm));
    assert!(db().0.borrow().resets().unwrap().levels.contains_key("bipa1"));
    assert!(!studied());
    assert!(db().0.borrow().completed_lessons().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "a backup was written first");
    assert!(t.html().contains(tr(En, R::UndoScope)));
    t.click_on("button", tr(En, R::Undo));
    assert!(db().0.borrow().resets().unwrap().is_empty());
    assert!(studied());
    assert!(t.html().contains(tr(En, R::Undone)));
}

#[test]
fn the_whole_course_waits_for_the_typed_word() {
    setup(Ok(()));
    let mut t = TestDom::new(host);
    t.click_on("button", tr(En, R::All));
    let go = t.button(tr(En, R::Go));
    assert!(t.disabled(go));
    let field = t.find_all("input").into_iter().find(|&e| t.attr(e, "type") == Some("text")).unwrap();
    t.input(field, tr(En, R::TypeWord));
    let go = t.button(tr(En, R::Go));
    assert!(!t.disabled(go));
    t.click(go);
    assert!(db().0.borrow().resets().unwrap().all.is_some());
    assert!(!studied());
}

#[test]
fn a_failed_backup_resets_nothing() {
    setup(Err(()));
    let mut t = TestDom::new(level_host);
    t.click_on("button", tr(En, R::LevelButton));
    t.click_on("button", tr(En, R::Go));
    t.click_on("button", tr(En, R::Confirm));
    assert!(db().0.borrow().resets().unwrap().is_empty());
    assert!(studied());
    let html = t.html();
    assert!(html.contains("no Downloads folder"), "{html}");
}

#[test]
fn every_string_has_three_languages() {
    use kabar_core::locale::UiLanguage::{Ar, ArEg};
    let all = [
        R::Title,
        R::Intro,
        R::Pick,
        R::All,
        R::LevelButton,
        R::Restarts,
        R::Cards,
        R::Lessons,
        R::Nothing,
        R::Stays,
        R::StaysLine,
        R::BackupFirst,
        R::Go,
        R::Confirm,
        R::Cancel,
        R::TypeToConfirm,
        R::TypeWord,
        R::BackupFailed,
        R::Failed,
        R::Done,
        R::BackupAt,
        R::Undo,
        R::Undone,
        R::InForce,
        R::UndoScope,
    ];
    for r in all {
        for lang in [ArEg, Ar, En] {
            assert!(!tr(lang, r).is_empty(), "{r:?} {lang:?}");
        }
    }
}
