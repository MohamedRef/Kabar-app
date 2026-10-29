//! Mind-map generator: a topic goes to the `mind_map` task, the validated
//! map is laid out radially as notebook items, and the learner extends it
//! with the pen on the same canvas.

use dioxus::prelude::*;
use kabar_core::ink::mindmap::{MindMap, layout, request};
use kabar_protocol::content::Lang;
use kabar_protocol::ink::{DocKind, InkDocument};

use super::ink::{InkCanvas, InkTools, Pen, use_ink};
use super::notebook::ItemsLayer;
use super::{T, ai_problem, new_id, tool_labels, tr};
use crate::store::now_ms;
use crate::teach::Ai;
use crate::{Db, Ui};

/// Where the map's center goes on a fresh canvas, page pixels.
const CENTER: (f32, f32) = (620.0, 520.0);

#[component]
pub fn MindMapScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let l = ui.lang();
    let mut topic = use_signal(String::new);
    let mut lang = use_signal(|| Lang::Id);
    let mut busy = use_signal(|| false);
    let mut problem = use_signal(|| None::<T>);
    let mut maps = use_signal(|| db.0.borrow().documents(DocKind::MindMap).unwrap_or_default());
    let mut open = use_signal(|| maps.peek().last().map(|d| d.id.clone()));
    let mut unverified = use_signal(Vec::<String>::new);

    let make = {
        let db = db.clone();
        move |_| {
            let t = topic().trim().to_string();
            if t.is_empty() {
                return;
            }
            let db = db.clone();
            let ai = ai.clone();
            spawn(async move {
                busy.set(true);
                problem.set(None);
                let l2 = lang();
                let result = ai.0.run::<MindMap>(&request(&t, "bipa2", &[], l2)).await;
                busy.set(false);
                let answer = match result {
                    Ok(a) => a,
                    Err(e) => {
                        problem.set(Some(ai_problem(&e)));
                        return;
                    }
                };
                let doc = InkDocument {
                    id: new_id(),
                    kind: DocKind::MindMap,
                    notebook: None,
                    title: answer.value.center.clone(),
                    lang: l2,
                    order: maps.read().len() as u32,
                    anchor: None,
                    size: None,
                    created_ms: now_ms(),
                };
                let items = layout(&answer.value, &doc.id, CENTER.0, CENTER.1, new_id);
                let saved = {
                    let mut store = db.0.borrow_mut();
                    store.save_ink_document(&doc).is_ok() && items.iter().all(|i| store.save_item(i).is_ok())
                };
                if saved {
                    if answer.needs_review {
                        unverified.write().push(doc.id.clone());
                    }
                    open.set(Some(doc.id.clone()));
                    maps.write().push(doc);
                    topic.set(String::new());
                }
            });
        }
    };

    let current = open().and_then(|id| maps.read().iter().find(|d| d.id == id).cloned());

    rsx! {
        div { class: "mindmap",
            // On wide windows this column sits beside the map.
            section { class: "card map-side",
                h2 { "{tr(l, T::MindMap)}" }
                p { class: "muted", "{tr(l, T::MapIntro)}" }
                form { class: "map-form", onsubmit: make,
                    input {
                        class: "latin",
                        dir: "ltr",
                        r#type: "text",
                        placeholder: "{tr(l, T::Topic)}",
                        aria_label: "{tr(l, T::Topic)}",
                        value: "{topic}",
                        oninput: move |e| topic.set(e.value()),
                    }
                    div { class: "map-langs", role: "group",
                        for (code, value) in [("id", Lang::Id), ("jv", Lang::Jv)] {
                            button {
                                key: "{code}",
                                r#type: "button",
                                aria_pressed: if lang() == value { "true" } else { "false" },
                                onclick: move |_| lang.set(value),
                                bdi { class: "latin", "{code}" }
                            }
                        }
                    }
                    button { class: "primary", r#type: "submit", disabled: busy() || topic().trim().is_empty(),
                        if busy() { "{tr(l, T::Making)}" } else { "{tr(l, T::Make)}" }
                    }
                }
                if let Some(p) = problem() {
                    p { class: "map-problem", role: "status", "{tr(l, p)}" }
                }
                if !maps.read().is_empty() {
                    h3 { class: "map-list-title", "{tr(l, T::Maps)}" }
                    div { class: "map-list",
                        for d in maps.read().iter() {
                            button {
                                key: "{d.id}",
                                aria_pressed: if open().as_deref() == Some(d.id.as_str()) { "true" } else { "false" },
                                onclick: {
                                    let id = d.id.clone();
                                    move |_| open.set(Some(id.clone()))
                                },
                                bdi { class: "latin", "{d.title}" }
                            }
                        }
                    }
                }
            }
            div { class: "map-main",
                if let Some(doc) = current {
                    MapView {
                        key: "{doc.id}",
                        unverified: doc.lang == Lang::Jv || unverified.read().contains(&doc.id),
                        doc,
                    }
                }
            }
        }
    }
}

#[component]
fn MapView(doc: InkDocument, unverified: bool) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let l = ui.lang();
    let handle = use_ink();
    let pen = use_signal(Pen::default);
    let items = use_hook(|| db.0.borrow().items(&doc.id).unwrap_or_default());
    // Open on the whole map.
    let focus = items
        .iter()
        .filter(|i| i.w > 0.0)
        .map(|i| [i.x, i.y, i.x + i.w, i.y + i.h])
        .reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
        .map(|[x0, y0, x1, y1]| [x0, y0, x1 - x0, y1 - y0]);
    rsx! {
        div { class: "row page-tools",
            InkTools { pen, labels: tool_labels(l) }
            if unverified {
                span { class: "badge", "{tr(l, T::Unverified)}" }
            }
        }
        InkCanvas { handle, doc: doc.id.clone(), pen, focus, class: "ink-page",
            ItemsLayer { items, on_remove: None }
        }
    }
}
