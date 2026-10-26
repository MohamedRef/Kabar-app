//! Reader & Screenshot Decoder (SPEC §11): paste, share, drop or open a
//! screenshot; every word is colored by status and explained from local data.

mod card;
mod content;
pub mod doc;
pub mod explain;
pub mod glossary;
mod web;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::OnceLock;

use dioxus::html::{FileData, HasFileData};
use dioxus::prelude::*;
use kabar_core::learn::PersonalDictionary;
use kabar_core::locale::UiLanguage;
use kabar_core::reader::screenshot::{ScreenDoc, Side, structure};
use kabar_protocol::reader::{SharedInput, WordStatus};
use serde::Deserialize;

use self::content::LocalGlossary;
use self::doc::{Lexicon, ReadDoc, SegKind};
use self::glossary::{Glossary, Shift};
use crate::account::Cloud;
use crate::i18n::Msg;
use crate::store::{Highlight, SavedText, TextKind};
use crate::{Db, Ui, platform};

const READER_JS: &str = include_str!("../../assets/reader.js");

/// The content pack merged over the seed; parsed once (~2.5k entries).
pub fn glossary() -> &'static dyn Glossary {
    static LOCAL: OnceLock<LocalGlossary> = OnceLock::new();
    LOCAL.get_or_init(LocalGlossary::load)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Inline,
    Split,
}

/// A floating peek card anchored to a word.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peek {
    pub i: usize,
    pub start: f64,
    pub below: bool,
    pub y: f64,
}

/// What the side panel / bottom sheet shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    Word(usize),
    Phrase {
        tokens: Vec<usize>,
        save_first: bool,
    },
    /// "Explain this sentence": the sentence around this token.
    Explain(usize),
}

/// The open text and everything the Reader shows about it.
#[derive(Clone, PartialEq)]
pub struct Open {
    pub text: SavedText,
    pub doc: Rc<ReadDoc>,
}

/// Reader state shared with the word card, through context.
#[derive(Clone, Copy)]
pub struct ReaderState {
    pub open: Signal<Option<Open>>,
    pub statuses: Signal<HashMap<String, WordStatus>>,
    pub highlights: Signal<Vec<Highlight>>,
    pub voice: Signal<UiLanguage>,
    pub mode: Signal<Mode>,
    pub peek: Signal<Option<Peek>>,
    pub panel: Signal<Option<Panel>>,
    pub toast: Signal<Option<Msg>>,
}

impl ReaderState {
    pub fn status(&self, key: &str) -> WordStatus {
        self.statuses.read().get(key).copied().unwrap_or_default()
    }

    pub fn set_status(mut self, db: &Db, key: &str, status: WordStatus) {
        if let Err(e) = db.0.borrow_mut().set_word_status(key, status) {
            eprintln!("kabar: saving word status failed: {e}");
            return;
        }
        self.statuses.write().insert(key.to_string(), status);
    }

    pub fn save_phrase(mut self, db: &Db, tokens: &[usize]) {
        let Some(open) = self.open.read().clone() else { return };
        let Some((start, end)) = open.doc.span_of(tokens) else { return };
        let phrase = open.doc.text(start, end).to_string();
        let mut store = db.0.borrow_mut();
        match store.add_highlight(&open.text.id, start, end, &phrase) {
            Ok(h) => {
                // A saved phrase is also a review item, met in its sentence.
                let sentence = open.doc.sentence_of(tokens[0]).to_string();
                let _ = store.save_word(
                    &format!("phrase:{}", phrase.to_lowercase()),
                    &phrase,
                    &sentence,
                    Some(&open.text.id),
                );
                self.highlights.write().push(h);
                self.toast.set(Some(Msg::PhraseSaved));
            }
            Err(e) => eprintln!("kabar: saving highlight failed: {e}"),
        }
    }

    pub fn highlight_at(&self, i: usize) -> Option<Highlight> {
        let open = self.open.read();
        let t = open.as_ref()?.doc.tokens.get(i)?.clone();
        self.highlights.read().iter().find(|h| h.start <= t.start && t.end <= h.end).cloned()
    }

