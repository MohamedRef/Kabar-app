//! Teach Kabar (spec §26): the learner gives Kabar an explanation of a word
//! it didn't know, reviews the draft Kabar writes from it, and keeps it in
//! the personal dictionary ("taught by you").
//!
//! The Reader's popover opens this screen with a [`TeachSeed`]; it is also
//! reachable from the navigation.

use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::ai::{AiError, Answer, FriendlyReason, Router};
use kabar_core::lang::analyze::Engine;
use kabar_core::learn::{self, LearnEntry, Prepared, SourceInput, TeachError, TeachRequest, text};
use kabar_core::locale::UiLanguage;
use kabar_protocol::content::{Lang, Trilingual};
use kabar_protocol::learn::{SourceKind, TaughtEntry};
use serde::Deserialize;

use crate::account::Cloud;
use crate::embedded::{Css, css};
use crate::{Db, Screen, Ui};

const TEACH_CSS: Css = css!("assets/teach.css");

/// What the popover hands over when the learner taps "Teach Kabar".
#[derive(Clone, Debug, PartialEq)]
pub struct TeachSeed {
    pub lang: Lang,
    pub word: String,
    /// The sentence the word was met in.
    pub context: Option<String>,
}

/// Shared through context: set it, then switch to `Screen::Teach`.
#[derive(Clone, Copy)]
pub struct TeachInbox(pub Signal<Option<TeachSeed>>);

/// The AI router, shared through context. Engines are registered where the
/// app connects to Kabar Cloud / Kabar Home; until then every task answers
/// "not available yet".
#[derive(Clone)]
pub struct Ai(pub Rc<Router>);

impl PartialEq for Ai {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Paste,
    Link,
    Wiktionary,
}

#[derive(Clone, Debug, PartialEq)]
enum Status {
    Idle,
    Fetching,
    Thinking,
    Problem(T),
    Invalid(String),
}

/// The draft on screen and what produced it, for "another version".
#[derive(Clone)]
struct Current {
    entry: TaughtEntry,
    origin: Option<(Prepared, Answer<LearnEntry>)>,
}

#[derive(Deserialize)]
struct Fetched {
    ok: bool,
    body: String,
}

const FETCH_JS: &str = r#"
const url = await dioxus.recv();
try {
  const r = await fetch(url, { headers: { "Accept": "text/html,application/json" } });
  dioxus.send({ ok: r.ok, body: await r.text() });
} catch (e) {
  dioxus.send({ ok: false, body: String(e) });
}
"#;

/// Fetches a page from the WebView. Sites that don't allow it (CORS) fail;
/// the screen then asks the learner to paste the text instead.
async fn fetch(url: &str) -> Option<String> {
    let mut eval = document::eval(FETCH_JS);
    eval.send(url.to_string()).ok()?;
    let f = eval.recv::<Fetched>().await.ok()?;
    f.ok.then_some(f.body)
}

