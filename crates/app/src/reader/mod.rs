//! Reader & Screenshot Decoder (SPEC §11): paste, share, drop or open a
//! screenshot; every word is colored by status and explained from local data.
//!
//! The Reader renders its words itself rather than through
//! `crate::annotated::Annotated`, on purpose: a Reader text is a whole page
//! (chat bubbles, speakers, timestamps, chrome lines), its words carry
//! learning status (new / learning / known, from Arabic, false friend),
//! support drag-selection, highlights and one delegated tap handler over
//! `data-i`, and the line translation is fetched for the whole text at once.
//! What both share lives in one place: `ReadDoc` for the analysis,
//! `annotated::parts` for word parts and colours, `card::InfoView` and
//! `grammar::GrammarAt` for the word card, and the same Settings
//! (translation On/Tap/Off, word colours, grammar hints).

pub(crate) mod card;
mod content;
pub mod doc;
pub mod explain;
pub mod glossary;
pub mod grammar;
mod library;
pub mod morph;
mod reading;
pub mod translate;
mod web;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::OnceLock;

use dioxus::html::{FileData, HasFileData};
use dioxus::prelude::*;
use kabar_core::learn::PersonalDictionary;
use kabar_core::locale::UiLanguage;
use kabar_core::reader::screen_ai::{self, StructureScreenshot};
use kabar_core::reader::screenshot::{ScreenDoc, Side, structure};
use kabar_protocol::ai::Task;
use kabar_protocol::lesson::{Example, Localized};
use kabar_protocol::reader::{OcrResult, SharedInput, WordStatus};
use kabar_protocol::srs::{ArabicOrigin, Card, CardKind, CardSource, MeaningShift, StudyLanguage};
use serde::Deserialize;

use self::content::LocalGlossary;
use self::doc::{Lexicon, ReadDoc, SegKind};
use self::glossary::{Glossary, Shift, WordInfo};
use crate::account::Cloud;
use crate::i18n::Msg;
use crate::store::{Highlight, SavedText, Store, StoreError, TextKind};
use crate::teach::Ai;
use crate::{Db, Ui, platform};

const READER_JS: &str = include_str!("../../assets/reader.js");

/// The content pack merged over the seed; parsed once (~13k entries,
/// ~150 ms and ~37 MB on a desktop). Only the Reader and Chat need it, so
/// they `warm_up` when they open, off the UI thread; screens that never
/// look a word up don't pay for it.
pub fn glossary() -> &'static dyn Glossary {
    static LOCAL: OnceLock<LocalGlossary> = OnceLock::new();
    LOCAL.get_or_init(LocalGlossary::load)
}

/// Parses the glossary in the background (SPEC §25: popover under 100 ms),
/// once per run.
pub fn warm_up() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let t = std::time::Instant::now();
            glossary();
            // `just logs` shows it: the cold first popover waits at most this long.
            crate::diag::report!("glossary ready in {} ms", t.elapsed().as_millis());
        });
    });
}

/// Every word of the content pack with its frequency rank, for spelling
/// suggestions (the Listening Companion).
pub fn lexicon() -> &'static [kabar_core::ink::spell::Known] {
    static WORDS: OnceLock<Vec<kabar_core::ink::spell::Known>> = OnceLock::new();
    WORDS.get_or_init(content::known_words)
}

pub(crate) use self::card::speak;

/// Where a peek floats next to a word: below or above it, whichever side
/// of the window has more room.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
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
    pub peek: Signal<Option<Peek>>,
    pub panel: Signal<Option<Panel>>,
    pub toast: Signal<Option<Msg>>,
    /// The open text's grammar, worked out once when it opens.
    pub grammar: Signal<Option<Rc<grammar::DocGrammar>>>,
    /// The translation under each line of the open text.
    pub translated: Signal<translate::Translated>,
    /// Bumped by "try again" to translate the text afresh.
    pub retry: Signal<u32>,
}

