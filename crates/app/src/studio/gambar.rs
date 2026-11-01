//! Gambar! Kabar names a word, the learner sketches it in a minute, and the
//! `guess_sketch` task tries to guess it from the drawing alone.

use dioxus::prelude::*;
use kabar_core::ai::FriendlyReason;
use kabar_core::ink::gambar::{DRAWABLE, ROUND_SECONDS, SketchGuess, Verdict, display, judge, next_word};
use kabar_core::ink::recognize::sketch_request;
use kabar_protocol::coach::{ActivityKind, Skill};
use kabar_protocol::content::Lang;
use kabar_protocol::ink::{DocKind, InkDocument, InkTool};
use kabar_protocol::reader::WordStatus;

use super::ink::{InkCanvas, InkTools, Pen, PenTool, use_ink};
use super::{T, friendly, new_id, tool_labels, tr, vision, word_lang};
use crate::cloud::link::CloudLink;
use crate::coach::Coach;
use crate::reader::glossary;
use crate::store::now_ms;
use crate::teach::Ai;
use crate::{Db, Ui};

const SHEET: [f32; 2] = [800.0, 600.0];

type Word = &'static (&'static str, &'static str);

#[derive(Clone, PartialEq)]
enum Phase {
    Ready,
    Drawing {
        word: Word,
        left: u32,
    },
    Guessing {
        word: Word,
    },
    Result {
        word: Word,
        verdict: Verdict,
        guess: Option<SketchGuess>,
    },
    /// No vision engine: the learner judges their own sketch.
    SelfCheck {
        word: Word,
        why: T,
    },
}

/// Drawable words the learner is learning or knows (Reader statuses).
fn known_drawables(db: &Db) -> Vec<&'static str> {
    let keys: Vec<String> = DRAWABLE.iter().map(|(w, _)| format!("id:{w}")).collect();
    let statuses = db.0.borrow().word_statuses(keys.iter().map(String::as_str)).unwrap_or_default();
    DRAWABLE
        .iter()
        .filter(|(w, _)| {
            matches!(statuses.get(&format!("id:{w}")), Some(WordStatus::Learning | WordStatus::Known))
        })
        .map(|(w, _)| *w)
        .collect()
}

