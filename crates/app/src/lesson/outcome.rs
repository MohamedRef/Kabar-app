//! Lesson completion, content checks and next activities.

use super::*;

pub(super) fn duration_text(ms: u64, lang: UiLanguage, eastern: bool) -> String {
    let s = ms / 1000;
    localize_digits(&format!("{}:{:02}", s / 60, s % 60), lang, eastern)
}
#[component]
pub(super) fn SummaryView(
    lesson: LessonContent,
    passed: bool,
    check_content: bool,
    summary_score: u32,
    summary_xp: u32,
    duration_ms: u64,
    mistakes: Vec<String>,
    perfect: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<crate::play::PlayNav>();
    // The gong rings once when the summary appears.
    use_hook(move || crate::sounds::play(ui, crate::sounds::Sound::LessonEnd));
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;
    let eastern = settings.eastern_digits;
    let target = target_tag(&lesson.language);
    let num = |n: u32| format_number(u64::from(n), lang, eastern);
    rsx! {
        article { class: "lp-summary",
            div { class: "lp-gong reveal", aria_hidden: "true",
                span { class: "lp-gong-ring" }
                crate::misc_art::Picture { art: crate::misc_art::Art::Gong, class: "" }
            }
            h2 { class: "lp-done reveal", "{lt(lang, L::Done)}" }
            p { class: "lp-lesson-title reveal", bdi { lang: target, dir: "ltr", "{lesson.title}" } }
            // Finishing (and passing) is the learner's progress; whether the
            // content is right is asked separately below.
            p { class: "lp-finished reveal",
                span { class: "status-chip good", "✓ {lt(lang, L::Finished)}" }
                if passed {
                    span { class: "status-chip good", "{lt(lang, L::Passed)}" }
                }
            }
            if perfect {
                p { class: "ok reveal", "{lt(lang, L::Perfect)}" }
            }
            dl { class: "lp-stats reveal d2",
                div { dt { "{lt(lang, L::Score)}" } dd { bdi { "{format_percent(u64::from(summary_score), lang, eastern)}" } } }
                div { dt { "{lt(lang, L::Xp)}" } dd { bdi { "+{num(summary_xp)}" } } }
                div { dt { "{lt(lang, L::Time)}" } dd { bdi { "{duration_text(duration_ms, lang, eastern)}" } } }
            }
            if check_content && !lesson.review.verified {
                ContentCheck { lesson_id: lesson.id.clone() }
            }
            if !lesson.new_words.is_empty() {
                crate::ui::Card { class: "reveal d2",
                    h3 { "{lt(lang, L::NewWords)}" }
                    ul { class: "lp-words",
                        for w in lesson.new_words.iter() {
                            li { key: "{w.word}", class: if w.arabic.is_some() { "bridge-row" } else { "" },
                                Annotated { text: w.word.clone(), tag: target, class: if w.arabic.is_some() { "bridge-word" } else { "" } }
                                span { class: "muted", Rich { text: pick(&w.meaning, lang).to_string(), target } }
                                if let Some(ff) = &w.false_friend {
                                    span { class: "lp-false-friend", title: lt(lang, L::FalseFriend),
                                        span { "≠ {lt(lang, L::FalseFriend)}" }
                                        bdi { lang: "id", dir: "ltr", class: "latin", "{ff}" }
                                    }
                                }
                                if let Some(ar) = &w.arabic {
                                    span { class: "lp-bridge", title: lt(lang, L::FromArabic),
                                        span { class: "lp-bridge-label", "{lt(lang, L::FromArabic)}" }
                                        span { lang: "ar", class: "ar", "{ar}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !mistakes.is_empty() {
                crate::ui::Card { class: "reveal d3",
                    h3 { "{lt(lang, L::Tomorrow)}" }
                    p { class: "row",
                        for m in mistakes.iter() {
                            bdi { key: "{m}", lang: target, dir: "ltr", class: "pill latin", "{m}" }
                        }
                    }
                }
            }
            if let Some(s) = &lesson.souvenir {
                crate::ui::Card { class: "lp-souvenir reveal d3",
                    crate::souvenir_art::SouvenirArt {
                        unit_id: crate::souvenir_art::unit_of(&s.id).to_string(),
                        size: crate::souvenir_art::Size::Card,
                    }
                    p { class: "muted lp-small", "{lt(lang, L::Souvenir)}" }
                    h3 { bdi { lang: target, dir: "ltr", "{s.title}" } }
                    p { Rich { text: pick(&s.note, lang).to_string(), target } }
                }
            }
            if let Some(route) = lesson.game.as_deref().and_then(game_route) {
                div { class: "row center reveal d3",
                    button {
                        onclick: move |_| {
                            ui.close_lesson();
                            crate::play::open(ui, nav, route);
                        },
                        "{lt(lang, L::PlayGame)}"
                    }
                }
            }
        }
    }
}
/// The learner's check of the lesson's content, apart from their score:
/// one tap marks it right (its "not reviewed" flag and its cards' come off),
/// and the tap can be taken back. A lesson counted as checked under the old
/// pass rule says so.
#[component]
pub(super) fn ContentCheck(lesson_id: String) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.settings.read().ui_language;
    let eastern = ui.settings.read().eastern_digits;
    let read = {
        let (db, id) = (db.clone(), lesson_id.clone());
        move || {
            db.0.borrow().verified_lessons().map(|v| v.get(&id).copied()).unwrap_or_else(|e| {
                crate::diag::report!("reading checked lessons failed: {e}");
                None
            })
        }
    };
    let mut state = use_signal(read.clone());
    let set = {
        let (db, id) = (db.clone(), lesson_id.clone());
        move |ok: bool| {
            let mut store = db.0.borrow_mut();
            let done = if ok { store.verify_lesson(&id) } else { store.unverify_lesson(&id) };
            if let Err(e) = done {
                crate::diag::report!("saving the content check failed: {e}");
            }
            drop(store);
            state.set(read());
        }
    };
    match *state.read() {
        None => {
            let mut set = set.clone();
            rsx! {
                crate::ui::Card {
                    class: "card-action lp-content reveal d2",
                    onclick: move |_| set(true),
                    h3 { "{lt(lang, L::ContentTitle)}" }
                    button { class: "tonal lp-content-ok", "{lt(lang, L::ContentOk)}" }
                }
            }
        }
        Some(Verification::Checked) => {
            let mut set = set.clone();
            rsx! {
                div { class: "lp-content-done reveal d2", role: "status",
                    span { "✓ {lt(lang, L::ContentChecked)}" }
                    button { class: "ghost lp-content-undo", onclick: move |_| set(false), "{lt(lang, L::ContentUndo)}" }
                }
            }
        }
        Some(Verification::PassedEarlier { score }) => rsx! {
            div { class: "lp-content-done reveal d2", role: "status",
                span {
                    "✓ "
                    {lt(lang, L::ContentPassedEarlier).replace("{score}", &format_percent(u64::from(score), lang, eastern))}
                }
            }
        },
    }
}
/// The Play screen a lesson's `game` id opens: `uu.*` is an Unggah-Ungguh
/// scene, `mp.*` a Minimal Pairs pair. Opens the game, not the exact item.
pub(super) fn game_route(id: &str) -> Option<crate::play::Route> {
    if id.starts_with("uu.") {
        Some(crate::play::Route::Unggah)
    } else if id.starts_with("mp.") {
        Some(crate::play::Route::Pairs)
    } else {
        None
    }
}
