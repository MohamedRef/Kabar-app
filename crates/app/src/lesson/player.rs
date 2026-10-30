//! Lesson player stages, progress and session orchestration.

use super::*;

// ---------------------------------------------------------------------------
// The player
// ---------------------------------------------------------------------------

/// A Sinetron episode's questions and new words, in the Lesson Player.
/// Finishing them marks the episode watched; `on_close` leaves.
#[component]
pub fn EpisodeQuiz(lesson: LessonContent, on_close: EventHandler) -> Element {
    rsx! {
        style { {LESSON_CSS.text()} }
        PlayerView { lesson, on_close }
    }
}
/// Runs a lesson. With `on_close` it runs an episode quiz instead: no
/// Lanjut position, and closing calls it rather than going to the Journey.
#[component]
pub(super) fn PlayerView(lesson: LessonContent, on_close: Option<EventHandler>) -> Element {
    let ui = use_context::<Ui>();
    let episode = on_close.is_some();
    let close = move || match on_close {
        Some(h) => h.call(()),
        None => ui.close_lesson(),
    };
    let db_ctx = use_context::<Db>();
    let db = use_hook(|| CopyValue::new(db_ctx));
    let coach = use_context::<Coach>();
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;

    // Where this lesson was left, with the answer that was on screen.
    let saved = use_hook(|| {
        let saved = if episode { None } else { db.read().0.borrow().position().ok().flatten() };
        saved.filter(|p| p.lesson_id == lesson.id && p.step != LessonStep::Summary)
    });
    let mut player = use_signal(|| match &saved {
        Some(p) => Player::resume(lesson.clone(), p),
        None => Player::new(lesson.clone()),
    });
    // The answer comes back before Check or Continue is decided from it.
    let draft = use_signal(|| {
        let p = player.peek();
        match (&saved, p.exercise()) {
            (Some(s), Some(ex)) if s.step == p.position().step => {
                s.response.as_ref().map(|r| restore_draft(ex, r, &p.tiles(ex))).unwrap_or_default()
            }
            _ => Draft::default(),
        }
    });
    // The draft as last saved, and a counter that cancels a pending save
    // of typed text.
    let mut saved_draft = use_hook(|| CopyValue::new(draft.peek().clone()));
    let mut typing = use_hook(|| CopyValue::new(0u32));
    let last = use_signal(now_ms);
    let revealed = use_signal(|| 1usize);
    let audio_missing = use_signal(|| false);
    let mut finished = use_signal(|| false);
    // Listening items skipped because the device has no voice for them.
    let mut voice_skips = use_signal(|| 0u32);
    // The one step that carries the "no voice" note (see `voice_note_step`).
    let mut voice_note_at = use_signal(|| None::<Step>);
    // The lesson notes pane on wide windows; the learner can tuck it away.
    let mut notes_open = use_signal(|| true);

    // Save where we are on open (Lanjut), with the restored answer so a
    // second reopen still has it, and play the first exercise's audio.
    use_hook(move || {
        let p = player.peek();
        if !episode {
            save_with(db, &p, &draft.peek());
            // Just in time: this unit's and the next unit's audio (SPEC §20).
            crate::packs::prefetch_for(&p.lesson().id);
        }
        if let Some((text, audio)) = p.exercise().and_then(autoplay_text) {
            speak(&text, &p.lesson().language, false, audio.as_deref(), audio_missing);
        }
    });

    // Mutates the player, then: time, Lanjut position, fresh draft, audio,
    // and the `completed` event once the summary is reached.
    let mut commit = move |op: &dyn Fn(&mut Player) -> Option<LessonPosition>| {
        let now = now_ms();
        let mut last = last;
        let delta = now.saturating_sub(*last.peek()).min(MAX_STEP_MS);
        last.set(now);
        let pos = {
            let mut p = player.write();
            p.add_elapsed(delta);
            op(&mut p)
        };
        let Some(pos) = pos else { return };
        // A new screen starts at its top, not where the last one was scrolled.
        document::eval(r#"document.querySelector(".lp-body")?.scrollTo(0, 0);"#);
        let mut draft = draft;
        draft.set(Draft::default());
        saved_draft.set(Draft::default());
        typing += 1;
        let mut revealed = revealed;
        revealed.set(1);
        let p = player.peek();
        if p.step() == Step::Summary {
            if !*finished.peek() {
                finished.set(true);
                let db = db.read();
                {
                    let mut store = db.0.borrow_mut();
                    if let Err(e) = store.record_lesson_event(&p.lesson().id, "completed", &p.completed()) {
                        crate::diag::report!("saving the lesson result failed: {e}");
                    }
                    // The new words join the review deck, checked only if the
                    // learner already checked this lesson's content.
                    let approved = store.verified_lessons().is_ok_and(|v| v.contains_key(&p.lesson().id));
                    if !episode && let Err(e) = store.add_lesson_cards(p.lesson(), approved) {
                        crate::diag::report!("adding the lesson's words to review failed: {e}");
                    }
                    if !episode && let Err(e) = store.clear_position() {
                        crate::diag::report!("clearing the lesson position failed: {e}");
                    }
                }
                // First tries on words already in the deck move their reviews.
                let language =
                    if p.lesson().language == "jv" { StudyLanguage::Jv } else { StudyLanguage::Id };
                let signals = kabar_core::review::lesson_signals(p.lesson(), &p.completed().results);
                crate::review::note_evidence(&db, &coach, language, &signals);
                if episode { log_episode(coach, &db, &p) } else { log_activity(coach, &db, &p) }
            }
        } else if !episode {
            save(db, &pos);
        }
        if let Some((text, audio)) = p.exercise().and_then(autoplay_text) {
            speak(&text, &p.lesson().language, false, audio.as_deref(), audio_missing);
        }
        if p.step() == Step::Dialogue
            && let Some(d) = p.lesson().dialogue.as_ref()
        {
            speak_line(d, 0, &p.lesson().language, audio_missing);
        }
    };

    let mut check = move |answer: Answer| {
        let g = player.write().check(&answer, lang);
        if matches!(g, Grade::Correct | Grade::Almost { .. }) {
            chime(ui);
        } else if g == Grade::Wrong {
            crate::sounds::missed(ui);
        }
        if !episode {
            save_with(db, &player.peek(), &draft.peek());
        }
    };

    // Every change to the answer is saved, so closing the lesson (or the
    // app) and coming back shows it again. Typing is saved once it pauses.
    use_effect(move || {
        let d = draft.read().clone();
        if episode || d == *saved_draft.peek() {
            return;
        }
        let typed_only = Draft { text: d.text.clone(), ..saved_draft.peek().clone() } == d;
        saved_draft.set(d);
        typing += 1;
        if !typed_only {
            save_with(db, &player.peek(), &draft.peek());
            return;
        }
        let ticket = *typing.peek();
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            if typing.try_peek().is_ok_and(|t| *t == ticket)
                && let (Ok(p), Ok(d)) = (player.try_peek(), draft.try_peek())
            {
                save_with(db, &p, &d);
            }
        });
    });

    // The one primary action: Enter and the big button both run it.
    let mut primary = move || {
        let step = player.peek().step();
        match step {
            Step::Slide(_) => commit(&|p| Some(p.advance())),
            Step::Exercise { .. } => {
                let pending = player.peek().pending().cloned();
                match pending {
                    Some(Grade::NeedsReview) => {}
                    Some(_) => commit(&|p| Some(p.advance())),
                    None => {
                        let answer = {
                            let p = player.peek();
                            let ex = p.exercise().cloned();
                            ex.and_then(|ex| draft_answer(&ex, &draft.peek(), &p.tiles(&ex)))
                        };
                        if let Some(a) = answer {
                            check(a);
                        }
                    }
                }
            }
            Step::Dialogue => {
                let n = player.peek().lesson().dialogue.as_ref().map_or(0, |d| d.lines.len());
                let shown = *revealed.peek();
                if shown < n {
                    let mut r = revealed;
                    r.set(shown + 1);
                    let p = player.peek();
                    if let Some(d) = p.lesson().dialogue.as_ref() {
                        speak_line(d, shown, &p.lesson().language, audio_missing);
                    }
                    // Keep the newest line in view (after it renders).
                    document::eval(
                        r#"setTimeout(() => document.querySelector(".lp-lines li:last-child")?.scrollIntoView({
                            block: "end",
                            behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
                        }), 60);"#,
                    );
                } else {
                    commit(&|p| Some(p.advance()));
                }
            }
            Step::Summary => close(),
        }
    };

    // No sound for the target language (the `missing` signal): listening
    // items are skipped the "can't listen now" way, with a note, instead of
    // asking for audio that can't play.
    use_effect(move || {
        if !*audio_missing.read() {
            return;
        }
        let (skip, from) = {
            let p = player.read();
            let skip = matches!(p.step(), Step::Exercise { .. })
                && p.pending().is_none()
                && p.exercise().is_some_and(needs_audio);
            (skip, p.step())
        };
        if skip {
            voice_skips += 1;
            commit(&|p| Some(p.skip()));
            let to = player.peek().step();
            let next = voice_note_step(*voice_note_at.peek(), from, to);
            voice_note_at.set(next);
        }
    });

    // Keys are caught on the document, not on the player element: a tapped
    // tile or option can be disabled under the focus, and the next Enter must
    // still check. Enter = check/continue (Shift+Enter is a new line in text
    // answers), 1–9 = pick an option or a fill-in chip. Buttons in the feedback sheet and the
    // close button keep their native Enter.
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            if (window.__kabarLessonKeys) document.removeEventListener("keydown", window.__kabarLessonKeys, true);
            window.__kabarLessonKeys = (e) => {
                if (!document.querySelector(".lp") || e.isComposing) return;
                const t = e.target;
                if (t.closest && t.closest(".lp-feedback, .lp-top")) return;
                const typing = t.tagName === "TEXTAREA" || t.tagName === "INPUT";
                if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    dioxus.send("enter");
                } else if (!typing && !e.ctrlKey && !e.metaKey && !e.altKey && /^[1-9]$/.test(e.key)) {
                    dioxus.send(e.key);
                }
            };
            document.addEventListener("keydown", window.__kabarLessonKeys, true);
            await new Promise(() => {});
            "#,
        );
        while let Ok(key) = eval.recv::<String>().await {
            match key.as_str() {
                "enter" => primary(),
                n => {
                    let Ok(n) = n.parse::<usize>() else { continue };
                    if player.peek().pending().is_some() {
                        continue;
                    }
                    let count = player.peek().exercise().map_or(0, option_count);
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
            r#"if (window.__kabarLessonKeys) { document.removeEventListener("keydown", window.__kabarLessonKeys, true); window.__kabarLessonKeys = null; }"#,
        );
    });

    // Leaving mid-answer keeps what was typed even if it hasn't paused yet.
    let mut leave = move || {
        let unsaved =
            matches!(player.peek().step(), Step::Exercise { .. }) && *draft.peek() != *saved_draft.peek();
        if !episode && unsaved {
            typing += 1;
            save_with(db, &player.peek(), &draft.peek());
        }
        close();
    };

    let p = player.read();
    let l = p.lesson();
    let target = target_tag(&l.language);
    let step = p.step();
    let pct = (p.fraction() * 100.0).round();
    let (count_n, count_total) = p.counter();
    let pending = p.pending().cloned();
    let exercise = p.exercise().cloned();
    let can_check =
        exercise.as_ref().is_some_and(|ex| draft_answer(ex, &draft.read(), &p.tiles(ex)).is_some());

    let (label, enabled) = match (step, &pending) {
        (Step::Exercise { .. }, Some(Grade::NeedsReview)) => (lt(lang, L::Continue), false),
        (Step::Exercise { .. }, Some(_)) => (lt(lang, L::Continue), true),
        (Step::Exercise { .. }, None) => match exercise.as_ref().map(|e| &e.kind) {
            Some(ExerciseKind::Speaking { .. }) => (lt(lang, L::ISaidIt), true),
            Some(ExerciseKind::MatchPairs { .. }) => (lt(lang, L::Check), false),
            _ => (lt(lang, L::Check), can_check),
        },
        (Step::Dialogue, _) => {
            let n = l.dialogue.as_ref().map_or(0, |d| d.lines.len());
            if *revealed.read() < n { (lt(lang, L::NextLine), true) } else { (lt(lang, L::Continue), true) }
        }
        (Step::Summary, _) => (lt(lang, if episode { L::BackToEpisodes } else { back_label(ui) }), true),
        (Step::Slide(_), _) => (lt(lang, L::Continue), true),
    };
    let feedback_class = match &pending {
        Some(Grade::Correct | Grade::Practiced) => "lp-actions good",
        Some(Grade::Almost { .. }) => "lp-actions almost",
        Some(Grade::Wrong) => "lp-actions miss",
        Some(Grade::NeedsReview) => "lp-actions review",
        None => "lp-actions",
    };
    let can_skip = pending.is_none()
        && matches!(
            exercise.as_ref().map(|e| &e.kind),
            Some(
                ExerciseKind::Listen { .. } | ExerciseKind::Dictation { .. } | ExerciseKind::Speaking { .. }
            )
        );
    let tiles = exercise.as_ref().map(|ex| p.tiles(ex)).unwrap_or_default();
    let pair_order = exercise.as_ref().map(|ex| p.pair_order(ex)).unwrap_or_default();
    let language = l.language.clone();
    let summary = p.summary();
    let retry = matches!(step, Step::Exercise { retry: true, .. });
    let lesson = l.clone();
    let wide = *ui.width.read() >= kabar_core::layout::WidthClass::Expanded;
    let marks = if wide { exercise_marks(&p) } else { Vec::new() };
    let answering = matches!(step, Step::Exercise { .. }) && pending.is_none();
    let hint = match (&exercise, answering && !enabled) {
        (Some(ex), true) => Some(lt(lang, check_hint(ex))),
        _ => None,
    };
    let passed = step == Step::Summary && kabar_core::lesson::passes(&p.completed());
    let eastern = settings.eastern_digits;
    let counter = localize_digits(&format!("{count_n} / {count_total}"), lang, eastern);
    let counter_label = lt(lang, L::StepOf)
        .replace("{n}", &format_number(count_n as u64, lang, eastern))
        .replace("{total}", &format_number(count_total as u64, lang, eastern));
    drop(p);

    let with_notes = wide && *notes_open.read();
    rsx! {
        div { class: if with_notes { "lp with-notes" } else { "lp" },
            div { class: "lp-main",
            div { class: "lp-top",
                button {
                    class: "lp-close",
                    "data-back": "true",
                    aria_label: lt(lang, L::Close),
                    title: lt(lang, L::Close),
                    onclick: move |_| leave(),
                    "×"
                }
                // Isolated left to right so "3 / 12" never reads "12 / 3".
                span { class: "lp-count ltr-count", dir: "ltr", aria_label: "{counter_label}", "{counter}" }
                div {
                    class: "lp-progress",
                    role: "progressbar",
                    aria_valuemin: "0",
                    aria_valuemax: "100",
                    aria_valuenow: "{pct}",
                    div { class: "lp-bar", style: "inline-size: {pct}%" }
                }
                if wide && !with_notes {
                    notes::ShowNotes { on_show: move |_| notes_open.set(true) }
                }
            }
            div { class: "lp-body",
                div { class: "lp-col", key: "{step:?}",
                    if show_voice_note(*voice_note_at.read(), *voice_skips.read(), step) {
                        p { class: "muted lp-small lp-voice-note", role: "status", "{lt(lang, L::NoVoiceSkipped)}" }
                    }
                    match step {
                        Step::Slide(i) => rsx! {
                            SlideView { lesson: lesson.clone(), index: i, missing: audio_missing }
                        },
                        Step::Exercise { .. } => match exercise.clone() {
                            Some(ex) => rsx! {
                                p { class: "lp-focus",
                                    span { class: "status-chip neutral", bdi { dir: "ltr", "{level_label(&lesson.id)}" } }
                                    bdi { lang: target, dir: "ltr", "{lesson.title}" }
                                    if retry {
                                        span { class: "pill lp-retry", "{lt(lang, L::Retry)}" }
                                    }
                                }
                                p { class: "lp-instruction", "{lt(lang, instruction(&ex, &language))}" }
                                ExerciseView {
                                    ex,
                                    language: language.clone(),
                                    draft,
                                    locked: pending.is_some(),
                                    tiles: tiles.clone(),
                                    pair_order: pair_order.clone(),
                                    missing: audio_missing,
                                    on_matched: move |missed: Vec<String>| check(Answer::Matched { missed }),
                                }
                            },
                            None => rsx! {},
                        },
                        Step::Dialogue => rsx! {
                            if let Some(d) = lesson.dialogue.clone() {
                                DialogueView { dialogue: d, language: language.clone(), shown: *revealed.read(), missing: audio_missing }
                            }
                        },
                        Step::Summary => rsx! {
                            SummaryView { lesson: lesson.clone(), passed, check_content: !episode, summary_score: summary.score, summary_xp: summary.xp, duration_ms: summary.duration_ms, mistakes: summary.mistakes.clone(), perfect: summary.graded > 0 && summary.correct == summary.graded }
                        },
                    }
                }
            }
            div { class: "{feedback_class}",
                div { class: "lp-col lp-actions-inner",
                    if let (Some(g), Some(ex)) = (pending.clone(), exercise.clone()) {
                        Feedback {
                            ex,
                            grade: g,
                            retry,
                            target,
                            on_assess: move |same: bool| {
                                player.write().self_assess(same);
                                if same {
                                    chime(ui);
                                }
                                if !episode {
                                    save_with(db, &player.peek(), &draft.peek());
                                }
                            },
                        }
                    }
                    // Read out with a disabled Check; the screen stays clean.
                    if let Some(h) = hint {
                        p { class: "visually-hidden", id: "lp-hint", "{h}" }
                    }
                    div { class: "lp-buttons",
                        if matches!(step, Step::Slide(i) if i > 0) {
                            button { onclick: move |_| commit(&|p| p.back()), "{lt(lang, L::Back)}" }
                        }
                        if can_skip {
                            button {
                                class: "lp-skip",
                                onclick: move |_| commit(&|p| Some(p.skip())),
                                "{lt(lang, L::CantListenNow)}"
                            }
                        }
                        if pending.is_some() && exercise.is_some() {
                            button {
                                class: "lp-again",
                                aria_label: "{lt(lang, L::TryAgain)}",
                                title: "{lt(lang, L::TryAgain)}",
                                onclick: move |_| commit(&|p| p.try_again()),
                                {retry_icon()}
                            }
                        }
                        button {
                            class: "primary lp-primary",
                            disabled: !enabled,
                            aria_describedby: if hint.is_some() { "lp-hint" } else { "" },
                            onclick: move |_| primary(),
                            "{label}"
                        }
                    }
                }
            }
            }
            if with_notes {
                notes::LessonNotes {
                    lesson: lesson.clone(),
                    step,
                    marks,
                    answering,
                    missing: audio_missing,
                    on_hide: move |_| notes_open.set(false),
                }
            }
        }
    }
}
/// One mark per exercise for the notes pane: how its first try went (or its
/// retry, once that's done), the one on screen, or not reached yet.
pub(super) fn exercise_marks(p: &Player) -> Vec<notes::Mark> {
    use notes::Mark;
    let results = p.completed().results;
    let current = match p.step() {
        Step::Exercise { index, .. } => Some(index),
        _ => None,
    };
    p.lesson()
        .exercises
        .iter()
        .enumerate()
        .map(|(i, ex)| {
            let done = results.iter().find(|r| r.id == ex.id).map(|r| Mark::of(r.retry.unwrap_or(r.first)));
            if current == Some(i) { Mark::Current } else { done.unwrap_or(Mark::Todo) }
        })
        .collect()
}
/// Saves the position with the answer on screen: what's picked or typed,
/// and once checked, its grade.
pub(super) fn save_with(db: CopyValue<Db>, p: &Player, d: &Draft) {
    let mut pos = p.position();
    if let Some(ex) = p.exercise() {
        let checked = pos.response.take().and_then(|r| r.checked);
        let r = SavedResponse { checked, ..save_draft(ex, d, &p.tiles(ex)) };
        pos.response = (!r.is_empty()).then_some(r);
    }
    save(db, &pos);
}
pub(super) fn save(db: CopyValue<Db>, pos: &LessonPosition) {
    if let Err(e) = db.read().0.borrow_mut().save_position(pos) {
        crate::diag::report!("saving the lesson position failed: {e}");
    }
}