#[component]
pub fn GambarScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let link = use_context::<CloudLink>();
    let coach = try_use_context::<Coach>();
    let l = ui.lang();
    let handle = use_ink();
    let pen = use_signal(|| Pen { tool: PenTool::Ink(InkTool::Brush), color: "ink", width: 3.0 });
    let mut phase = use_signal(|| Phase::Ready);
    let mut recent = use_signal(Vec::<&'static str>::new);
    let mut score = use_signal(|| (0u32, 0u32));
    // The current round's sheet; stays on screen while Kabar looks at it.
    let mut sheet = use_signal(|| None::<String>);

    let start = {
        let db = db.clone();
        move |_| {
            let known = known_drawables(&db);
            let word = next_word(&known, &recent.read(), now_ms());
            let doc = InkDocument {
                id: new_id(),
                kind: DocKind::Gambar,
                notebook: None,
                title: word.0.to_string(),
                lang: Lang::Id,
                order: 0,
                anchor: None,
                size: Some(SHEET),
                created_ms: now_ms(),
            };
            if db.0.borrow_mut().save_ink_document(&doc).is_ok() {
                recent.write().push(word.0);
                let len = recent.read().len();
                if len > 10 {
                    recent.write().drain(..len - 10);
                }
                sheet.set(Some(doc.id));
                phase.set(Phase::Drawing { word, left: ROUND_SECONDS });
            }
        }
    };

    let record = {
        let db = db.clone();
        move |word: Word, got: bool| {
            score.with_mut(|(g, n)| {
                *g += u32::from(got);
                *n += 1;
            });
            if let Some(c) = coach {
                let kind = ActivityKind::Practice {
                    skill: Skill::Vocabulary,
                    topic: format!("gambar:{}", word.0),
                    correct: u32::from(got),
                    total: 1,
                };
                c.record(&db, kind, ROUND_SECONDS);
            }
        }
    };

    let finish = {
        let handle = handle.clone();
        let record = record.clone();
        move || {
            let Phase::Drawing { word, .. } = phase() else { return };
            phase.set(Phase::Guessing { word });
            let handle = handle.clone();
            let ai = ai.clone();
            let link = link.clone();
            let mut record = record.clone();
            spawn(async move {
                let result = async {
                    let png = handle.snapshot(None).await.ok_or(FriendlyReason::TryAgain)?;
                    let sha = vision::upload(&link, &png).await?;
                    ai.0.run::<SketchGuess>(&sketch_request(&sha)).await.map_err(|e| e.friendly())
                }
                .await;
                match result {
                    Ok(answer) => {
                        let verdict = judge(word.0, &answer.value);
                        record(word, verdict == Verdict::Got);
                        phase.set(Phase::Result { word, verdict, guess: Some(answer.value) });
                    }
                    Err(f) => phase.set(Phase::SelfCheck { word, why: friendly(f) }),
                }
            });
        }
    };

    // One tick a second while drawing.
    {
        let finish = finish.clone();
        use_future(move || {
            let mut finish = finish.clone();
            async move {
                let mut eval = document::eval(
                    r#"setInterval(() => dioxus.send(1), 1000); await new Promise(() => {});"#,
                );
                while eval.recv::<u8>().await.is_ok() {
                    let next = match phase.peek().clone() {
                        Phase::Drawing { word, left } => Some((word, left.saturating_sub(1))),
                        _ => None,
                    };
                    if let Some((word, left)) = next {
                        if left == 0 {
                            finish();
                        } else {
                            phase.set(Phase::Drawing { word, left });
                        }
                    }
                }
            }
        });
    }

    let gloss = move |word: Word| {
        glossary()
            .lookup(Some(word_lang(Lang::Id)), &[word.0])
            .map(|i| i.gloss.get(l).to_string())
            .unwrap_or_else(|| word.1.to_string())
    };
    let (got, rounds) = score();

    rsx! {
        div { class: "gambar",
            crate::ui::DivCard { class: "gambar-head",
                match phase() {
                    Phase::Ready => rsx! {
                        h2 { "Gambar!" }
                        button { class: "primary", onclick: start.clone(), "{tr(l, T::Start)}" }
                    },
                    Phase::Drawing { word, left, .. } => rsx! {
                        div { class: "row",
                            span { "{tr(l, T::DrawThis)} " }
                            bdi { class: "latin target", lang: "id", "{display(word.0)}" }
                            span { class: "muted", " · {gloss(word)}" }
                        }
                        div { class: "row",
                            span { class: "timer", role: "timer", "{ui.num(left)} {tr(l, T::SecondsLeft)}" }
                            button {
                                class: "primary",
                                onclick: {
                                    let mut finish = finish.clone();
                                    move |_| finish()
                                },
                                "{tr(l, T::Done)}"
                            }
                        }
                    },
                    Phase::Guessing { .. } => rsx! { p { role: "status", "{tr(l, T::Guessing)}" } },
                    Phase::Result { word, verdict, guess } => rsx! {
                        p { class: "verdict",
                            {tr(l, match verdict {
                                Verdict::Got => T::Got,
                                Verdict::Close => T::Close,
                                Verdict::Missed => T::Missed,
                                Verdict::Unclear => T::Unclear,
                            })}
                        }
                        if let Some(g) = guess {
                            p {
                                "{tr(l, T::KabarSaw)} "
                                bdi { class: "latin", lang: "id", {display(&g.guess_id)} }
                            }
                        }
                        p {
                            bdi { class: "latin target", lang: "id", "{display(word.0)}" }
                            span { class: "muted", " · {gloss(word)}" }
                        }
                        button { class: "primary", onclick: start.clone(), "{tr(l, T::NextRound)}" }
                    },
                    Phase::SelfCheck { word, why } => rsx! {
                        p { class: "muted small", "{tr(l, why)}" }
                        p { "{tr(l, T::SelfCheck)}" }
                        p {
                            bdi { class: "latin target", lang: "id", "{display(word.0)}" }
                            span { class: "muted", " · {gloss(word)}" }
                        }
                        div { class: "row",
                            for (label, got) in [(T::Yes, true), (T::NotReally, false)] {
                                button {
                                    key: "{label:?}",
                                    onclick: {
                                        let mut record = record.clone();
                                        let mut start = start.clone();
                                        move |e| {
                                            record(word, got);
                                            start(e);
                                        }
                                    },
                                    "{tr(l, label)}"
                                }
                            }
                        }
                    },
                }
                if rounds > 0 {
                    p { class: "muted small", "{ui.num(got)} / {ui.num(rounds)} {tr(l, T::Score)}" }
                }
            }
            if matches!(phase(), Phase::Drawing { .. }) {
                InkTools { pen, labels: tool_labels(l), brush: true }
            }
            if let Some(doc) = sheet() {
                InkCanvas { key: "{doc}", handle: handle.clone(), doc, size: Some(SHEET), pen, class: "ink-sheet" }
            }
        }
    }
}