    /// Made once for the whole app, so the open text survives a trip to
    /// another screen (Teach Kabar) and back.
    pub fn new(voice: UiLanguage) -> Self {
        Self {
            open: Signal::new(None),
            statuses: Signal::new(HashMap::new()),
            highlights: Signal::new(Vec::new()),
            voice: Signal::new(voice),
            mode: Signal::new(Mode::Inline),
            peek: Signal::new(None),
            panel: Signal::new(None),
            toast: Signal::new(None),
        }
    }

    fn load(mut self, db: &Db, text: SavedText) {
        let store = db.0.borrow();
        let taught = PersonalDictionary::new(store.taught_entries().unwrap_or_default());
        let lex = Lexicon { glossary: glossary(), taught: &taught };
        let doc = match text.layout.as_deref().and_then(|l| serde_json::from_str::<ScreenDoc>(l).ok()) {
            Some(screen) => ReadDoc::from_screen(&screen, lex),
            None => ReadDoc::from_text(&text.body, lex),
        };
        self.statuses.set(store.word_statuses(doc.keys()).unwrap_or_default());
        self.highlights.set(store.highlights(&text.id).unwrap_or_default());
        drop(store);
        self.peek.set(None);
        self.panel.set(None);
        self.open.set(Some(Open { text, doc: Rc::new(doc) }));
    }

    /// Statuses and highlights changed on another device: recolor in place.
    fn reload_marks(mut self, db: &Db) {
        let Some(open) = self.open.peek().clone() else { return };
        let store = db.0.borrow();
        self.statuses.set(store.word_statuses(open.doc.keys()).unwrap_or_default());
        self.highlights.set(store.highlights(&open.text.id).unwrap_or_default());
    }

    /// Re-reads the open text, picking up words taught since it was opened;
    /// the word card stays open.
    fn refresh(mut self, db: &Db) {
        let Some(open) = self.open.peek().clone() else { return };
        let panel = self.panel.peek().clone();
        self.load(db, open.text);
        self.panel.set(panel);
    }

    fn close(mut self) {
        self.open.set(None);
        self.peek.set(None);
        self.panel.set(None);
    }
}

/// Input that arrived from outside (Android share sheet), waiting for the Reader.
#[derive(Clone, Copy)]
pub struct Inbox(pub Signal<Option<SharedInput>>);

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum FromJs {
    Peek { i: usize, start: f64, below: bool, y: f64 },
    Unpeek,
    Open { i: usize },
    Phrase { tokens: Vec<usize> },
    Highlight { tokens: Vec<usize> },
    Menu { tokens: Vec<usize> },
    Escape,
}

