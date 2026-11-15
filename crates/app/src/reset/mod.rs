//! «ابدأ من الأول»: restart progress for one level or the whole course
//! (ported from Batna #1341; adapted from batna-app main:
//! crates/app/src/reset/mod.rs). The rule lives in `kabar_core::reset`: one
//! synced marker per scope, nothing deleted, undo removes the marker.
//!
//! Settings › Offline & storage holds the card (pick a level or the whole
//! course); each level on the Journey has its own button. A backup is saved
//! first and no backup means no reset. One level takes a second tap; the
//! whole course takes the typed word. Both screens are away from any open
//! lesson or review, so nothing can run under a reset.

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_core::reset::{Resets, Scope, level_of_card, level_of_lesson};

use crate::coach::Coach;
use crate::{Db, Ui};

/// Bumped after a reset or an undo here, so the Journey re-reads.
pub static REV: GlobalSignal<u64> = Signal::global(|| 0);

/// What the card says. Three languages, like every Kabar string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R {
    Title,
    Intro,
    Pick,
    All,
    LevelButton,
    Restarts,
    Cards,
    Lessons,
    Nothing,
    Stays,
    StaysLine,
    BackupFirst,
    Go,
    Confirm,
    Cancel,
    TypeToConfirm,
    TypeWord,
    BackupFailed,
    Failed,
    Done,
    BackupAt,
    Undo,
    Undone,
    InForce,
    UndoScope,
}

pub fn tr(lang: UiLanguage, r: R) -> &'static str {
    use R::*;
    use UiLanguage::*;
    match (r, lang) {
        (Title, En) => "Start over",
        (Title, _) => "ابدأ من الأول",
        (Intro, ArEg) => "لو عايز تلف لفة تانية: الكروت والدروس ترجع جديدة، وكل حاجة حفظتها تفضل زي ما هي.",
        (Intro, Ar) => "إن أردت جولة ثانية: تعود البطاقات والدروس جديدة، ويبقى كل ما حفظته كما هو.",
        (Intro, En) => "For a second pass: cards and lessons start fresh, and everything you saved stays.",
        (Pick, ArEg) => "تبدأ إيه من الأول؟",
        (Pick, Ar) => "ماذا تبدأ من جديد؟",
        (Pick, En) => "What should start over?",
        (All, ArEg) => "الكورس كله",
        (All, Ar) => "الدورة كلها",
        (All, En) => "The whole course",
        (LevelButton, ArEg) => "ابدأ المستوى ده من الأول",
        (LevelButton, Ar) => "ابدأ هذا المستوى من جديد",
        (LevelButton, En) => "Start this level over",
        (Restarts, ArEg) => "هيرجع جديد:",
        (Restarts, Ar) => "سيعود جديدًا:",
        (Restarts, En) => "Starts fresh:",
        (Cards, ArEg) => "كروت ذاكرتها",
        (Cards, Ar) => "بطاقات راجعتها",
        (Cards, En) => "cards you studied",
        (Lessons, ArEg) => "دروس خلصتها",
        (Lessons, Ar) => "دروس أنهيتها",
        (Lessons, En) => "lessons you finished",
        (Nothing, ArEg) => "مفيش تقدّم لسه هنا، فمفيش حاجة هتتغيّر.",
        (Nothing, Ar) => "لا تقدّم هنا بعد، فلن يتغيّر شيء.",
        (Nothing, En) => "No progress here yet, so nothing changes.",
        (Stays, ArEg) => "هيفضل زي ما هو:",
        (Stays, Ar) => "يبقى كما هو:",
        (Stays, En) => "Stays as it is:",
        (StaysLine, ArEg) => {
            "كلماتك المحفوظة، القاموس بتاعك، الهايلايت، تأكيدك على الدروس، النقط والسلسلة، اليوميات، والإعدادات."
        }
        (StaysLine, Ar) => {
            "كلماتك المحفوظة، قاموسك، التظليل، تأكيدك للدروس، النقاط والسلسلة، اليوميات، والإعدادات."
        }
        (StaysLine, En) => {
            "Saved words, your dictionary, highlights, lessons you checked, XP and streak, the journal and settings."
        }
        (BackupFirst, ArEg) => {
            "هحفظ نسخة احتياطية الأول، ولو ما عرفتش مش هبدأ. وتقدر ترجّع كل حاجة من هنا في أي وقت."
        }
        (BackupFirst, Ar) => {
            "سأحفظ نسخة احتياطية أولًا، وإن تعذّر ذلك فلن أبدأ. ويمكنك استعادة كل شيء من هنا في أي وقت."
        }
        (BackupFirst, En) => {
            "A backup is saved first; if that fails, nothing starts over. You can undo it here any time."
        }
        (Go, En) => "Start over",
        (Go, _) => "ابدأ من الأول",
        (Confirm, ArEg) => "أيوه، ابدأ من الأول",
        (Confirm, Ar) => "نعم، ابدأ من جديد",
        (Confirm, En) => "Yes, start over",
        (Cancel, ArEg) => "لأ، سيبها",
        (Cancel, Ar) => "لا، اتركها",
        (Cancel, En) => "No, leave it",
        (TypeToConfirm, ArEg) => "للكورس كله، اكتب «ابدأ» علشان نتأكد:",
        (TypeToConfirm, Ar) => "للدورة كلها، اكتب «ابدأ» للتأكيد:",
        (TypeToConfirm, En) => "For the whole course, type «ابدأ» to confirm:",
        (TypeWord, _) => "ابدأ",
        (BackupFailed, ArEg) => "ما عرفتش أحفظ النسخة الاحتياطية، فما بدأتش حاجة:",
        (BackupFailed, Ar) => "تعذّر حفظ النسخة الاحتياطية، فلم يبدأ شيء:",
        (BackupFailed, En) => "The backup could not be saved, so nothing started over:",
        (Failed, ArEg) => "ما عرفتش أكمّل، ومفيش حاجة اتغيّرت:",
        (Failed, Ar) => "تعذّر الإكمال، ولم يتغيّر شيء:",
        (Failed, En) => "That didn't work, and nothing changed:",
        (Done, ArEg) => "تمام، بدأ من الأول:",
        (Done, Ar) => "تمّ، بدأ من جديد:",
        (Done, En) => "Done, started over:",
        (BackupAt, ArEg) => "النسخة الاحتياطية اتحفظت في:",
        (BackupAt, Ar) => "حُفظت النسخة الاحتياطية في:",
        (BackupAt, En) => "The backup is at:",
        (Undo, ArEg) => "رجّع زي ما كان",
        (Undo, Ar) => "أعِده كما كان",
        (Undo, En) => "Put it back",
        (Undone, ArEg) => "رجع كل حاجة زي ما كانت.",
        (Undone, Ar) => "عاد كل شيء كما كان.",
        (Undone, En) => "Everything is back as it was.",
        (InForce, ArEg) => "اللي بدأته من الأول",
        (InForce, Ar) => "ما بدأته من جديد",
        (InForce, En) => "Started over",
        (UndoScope, ArEg) => "رجّع تقدّم",
        (UndoScope, Ar) => "استعد تقدّم",
        (UndoScope, En) => "Restore progress of",
    }
}

