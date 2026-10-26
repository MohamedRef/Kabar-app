//! Write-mode answers for the Lesson Player: [`InkPad`] captures a
//! handwritten answer and hands over its text; [`InkFeedback`] circles the
//! letters that differ from the expected answer. The player grades the text
//! with its usual grader and never sees the strokes (they stay in this doc).

use dioxus::prelude::*;
use kabar_core::ai::FriendlyReason;
use kabar_core::ink::answer::{Mark, letter_diff};
use kabar_core::ink::group::group_lines;
use kabar_core::ink::recognize::{Handwriting, handwriting_request, transcript};
use kabar_protocol::content::Lang;
use kabar_protocol::ink::{DocKind, InkDocument, TranscriptSource};

use super::ink::{InkCanvas, Pen, use_ink};
use super::{T, friendly, new_id, tr, vision};
use crate::store::now_ms;
use crate::teach::Ai;
use crate::{Db, Ui};

/// What the pad hands to the player. `text` is always set when the pad
/// submits: recognized, or typed by the learner when recognition isn't there.
#[derive(Clone, Debug, PartialEq)]
pub struct InkAnswer {
    pub text: Option<String>,
    /// Strokes on the pad.
    pub strokes: usize,
    /// The `ink_documents` row holding the strokes.
    pub doc_id: String,
}

/// Page size of an answer sheet, page pixels.
const SHEET: [f32; 2] = [720.0, 220.0];

/// A handwriting box with a typed fallback. `expected` (dictation) helps the
/// recognizer read; it is never filled in for the learner.
#[component]
pub fn InkPad(
    lang: Lang,
    #[props(default)] expected: Option<String>,
    on_submit: EventHandler<InkAnswer>,
) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let handle = use_ink();
    let pen = use_signal(Pen::default);
    let mut typed = use_signal(String::new);
    let mut problem = use_signal(|| None::<T>);
    let mut busy = use_signal(|| false);
    let doc_id = use_hook(|| {
        let doc = InkDocument {
            id: new_id(),
            kind: DocKind::LessonAnswer,
            notebook: None,
            title: String::new(),
            lang,
            order: 0,
            anchor: None,
            size: Some(SHEET),
            created_ms: now_ms(),
        };
        if let Err(e) = db.0.borrow_mut().save_ink_document(&doc) {
            eprintln!("kabar: saving the answer sheet failed: {e}");
        }
        doc.id
    });
    let l = ui.lang();

    let clear = {
        let db = db.clone();
        let handle = handle.clone();
        let doc_id = doc_id.clone();
        move |_| {
            let strokes = db.0.borrow().strokes(&doc_id).unwrap_or_default();
            for s in strokes {
                let _ = db.0.borrow_mut().erase_stroke(&s.id);
            }
            handle.clear();
            typed.set(String::new());
            problem.set(None);
        }
    };

    let check = {
        let db = db.clone();
        let handle = handle.clone();
        let doc_id = doc_id.clone();
        move |_| {
            let db = db.clone();
            let ai = ai.clone();
            let handle = handle.clone();
            let doc_id = doc_id.clone();
            let expected = expected.clone();
            spawn(async move {
                let strokes = db.0.borrow().strokes(&doc_id).unwrap_or_default();
                let save = |text: &str, source: TranscriptSource| {
                    let groups = group_lines(&strokes);
                    if let Some(g) = groups.first() {
                        let mut t = transcript(&doc_id, g, text, lang, source, None);
                        t.strokes = strokes.iter().map(|s| s.id.clone()).collect();
                        let _ = db.0.borrow_mut().save_transcript(&t);
                    }
                };
                let text = typed().trim().to_string();
                if !text.is_empty() {
                    save(&text, TranscriptSource::Typed);
                    on_submit.call(InkAnswer { text: Some(text), strokes: strokes.len(), doc_id });
                    return;
                }
                if strokes.is_empty() {
                    problem.set(Some(T::WriteHere));
                    return;
                }
                busy.set(true);
                let read = async {
                    let png = handle.snapshot(None).await.ok_or(FriendlyReason::TryAgain)?;
                    let sha = vision::upload(&png).await?;
                    let req = handwriting_request(lang, expected.as_deref(), &sha);
                    ai.0.run::<Handwriting>(&req).await.map_err(|e| e.friendly())
                };
                let result = read.await;
                busy.set(false);
                match result {
                    Ok(answer) if !answer.value.text.trim().is_empty() => {
                        let text = answer.value.text.trim().to_string();
                        save(&text, TranscriptSource::Ai);
                        typed.set(text.clone());
                        on_submit.call(InkAnswer { text: Some(text), strokes: strokes.len(), doc_id });
                    }
                    Ok(_) => problem.set(Some(T::TypeWhatYouWrote)),
                    Err(f) => problem.set(Some(friendly(f))),
                }
            });
        }
    };

    rsx! {
        div { class: "ink-pad",
            InkCanvas { handle: handle.clone(), doc: doc_id.clone(), size: Some(SHEET), pen, class: "ink-sheet" }
            div { class: "row ink-pad-row",
                input {
                    class: "latin",
                    dir: "ltr",
                    lang: lang.code(),
                    r#type: "text",
                    autocapitalize: "off",
                    spellcheck: "false",
                    placeholder: "{tr(l, T::TypeWhatYouWrote)}",
                    value: "{typed}",
                    oninput: move |e| typed.set(e.value()),
                }
                button { onclick: clear, "{tr(l, T::Clear)}" }
                button { class: "primary", disabled: busy(), onclick: check,
                    if busy() { "{tr(l, T::Reading)}" } else { "{tr(l, T::Check)}" }
                }
            }
            if let Some(p) = problem() {
                p { class: "muted small", role: "status", "{tr(l, p)}" }
            }
        }
    }
}

