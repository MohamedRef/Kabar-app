//! Kabar Notebook: notebooks of infinite pages, live word cards, and search
//! across everything written (recognized or typed lines, cards, map nodes).

use std::collections::HashMap;

use dioxus::prelude::*;
use kabar_core::ai::FriendlyReason;
use kabar_core::ink::group::{StrokeGroup, group_lines};
use kabar_core::ink::recognize::{Handwriting, handwriting_request, should_replace, transcript};
use kabar_core::ink::search::{HitSource, NotebookIndex};
use kabar_protocol::content::Lang;
use kabar_protocol::ink::{
    DocKind, InkDocument, InkTranscript, ItemKind, Notebook, NotebookItem, TranscriptSource,
};

use super::ink::{InkCanvas, InkTools, Pen, use_ink};
use super::{T, friendly, new_id, tool_labels, tr, vision, word_lang};
use crate::account::Cloud;
use crate::cloud::link::CloudLink;
use crate::reader::{glossary, speak};
use crate::store::now_ms;
use crate::teach::Ai;
use crate::{Db, Ui};

/// Card size, page pixels.
const CARD: [f32; 2] = [180.0, 76.0];

fn ensure_notebook(db: &Db, title: &str) -> Vec<Notebook> {
    let mut all = db.0.borrow().notebooks().unwrap_or_default();
    if all.is_empty() {
        let nb =
            Notebook { id: new_id(), title: title.into(), unit: None, lang: Lang::Id, created_ms: now_ms() };
        if db.0.borrow_mut().save_notebook(&nb).is_ok() {
            all.push(nb);
        }
    }
    all
}

fn new_page(db: &Db, notebook: &Notebook, order: u32, label: &str) -> Option<InkDocument> {
    let doc = InkDocument {
        id: new_id(),
        kind: DocKind::Page,
        notebook: Some(notebook.id.clone()),
        title: format!("{label} {order}"),
        lang: notebook.lang,
        order,
        anchor: None,
        size: None,
        created_ms: now_ms(),
    };
    db.0.borrow_mut().save_ink_document(&doc).ok().map(|_| doc)
}