#[component]
pub fn TeachScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let inbox = use_context::<TeachInbox>();
    let words = use_context::<crate::annotated::Words>();
    let cloud = use_context::<Cloud>();
    let engine = use_hook(|| Rc::new(Engine::default()));

    let mut lang = use_signal(|| Lang::Id);
    let mut word = use_signal(String::new);
    let mut context = use_signal(String::new);
    let mut mode = use_signal(|| Mode::Paste);
    let mut pasted = use_signal(String::new);
    let mut link = use_signal(String::new);
    let mut status = use_signal(|| Status::Idle);
    let mut current = use_signal(|| None::<Current>);
    let mut entries = use_signal(|| db.0.borrow().taught_entries().unwrap_or_default());

    // A seed from the popover fills the form once.
    use_effect(move || {
        let mut seed = inbox.0;
        if let Some(s) = seed.write().take() {
            lang.set(s.lang);
            word.set(s.word);
            context.set(s.context.unwrap_or_default());
            current.set(None);
            status.set(Status::Idle);
        }
    });

    let l = ui.settings.read().ui_language;
    let tr = move |m: T| tr(l, m);

    let save = {
        let db = db.clone();
        move |entry: &TaughtEntry| {
            if db.0.borrow_mut().save_taught(entry).is_ok() {
                entries.set(db.0.borrow().taught_entries().unwrap_or_default());
                words.refresh(&db);
            }
        }
    };

    let teach = {
        let ai = ai.clone();
        let engine = engine.clone();
        let save = save.clone();
        move |_| {
            let ai = ai.clone();
            let engine = engine.clone();
            let mut save = save.clone();
            spawn(async move {
                let w = word();
                let source = match mode() {
                    Mode::Paste => SourceInput::Pasted { text: pasted() },
                    Mode::Link => {
                        let url = link().trim().to_string();
                        status.set(Status::Fetching);
                        let html = fetch(&url).await;
                        SourceInput::Link { url, html }
                    }
                    Mode::Wiktionary => {
                        status.set(Status::Fetching);
                        let body = fetch(&text::wiktionary_api_url(&w)).await;
                        let json = body.and_then(|b| serde_json::from_str(&b).ok());
                        match json {
                            Some(json) => SourceInput::Wiktionary { json },
                            None => {
                                status.set(Status::Problem(T::WiktionaryFailed));
                                return;
                            }
                        }
                    }
                };
                let ctx = context();
                let req = TeachRequest {
                    lang: lang(),
                    word: w,
                    context: (!ctx.trim().is_empty()).then_some(ctx),
                    sources: vec![source],
                };
                let prepared = match learn::prepare(&req, &engine) {
                    Ok(p) => p,
                    Err(e) => {
                        status.set(Status::Problem(teach_error(&e)));
                        return;
                    }
                };
                status.set(Status::Thinking);
                match ai.0.run::<LearnEntry>(&prepared.request).await {
                    Ok(answer) => {
                        let entry = learn::draft(&prepared, &answer);
                        save(&entry);
                        current.set(Some(Current { entry, origin: Some((prepared, answer)) }));
                        status.set(Status::Idle);
                    }
                    Err(e) => status.set(Status::Problem(ai_error(&e))),
                }
            });
        }
    };

    let busy = matches!(status(), Status::Fetching | Status::Thinking);
    let can_teach = !busy && !word().trim().is_empty();
    let google = text::google_search_url(lang(), &word());

    rsx! {
        style { {TEACH_CSS.text()} }
        div { class: "page-head",
            div {
                h1 { "{tr(T::Title)}" }
                p { class: "sub", "{tr(T::Intro)}" }
            }
        }
        crate::ui::DivCard { class: "teach",
            div { class: "field",
                label { r#for: "teach-word", "{tr(T::Word)}" }
                div { class: "row",
                    input {
                        id: "teach-word",
                        class: "latin",
                        dir: "ltr",
                        lang: lang().code(),
                        value: "{word}",
                        placeholder: "mantul",
                        oninput: move |e| word.set(e.value()),
                    }
                    div { class: "segmented", role: "radiogroup", aria_label: "{tr(T::Word)}",
                        for (code, name) in [(Lang::Id, "Bahasa Indonesia"), (Lang::Jv, "Basa Jawa")] {
                            button {
                                key: "{code.code()}",
                                role: "radio",
                                aria_checked: if lang() == code { "true" } else { "false" },
                                onclick: move |_| lang.set(code),
                                bdi { lang: code.code(), "{name}" }
                            }
                        }
                    }
                }
            }

            div { class: "field",
                label { r#for: "teach-context", "{tr(T::Context)}" }
                input {
                    id: "teach-context",
                    class: "latin",
                    dir: "ltr",
                    value: "{context}",
                    placeholder: "arp dicul piro",
                    oninput: move |e| context.set(e.value()),
                }
            }

            div { class: "field",
                span { class: "label", "{tr(T::Source)}" }
                div { class: "row", role: "tablist",
                    for (m, label) in [(Mode::Paste, T::ModePaste), (Mode::Link, T::ModeLink), (Mode::Wiktionary, T::ModeWiktionary)] {
                        button {
                            key: "{m:?}",
                            role: "tab",
                            aria_selected: if mode() == m { "true" } else { "false" },
                            onclick: move |_| mode.set(m),
                            "{tr(label)}"
                        }
                    }
                    a {
                        class: "button",
                        href: "{google}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "{tr(T::SearchGoogle)}"
                    }
                }
                match mode() {
                    Mode::Paste => rsx! {
                        textarea {
                            rows: "6",
                            dir: "auto",
                            value: "{pasted}",
                            placeholder: "{tr(T::PastePlaceholder)}",
                            oninput: move |e| pasted.set(e.value()),
                        }
                    },
                    Mode::Link => rsx! {
                        input {
                            r#type: "url",
                            dir: "ltr",
                            value: "{link}",
                            placeholder: "https://",
                            oninput: move |e| link.set(e.value()),
                        }
                        p { class: "muted small", "{tr(T::LinkHint)}" }
                    },
                    Mode::Wiktionary => rsx! {
                        p { class: "muted small", "{tr(T::WiktionaryHint)}" }
                    },
                }
                p { class: "muted small", "{tr(T::ScreenshotHint)}" }
            }

            div { class: "row",
                button { class: "primary", disabled: !can_teach, onclick: teach, "{tr(T::Teach)}" }
                match status() {
                    Status::Fetching => rsx! { span { class: "muted", role: "status", "{tr(T::Fetching)}" } },
                    Status::Thinking => rsx! { span { class: "muted", role: "status", "{tr(T::Thinking)}" } },
                    // Kabar Cloud is the fix, so say which step is missing
                    // and open it, instead of blaming the internet.
                    Status::Problem(T::NotConnected) => {
                        let configured = cloud.status.read().configured;
                        rsx! {
                            span { class: "no", role: "status",
                                if configured { "{tr(T::NotConnected)}" } else { "{tr(T::NoCloudInBuild)}" }
                            }
                            if configured {
                                button {
                                    onclick: move |_| {
                                        let mut s = ui.screen;
                                        s.set(Screen::Devices);
                                    },
                                    "{tr(T::SignIn)}"
                                }
                            }
                        }
                    }
                    Status::Problem(m) => rsx! { span { class: "no", role: "status", "{tr(m)}" } },
                    Status::Invalid(why) => rsx! { span { class: "no", role: "status", bdi { "{why}" } } },
                    Status::Idle => rsx! {},
                }
            }
            if busy {
                crate::reader::wait_word::WaitWord {}
            }
        }

        if let Some(cur) = current() {
            Review {
                key: "{cur.entry.row_id()}",
                entry: cur.entry.clone(),
                can_regenerate: cur.origin.is_some(),
                on_approve: {
                    let mut save = save.clone();
                    move |e: TaughtEntry| {
                        let e = learn::approve(&e);
                        save(&e);
                        current.set(None);
                    }
                },
                on_save: {
                    let mut save = save.clone();
                    move |e: TaughtEntry| {
                        save(&e);
                        current.with_mut(|c| if let Some(c) = c { c.entry = e });
                    }
                },
                on_invalid: move |why: String| status.set(Status::Invalid(why)),
                on_regenerate: {
                    let ai = ai.clone();
                    let save = save.clone();
                    move |_| {
                        let Some(Current { origin: Some((prepared, answer)), .. }) = current() else { return };
                        let ai = ai.clone();
                        let mut save = save.clone();
                        status.set(Status::Thinking);
                        spawn(async move {
                            match ai.0.regenerate::<LearnEntry>(&prepared.request, &answer).await {
                                Ok(answer) => {
                                    let entry = learn::draft(&prepared, &answer);
                                    save(&entry);
                                    current.set(Some(Current { entry, origin: Some((prepared, answer)) }));
                                    status.set(Status::Idle);
                                }
                                Err(e) => status.set(Status::Problem(ai_error(&e))),
                            }
                        });
                    }
                },
                on_discard: {
                    let db = db.clone();
                    move |e: TaughtEntry| {
                        if db.0.borrow_mut().delete_taught(&e.row_id()).is_ok() {
                            entries.set(db.0.borrow().taught_entries().unwrap_or_default());
                            words.refresh(&db);
                        }
                        current.set(None);
                    }
                },
            }
        }

        MyDictionary {
            entries: entries(),
            on_open: move |e: TaughtEntry| current.set(Some(Current { entry: e, origin: None })),
        }
    }
}

fn voice(t: &Trilingual, l: UiLanguage) -> &str {
    match l {
        UiLanguage::ArEg => &t.arz,
        UiLanguage::Ar => &t.ar,
        UiLanguage::En => &t.en,
    }
}

#[component]
fn Review(
    entry: TaughtEntry,
    can_regenerate: bool,
    on_approve: EventHandler<TaughtEntry>,
    on_save: EventHandler<TaughtEntry>,
    on_invalid: EventHandler<String>,
    on_regenerate: EventHandler<()>,
    on_discard: EventHandler<TaughtEntry>,
) -> Element {
    let ui = use_context::<Ui>();
    let l = ui.settings.read().ui_language;
    let tr = move |m: T| tr(l, m);
    let mut work = use_signal(|| entry.clone());
    let mut editing = use_signal(|| false);
    let g = work.read().entry.clone();
    let w = work.read().clone();
    let original = entry.clone();

    // Applies the working copy through the core's checks.
    let checked = move || learn::edit(&original, |e| *e = work());
    let checked_too = checked.clone();

    rsx! {
        crate::ui::DivCard { class: "teach-review",
            div { class: "row",
                h2 { bdi { lang: g.lang.code(), class: "latin", "{g.word}" } }
                if w.is_approved() {
                    span { class: "pill taught", "{tr(T::TaughtByYou)}" }
                } else {
                    span { class: "pill draft", "{tr(T::Draft)}" }
                }
                if g.lang == Lang::Jv || !w.is_approved() {
                    span { class: "pill", "{tr(T::Unverified)}" }
                }
            }
            p { class: "muted",
                bdi { class: "latin", dir: "ltr", "{breakdown(&g)}" }
            }

            if !w.doubts.is_empty() {
                div { class: "doubts", role: "note",
                    strong { "{tr(T::Doubts)}" }
                    ul {
                        for (i, d) in w.doubts.iter().enumerate() {
                            li { key: "{i}", "{voice(d, l)}" }
                        }
                    }
                }
            }

            if editing() {
                div { class: "field",
                    label { "{tr(T::Lemma)}" }
                    input {
                        class: "latin",
                        dir: "ltr",
                        value: "{g.lemma}",
                        oninput: move |e| work.with_mut(|w| w.entry.lemma = e.value()),
                    }
                }
                for (i, s) in g.senses.iter().enumerate() {
                    fieldset { key: "{i}", class: "sense-edit",
                        legend { "{i + 1}" }
                        for (voice_name, get) in VOICES {
                            label { key: "{voice_name}",
                                span { class: "label", "{voice_name}" }
                                input {
                                    dir: "auto",
                                    value: "{get(&s.gloss)}",
                                    oninput: move |e| work.with_mut(|w| set_voice(&mut w.entry.senses[i].gloss, voice_name, e.value())),
                                }
                            }
                        }
                    }
                }
                for (i, ex) in g.examples.iter().enumerate() {
                    fieldset { key: "ex{i}", class: "sense-edit",
                        legend { "{tr(T::Example)} {i + 1}" }
                        input {
                            class: "latin",
                            dir: "ltr",
                            value: "{ex.text}",
                            oninput: move |e| work.with_mut(|w| w.entry.examples[i].text = e.value()),
                        }
                        for (voice_name, get) in VOICES {
                            label { key: "{voice_name}",
                                span { class: "label", "{voice_name}" }
                                input {
                                    dir: "auto",
                                    value: "{get(&ex.translation)}",
                                    oninput: move |e| work.with_mut(|w| set_voice(&mut w.entry.examples[i].translation, voice_name, e.value())),
                                }
                            }
                        }
                    }
                }
            } else {
                ol { class: "senses",
                    for (i, s) in g.senses.iter().enumerate() {
                        li { key: "{i}",
                            span { class: "pill latin", "{pos_name(s.pos)}" }
                            " {voice(&s.gloss, l)}"
                            if l != UiLanguage::En {
                                span { class: "muted", dir: "ltr", " · {s.gloss.en}" }
                            }
                        }
                    }
                }
                ul { class: "examples",
                    for (i, ex) in g.examples.iter().enumerate() {
                        li { key: "{i}",
                            p { class: "latin", dir: "ltr", lang: g.lang.code(), "{ex.text}" }
                            p { class: "muted", "{voice(&ex.translation, l)}" }
                        }
                    }
                }
                if let Some(n) = &g.note {
                    p { class: "note", "{voice(n, l)}" }
                }
                if let Some(ff) = &g.false_friend {
                    p { class: "false-friend", role: "note", "⚠ {voice(ff, l)}" }
                }
                if let Some(b) = &w.bridge {
                    div { class: "bridge-card",
                        span { class: "ar", lang: "ar", "{b.arabic}" }
                        if let Some(root) = &b.root {
                            span { class: "muted", lang: "ar", " ({root})" }
                        }
                        if let Some(note) = &b.shift_note {
                            p { "{voice(note, l)}" }
                        }
                    }
                }
                if let Some(lv) = &w.levels {
                    p { class: "latin", dir: "ltr",
                        "{lv.level}"
                        if let Some(k) = &lv.ngoko { " · ngoko: {k}" }
                        if let Some(k) = &lv.krama { " · krama: {k}" }
                        if let Some(k) = &lv.krama_inggil { " · krama inggil: {k}" }
                    }
                }
            }

            details { class: "sources",
                summary { "{tr(T::Sources)}" }
                ul {
                    for (i, s) in w.sources.iter().enumerate() {
                        li { key: "{i}",
                            span { class: "pill", "{source_name(s.kind, l)}" }
                            if let Some(url) = &s.url {
                                " "
                                a { href: "{url}", target: "_blank", rel: "noopener noreferrer", dir: "ltr",
                                    {s.title.clone().unwrap_or_else(|| url.clone())}
                                }
                            }
                            pre { dir: "auto", "{s.excerpt}" }
                        }
                    }
                }
                if !w.engine.is_empty() {
                    p { class: "muted small", dir: "ltr", "{w.engine} · {g.provenance.prompt}" }
                }
            }

            div { class: "row",
                button {
                    class: "primary",
                    onclick: move |_| match checked() {
                        Ok(e) => on_approve.call(e),
                        Err(why) => on_invalid.call(why),
                    },
                    "{tr(T::Approve)}"
                }
                if editing() {
                    button {
                        onclick: move |_| match checked_too() {
                            Ok(e) => {
                                editing.set(false);
                                on_save.call(e);
                            }
                            Err(why) => on_invalid.call(why),
                        },
                        "{tr(T::SaveEdits)}"
                    }
                } else {
                    button { onclick: move |_| editing.set(true), "{tr(T::Edit)}" }
                }
                if can_regenerate {
                    button { onclick: move |_| on_regenerate.call(()), "👎 {tr(T::Another)}" }
                }
                button {
                    class: "quiet",
                    onclick: {
                        let entry = entry.clone();
                        move |_| on_discard.call(entry.clone())
                    },
                    "{tr(T::Discard)}"
                }
            }
        }
    }
}

type VoiceGet = fn(&Trilingual) -> &str;
const VOICES: [(&str, VoiceGet); 3] = [("arz", |t| &t.arz), ("ar", |t| &t.ar), ("en", |t| &t.en)];

fn set_voice(t: &mut Trilingual, voice: &str, value: String) {
    match voice {
        "arz" => t.arz = value,
        "ar" => t.ar = value,
        _ => t.en = value,
    }
}

fn breakdown(g: &kabar_protocol::content::GlossEntry) -> String {
    let mut s = String::new();
    if g.lemma != g.word {
        s.push_str(&format!("{} → {}", g.word, g.lemma));
    }
    if !g.affixes.is_empty() {
        if !s.is_empty() {
            s.push_str(" · ");
        }
        s.push_str(&g.affixes.join(" + "));
    }
    s
}

fn pos_name(p: kabar_protocol::content::Pos) -> String {
    serde_json::to_value(p).ok().and_then(|v| v.as_str().map(|s| s.replace('_', " "))).unwrap_or_default()
}

#[component]
fn MyDictionary(entries: Vec<TaughtEntry>, on_open: EventHandler<TaughtEntry>) -> Element {
    let ui = use_context::<Ui>();
    let l = ui.settings.read().ui_language;
    if entries.is_empty() {
        return rsx! {};
    }
    let (drafts, approved): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| !e.is_approved());
    rsx! {
        crate::ui::DivCard {
            h2 { "{tr(l, T::MyDictionary)}" }
            if !drafts.is_empty() {
                h3 { "{tr(l, T::ToReview)}" }
                WordList { entries: drafts, on_open }
            }
            if !approved.is_empty() {
                h3 { "{tr(l, T::TaughtByYou)}" }
                WordList { entries: approved, on_open }
            }
        }
    }
}