/// A scope's name: the level's code (`BIPA 1`), or «الكورس كله».
pub fn name(lang: UiLanguage, scope: &Scope) -> String {
    match scope {
        Scope::All => tr(lang, R::All).to_string(),
        Scope::Level(l) => crate::journey::levels()
            .into_iter()
            .find(|(id, _)| id == l)
            .map_or_else(|| l.clone(), |(_, code)| code),
    }
}

/// Whether a reset of this level is in force (its undo shows on the Journey).
pub fn in_force(level: &str) -> bool {
    let _ = REV();
    try_consume_context::<Db>()
        .and_then(|db| db.0.borrow().resets().ok())
        .is_some_and(|r| r.levels.contains_key(level))
}

/// What a reset of `scope` would restart, with real numbers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Preview {
    pub cards: usize,
    pub lessons: usize,
}

pub fn preview(db: &Db, scope: &Scope) -> Preview {
    let store = db.0.borrow();
    let in_scope_card = |id: &str| match scope {
        Scope::All => true,
        Scope::Level(l) => level_of_card(id) == Some(l.as_str()),
    };
    let cards: std::collections::BTreeSet<String> = store
        .progress_log()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| in_scope_card(&e.card_id))
        .map(|e| e.card_id)
        .collect();
    let lessons = store
        .completed_lessons()
        .unwrap_or_default()
        .iter()
        .filter(|id| {
            matches!(scope, Scope::All) || matches!(scope, Scope::Level(l) if level_of_lesson(id) == l)
        })
        .count();
    Preview { cards: cards.len(), lessons }
}