impl ReaderState {
    pub fn status(&self, key: &str) -> WordStatus {
        self.statuses.read().get(key).copied().unwrap_or_default()
    }

    pub fn set_status(mut self, db: &Db, key: &str, status: WordStatus) {
        if let Err(e) = db.0.borrow_mut().set_word_status(key, status) {
            self.write_failed("saving word status", &e);
            return;
        }
        self.statuses.write().insert(key.to_string(), status);
    }

    /// A store write didn't happen: say so kindly, and leave the screen as
    /// it was so it never shows something saved that isn't.
    pub fn write_failed(mut self, what: &str, e: &dyn std::fmt::Display) {
        crate::diag::report!("{what} failed: {e}");
        self.toast.set(Some(Msg::ReaderWriteFailed));
    }

    pub fn save_phrase(mut self, db: &Db, tokens: &[usize]) {
        let Some(open) = self.open.read().clone() else { return };
        let Some((start, end)) = open.doc.span_of(tokens) else { return };
        let phrase = open.doc.text(start, end).to_string();
        let sentence = open.doc.sentence_of(tokens[0]).to_string();
        let saved = save_phrase(&mut db.0.borrow_mut(), &open.text.id, start, end, &phrase, &sentence);
        match saved {
            Ok(h) => {
                self.highlights.write().push(h);
                self.toast.set(Some(Msg::PhraseSaved));
            }
            Err(e) => self.write_failed("saving a phrase", &e),
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
            peek: Signal::new(None),
            panel: Signal::new(None),
            toast: Signal::new(None),
            grammar: Signal::new(None),
            translated: Signal::new(translate::Translated::default()),
            retry: Signal::new(0),
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
        self.grammar.set(Some(Rc::new(grammar::DocGrammar::new(&doc, &grammar::syllabus(db)))));
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

    pub(crate) fn close(mut self) {
        self.open.set(None);
        self.grammar.set(None);
        self.peek.set(None);
        self.panel.set(None);
    }
}

/// A highlighted phrase is also a review item, met in its sentence. Both
/// rows or neither: when the review item can't be saved, the highlight goes too.
fn save_phrase(
    store: &mut Store,
    text_id: &str,
    start: usize,
    end: usize,
    phrase: &str,
    sentence: &str,
) -> Result<Highlight, StoreError> {
    let h = store.add_highlight(text_id, start, end, phrase)?;
    let key = format!("phrase:{}", phrase.to_lowercase());
    if let Err(e) = store.save_word(&key, phrase, sentence, Some(text_id)) {
        store.remove_highlight(&h.id).ok();
        return Err(e);
    }
    Ok(h)
}

/// "Add to review": the word becomes a real review card (when Kabar can
/// explain it) and is remembered with its sentence for the Library; only
/// once that's done does a new word count as learning. Returns whether the
/// word was promoted to learning.
pub(crate) fn add_to_review(
    store: &mut Store,
    key: &str,
    surface: &str,
    sentence: &str,
    text_id: &str,
    info: Option<&WordInfo>,
) -> Result<bool, StoreError> {
    if let Some(card) = reader_card(key, surface, info, sentence, text_id, &store.cards()?) {
        store.upsert_card(&card)?;
    }
    store.save_word(key, surface, sentence, Some(text_id))?;
    let promote = store.word_statuses([key])?.get(key).copied().unwrap_or_default() == WordStatus::New;
    if promote {
        store.set_word_status(key, WordStatus::Learning)?;
    }
    Ok(promote)
}

/// The recognition card "Add to review" makes: the word as it was met, its
/// meaning, the sentence it was met in and its Arabic Bridge. None when
/// there's no meaning to recall (Teach Kabar covers those) or when the deck
/// already holds a recognition card for the word. The id is stable, so the
/// same word saved on two devices is one card.
pub(crate) fn reader_card(
    key: &str,
    surface: &str,
    info: Option<&WordInfo>,
    sentence: &str,
    text_id: &str,
    existing: &[Card],
) -> Option<Card> {
    let info = info?;
    let language = if key.starts_with("jv:") { StudyLanguage::Jv } else { StudyLanguage::Id };
    let word = surface.trim().to_lowercase();
    if word.is_empty() || info.gloss.en.trim().is_empty() && info.gloss.arz.trim().is_empty() {
        return None;
    }
    let held = existing.iter().any(|c| {
        c.kind == CardKind::Recognition
            && c.language == language
            && (c.word.eq_ignore_ascii_case(&word) || c.word.eq_ignore_ascii_case(&info.lemma))
    });
    if held {
        return None;
    }
    let localized =
        |g: &glossary::Gloss| Localized { ar_eg: g.arz.clone(), ar: g.ar.clone(), en: g.en.clone() };
    let empty = Localized { ar_eg: String::new(), ar: String::new(), en: String::new() };
    Some(Card {
        v: kabar_protocol::srs::CARD_VERSION,
        id: format!("reader.{}.{word}.recognition", language.tag()),
        word,
        language,
        kind: CardKind::Recognition,
        meaning: localized(&info.gloss),
        note: info.note.as_ref().map(localized),
        example: Some(Example { text: sentence.trim().to_string(), meaning: empty }),
        arabic: info.bridge.as_ref().map(|b| ArabicOrigin {
            word: b.arabic.clone(),
            root: b.root.clone(),
            shift: match b.shift {
                Shift::Same | Shift::Narrowed | Shift::Broadened => MeaningShift::Same,
                Shift::Shifted => MeaningShift::Shifted,
                Shift::FalseFriend => MeaningShift::FalseFriend,
            },
            note: b.note.as_ref().map(localized),
        }),
        source: CardSource::Reader,
        source_ref: Some(text_id.to_string()),
        // Javanese and AI-drafted glosses stay flagged until approved.
        verified: info.verified,
    })
}

/// Input that arrived from outside (Android share sheet), waiting for the Reader.
#[derive(Clone, Copy)]
pub struct Inbox(pub Signal<Option<SharedInput>>);

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum FromJs {
    Peek(Peek),
    Unpeek,
    /// The word sheet for a word.
    Open {
        i: usize,
    },
    Phrase {
        tokens: Vec<usize>,
    },
    Highlight {
        tokens: Vec<usize>,
    },
    Menu {
        tokens: Vec<usize>,
    },
    Escape,
}

#[component]
pub fn Reader() -> Element {
    use_hook(warm_up);
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
    translate::use_translation(st);
    let busy = use_signal(|| None::<Msg>);
    let recent = use_signal(|| db.0.borrow().recent_texts(library::TEXTS).unwrap_or_default());
    // The reading settings popover under Aa.
    let sheet = use_signal(|| false);

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
                        FromJs::Peek(p) => {
                            // The word's card is already open: don't repeat it in a popover.
                            if *st.panel.peek() != Some(Panel::Word(p.i)) {
                                st.peek.set(Some(p));
                            }
                        }
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
                        // One layer at a time: the settings popover first.
                        FromJs::Escape if *sheet.peek() => {
                            let mut sheet = sheet;
                            sheet.set(false);
                        }
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
                    None => {
                        open_new(&db, st, &text, TextKind::Shared, source.as_deref(), None, recent);
                    }
                },
                SharedInput::Image { path, source, more, .. } => {
                    if more > 0 {
                        let mut toast = st.toast;
                        toast.set(Some(Msg::ShareFirstImageOnly));
                    }
                    run_ocr(db.clone(), st, busy, recent, std::path::PathBuf::from(path), source);
                }
            }
        }
    });

    let open = st.open.read().clone();
    let prefs = use_context::<crate::settings::device::DevicePrefs>();
    let read = reading::attrs(*prefs.reading.read(), open.is_some(), *ui.width.read());

    rsx! {
        div {
            class: if read.focus { "reader focus" } else { "reader" },
            "data-read-size": read.size,
            "data-read-spacing": read.spacing,
            "data-read-width": read.width,
            if open.is_some() {
                reading::OpenBar { sheet }
            } else {
                div { class: "page-head reader-head",
                    div {
                        h1 { "{ui.t(Msg::NavReader)}" }
                        p { class: "sub", "{reading::tagline(ui.lang())}" }
                    }
                }
            }

            if let Some(m) = *busy.read() {
                div { class: "card", role: "status", "{ui.t(m)}" }
            }

            match open {
                None => rsx! {
                    // Side by side on a wide window: open a text, or pick one up again.
                    div { class: "reader-home",
                        InputCard { busy, recent }
                        library::Library { recent }
                    }
                },
                Some(open) => rsx! {
                    ReadView { open }
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
) -> Option<SavedText> {
    if body.trim().is_empty() {
        return None;
    }
    let saved = db.0.borrow_mut().save_text(body, kind, source, layout);
    match saved {
        Ok(text) => {
            st.load(db, text.clone());
            recent.set(db.0.borrow().recent_texts(library::TEXTS).unwrap_or_default());
            Some(text)
        }
        Err(e) => {
            crate::diag::report!("saving text failed: {e}");
            None
        }
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
    let ai = try_consume_context::<Ai>();
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
                let saved = open_new(
                    &db,
                    st,
                    &body,
                    TextKind::Screenshot,
                    source.as_deref(),
                    layout.as_deref(),
                    recent,
                );
                if let (Some(ai), Some(text)) = (ai, saved) {
                    refine_with_ai(ai, db, st, recent, ocr, text);
                }
            }
            Err(e) => {
                crate::diag::report!("OCR failed: {e}");
                st.toast.set(Some(Msg::OcrFailed));
            }
        }
    });
}