#[component]
fn WordList(entries: Vec<TaughtEntry>, on_open: EventHandler<TaughtEntry>) -> Element {
    let ui = use_context::<Ui>();
    let l = ui.settings.read().ui_language;
    rsx! {
        ul { class: "taught-list",
            for e in entries {
                li { key: "{e.row_id()}",
                    button {
                        class: "quiet",
                        onclick: {
                            let e = e.clone();
                            move |_| on_open.call(e.clone())
                        },
                        bdi { class: "latin", lang: e.entry.lang.code(), "{e.entry.word}" }
                        " "
                        span { class: "muted",
                            {e.entry.senses.first().map(|s| voice(&s.gloss, l).to_string()).unwrap_or_default()}
                        }
                    }
                }
            }
        }
    }
}

fn teach_error(e: &TeachError) -> T {
    match e {
        TeachError::EmptyWord => T::NeedWord,
        TeachError::WordTooLong => T::TooLong,
        TeachError::NothingToLearnFrom => T::NeedSource,
        TeachError::LinkUnreadable { .. } => T::LinkFailed,
        TeachError::NotInWiktionary => T::NotInWiktionary,
    }
}

fn ai_error(e: &AiError) -> T {
    match e.friendly() {
        FriendlyReason::ResetsLater => T::ResetsLater,
        FriendlyReason::Unreachable => T::Unreachable,
        FriendlyReason::NotAvailable => T::NotConnected,
        FriendlyReason::TryAgain => T::TryAgain,
    }
}

