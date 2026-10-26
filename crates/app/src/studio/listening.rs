//! Listening Companion: a narrow pane for split screen next to a video.
//! The learner writes a word the way it sounded; Kabar suggests real words
//! (sound-folded edit distance over the content pack), each with its gloss,
//! audio, and one tap to Review or Teach Kabar.

use dioxus::prelude::*;
use kabar_core::ai::FriendlyReason;
use kabar_core::ink::recognize::{Handwriting, handwriting_request};
use kabar_core::ink::spell::suggest;
use kabar_protocol::content::Lang;
use kabar_protocol::ink::{DocKind, InkDocument};

use super::ink::{InkCanvas, Pen, use_ink};
use super::{T, friendly, new_id, tr, vision, word_lang};
use crate::cloud::link::CloudLink;
use crate::reader::{glossary, lexicon, speak};
use crate::store::now_ms;
use crate::teach::{Ai, TeachInbox, TeachSeed};
use crate::{Db, Screen, Ui};

const SCRIBBLE: [f32; 2] = [600.0, 170.0];

#[component]
pub fn ListeningScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let link = use_context::<CloudLink>();
    let inbox = use_context::<TeachInbox>();
    let l = ui.lang();
    let handle = use_ink();
    let pen = use_signal(Pen::default);
    let mut lang = use_signal(|| Lang::Id);
    let mut heard = use_signal(String::new);
    let mut saved = use_signal(Vec::<String>::new);
    let mut problem = use_signal(|| None::<T>);
    let mut reading = use_signal(|| false);
    let mut sheet = use_signal(|| None::<String>);

    let new_sheet = {
        let db = db.clone();
        move || {
            let doc = InkDocument {
                id: new_id(),
                kind: DocKind::Listening,
                notebook: None,
                title: String::new(),
                lang: lang(),
                order: 0,
                anchor: None,
                size: Some(SCRIBBLE),
                created_ms: now_ms(),
            };
            if db.0.borrow_mut().save_ink_document(&doc).is_ok() {
                sheet.set(Some(doc.id));
            }
        }
    };
    {
        let new_sheet = new_sheet.clone();
        use_hook(new_sheet);
    }

    let read = {
        let handle = handle.clone();
        move |_| {
            let handle = handle.clone();
            let ai = ai.clone();
            let link = link.clone();
            spawn(async move {
                reading.set(true);
                problem.set(None);
                let result = async {
                    let png = handle.snapshot(None).await.ok_or(FriendlyReason::TryAgain)?;
                    let sha = vision::upload(&link, &png).await?;
                    ai.0.run::<Handwriting>(&handwriting_request(lang(), None, &sha))
                        .await
                        .map_err(|e| e.friendly())
                }
                .await;
                reading.set(false);
                match result {
                    Ok(a) => heard.set(a.value.text.trim().to_string()),
                    Err(f) => problem.set(Some(friendly(f))),
                }
            });
        }
    };

    let suggestions = suggest(&heard(), lang(), lexicon(), 6);
    let tag = if lang() == Lang::Jv { "jv-ID" } else { "id-ID" };

    rsx! {
        div { class: "listening",
            p { class: "muted small", "{tr(l, T::ListenIntro)}" }
            div { class: "row",
                for (code, value) in [("id", Lang::Id), ("jv", Lang::Jv)] {
                    button {
                        key: "{code}",
                        aria_pressed: if lang() == value { "true" } else { "false" },
                        onclick: move |_| lang.set(value),
                        bdi { class: "latin", "{code}" }
                    }
                }
            }
            if let Some(doc) = sheet() {
                div { aria_label: "{tr(l, T::ScribbleHere)}", title: "{tr(l, T::ScribbleHere)}",
                    InkCanvas { key: "{doc}", handle: handle.clone(), doc, size: Some(SCRIBBLE), pen, class: "ink-sheet scribble" }
                }
            }
            div { class: "row",
                button { class: "quiet", disabled: reading(), onclick: read,
                    if reading() { "{tr(l, T::Reading)}" } else { "{tr(l, T::ReadWriting)}" }
                }
                button {
                    class: "quiet",
                    onclick: {
                        let mut new_sheet = new_sheet.clone();
                        move |_| {
                            new_sheet();
                            heard.set(String::new());
                        }
                    },
                    "{tr(l, T::Clear)}"
                }
            }
            if let Some(p) = problem() {
                p { class: "muted small", role: "status", "{tr(l, p)}" }
            }
            input {
                class: "latin heard",
                dir: "ltr",
                lang: lang().code(),
                r#type: "text",
                autocapitalize: "off",
                spellcheck: "false",
                placeholder: "{tr(l, T::HeardIt)}",
                aria_label: "{tr(l, T::HeardIt)}",
                value: "{heard}",
                oninput: move |e| heard.set(e.value()),
            }
            if !heard().trim().is_empty() && suggestions.is_empty() {
                p { class: "muted small", "{tr(l, T::NoSuggestions)}" }
            }
            ul { class: "suggestions",
                for s in suggestions {
                    li { key: "{s.word}",
                        div { class: "row",
                            bdi { class: "latin word", lang: s.lang.code(), "{s.word}" }
                            button {
                                class: "quiet",
                                aria_label: "{tr(l, T::Listen)}",
                                onclick: {
                                    let w = s.word.clone();
                                    move |_| speak(&w, tag)
                                },
                                "🔊"
                            }
                        }
                        if let Some(info) = glossary().lookup(Some(word_lang(s.lang)), &[s.word.as_str()]) {
                            div { class: "gloss", dir: "auto", {info.gloss.get(l).to_string()} }
                        }
                        div { class: "row",
                            if saved.read().contains(&s.word) {
                                span { class: "ok small", "{tr(l, T::Saved)}" }
                            } else {
                                button {
                                    onclick: {
                                        let db = db.clone();
                                        let w = s.word.clone();
                                        let key = format!("{}:{}", s.lang.code(), s.word);
                                        move |_| {
                                            if db.0.borrow_mut().save_word(&key, &w, "", None).is_ok() {
                                                saved.write().push(w.clone());
                                            }
                                        }
                                    },
                                    "{tr(l, T::AddReview)}"
                                }
                            }
                        }
                    }
                }
            }
            if !heard().trim().is_empty() {
                button {
                    onclick: move |_| {
                        let (mut seed, mut screen) = (inbox.0, ui.screen);
                        seed.set(Some(TeachSeed { lang: lang(), word: heard().trim().to_string(), context: None }));
                        screen.set(Screen::Teach);
                    },
                    "{tr(l, T::TeachIt)}"
                }
            }
        }
    }
}