#[component]
pub fn Reader() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let inbox = use_context::<Inbox>();
    let st = use_context::<ReaderState>();
    use_hook({
        let db = db.clone();
        move || st.refresh(&db)
    });
    // Synced from another device while the Reader is open.
    let cloud = use_context::<Cloud>();
    use_effect({
        let db = db.clone();
        move || {
            if *cloud.revision.read() > 0 {
                st.reload_marks(&db);
            }
        }
    });
    let busy = use_signal(|| None::<Msg>);
    let recent = use_signal(|| db.0.borrow().recent_texts(12).unwrap_or_default());

    // Pen, mouse and touch input from the WebView.
    use_future({
        let db = db.clone();
        move || {
            let db = db.clone();
            async move {
                let mut st = st;
                let mut eval = document::eval(READER_JS);
                while let Ok(msg) = eval.recv::<FromJs>().await {
                    match msg {
                        FromJs::Peek { i, start, below, y } => st.peek.set(Some(Peek { i, start, below, y })),
                        FromJs::Unpeek => st.peek.set(None),
                        FromJs::Open { i } => {
                            st.peek.set(None);
                            st.panel.set(Some(Panel::Word(i)));
                        }
                        FromJs::Phrase { tokens } => {
                            st.peek.set(None);
                            st.panel.set(Some(Panel::Phrase { tokens, save_first: false }));
                        }
                        FromJs::Menu { tokens } => {
                            st.peek.set(None);
                            st.panel.set(Some(Panel::Phrase { tokens, save_first: true }));
                        }
                        FromJs::Highlight { tokens } => st.save_phrase(&db, &tokens),
                        FromJs::Escape => {
                            st.peek.set(None);
                            st.panel.set(None);
                        }
                    }
                }
            }
        }
    });

    // Something was shared to Kabar: open it.
    use_effect({
        let db = db.clone();
        move || {
            let Some(input) = inbox.0.read().clone() else { return };
            let mut slot = inbox.0;
            slot.set(None);
            match input {
                SharedInput::Text { text, source } => match web::as_url(&text) {
                    Some(url) => import_url(db.clone(), st, busy, recent, url.to_string()),
                    None => open_new(&db, st, &text, TextKind::Shared, source.as_deref(), None, recent),
                },
                SharedInput::Image { path, source, .. } => {
                    run_ocr(db.clone(), st, busy, recent, std::path::PathBuf::from(path), source);
                }
            }
        }
    });

    let open = st.open.read().clone();
    let mode = *st.mode.read();
    let voice = *st.voice.read();

    rsx! {
        div { class: "reader",
            div { class: "card reader-bar",
                div { class: "row",
                    h2 { style: "margin: 0; flex: 1", "{ui.t(Msg::NavReader)}" }
                    if open.is_some() {
                        button { onclick: move |_| st.close(), "{ui.t(Msg::ReaderNew)}" }
                    }
                }
                div { class: "row",
                    div { class: "row", role: "group", aria_label: "{ui.t(Msg::NavReader)}",
                        for (m , label) in [(Mode::Inline, Msg::ModeInline), (Mode::Split, Msg::ModeSplit)] {
                            button {
                                key: "{label:?}",
                                aria_pressed: if mode == m { "true" } else { "false" },
                                onclick: move |_| {
                                    let mut mode = st.mode;
                                    mode.set(m);
                                },
                                "{ui.t(label)}"
                            }
                        }
                    }
                    span { class: "muted", "{ui.t(Msg::Voice)}" }
                    div { class: "row", role: "group", aria_label: "{ui.t(Msg::Voice)}",
                        for v in UiLanguage::ALL {
                            button {
                                key: "{v.tag()}",
                                lang: v.tag(),
                                aria_pressed: if voice == v { "true" } else { "false" },
                                onclick: move |_| {
                                    let mut voice = st.voice;
                                    voice.set(v);
                                },
                                "{v.native_name()}"
                            }
                        }
                    }
                }
            }

            if let Some(m) = *busy.read() {
                div { class: "card", role: "status", "{ui.t(m)}" }
            }

            match open {
                None => rsx! {
                    InputCard { busy, recent }
                },
                Some(open) => rsx! {
                    ReadView { open, mode }
                },
            }

            if let Some(msg) = *st.toast.read() {
                div { class: "toast", role: "status",
                    span { "{ui.t(msg)}" }
                    button {
                        class: "ghost",
                        aria_label: "{ui.t(Msg::Close)}",
                        onclick: move |_| {
                            let mut t = st.toast;
                            t.set(None);
                        },
                        "×"
                    }
                }
            }
        }
    }
}

/// Saves a new text and opens it.
fn open_new(
    db: &Db,
    st: ReaderState,
    body: &str,
    kind: TextKind,
    source: Option<&str>,
    layout: Option<&str>,
    mut recent: Signal<Vec<SavedText>>,
) {
    if body.trim().is_empty() {
        return;
    }
    let saved = db.0.borrow_mut().save_text(body, kind, source, layout);
    match saved {
        Ok(text) => {
            st.load(db, text);
            recent.set(db.0.borrow().recent_texts(12).unwrap_or_default());
        }
        Err(e) => eprintln!("kabar: saving text failed: {e}"),
    }
}