fn source_name(k: SourceKind, l: UiLanguage) -> &'static str {
    tr(
        l,
        match k {
            SourceKind::Pasted => T::ModePaste,
            SourceKind::Link => T::ModeLink,
            SourceKind::Screenshot => T::Screenshot,
            SourceKind::Wiktionary => T::ModeWiktionary,
        },
    )
}

/// This screen's strings. A typed catalog like `i18n::Msg`: a missing
/// translation is a compile error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    Title,
    Intro,
    Word,
    Context,
    Source,
    ModePaste,
    ModeLink,
    ModeWiktionary,
    Screenshot,
    SearchGoogle,
    PastePlaceholder,
    LinkHint,
    WiktionaryHint,
    ScreenshotHint,
    Teach,
    Fetching,
    Thinking,
    TaughtByYou,
    Draft,
    Unverified,
    Doubts,
    Lemma,
    Example,
    Sources,
    Approve,
    Edit,
    SaveEdits,
    Another,
    Discard,
    MyDictionary,
    ToReview,
    NeedWord,
    TooLong,
    NeedSource,
    LinkFailed,
    NotInWiktionary,
    WiktionaryFailed,
    ResetsLater,
    Unreachable,
    NotConnected,
    NoCloudInBuild,
    SignIn,
    TryAgain,
}