#[component]
pub fn NotebookScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let cloud = try_use_context::<Cloud>();
    let l = ui.lang();
    let mut notebooks = use_signal(|| ensure_notebook(&db, tr(l, T::MyNotebook)));
    let mut current = use_signal(|| notebooks.peek().first().map(|n| n.id.clone()));
    let mut page = use_signal(|| None::<String>);
    let mut focus = use_signal(|| None::<[f32; 4]>);
    let mut query = use_signal(String::new);
    let mut rev = use_signal(|| 0u64);
    // Local changes bump `rev`; remote ones bump the cloud's revision.
    rev.read();
    if let Some(c) = cloud {
        c.revision.read();
    }

    let notebook = current().and_then(|id| notebooks.read().iter().find(|n| n.id == id).cloned());
    let mut pages =
        notebook.as_ref().map(|n| db.0.borrow().pages(&n.id).unwrap_or_default()).unwrap_or_default();
    // Every notebook opens on a page.
    if let Some(nb) = &notebook
        && pages.is_empty()
        && let Some(first) = new_page(&db, nb, 1, tr(l, T::Page))
    {
        pages.push(first);
    }
    let shown =
        page().filter(|p| pages.iter().any(|d| &d.id == p)).or_else(|| pages.first().map(|d| d.id.clone()));
    let doc = shown.as_ref().and_then(|id| pages.iter().find(|d| &d.id == id).cloned());

    let hits = {
        let q = query();
        if q.trim().is_empty() {
            Vec::new()
        } else {
            let (ts, items) = db.0.borrow().ink_search_rows().unwrap_or_default();
            let mut idx = NotebookIndex::default();
            ts.iter().for_each(|t| idx.add_transcript(t));
            items.iter().for_each(|i| idx.add_item(i));
            idx.search(&q).into_iter().take(20).collect::<Vec<_>>()
        }
    };
    let doc_names: HashMap<String, InkDocument> = {
        let store = db.0.borrow();
        hits.iter()
            .filter_map(|h| store.ink_document(&h.doc).ok().flatten())
            .map(|d| (d.id.clone(), d))
            .collect()
    };

    rsx! {
        div { class: "notebook",
            aside { class: "notebook-side card",
                div { class: "row",
                    select {
                        aria_label: "{tr(l, T::Notebook)}",
                        onchange: move |e| {
                            current.set(Some(e.value()));
                            page.set(None);
                        },
                        for n in notebooks.read().iter() {
                            option { key: "{n.id}", value: "{n.id}", selected: current().as_deref() == Some(n.id.as_str()), "{n.title}" }
                        }
                    }
                    button {
                        class: "quiet",
                        onclick: {
                            let db = db.clone();
                            move |_| {
                                let n = notebooks.read().len() + 1;
                                let nb = Notebook {
                                    id: new_id(),
                                    title: format!("{} {n}", tr(l, T::MyNotebook)),
                                    unit: None,
                                    lang: Lang::Id,
                                    created_ms: now_ms(),
                                };
                                if db.0.borrow_mut().save_notebook(&nb).is_ok() {
                                    current.set(Some(nb.id.clone()));
                                    notebooks.write().push(nb);
                                    page.set(None);
                                }
                            }
                        },
                        "+ {tr(l, T::NewNotebook)}"
                    }
                }
                input {
                    r#type: "search",
                    placeholder: "{tr(l, T::Search)}",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
                if !query().trim().is_empty() {
                    if hits.is_empty() {
                        p { class: "muted small", "{tr(l, T::NoResults)}" }
                    }
                    ul { class: "hits",
                        for (i, h) in hits.iter().enumerate() {
                            li { key: "{i}",
                                button {
                                    class: "hit",
                                    onclick: {
                                        let h = h.clone();
                                        let d = doc_names.get(&h.doc).cloned();
                                        move |_| {
                                            if let Some(d) = &d {
                                                if d.notebook.is_some() {
                                                    current.set(d.notebook.clone());
                                                }
                                                page.set(Some(d.id.clone()));
                                            }
                                            focus.set(Some(h.bbox.to_array()));
                                        }
                                    },
                                    bdi { class: "latin", "{h.text}" }
                                    span { class: "muted small",
                                        " · "
                                        {doc_names.get(&h.doc).map(|d| d.title.clone()).unwrap_or_default()}
                                        if matches!(h.source, HitSource::Handwriting { .. }) { " ✎" }
                                    }
                                }
                            }
                        }
                    }
                }
                h3 { "{tr(l, T::Pages)}" }
                ul { class: "pages",
                    for d in pages.iter() {
                        li { key: "{d.id}",
                            button {
                                aria_pressed: if shown.as_deref() == Some(d.id.as_str()) { "true" } else { "false" },
                                onclick: {
                                    let id = d.id.clone();
                                    move |_| {
                                        page.set(Some(id.clone()));
                                        focus.set(None);
                                    }
                                },
                                "{d.title}"
                            }
                        }
                    }
                }
                if let Some(nb) = notebook.clone() {
                    button {
                        onclick: {
                            let db = db.clone();
                            let order = pages.iter().map(|d| d.order).max().unwrap_or(0) + 1;
                            move |_| {
                                if let Some(d) = new_page(&db, &nb, order, tr(l, T::Page)) {
                                    page.set(Some(d.id));
                                    rev += 1;
                                }
                            }
                        },
                        "+ {tr(l, T::NewPage)}"
                    }
                }
            }
            if let Some(d) = doc {
                PageView { key: "{d.id}-{focus():?}", doc: d, focus: focus() }
            }
        }
    }
}