/// Why a reset did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Backup(String),
    Store(String),
}

/// Saves a backup, then writes the marker. Returns where the backup went.
pub fn run(db: &Db, scope: &Scope, now_ms: u64, offset: i16) -> Result<String, Refused> {
    let backup = crate::data_export::backup_now(&db.0.borrow(), now_ms, offset).map_err(Refused::Backup)?;
    db.0.borrow_mut().put_reset(scope).map_err(|e| Refused::Store(e.to_string()))?;
    changed();
    Ok(backup)
}

/// Takes a reset back.
pub fn undo(db: &Db, scope: &Scope) -> Result<(), String> {
    db.0.borrow_mut().remove_reset(scope).map_err(|e| e.to_string())?;
    changed();
    Ok(())
}

/// Every screen that reads progress reads again.
fn changed() {
    *REV.write() += 1;
    if let Some(mut cloud) = try_consume_context::<crate::account::Cloud>() {
        *cloud.revision.write() += 1;
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Shown {
    Idle,
    Done { scope: Scope, backup: String },
    Failed(Refused),
    Undone,
}

/// Settings: pick a level or the whole course, then confirm; the resets in
/// force, each with its undo.
#[component]
pub fn ResetCard() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let mut picked = use_signal(|| None::<Scope>);
    let scopes: Vec<Scope> =
        crate::journey::levels().into_iter().map(|(id, _)| Scope::Level(id)).chain([Scope::All]).collect();
    rsx! {
        crate::ui::DivCard { class: "reset-card",
            h2 { "{tr(lang, R::Title)}" }
            p { class: "muted", "{tr(lang, R::Intro)}" }
            div { role: "radiogroup", aria_label: "{tr(lang, R::Pick)}",
                p { class: "label", "{tr(lang, R::Pick)}" }
                div { class: "chips",
                    for s in scopes {
                        button {
                            key: "{s.id()}",
                            class: "chip",
                            role: "radio",
                            "aria-checked": if picked() == Some(s.clone()) { "true" } else { "false" },
                            onclick: {
                                let s = s.clone();
                                move |_| {
                                    let same = picked() == Some(s.clone());
                                    picked.set(if same { None } else { Some(s.clone()) });
                                }
                            },
                            bdi { "{name(lang, &s)}" }
                        }
                    }
                }
            }
            if let Some(scope) = picked() {
                Confirm { key: "{scope.id()}", scope, on_cancel: move |_| picked.set(None) }
            }
            InForce {}
        }
    }
}

/// A level on the Journey: one button, then the same confirm.
#[component]
pub fn LevelReset(level: String, code: String) -> Element {
    let ui = use_context::<Ui>();
    let mut open = use_signal(|| false);
    let scope = Scope::Level(level);
    rsx! {
        div { class: "level-reset",
            if open() {
                Confirm { scope: scope.clone(), on_cancel: move |_| open.set(false) }
            } else {
                button { class: "ghost reset-open", aria_label: "{tr(ui.lang(), R::LevelButton)} {code}", onclick: move |_| open.set(true),
                    "{tr(ui.lang(), R::LevelButton)}"
                }
                UndoOne { scope }
            }
        }
    }
}

#[component]
fn Confirm(scope: Scope, on_cancel: EventHandler) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let lang = ui.lang();
    let _ = REV();
    let p = preview(&db, &scope);
    let mut armed = use_signal(|| false);
    let mut typed = use_signal(String::new);
    let mut shown = use_signal(|| Shown::Idle);
    let all = scope == Scope::All;
    let ready = !all || typed().trim() == tr(lang, R::TypeWord);
    let go = {
        let scope = scope.clone();
        let db = db.clone();
        move |_| {
            shown.set(match run(&db, &scope, coach.now(), coach.offset()) {
                Ok(backup) => Shown::Done { scope: scope.clone(), backup },
                Err(e) => Shown::Failed(e),
            });
            armed.set(false);
            typed.set(String::new());
        }
    };
    let n = name(lang, &scope);
    rsx! {
        div { class: "reset-confirm", role: "region", aria_label: "{tr(lang, R::Title)}",
            match shown() {
                Shown::Done { scope, backup } => rsx! {
                    Done { scope, backup, on_undone: move |_| shown.set(Shown::Undone) }
                },
                Shown::Undone => rsx! { p { role: "status", "{tr(lang, R::Undone)}" } },
                other => rsx! {
                    p {
                        strong { "{tr(lang, R::Restarts)} " }
                        bdi { "{n}" }
                        " · "
                        if p.cards == 0 && p.lessons == 0 {
                            "{tr(lang, R::Nothing)}"
                        } else {
                            "{tr(lang, R::Cards)}: {ui.num(p.cards as u64)} · {tr(lang, R::Lessons)}: {ui.num(p.lessons as u64)}"
                        }
                    }
                    p { strong { "{tr(lang, R::Stays)} " } "{tr(lang, R::StaysLine)}" }
                    p { class: "muted", "{tr(lang, R::BackupFirst)}" }
                    if let Shown::Failed(e) = other {
                        p { role: "alert",
                            match e {
                                Refused::Backup(d) => rsx! { "{tr(lang, R::BackupFailed)} " bdi { dir: "ltr", "{d}" } },
                                Refused::Store(d) => rsx! { "{tr(lang, R::Failed)} " bdi { dir: "ltr", "{d}" } },
                            }
                        }
                    }
                    if all {
                        label { class: "reset-type",
                            span { "{tr(lang, R::TypeToConfirm)}" }
                            input {
                                r#type: "text",
                                value: "{typed}",
                                aria_label: "{tr(lang, R::TypeToConfirm)}",
                                oninput: move |e| typed.set(e.value()),
                            }
                        }
                        div { class: "row",
                            button { class: "primary", disabled: !ready, onclick: go, "{tr(lang, R::Go)}" }
                            button { class: "ghost", onclick: move |_| on_cancel.call(()), "{tr(lang, R::Cancel)}" }
                        }
                    } else if armed() {
                        div { class: "row",
                            button { class: "primary", onclick: go, "{tr(lang, R::Confirm)}" }
                            button { class: "ghost", onclick: move |_| on_cancel.call(()), "{tr(lang, R::Cancel)}" }
                        }
                    } else {
                        div { class: "row",
                            button { class: "primary", onclick: move |_| armed.set(true), "{tr(lang, R::Go)}" }
                            button { class: "ghost", onclick: move |_| on_cancel.call(()), "{tr(lang, R::Cancel)}" }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn Done(scope: Scope, backup: String, on_undone: EventHandler) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.lang();
    let mut failed = use_signal(|| None::<String>);
    rsx! {
        div { role: "status",
            p { "{tr(lang, R::Done)} " bdi { "{name(lang, &scope)}" } }
            p { class: "muted", "{tr(lang, R::BackupAt)} " bdi { dir: "ltr", "{backup}" } }
            button {
                class: "ghost reset-undo",
                onclick: move |_| match undo(&db, &scope) {
                    Ok(()) => on_undone.call(()),
                    Err(e) => failed.set(Some(e)),
                },
                "{tr(lang, R::Undo)}"
            }
            if let Some(e) = failed() {
                p { role: "alert", "{tr(lang, R::Failed)} " bdi { dir: "ltr", "{e}" } }
            }
        }
    }
}

#[component]
fn InForce() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let _ = REV();
    let resets: Resets = db.0.borrow().resets().unwrap_or_default();
    if resets.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "reset-in-force",
            h3 { "{tr(ui.lang(), R::InForce)}" }
            for scope in resets.scopes() {
                UndoOne { key: "{scope.id()}", scope }
            }
        }
    }
}

/// «رجّع تقدّم …» for one reset in force; nothing when there is none.
#[component]
fn UndoOne(scope: Scope) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.lang();
    let mut failed = use_signal(|| None::<String>);
    let _ = REV();
    let standing = db.0.borrow().resets().ok().is_some_and(|r| r.get(&scope).is_some());
    if !standing {
        return rsx! {};
    }
    rsx! {
        button {
            class: "ghost reset-undo",
            onclick: {
                let scope = scope.clone();
                let db = db.clone();
                move |_| {
                    if let Err(e) = undo(&db, &scope) {
                        failed.set(Some(e));
                    }
                }
            },
            "{tr(lang, R::UndoScope)} "
            bdi { "{name(lang, &scope)}" }
        }
        if let Some(e) = failed() {
            p { role: "alert", "{tr(lang, R::Failed)} " bdi { dir: "ltr", "{e}" } }
        }
    }
}

#[cfg(test)]
mod tests;