pub fn tr(lang: UiLanguage, msg: T) -> &'static str {
    use T::*;
    use UiLanguage::*;
    match (msg, lang) {
        (Title, ArEg) => "علّم \u{2068}Kabar\u{2069} كلمة جديدة",
        (Title, Ar) => "علّم \u{2068}Kabar\u{2069} كلمة جديدة",
        (Title, En) => "Teach Kabar a word",
        (Intro, ArEg) => {
            "لقيت كلمة \u{2068}Kabar\u{2069} مش عارفها؟ دوّر عليها، والصق الشرح أو اللينك هنا. \u{2068}Kabar\u{2069} هيكتبها بأسلوبه، وإنت تراجعها قبل ما تتحفظ."
        }
        (Intro, Ar) => {
            "وجدت كلمة لا يعرفها \u{2068}Kabar\u{2069}؟ ابحث عنها، والصق الشرح أو الرابط هنا. سيكتبها \u{2068}Kabar\u{2069} بأسلوبه، وتراجعها أنت قبل حفظها."
        }
        (Intro, En) => {
            "Found a word Kabar doesn't know? Look it up and paste the explanation or the link here. Kabar writes it up in its own style, and you check it before it's saved."
        }
        (Word, ArEg) => "الكلمة أو العبارة",
        (Word, Ar) => "الكلمة أو العبارة",
        (Word, En) => "Word or phrase",
        (Context, ArEg) => "الجملة اللي قابلتها فيها (اختياري)",
        (Context, Ar) => "الجملة التي وردت فيها (اختياري)",
        (Context, En) => "The sentence you met it in (optional)",
        (Source, ArEg) => "الشرح اللي لقيته",
        (Source, Ar) => "الشرح الذي وجدته",
        (Source, En) => "What you found",
        (ModePaste, ArEg) => "لزق نص",
        (ModePaste, Ar) => "لصق نص",
        (ModePaste, En) => "Paste text",
        (ModeLink, ArEg | Ar) => "رابط",
        (ModeLink, En) => "Link",
        (ModeWiktionary, ArEg) => "ويكاموس (ببلاش)",
        (ModeWiktionary, Ar) => "ويكاموس (مجاني)",
        (ModeWiktionary, En) => "Wiktionary (free)",
        (Screenshot, ArEg) => "سكرين شوت",
        (Screenshot, Ar) => "لقطة شاشة",
        (Screenshot, En) => "Screenshot",
        (SearchGoogle, ArEg) => "دوّر في جوجل",
        (SearchGoogle, Ar) => "ابحث في جوجل",
        (SearchGoogle, En) => "Search Google",
        (PastePlaceholder, ArEg) => "الصق هنا الشرح من جوجل أو من أي قاموس",
        (PastePlaceholder, Ar) => "الصق هنا الشرح من جوجل أو من أي قاموس",
        (PastePlaceholder, En) => "Paste the explanation from Google or any dictionary",
        (LinkHint, ArEg) => "لو الموقع مارضيش يفتح جوه \u{2068}Kabar\u{2069}، انسخ الجزء المهم والصقه.",
        (LinkHint, Ar) => "إذا رفض الموقع الفتح داخل \u{2068}Kabar\u{2069}، انسخ الجزء المهم والصقه.",
        (LinkHint, En) => "If the site won't open inside Kabar, copy the part that matters and paste it.",
        (WiktionaryHint, ArEg) => {
            "\u{2068}Kabar\u{2069} هيجيب تعريف الكلمة من ويكاموس الإنجليزي، ببلاش ومن غير حساب."
        }
        (WiktionaryHint, Ar) => {
            "سيجلب \u{2068}Kabar\u{2069} تعريف الكلمة من ويكاموس الإنجليزي مجانًا ودون حساب."
        }
        (WiktionaryHint, En) => "Kabar fetches the word's entry from English Wiktionary: free, no account.",
        (ScreenshotHint, ArEg) => {
            "عندك سكرين شوت للشرح؟ قراية الصور على الجهاز جاية مع القارئ؛ لحد كده انسخ النص منها."
        }
        (ScreenshotHint, Ar) => {
            "لديك لقطة شاشة للشرح؟ قراءة الصور على الجهاز تأتي مع القارئ؛ حتى ذلك الحين انسخ النص منها."
        }
        (ScreenshotHint, En) => {
            "Got a screenshot of the explanation? On-device reading comes with the Reader; until then, copy its text."
        }
        (Teach, ArEg) => "علّم \u{2068}Kabar\u{2069}",
        (Teach, Ar) => "علّم \u{2068}Kabar\u{2069}",
        (Teach, En) => "Teach Kabar",
        (Fetching, ArEg) => "بيقرا الصفحة…",
        (Fetching, Ar) => "يقرأ الصفحة…",
        (Fetching, En) => "Reading the page…",
        (Thinking, ArEg) => "\u{2068}Kabar\u{2069} بيكتب الشرح بأسلوبه…",
        (Thinking, Ar) => "يكتب \u{2068}Kabar\u{2069} الشرح بأسلوبه…",
        (Thinking, En) => "Kabar is writing it up…",
        (TaughtByYou, ArEg) => "إنت اللي علّمته",
        (TaughtByYou, Ar) => "من تعليمك",
        (TaughtByYou, En) => "Taught by you",
        (Draft, ArEg) => "مسودة",
        (Draft, Ar) => "مسودة",
        (Draft, En) => "Draft",
        (Unverified, ArEg) => "لسه متراجعش",
        (Unverified, Ar) => "غير مُراجَع",
        (Unverified, En) => "Unverified",
        (Doubts, ArEg) => "حاجات تستاهل تبص عليها:",
        (Doubts, Ar) => "نقاط تستحق المراجعة:",
        (Doubts, En) => "Worth a look:",
        (Lemma, ArEg | Ar) => "أصل الكلمة في القاموس",
        (Lemma, En) => "Dictionary form",
        (Example, ArEg | Ar) => "مثال",
        (Example, En) => "Example",
        (Sources, ArEg) => "المصادر",
        (Sources, Ar) => "المصادر",
        (Sources, En) => "Sources",
        (Approve, ArEg) => "تمام، احفظها",
        (Approve, Ar) => "موافق، احفظها",
        (Approve, En) => "Looks right, save it",
        (Edit, ArEg) => "عدّل",
        (Edit, Ar) => "عدّل",
        (Edit, En) => "Edit",
        (SaveEdits, ArEg) => "احفظ التعديل",
        (SaveEdits, Ar) => "احفظ التعديل",
        (SaveEdits, En) => "Save edits",
        (Another, ArEg) => "جرّب نسخة تانية",
        (Another, Ar) => "جرّب نسخة أخرى",
        (Another, En) => "Try another version",
        (Discard, ArEg) => "امسحها",
        (Discard, Ar) => "احذفها",
        (Discard, En) => "Discard",
        (MyDictionary, ArEg) => "قاموسي",
        (MyDictionary, Ar) => "قاموسي",
        (MyDictionary, En) => "My dictionary",
        (ToReview, ArEg) => "مستنية مراجعتك",
        (ToReview, Ar) => "بانتظار مراجعتك",
        (ToReview, En) => "Waiting for your review",
        (NeedWord, ArEg) => "اكتب الكلمة الأول.",
        (NeedWord, Ar) => "اكتب الكلمة أولًا.",
        (NeedWord, En) => "Type the word first.",
        (TooLong, ArEg) => "دي طويلة شوية؛ كلمة أو عبارة قصيرة بس.",
        (TooLong, Ar) => "هذا طويل قليلًا؛ كلمة أو عبارة قصيرة فقط.",
        (TooLong, En) => "That's a bit long; a word or short phrase works best.",
        (NeedSource, ArEg) => "الصق الشرح اللي لقيته، و \u{2068}Kabar\u{2069} يكمل.",
        (NeedSource, Ar) => "الصق الشرح الذي وجدته، وسيكمل \u{2068}Kabar\u{2069}.",
        (NeedSource, En) => "Paste what you found and Kabar takes it from there.",
        (LinkFailed, ArEg) => {
            "الموقع ده مش راضي يفتح جوه \u{2068}Kabar\u{2069}. انسخ الجزء المهم والصقه في «لزق نص»."
        }
        (LinkFailed, Ar) => {
            "هذا الموقع لا يفتح داخل \u{2068}Kabar\u{2069}. انسخ الجزء المهم والصقه في «لصق نص»."
        }
        (LinkFailed, En) => {
            "This site won't open inside Kabar. Copy the part that matters into “Paste text”."
        }
        (NotInWiktionary, ArEg) => "ويكاموس مفيهوش الكلمة دي باللغة دي. جرّب جوجل.",
        (NotInWiktionary, Ar) => "لا يحتوي ويكاموس هذه الكلمة بهذه اللغة. جرّب جوجل.",
        (NotInWiktionary, En) => "Wiktionary doesn't have this word in this language. Try Google.",
        (WiktionaryFailed, ArEg) => "مقدرناش نوصل لويكاموس دلوقتي. جرّب جوجل أو الصق الشرح.",
        (WiktionaryFailed, Ar) => "تعذّر الوصول إلى ويكاموس الآن. جرّب جوجل أو الصق الشرح.",
        (WiktionaryFailed, En) => {
            "Couldn't reach Wiktionary right now. Try Google, or paste the explanation."
        }
        (ResetsLater, ArEg) => "الحصة المجانية خلصت النهارده. جرّب تاني بكرة.",
        (ResetsLater, Ar) => "نفدت الحصة المجانية اليوم. جرّب مجددًا غدًا.",
        (ResetsLater, En) => "Today's free quota is used up. Try again tomorrow.",
        (Unreachable, ArEg) => "\u{2068}Kabar\u{2069} مش قادر يوصل للنت دلوقتي. أول ما ترجع أونلاين جرّب تاني.",
        (Unreachable, Ar) => "لا يستطيع \u{2068}Kabar\u{2069} الاتصال الآن. جرّب مجددًا عند عودة الاتصال.",
        (Unreachable, En) => "Kabar can't reach its helpers right now. Try again once you're online.",
        (NotConnected, ArEg) => {
            "\u{2068}Kabar\u{2069} بيكتب الشرح عن طريق \u{2068}Kabar Cloud\u{2069}، وإنت لسه مش مسجّل دخول فيه. سجّل دخول مرة واحدة وارجع جرّب."
        }
        (NotConnected, Ar) => {
            "يكتب \u{2068}Kabar\u{2069} الشرح عبر \u{2068}Kabar Cloud\u{2069}، ولم تسجّل الدخول إليه بعد. سجّل الدخول مرة واحدة ثم عُد وجرّب."
        }
        (NotConnected, En) => {
            "Kabar writes this up through Kabar Cloud, and you're not signed in to it yet. Sign in once, then come back and try again."
        }
        (NoCloudInBuild, ArEg) => {
            "النسخة دي من \u{2068}Kabar\u{2069} لسه مش متوصلة بـ \u{2068}Kabar Cloud\u{2069}، فمش هتقدر تكتب الشرح. النت عندك سليم؛ أول تحديث فيه السيرفر هيشغّلها."
        }
        (NoCloudInBuild, Ar) => {
            "هذه النسخة من \u{2068}Kabar\u{2069} غير مربوطة بـ \u{2068}Kabar Cloud\u{2069} بعد، لذا لا يمكنها كتابة الشرح. اتصالك سليم؛ سيعمل هذا مع أول تحديث يربط الخادم."
        }
        (NoCloudInBuild, En) => {
            "This build of Kabar isn't connected to Kabar Cloud yet, so it can't write this up. Your internet is fine; the first update with the server switches it on."
        }
        (SignIn, ArEg) => "سجّل دخول",
        (SignIn, Ar) => "سجّل الدخول",
        (SignIn, En) => "Sign in",
        (TryAgain, ArEg) => "حصلت لخبطة في الرد. جرّب تاني.",
        (TryAgain, Ar) => "حدث خلل في الرد. حاول مرة أخرى.",
        (TryAgain, En) => "Something went wrong with the answer. Try again.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_problem_has_a_message() {
        for e in [
            TeachError::EmptyWord,
            TeachError::WordTooLong,
            TeachError::NothingToLearnFrom,
            TeachError::LinkUnreadable { url: "x".into() },
            TeachError::NotInWiktionary,
        ] {
            for l in UiLanguage::ALL {
                assert!(!tr(l, teach_error(&e)).is_empty());
            }
        }
    }
}