/// OCR off the UI thread, then structure the screenshot and open it.
fn run_ocr(
    db: Db,
    mut st: ReaderState,
    mut busy: Signal<Option<Msg>>,
    recent: Signal<Vec<SavedText>>,
    path: std::path::PathBuf,
    source: Option<String>,
) {
    if !platform::ocr_available() {
        st.toast.set(Some(Msg::OcrUnavailable));
        return;
    }
    busy.set(Some(Msg::OcrWorking));
    spawn(async move {
        let (tx, rx) = futures_channel::oneshot::channel();
        std::thread::spawn(move || {
            let _ = tx.send(platform::ocr_image(&path));
        });
        let result = rx.await.unwrap_or_else(|_| Err("OCR thread stopped".into()));
        busy.set(None);
        match result {
            Ok(ocr) => {
                let screen = structure(&ocr);
                if screen.items.is_empty() {
                    st.toast.set(Some(Msg::ScreenshotEmpty));
                    return;
                }
                let body = screen.reading_text();
                let layout = serde_json::to_string(&screen).ok();
                open_new(&db, st, &body, TextKind::Screenshot, source.as_deref(), layout.as_deref(), recent);
            }
            Err(e) => {
                eprintln!("kabar: OCR failed: {e}");
                st.toast.set(Some(Msg::OcrFailed));
            }
        }
    });
}

/// A pasted, dropped or shared link: fetch the page and read its article.
fn import_url(
    db: Db,
    st: ReaderState,
    mut busy: Signal<Option<Msg>>,
    mut recent: Signal<Vec<SavedText>>,
    url: String,
) {
    busy.set(Some(Msg::UrlFetching));
    spawn(async move {
        let page = web::fetch(url.clone()).await;
        busy.set(None);
        let page = match page {
            Ok(p) => p,
            Err(e) => {
                eprintln!("kabar: importing {url} failed: {e}");
                let mut t = st.toast;
                t.set(Some(Msg::UrlImportFailed));
                return;
            }
        };
        let saved = db.0.borrow_mut().save_web_text(page.title.as_deref(), &page.text, &url);
        match saved {
            Ok(text) => {
                st.load(&db, text);
                recent.set(db.0.borrow().recent_texts(12).unwrap_or_default());
            }
            Err(e) => eprintln!("kabar: saving the page failed: {e}"),
        }
    });
}