/// One notebook page: tools, the canvas with its cards, and its lines.
#[component]
fn PageView(doc: InkDocument, focus: Option<[f32; 4]>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let link = use_context::<CloudLink>();
    let l = ui.lang();
    let handle = use_ink();
    let pen = use_signal(Pen::default);
    let mut items = use_signal(|| db.0.borrow().items(&doc.id).unwrap_or_default());
    let mut transcripts = use_signal(|| db.0.borrow().transcripts(&doc.id).unwrap_or_default());
    let mut word = use_signal(String::new);
    let mut problem = use_signal(|| None::<T>);
    let mut reading = use_signal(|| false);
    let revision = handle.revision;
    let groups = {
        revision.read();
        group_lines(&db.0.borrow().strokes(&doc.id).unwrap_or_default())
    };
    let lang = doc.lang;

    let add_card = {
        let db = db.clone();
        let doc_id = doc.id.clone();
        let view = handle.view;
        move |_| {
            let w = word().trim().to_string();
            if w.is_empty() {
                return;
            }
            let [x, y, vw, vh] = view();
            let n = items.read().len() as f32;
            let item = NotebookItem {
                id: new_id(),
                doc: doc_id.clone(),
                kind: ItemKind::WordCard { lang, word: w },
                x: x + vw / 2.0 - CARD[0] / 2.0 + (n % 5.0) * 16.0,
                y: y + vh / 3.0 + (n % 5.0) * 16.0,
                w: CARD[0],
                h: CARD[1],
            };
            if db.0.borrow_mut().save_item(&item).is_ok() {
                items.write().push(item);
                word.set(String::new());
            }
        }
    };

    let save_line = {
        let db = db.clone();
        let doc_id = doc.id.clone();
        move |(g, text, source): (StrokeGroup, String, TranscriptSource)| {
            let t = transcript(&doc_id, &g, &text, lang, source, None);
            if db.0.borrow_mut().save_transcript(&t).is_ok() {
                transcripts.set(db.0.borrow().transcripts(&doc_id).unwrap_or_default());
            }
        }
    };

    let read_all = {
        let handle = handle.clone();
        let groups = groups.clone();
        let save_line = save_line.clone();
        move |_| {
            let handle = handle.clone();
            let ai = ai.clone();
            let link = link.clone();
            let groups = groups.clone();
            let mut save_line = save_line.clone();
            spawn(async move {
                reading.set(true);
                problem.set(None);
                for g in groups {
                    let old = transcripts.read().iter().find(|t| t.group == g.id).cloned();
                    let changed = old.as_ref().is_some_and(|t| t.strokes != g.strokes);
                    if old.is_some() && !changed {
                        continue;
                    }
                    if !should_replace(old.as_ref(), TranscriptSource::Ai, changed) {
                        continue;
                    }
                    let result = async {
                        let png =
                            handle.snapshot(Some(g.strokes.clone())).await.ok_or(FriendlyReason::TryAgain)?;
                        let sha = vision::upload(&link, &png).await?;
                        ai.0.run::<Handwriting>(&handwriting_request(lang, None, &sha))
                            .await
                            .map_err(|e| e.friendly())
                    }
                    .await;
                    match result {
                        Ok(a) => save_line((g, a.value.text, TranscriptSource::Ai)),
                        Err(f) => {
                            problem.set(Some(friendly(f)));
                            break;
                        }
                    }
                }
                reading.set(false);
            });
        }
    };

    let remove_item = {
        let db = db.clone();
        move |id: String| {
            if db.0.borrow_mut().delete_item(&id).is_ok() {
                items.write().retain(|i| i.id != id);
            }
        }
    };

    rsx! {
        div { class: "page-view",
            div { class: "row page-tools",
                InkTools { pen, labels: tool_labels(l) }
                form {
                    class: "row add-card",
                    onsubmit: add_card,
                    input {
                        class: "latin",
                        dir: "ltr",
                        r#type: "text",
                        autocapitalize: "off",
                        placeholder: "{tr(l, T::CardPlaceholder)}",
                        aria_label: "{tr(l, T::AddCard)}",
                        value: "{word}",
                        oninput: move |e| word.set(e.value()),
                    }
                    button { r#type: "submit", "+ {tr(l, T::AddCard)}" }
                }
            }
            InkCanvas { handle: handle.clone(), doc: doc.id.clone(), pen, focus, class: "ink-page",
                ItemsLayer { items: items(), on_remove: remove_item }
            }
            div { class: "card lines",
                div { class: "row",
                    h3 { "{tr(l, T::Lines)}" }
                    button { class: "quiet", disabled: reading() || groups.is_empty(), onclick: read_all,
                        if reading() { "{tr(l, T::Reading)}" } else { "{tr(l, T::ReadWriting)}" }
                    }
                }
                if let Some(p) = problem() {
                    p { class: "muted small", role: "status", "{tr(l, p)}" }
                }
                if groups.is_empty() {
                    p { class: "muted small", "{tr(l, T::NoLines)}" }
                }
                for g in groups.iter() {
                    LineRow {
                        key: "{g.id}",
                        group: g.clone(),
                        saved: transcripts.read().iter().find(|t| t.group == g.id).cloned(),
                        lang,
                        on_focus: {
                            let handle = handle.clone();
                            move |r: [f32; 4]| handle.focus(r)
                        },
                        on_save: {
                            let mut save_line = save_line.clone();
                            move |(g, text): (StrokeGroup, String)| save_line((g, text, TranscriptSource::Typed))
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn LineRow(
    group: StrokeGroup,
    saved: Option<InkTranscript>,
    lang: Lang,
    on_focus: EventHandler<[f32; 4]>,
    on_save: EventHandler<(StrokeGroup, String)>,
) -> Element {
    let ui = use_context::<Ui>();
    let l = ui.lang();
    let mut text = use_signal(|| saved.as_ref().map(|t| t.text.clone()).unwrap_or_default());
    let source = saved.as_ref().map(|t| t.source);
    let bbox = group.bbox.to_array();
    rsx! {
        form {
            class: "row line",
            onsubmit: move |_| on_save.call((group.clone(), text())),
            button { r#type: "button", class: "quiet", title: "↗", onclick: move |_| on_focus.call(bbox), "✎" }
            input {
                class: "latin",
                dir: "ltr",
                lang: lang.code(),
                r#type: "text",
                placeholder: "{tr(l, T::TypeWhatYouWrote)}",
                value: "{text}",
                oninput: move |e| text.set(e.value()),
            }
            if source == Some(TranscriptSource::Ai) {
                span { class: "muted small", "AI" }
            }
            button { r#type: "submit", "{tr(l, T::Save)}" }
        }
    }
}

/// Cards, notes and mind-map nodes on a page, in page pixels.
#[component]
pub fn ItemsLayer(items: Vec<NotebookItem>, on_remove: Option<EventHandler<String>>) -> Element {
    let centers: HashMap<&str, (f32, f32)> =
        items.iter().map(|i| (i.id.as_str(), (i.x + i.w / 2.0, i.y + i.h / 2.0))).collect();
    let lines: Vec<(String, f32, f32, f32, f32)> = items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Arrow { from, to, x2, y2 } => {
                let a = from.as_deref().and_then(|f| centers.get(f)).copied()?;
                let b = to.as_deref().and_then(|t| centers.get(t)).copied().unwrap_or((*x2, *y2));
                Some((i.id.clone(), a.0, a.1, b.0, b.1))
            }
            _ => None,
        })
        .collect();
    rsx! {
        svg { class: "ink-arrows", width: "1", height: "1",
            for (id, x1, y1, x2, y2) in lines {
                line { key: "{id}", x1: "{x1}", y1: "{y1}", x2: "{x2}", y2: "{y2}" }
            }
        }
        for item in items.into_iter().filter(|i| !matches!(i.kind, ItemKind::Arrow { .. })) {
            ItemView { key: "{item.id}", item, on_remove }
        }
    }
}

#[component]
fn ItemView(item: NotebookItem, on_remove: Option<EventHandler<String>>) -> Element {
    let ui = use_context::<Ui>();
    let l = ui.lang();
    let style = format!(
        "inset-inline-start: {}px; inset-block-start: {}px; inline-size: {}px; min-block-size: {}px;",
        item.x, item.y, item.w, item.h
    );
    let remove = on_remove.map(|h| {
        let id = item.id.clone();
        rsx! {
            button { class: "item-remove", aria_label: "{tr(l, T::Remove)}", onclick: move |_| h.call(id.clone()), "×" }
        }
    });
    match item.kind {
        ItemKind::WordCard { lang, word } => {
            let info = glossary().lookup(Some(word_lang(lang)), &[word.to_lowercase().as_str()]);
            let gloss = info.map(|i| i.gloss.get(l).to_string());
            let tag = if lang == Lang::Jv { "jv-ID" } else { "id-ID" };
            rsx! {
                div { class: "ink-item word-card", style: "{style}",
                    div { class: "row",
                        bdi { class: "latin word", lang: lang.code(), "{word}" }
                        button {
                            class: "quiet",
                            aria_label: "{tr(l, T::Listen)}",
                            onclick: {
                                let word = word.clone();
                                move |_| speak(&word, tag)
                            },
                            "🔊"
                        }
                        {remove}
                    }
                    if let Some(g) = gloss {
                        div { class: "gloss", dir: "auto", "{g}" }
                    }
                    if info.is_some_and(|i| !i.verified) && lang == Lang::Jv {
                        div { class: "muted small", "{tr(l, T::Unverified)}" }
                    }
                }
            }
        }
        ItemKind::Text { text } => rsx! {
            div { class: "ink-item note", style: "{style}", dir: "auto", "{text}" {remove} }
        },
        ItemKind::MapNode { label, gloss, depth, .. } => rsx! {
            div { class: "ink-item map-node depth-{depth}", style: "{style}",
                bdi { class: "latin", "{label}" }
                if let Some(g) = gloss {
                    div { class: "gloss small", "{g}" }
                }
            }
        },
        ItemKind::Arrow { .. } => rsx! {},
    }
}