/// The AI pass (`structure_screenshot`, spec §11) after the deterministic
/// one is already on screen: when an engine is connected, ask it in the
/// background and, if its answer validates, swap the saved text's body and
/// layout for it. Any error keeps the deterministic layout, silently. The
/// request is personal (a screenshot is someone's chat), so it never reaches
/// an engine that trains on inputs.
fn refine_with_ai(
    ai: Ai,
    db: Db,
    st: ReaderState,
    mut recent: Signal<Vec<SavedText>>,
    ocr: OcrResult,
    text: SavedText,
) {
    let req = screen_ai::request(&ocr);
    if ai.0.plan(Task::StructureScreenshot, req.personal).is_err() {
        return;
    }
    spawn(async move {
        let answer = match ai.0.run::<StructureScreenshot>(&req).await {
            Ok(a) => a,
            Err(e) => {
                crate::diag::report!("structure_screenshot failed, keeping the on-device layout: {e:?}");
                return;
            }
        };
        let screen = screen_ai::to_screen_doc(&answer.value, &ocr);
        let body = screen.reading_text();
        // Highlights are byte ranges in the old body: don't move text under them.
        let marked = db.0.borrow().highlights(&text.id).map_or(true, |h| !h.is_empty());
        if body.trim().is_empty() || marked {
            return;
        }
        let Ok(mut layout) = serde_json::to_value(&screen) else { return };
        // Readers of ScreenDoc ignore this; it says where the layout came from.
        layout["structured_by"] = serde_json::json!({
            "task": "structure_screenshot",
            "engine": answer.engine.as_str(),
            "prompt_version": answer.prompt_version,
            "provisional": answer.provisional,
            "needs_review": answer.needs_review,
        });
        let updated = db.0.borrow_mut().update_text(&text, &body, Some(&layout.to_string()));
        match updated {
            Ok(t) => {
                if st.open.peek().as_ref().is_some_and(|o| o.text.id == t.id) {
                    st.load(&db, t);
                }
                recent.set(db.0.borrow().recent_texts(library::TEXTS).unwrap_or_default());
            }
            Err(e) => crate::diag::report!("saving the AI layout failed: {e}"),
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
                crate::diag::report!("importing {url} failed: {e}");
                let mut t = st.toast;
                t.set(Some(Msg::UrlImportFailed));
                return;
            }
        };
        let saved = db.0.borrow_mut().save_web_text(page.title.as_deref(), &page.text, &url);
        match saved {
            Ok(text) => {
                st.load(&db, text);
                recent.set(db.0.borrow().recent_texts(library::TEXTS).unwrap_or_default());
            }
            Err(e) => crate::diag::report!("saving the page failed: {e}"),
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

/// Android's "Open a screenshot": the system photo picker hands the image
/// over like a share (see `ImagePickActivity`), so it lands in the [`Inbox`]
/// and opens exactly like a screenshot shared from another app. The app's own
/// resume check may take it first; either way it arrives once.
fn pick_native(st: ReaderState, inbox: Inbox) {
    if let Err(e) = platform::pick_image() {
        crate::diag::report!("opening the photo picker failed: {e}");
        let mut t = st.toast;
        t.set(Some(Msg::PickFailed));
        return;
    }
    spawn(async move {
        // Ten minutes is far longer than anyone browses their photos.
        for _ in 0..2400 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            // State first: once it says done, the picked image is already stored.
            let state = platform::pick_state();
            if let Some(input) = platform::take_shared_input() {
                let mut slot = inbox.0;
                slot.set(Some(input));
                return;
            }
            match state {
                platform::PickState::Open => {}
                platform::PickState::Done => return,
                platform::PickState::Failed => {
                    let mut t = st.toast;
                    t.set(Some(Msg::PickFailed));
                    return;
                }
            }
        }
    });
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
    let inbox = use_context::<Inbox>();
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
                        crate::diag::report!("reading the image failed: {e}");
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
                                None => {
        open_new(&db, st, &text, TextKind::Pasted, None, None, recent);
    }
                            }
                        }
                    }
                },
                p { class: "muted", "{ui.t(Msg::ReaderIntro)}" }
                reading::VoicePicker {}
                textarea {
                    class: "reader-textarea",
                    // `auto` follows the typed text, but an empty box would lay
                    // the Arabic placeholder out left-to-right.
                    dir: if draft.read().is_empty() { ui.lang().dir().as_str() } else { "auto" },
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
                                    None => {
        open_new(&db, st, &body, TextKind::Pasted, None, None, recent);
    }
                                }
                            }
                        },
                        "{ui.t(Msg::ReaderRead)}"
                    }
                    if platform::NATIVE_IMAGE_PICKER {
                        button {
                            disabled: busy.read().is_some(),
                            onclick: move |_| pick_native(st, inbox),
                            "{ui.t(Msg::ReaderScreenshot)}"
                        }
                    } else {
                        label { class: "button-like",
                            "{ui.t(Msg::ReaderScreenshot)}"
                            input {
                                r#type: "file",
                                // macOS Vision also reads HEIC/TIFF (iPhone photos, older screenshots).
                                accept: "image/*,.heic,.heif,.tif,.tiff,.bmp",
                                class: "visually-hidden",
                                onchange: {
                                    let pick = pick.clone();
                                    move |e: FormEvent| pick(e.files())
                                },
                            }
                        }
                    }
                    span { class: "muted", "{ui.t(Msg::ReaderDrop)}" }
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
fn ReadView(open: Open) -> Element {
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
        if let Some(class) = w.class {
            c.push_str(" wc-ink ");
            c.push_str(class.css());
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
                // English the learner reads needs no chip.
                if w.is_english_aside() {
                    return None;
                }
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
                if !chrome {
                    // English glosses read left to right even in the Arabic UI.
                    div { class: "under", dir: if voice == UiLanguage::En { "ltr" } else { "rtl" },
                        if !gloss_chips.is_empty() {
                            div { class: "explain",
                                for (i , surface , gloss) in gloss_chips {
                                    span { key: "{i}", class: "chip {class_of(i)}", "data-i": "{i}",
                                        bdi { class: "latin", lang: "id", "{surface}" }
                                        span { class: "chip-gloss", "{gloss}" }
                                    }
                                }
                            }
                        }
                        translate::LineUnder { seg: si }
                        grammar::LineGrammar { seg: si }
                    }
                }
            }
        }
    });

    rsx! {
        div { class: if panel_open { "read-layout with-panel" } else { "read-layout" },
            crate::ui::DivCard { class: "read-card",
                if let Some(src) = &open.text.source {
                    p { class: "muted small",
                        "{ui.t(Msg::SharedFrom)} "
                        bdi { "{src}" }
                    }
                }
                translate::TranslationStatus {}
                div { class: "split-head",
                    span { "{ui.t(Msg::Original)}" }
                    span { "{ui.t(Msg::Explanation)}" }
                }
                div { id: "reader-doc", class: "doc split", {rows} }
                p { class: "muted small pen-hint", "{input_hint(ui.lang(), *ui.tablet.read())}" }
                grammar::TextGrammar {}
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

/// How to use the words below the text, for this device's input: the pen
/// sentence on the tablet (S Pen), touch on a phone, the mouse on the Mac.
fn input_hint(lang: kabar_core::locale::UiLanguage, tablet: bool) -> &'static str {
    use kabar_core::locale::UiLanguage::*;
    if tablet {
        return crate::i18n::t(lang, Msg::PenHint);
    }
    match (cfg!(target_os = "android"), lang) {
        (true, ArEg) => {
            "دوس على أي كلمة تشوف معناها، ودوس مطوّل تفتح الكارت أو تحفظها. اسحب على عبارة تشرحها."
        }
        (true, Ar) => "المس أي كلمة لترى معناها، والمس مطوّلًا لفتح البطاقة أو حفظها. اسحب على عبارة لشرحها.",
        (true, En) => {
            "Tap any word to see its meaning; long-press to open its card or save it. Drag across a phrase to explain it."
        }
        (false, ArEg) => {
            "وقّف الماوس على أي كلمة تشوف معناها، واضغط عليها تفتح الكارت. ظلّل عبارة تشرحها، وكليك يمين تحفظ."
        }
        (false, Ar) => {
            "قف بالفأرة على أي كلمة لترى معناها، وانقرها لفتح البطاقة. ظلّل عبارة لشرحها، وانقر يمينًا للحفظ."
        }
        (false, En) => {
            "Rest the mouse on any word to see its meaning; click it for its card. Select a phrase to explain it; right-click to save."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with_text() -> (Store, SavedText) {
        let mut store = Store::open_in_memory().unwrap();
        let text = store.save_text("Saya sedang membaca buku.", TextKind::Pasted, None, None).unwrap();
        (store, text)
    }

    #[test]
    fn adding_to_review_promotes_a_new_word_once() {
        let (mut store, text) = store_with_text();
        let s = "Saya sedang membaca buku.";
        assert!(add_to_review(&mut store, "id:baca", "membaca", s, &text.id, None).unwrap());
        assert!(!add_to_review(&mut store, "id:baca", "membaca", s, &text.id, None).unwrap());
        assert_eq!(store.word_statuses(["id:baca"]).unwrap()["id:baca"], WordStatus::Learning);
        assert_eq!(store.saved_word_count("id:baca").unwrap(), 2);
    }

    #[test]
    fn a_failed_write_changes_nothing() {
        let (mut store, text) = store_with_text();
        store.refuse_writes();
        let s = "Saya sedang membaca buku.";
        assert!(add_to_review(&mut store, "id:baca", "membaca", s, &text.id, None).is_err());
        assert_eq!(
            store.word_statuses(["id:baca"]).unwrap().get("id:baca").copied().unwrap_or_default(),
            WordStatus::New
        );
        assert!(save_phrase(&mut store, &text.id, 0, 4, "Saya", s).is_err());
        assert!(store.highlights(&text.id).unwrap().is_empty());
    }

    fn baca() -> WordInfo {
        let g = |s: &str| glossary::Gloss { arz: s.into(), ar: s.into(), en: s.into() };
        WordInfo {
            lemma: "baca".into(),
            lang: glossary::WordLang::Indonesian,
            pos: None,
            gloss: g("read"),
            indonesian: None,
            bridge: None,
            levels: None,
            note: None,
            example: None,
            verified: true,
        }
    }

    #[test]
    fn adding_to_review_makes_one_real_card() {
        let (mut store, text) = store_with_text();
        let s = "Saya sedang membaca buku.";
        let info = baca();
        add_to_review(&mut store, "id:baca", "Membaca", s, &text.id, Some(&info)).unwrap();
        add_to_review(&mut store, "id:baca", "membaca", s, &text.id, Some(&info)).unwrap();
        let cards: Vec<Card> =
            store.cards().unwrap().into_iter().filter(|c| c.source == CardSource::Reader).collect();
        assert_eq!(cards.len(), 1, "saving twice is one card");
        let c = &cards[0];
        assert_eq!((c.id.as_str(), c.word.as_str()), ("reader.id.membaca.recognition", "membaca"));
        assert_eq!(c.meaning.en, "read");
        assert_eq!(c.example.as_ref().unwrap().text, s);
        assert_eq!(c.source_ref.as_deref(), Some(text.id.as_str()));
        assert_eq!(store.saved_words(10).unwrap().len(), 1);
    }

    #[test]
    fn no_card_without_a_meaning_or_when_the_deck_has_the_word() {
        let (_, text) = store_with_text();
        assert!(reader_card("id:x", "x", None, "x", &text.id, &[]).is_none());
        let info = baca();
        let card = reader_card("id:baca", "baca", Some(&info), "baca", &text.id, &[]).unwrap();
        assert!(
            reader_card("id:baca", "membaca", Some(&info), "s", &text.id, &[card]).is_none(),
            "lemma held"
        );
        let jv =
            reader_card("jv:baca", "baca", Some(&WordInfo { verified: false, ..info }), "", &text.id, &[]);
        assert!(jv.is_some_and(|c| c.language == StudyLanguage::Jv && !c.verified));
    }

    #[test]
    fn saved_words_list_each_word_once_newest_first() {
        let (mut store, text) = store_with_text();
        store.save_word("id:baca", "membaca", "one", Some(&text.id)).unwrap();
        store.save_word("id:buku", "buku", "two", Some(&text.id)).unwrap();
        store.save_word("id:baca", "baca", "three", None).unwrap();
        let words = store.saved_words(10).unwrap();
        let got: Vec<(&str, &str)> = words.iter().map(|w| (w.lexeme.as_str(), w.sentence.as_str())).collect();
        assert_eq!(got, [("id:baca", "three"), ("id:buku", "two")]);
        assert_eq!(store.text(&text.id).unwrap().unwrap().id, text.id);
        store.delete_text(&text.id).unwrap();
        assert!(store.text(&text.id).unwrap().is_none());
    }

    #[test]
    fn a_saved_phrase_is_a_highlight_and_a_review_item() {
        let (mut store, text) = store_with_text();
        let h = save_phrase(&mut store, &text.id, 5, 11, "sedang", "Saya sedang membaca buku.").unwrap();
        assert_eq!(store.highlights(&text.id).unwrap(), vec![h]);
        assert_eq!(store.saved_word_count("phrase:sedang").unwrap(), 1);
    }
}