/// The learner's answer with wrong letters circled, missing ones marked and
/// extra ones struck, then the correct form underneath.
#[component]
pub fn InkFeedback(written: String, expected: String) -> Element {
    let ui = use_context::<Ui>();
    let marks = letter_diff(&expected, &written);
    rsx! {
        div { class: "ink-feedback",
            p { class: "ink-letters latin", dir: "ltr",
                for (i, m) in marks.iter().enumerate() {
                    match m.mark {
                        Mark::Keep => rsx! { span { key: "{i}", "{m.written.unwrap_or(' ')}" } },
                        Mark::Wrong => rsx! { span { key: "{i}", class: "ink-wrong", "{m.written.unwrap_or(' ')}" } },
                        Mark::Extra => rsx! { span { key: "{i}", class: "ink-extra", "{m.written.unwrap_or(' ')}" } },
                        Mark::Missing => rsx! { span { key: "{i}", class: "ink-missing", aria_label: "{m.expected.unwrap_or(' ')}", "‸" } },
                    }
                }
            }
            p { class: "small",
                "{tr(ui.lang(), T::CorrectForm)} "
                bdi { class: "latin", "{expected}" }
            }
        }
    }
}

/// Dictation practice in the Studio: Kabar says a word, the learner writes
/// it on an [`InkPad`] and sees [`InkFeedback`]. The same pair serves
/// write-mode lessons in the Lesson Player.
#[component]
pub fn DictationScreen() -> Element {
    let ui = use_context::<Ui>();
    let l = ui.lang();
    let mut round = use_signal(|| 0u64);
    let mut answer = use_signal(|| None::<String>);
    let seed = use_hook(now_ms);
    let word = use_memo(move || {
        let (lemma, _) = kabar_core::ink::gambar::next_word(&[], &[], seed.wrapping_add(round()));
        kabar_core::ink::gambar::display(lemma)
    });
    let say = move || crate::reader::speak(&word(), "id-ID");
    rsx! {
        div { class: "card dictation",
            p { class: "muted", "{tr(l, T::DictationIntro)}" }
            div { class: "row",
                button { class: "primary", onclick: move |_| say(), "🔊 {tr(l, T::PlayAgain)}" }
                if answer().is_some() {
                    button {
                        onclick: move |_| {
                            answer.set(None);
                            round += 1;
                        },
                        "{tr(l, T::NextRound)}"
                    }
                }
            }
            match answer() {
                None => rsx! {
                    InkPad {
                        key: "{round}",
                        lang: Lang::Id,
                        expected: word(),
                        on_submit: move |a: InkAnswer| answer.set(a.text),
                    }
                    p { class: "muted small", "{tr(l, T::WriteHere)}" }
                },
                Some(written) => rsx! { InkFeedback { written, expected: word() } },
            }
        }
    }
}