/// Copies picked or dropped image bytes into Kabar's inbox folder, so every
/// platform's OCR gets a plain file path.
async fn stash_image(file: &FileData) -> Result<std::path::PathBuf, String> {
    let bytes = file.read_bytes().await.map_err(|e| e.to_string())?;
    let dir = platform::data_dir()?.join("inbox");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ext = file.name().rsplit_once('.').map_or("png".to_string(), |(_, e)| e.to_lowercase());
    let path = dir.join(format!("{}.{ext}", uuid::Uuid::new_v4()));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

fn is_image(file: &FileData) -> bool {
    file.content_type().is_some_and(|t| t.starts_with("image/"))
        || file.name().rsplit_once('.').is_some_and(|(_, e)| {
            matches!(e.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp" | "heic" | "gif" | "bmp")
        })
}

#[component]
fn InputCard(busy: Signal<Option<Msg>>, recent: Signal<Vec<SavedText>>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let st = use_context::<ReaderState>();
    let mut draft = use_signal(String::new);
    let mut dragging = use_signal(|| false);

    let pick = {
        let db = db.clone();
        move |files: Vec<FileData>| {
            let db = db.clone();
            let Some(file) = files.into_iter().find(is_image) else { return };
            spawn(async move {
                match stash_image(&file).await {
                    Ok(path) => run_ocr(db, st, busy, recent, path, None),
                    Err(e) => {
                        eprintln!("kabar: reading the image failed: {e}");
                        let mut t = st.toast;
                        t.set(Some(Msg::OcrFailed));
                    }
                }
            });
        }
    };

    rsx! {
        div {
            class: if *dragging.read() { "card reader-input dragging" } else { "card reader-input" },
            ondragover: move |e| {
                e.prevent_default();
                dragging.set(true);
            },
            ondragleave: move |_| dragging.set(false),
            ondrop: {
                let db = db.clone();
                let pick = pick.clone();
                move |e: DragEvent| {
                    e.prevent_default();
                    dragging.set(false);
                    let files = e.files();
                    if files.iter().any(is_image) {
                        pick(files);
                    } else if let Some(text) = e.data_transfer().get_as_text() {
                        match web::as_url(&text) {
                            Some(url) => import_url(db.clone(), st, busy, recent, url.to_string()),
                            None => open_new(&db, st, &text, TextKind::Pasted, None, None, recent),
                        }
                    }
                }
            },
            p { class: "muted", "{ui.t(Msg::ReaderIntro)}" }
            textarea {
                class: "reader-textarea",
                dir: "auto",
                lang: "id",
                rows: "5",
                placeholder: "{ui.t(Msg::ReaderPlaceholder)}",
                // Uncontrolled: fast typing, IMEs and Samsung's handwriting
                // keyboard never lose characters to a re-render.
                oninput: move |e| draft.set(e.value()),
            }
            div { class: "row",
                button {
                    class: "primary",
                    disabled: draft.read().trim().is_empty(),
                    onclick: {
                        let db = db.clone();
                        move |_| {
                            let body = draft.read().clone();
                            match web::as_url(&body) {
                                Some(url) => import_url(db.clone(), st, busy, recent, url.to_string()),
                                None => open_new(&db, st, &body, TextKind::Pasted, None, None, recent),
                            }
                        }
                    },
                    "{ui.t(Msg::ReaderRead)}"
                }
                label { class: "button-like",
                    "{ui.t(Msg::ReaderScreenshot)}"
                    input {
                        r#type: "file",
                        accept: "image/*",
                        class: "visually-hidden",
                        onchange: {
                            let pick = pick.clone();
                            move |e: FormEvent| pick(e.files())
                        },
                    }
                }
                span { class: "muted", "{ui.t(Msg::ReaderDrop)}" }
            }
        }
        if !recent.read().is_empty() {
            div { class: "card",
                h3 { "{ui.t(Msg::ReaderRecent)}" }
                ul { class: "recent",
                    for text in recent.read().iter().cloned() {
                        li { key: "{text.id}",
                            button {
                                class: "recent-item",
                                onclick: {
                                    let db = db.clone();
                                    let text = text.clone();
                                    move |_| st.load(&db, text.clone())
                                },
                                span { class: "pill", "{kind_icon(text.kind)}" }
                                bdi { lang: "id", " {text.title}" }
                            }
                            button {
                                class: "ghost",
                                aria_label: "{ui.t(Msg::Delete)}",
                                onclick: {
                                    let db = db.clone();
                                    let id = text.id.clone();
                                    move |_| {
                                        let _ = db.0.borrow_mut().delete_text(&id);
                                        recent.set(db.0.borrow().recent_texts(12).unwrap_or_default());
                                    }
                                },
                                "×"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn kind_icon(kind: TextKind) -> &'static str {
    match kind {
        TextKind::Pasted => "✎",
        TextKind::Shared => "↪",
        TextKind::Screenshot => "▣",
        TextKind::Web => "🔗",
    }
}

#[component]
fn ReadView(open: Open, mode: Mode) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let doc = open.doc.clone();
    let statuses = st.statuses.read();
    let highlighted: HashSet<usize> =
        st.highlights.read().iter().flat_map(|h| doc.tokens_in(h.start, h.end).collect::<Vec<_>>()).collect();
    let selected: HashSet<usize> = match &*st.panel.read() {
        Some(Panel::Word(i)) => [*i].into(),
        Some(Panel::Phrase { tokens, .. }) => tokens.iter().copied().collect(),
        Some(Panel::Explain(i)) if *i < doc.tokens.len() => {
            let (start, end) = doc.sentence_span(*i);
            doc.tokens_in(start, end).collect()
        }
        Some(Panel::Explain(_)) | None => HashSet::new(),
    };
    let voice = *st.voice.read();
    let split = mode == Mode::Split;
    let panel_open = st.panel.read().is_some();

    let class_of = |i: usize| -> String {
        let t = &doc.tokens[i];
        let Some(w) = &t.word else { return String::new() };
        let mut c = String::from("w");
        match &w.key {
            Some(k) => {
                c.push_str(" st-");
                c.push_str(statuses.get(k).copied().unwrap_or_default().as_str());
            }
            None => c.push_str(" name"),
        }
        if w.is_bridge() {
            c.push_str(" from-arabic");
            if w.info.as_ref().and_then(|i| i.bridge.as_ref()).is_some_and(|b| b.shift == Shift::FalseFriend)
            {
                c.push_str(" false-friend");
            }
        }
        if !w.has_gloss() && w.key.is_some() {
            c.push_str(" no-gloss");
        }
        if highlighted.contains(&i) {
            c.push_str(" hl");
        }
        if selected.contains(&i) {
            c.push_str(" selected");
        }
        c
    };

    let mut last_speaker: Option<String> = None;
    let rows = doc.segments.iter().enumerate().map(|(si, seg)| {
        let mut parts: Vec<(Option<usize>, String)> = Vec::new();
        let mut at = seg.start;
        for i in seg.tokens.clone() {
            let t = &doc.tokens[i];
            if t.start > at {
                parts.push((None, doc.text(at, t.start).to_string()));
            }
            parts.push((t.word.as_ref().map(|_| i), doc.text(t.start, t.end).to_string()));
            at = t.end;
        }
        if at < seg.end {
            parts.push((None, doc.text(at, seg.end).to_string()));
        }
        let (row_class, time, speaker) = match &seg.kind {
            SegKind::Para => ("seg para", None, None),
            SegKind::Chrome => ("seg chrome", None, None),
            SegKind::Bubble { side, time, speaker } => {
                let who = match side {
                    Side::Right => Some(ui.t(Msg::Me).to_string()),
                    _ => speaker.clone(),
                };
                let show = who.is_some() && who != last_speaker;
                last_speaker = who.clone();
                let class = match side {
                    Side::Left => "seg bubble left",
                    Side::Right => "seg bubble right",
                    Side::Center => "seg bubble center",
                };
                (class, time.clone(), if show { who } else { None })
            }
        };
        let gloss_chips: Vec<(usize, String, String)> = seg
            .tokens
            .clone()
            .filter_map(|i| {
                let w = doc.tokens[i].word.as_ref()?;
                let gloss =
                    w.shown().map_or_else(|| "…".to_string(), |info| info.gloss.get(voice).to_string());
                Some((i, doc.text(doc.tokens[i].start, doc.tokens[i].end).to_string(), gloss))
            })
            .collect();
        let chrome = seg.kind == SegKind::Chrome;
        rsx! {
            div { key: "{si}", class: "{row_class}",
                if let Some(who) = speaker {
                    div { class: "speaker", bdi { "{who}" } }
                }
                div { class: "orig", dir: "ltr", lang: "id",
                    for (j , (i , text)) in parts.into_iter().enumerate() {
                        match i {
                            Some(i) => rsx! {
                                span { key: "{j}", class: class_of(i), "data-i": "{i}", "{text}" }
                            },
                            None => rsx! {
                                span { key: "{j}", "{text}" }
                            },
                        }
                    }
                    if let Some(time) = time {
                        span { class: "time", "{time}" }
                    }
                }
                if split && !chrome {
                    div { class: "explain",
                        for (i , surface , gloss) in gloss_chips {
                            span { key: "{i}", class: "chip {class_of(i)}", "data-i": "{i}",
                                bdi { class: "latin", lang: "id", "{surface}" }
                                " "
                                span { "{gloss}" }
                            }
                        }
                    }
                }
            }
        }
    });

    rsx! {
        div { class: if panel_open { "read-layout with-panel" } else { "read-layout" },
            div { class: "card read-card",
                if let Some(src) = &open.text.source {
                    p { class: "muted small",
                        "{ui.t(Msg::SharedFrom)} "
                        bdi { "{src}" }
                    }
                }
                if split {
                    div { class: "split-head",
                        span { "{ui.t(Msg::Original)}" }
                        span { "{ui.t(Msg::Explanation)}" }
                    }
                }
                div {
                    id: "reader-doc",
                    class: if split { "doc split" } else { "doc inline" },
                    {rows}
                }
                p { class: "muted small pen-hint", "{ui.t(Msg::PenHint)}" }
            }
            if panel_open {
                card::PanelView {}
            }
        }
        if let Some(p) = *st.peek.read() {
            card::PeekView { peek: p }
        }
    }
}
