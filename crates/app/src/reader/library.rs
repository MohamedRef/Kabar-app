//! The Library on the Reader's home (redesign 2026-10-29, K3): every saved
//! text with a search, the words and phrases saved from the Reader, the
//! words taught to Kabar, a way into the Arabic Bridge, and the offline
//! packs. Everything here opens what it shows: a text opens in the Reader,
//! a saved word opens in the text it was saved from, the Bridge row opens
//! the Bridge deck and the taught row opens Teach Kabar.

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::reader::WordStatus;

use super::glossary::WordLang;
use super::{Panel, ReaderState, glossary};
use crate::embedded::{Css, css};
use crate::play::{self, PlayNav, Route};
use crate::store::{SavedText, SavedWord, TextKind};
use crate::{Db, Screen, Ui};

pub const CSS: Css = css!("assets/library.css");

/// Texts the Library lists (newest first); older ones are still searchable
/// once a newer one is deleted, and still sync.
pub const TEXTS: usize = 300;
/// Saved words the Library lists.
const WORDS: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Texts,
    Words,
    Downloads,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum L {
    Title,
    Intro,
    Search,
    Texts,
    Words,
    Downloads,
    NoTexts,
    NoMatch,
    NoWords,
    BridgeTag,
    BridgeTitle,
    BridgeCount,
    Taught,
    TaughtNote,
    OpenInText,
    TextGone,
    Pasted,
    Shared,
    Screenshot,
    Web,
    Phrase,
    InReview,
}

fn lt(lang: UiLanguage, l: L) -> &'static str {
    use L::*;
    use UiLanguage::*;
    match (l, lang) {
        (Title, ArEg) => "مكتبتك",
        (Title, Ar) => "مكتبتك",
        (Title, En) => "Your library",
        (Intro, ArEg) => "النصوص اللي قريتها، والكلمات اللي حفظتها، والتنزيلات.",
        (Intro, Ar) => "النصوص التي قرأتها، والكلمات التي حفظتها، والتنزيلات.",
        (Intro, En) => "Texts you've read, words you've saved, and downloads.",
        (Search, ArEg) => "دوّر في نصوصك وكلماتك…",
        (Search, Ar) => "ابحث في نصوصك وكلماتك…",
        (Search, En) => "Search your texts and words…",
        (Texts, ArEg | Ar) => "النصوص",
        (Texts, En) => "Texts",
        (Words, ArEg | Ar) => "الكلمات",
        (Words, En) => "Words",
        (Downloads, ArEg | Ar) => "التنزيلات",
        (Downloads, En) => "Downloads",
        (NoTexts, ArEg) => "لسه مافيش نصوص هنا. الصق نص فوق أو افتح سكرين شوت، وهيتحفظ هنا لوحده.",
        (NoTexts, Ar) => "لا نصوص بعد. الصق نصًا في الأعلى أو افتح لقطة شاشة، وستُحفظ هنا تلقائيًا.",
        (NoTexts, En) => "No texts yet. Paste one above or open a screenshot; it's saved here by itself.",
        (NoMatch, ArEg) => "مالقيناش حاجة بالكلمة دي.",
        (NoMatch, Ar) => "لا نتائج لهذا البحث.",
        (NoMatch, En) => "Nothing matches that.",
        (NoWords, ArEg) => {
            "الكلمات اللي تضيفها للمراجعة من القارئ هتظهر هنا، كل كلمة مع الجملة اللي قابلتها فيها."
        }
        (NoWords, Ar) => "الكلمات التي تضيفها إلى المراجعة من القارئ ستظهر هنا، كل كلمة مع جملتها.",
        (NoWords, En) => "Words you add to review from the Reader show up here, each with its sentence.",
        (BridgeTag, ArEg | Ar) => "من العربي للإندونيسي",
        (BridgeTag, En) => "From Arabic to Indonesian",
        (BridgeTitle, ArEg | Ar) => "الجسر العربي",
        (BridgeTitle, En) => "Arabic Bridge",
        (BridgeCount, ArEg) => "كلمة إندونيسية أصلها عربي، ومعناها تعرفه من دلوقتي",
        (BridgeCount, Ar) => "كلمة إندونيسية أصلها عربي وتعرف معناها بالفعل",
        (BridgeCount, En) => "Indonesian words from Arabic you already understand",
        (Taught, ArEg) => "كلمات علّمتها لـ Kabar",
        (Taught, Ar) => "كلمات علّمتها لـ Kabar",
        (Taught, En) => "Words you taught Kabar",
        (TaughtNote, ArEg) => "راجعها أو علّم كلمة جديدة",
        (TaughtNote, Ar) => "راجعها أو علّم كلمة جديدة",
        (TaughtNote, En) => "Check them or teach a new one",
        (OpenInText, ArEg) => "افتحها في النص",
        (OpenInText, Ar) => "افتحها في النص",
        (OpenInText, En) => "Open in its text",
        (TextGone, ArEg) => "النص اللي اتحفظت منه اتمسح.",
        (TextGone, Ar) => "حُذف النص الذي حُفظت منه.",
        (TextGone, En) => "The text it came from was deleted.",
        (Pasted, ArEg) => "نص لصقته",
        (Pasted, Ar) => "نص ملصوق",
        (Pasted, En) => "Pasted text",
        (Shared, ArEg) => "اتبعت لـ Kabar",
        (Shared, Ar) => "مُرسل إلى Kabar",
        (Shared, En) => "Shared to Kabar",
        (Screenshot, ArEg) => "سكرين شوت",
        (Screenshot, Ar) => "لقطة شاشة",
        (Screenshot, En) => "Screenshot",
        (Web, ArEg) => "صفحة من النت",
        (Web, Ar) => "صفحة ويب",
        (Web, En) => "Web page",
        (Phrase, ArEg | Ar) => "عبارة",
        (Phrase, En) => "Phrase",
        (InReview, ArEg) => "في المراجعة",
        (InReview, Ar) => "في المراجعة",
        (InReview, En) => "In review",
    }
}

fn kind_label(kind: TextKind) -> L {
    match kind {
        TextKind::Pasted => L::Pasted,
        TextKind::Shared => L::Shared,
        TextKind::Screenshot => L::Screenshot,
        TextKind::Web => L::Web,
    }
}

/// Whether `query` (already lowercase) is in any of `fields`.
pub(crate) fn matches(query: &str, fields: &[&str]) -> bool {
    query.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(query))
}

/// `id:baca` → (`baca`, Indonesian); `phrase:…` → None.
fn word_of(lexeme: &str) -> Option<(&str, WordLang)> {
    match lexeme.split_once(':') {
        Some(("phrase", _)) => None,
        Some(("jv", w)) => Some((w, WordLang::Javanese)),
        Some((_, w)) => Some((w, WordLang::Indonesian)),
        None => Some((lexeme, WordLang::Indonesian)),
    }
}

/// Opens a saved text with the saved word's card (or the saved phrase)
/// showing, so the learner lands on the word in its sentence.
fn open_at(db: &Db, st: ReaderState, text: SavedText, lexeme: &str) {
    st.load(db, text);
    let Some(open) = st.open.peek().clone() else { return };
    let panel = if lexeme.starts_with("phrase:") {
        let phrase = lexeme.trim_start_matches("phrase:");
        st.highlights.peek().iter().find(|h| h.phrase.to_lowercase() == phrase).map(|h| Panel::Phrase {
            tokens: open.doc.tokens_in(h.start, h.end).collect(),
            save_first: false,
        })
    } else {
        open.doc
            .tokens
            .iter()
            .position(|t| t.word.as_ref().and_then(|w| w.key.as_deref()) == Some(lexeme))
            .map(Panel::Word)
    };
    let mut p = st.panel;
    p.set(panel);
}

#[component]
pub fn Library(recent: Signal<Vec<SavedText>>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let lang = ui.lang();
    let mut tab = use_signal(|| Tab::Texts);
    let mut query = use_signal(String::new);
    let words = use_signal(|| db.0.borrow().saved_words(WORDS).unwrap_or_default());
    let taught = use_signal(|| db.0.borrow().taught_entries().map(|t| t.len()).unwrap_or(0));
    let q = query.read().trim().to_lowercase();
    let current = *tab.read();

    rsx! {
        style { {CSS.text()} }
        section { class: "library", aria_labelledby: "library-title",
            div { class: "lib-head",
                h2 { id: "library-title", "{lt(lang, L::Title)}" }
                p { class: "sub", "{lt(lang, L::Intro)}" }
            }
            BridgeEntry {}
            if current != Tab::Downloads {
                label { class: "search-field",
                    {crate::icons::search()}
                    input {
                        r#type: "search",
                        dir: "auto",
                        placeholder: "{lt(lang, L::Search)}",
                        aria_label: "{lt(lang, L::Search)}",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                }
            }
            div {
                class: "segmented lib-tabs",
                role: "radiogroup",
                aria_label: "{lt(lang, L::Title)}",
                for (t , label , count) in [
                    (Tab::Texts, L::Texts, Some(recent.read().len())),
                    (Tab::Words, L::Words, Some(words.read().len())),
                    (Tab::Downloads, L::Downloads, None),
                ]
                {
                    button {
                        key: "{label:?}",
                        role: "radio",
                        aria_checked: if t == current { "true" } else { "false" },
                        onclick: move |_| tab.set(t),
                        "{lt(lang, label)}"
                        if let Some(n) = count.filter(|n| *n > 0) {
                            span { class: "ltr-count lib-count", "{ui.num(n as u64)}" }
                        }
                    }
                }
            }
            match current {
                Tab::Texts => rsx! { TextList { recent, q } },
                Tab::Words => rsx! { WordList { words: words.read().clone(), taught: *taught.read(), q } },
                Tab::Downloads => rsx! { crate::packs::settings::OfflinePacks {} },
            }
        }
    }
}

/// The Arabic Bridge, one tap from the Reader: opens the deck of
/// Indonesian words an Arabic speaker already understands.
#[component]
fn BridgeEntry() -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<PlayNav>();
    let lang = ui.lang();
    let count = play::content::bridge().as_ref().ok().map(kabar_core::games::bridge::known_count);
    rsx! {
        crate::ui::DivCard { class: "grouped bridge-entry",
            button {
                class: "activity-row",
                onclick: move |_| play::open(ui, nav, Route::Deck),
                span { class: "row-icon gold", aria_hidden: "true", {crate::icons::game(Route::Deck)} }
                span { class: "row-copy",
                    strong { "{lt(lang, L::BridgeTitle)}" }
                    small {
                        if let Some(n) = count {
                            span { class: "ltr-count bridge-entry-n", "{ui.num(n as u64)}" }
                            " {lt(lang, L::BridgeCount)}"
                        } else {
                            "{lt(lang, L::BridgeTag)}"
                        }
                    }
                }
                span { class: "row-end",
                    span { class: "bridge-entry-ar", lang: "ar", "خبر" }
                    span { class: "chev" }
                }
            }
        }
    }
}

#[component]
fn TextList(recent: Signal<Vec<SavedText>>, q: String) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let st = use_context::<ReaderState>();
    let lang = ui.lang();
    let pick = TEXTS_VIEW.pick(&db);
    let mut texts: Vec<SavedText> = recent
        .read()
        .iter()
        .filter(|t| matches(&q, &[&t.title, &t.body, t.source.as_deref().unwrap_or("")]))
        .cloned()
        .collect();
    crate::views::sort_by(&mut texts, pick.sort, |_| false, |t| t.title.clone());
    if recent.read().is_empty() {
        return rsx! {
            crate::ui::Card { element: crate::ui::CardElement::Paragraph, class: "flat lib-empty", "{lt(lang, L::NoTexts)}" }
        };
    }
    if texts.is_empty() {
        return rsx! {
            crate::ui::Card { element: crate::ui::CardElement::Paragraph, class: "flat lib-empty", role: "status", "{lt(lang, L::NoMatch)}" }
        };
    }
    rsx! {
        crate::views::ViewBar { of: TEXTS_VIEW }
        ul { class: "coll lib-texts", "data-layout": pick.layout.as_str(),
            for text in texts {
                li { key: "{text.id}",
                    button {
                        class: "coll-btn",
                        onclick: {
                            let db = db.clone();
                            let text = text.clone();
                            move |_| st.load(&db, text.clone())
                        },
                        span { class: "coll-lead", span { class: "row-icon", aria_hidden: "true", "{super::kind_icon(text.kind)}" } }
                        span { class: "coll-main",
                            strong { class: "coll-title", bdi { class: "lib-title", lang: "id", dir: "ltr", "{text.title}" } }
                            small { class: "coll-sub",
                                "{lt(lang, kind_label(text.kind))}"
                                if let Some(src) = text.source.as_deref() {
                                    " · "
                                    bdi { "{source_name(src)}" }
                                }
                            }
                        }
                        span { class: "chev", aria_hidden: "true" }
                    }
                    button {
                        class: "icon-button lib-delete coll-aside",
                        aria_label: "{ui.t(crate::i18n::Msg::Delete)}",
                        title: "{ui.t(crate::i18n::Msg::Delete)}",
                        onclick: {
                            let db = db.clone();
                            let id = text.id.clone();
                            move |_| {
                                let deleted = db.0.borrow_mut().delete_text(&id);
                                match deleted {
                                    Ok(()) => recent.set(db.0.borrow().recent_texts(TEXTS).unwrap_or_default()),
                                    Err(e) => st.write_failed("deleting a text", &e),
                                }
                            }
                        },
                        "×"
                    }
                }
            }
        }
    }
}

/// Saved texts: rows by default, newest first or by title.
const TEXTS_VIEW: crate::views::Collection = crate::views::Collection {
    key: "reader.texts",
    layout: crate::views::Layout::DEFAULT,
    sorts: &[crate::views::Sort::Recent, crate::views::Sort::Name],
};

/// A URL's host, an app's name as is.
fn source_name(src: &str) -> &str {
    let rest = src.split_once("://").map_or(src, |(_, r)| r);
    if rest.len() < src.len() {
        rest.split('/').next().unwrap_or(rest).trim_start_matches("www.")
    } else {
        src
    }
}

#[component]
fn WordList(words: Vec<SavedWord>, taught: usize, q: String) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let st = use_context::<ReaderState>();
    let voice = *st.voice.read();
    let shown: Vec<(SavedWord, Option<String>)> = words
        .into_iter()
        .map(|w| {
            let gloss = word_of(&w.lexeme).and_then(|(word, l)| {
                glossary()
                    .lookup(Some(l), &[word, &w.surface.to_lowercase()])
                    .map(|i| i.gloss.get(voice).to_string())
            });
            (w, gloss)
        })
        .filter(|(w, g)| matches(&q, &[&w.surface, &w.sentence, g.as_deref().unwrap_or("")]))
        .collect();
    rsx! {
        if taught > 0 {
            crate::ui::DivCard { class: "grouped lib-taught",
            button {
                class: "activity-row",
                onclick: move |_| {
                    let mut screen = ui.screen;
                    screen.set(Screen::Teach);
                },
                span { class: "row-icon", aria_hidden: "true", {crate::icons::nav(Screen::Teach)} }
                span { class: "row-copy",
                    strong { "{lt(lang, L::Taught)}" }
                    small { "{lt(lang, L::TaughtNote)}" }
                }
                span { class: "row-end",
                    span { class: "ltr-count", "{ui.num(taught as u64)}" }
                    span { class: "chev" }
                }
            }
            }
        }
        if shown.is_empty() {
            crate::ui::Card { element: crate::ui::CardElement::Paragraph, class: "flat lib-empty", if q.is_empty() { "{lt(lang, L::NoWords)}" } else { "{lt(lang, L::NoMatch)}" } }
        } else {
            crate::ui::Card { element: crate::ui::CardElement::List, class: "grouped lib-list",
                for (w , gloss) in shown {
                    li { key: "{w.lexeme}", SavedWordRow { w, gloss } }
                }
            }
        }
    }
}

/// One saved word: tap to see the sentence it was saved from and open it
/// there.
#[component]
fn SavedWordRow(w: SavedWord, gloss: Option<String>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let st = use_context::<ReaderState>();
    let lang = ui.lang();
    let phrase = w.lexeme.starts_with("phrase:");
    let learning = !phrase && st.status(&w.lexeme) == WordStatus::Learning;
    let text = w.text_id.as_deref().and_then(|id| db.0.borrow().text(id).ok().flatten());
    rsx! {
        details { class: "lib-word",
            summary {
                bdi { class: "lib-word-surface", lang: "id", dir: "ltr", "{w.surface}" }
                if let Some(g) = &gloss {
                    span { class: "lib-word-gloss", "{g}" }
                } else if phrase {
                    span { class: "lib-word-gloss muted", "{lt(lang, L::Phrase)}" }
                }
                if learning {
                    span { class: "status-chip accent", "{lt(lang, L::InReview)}" }
                }
            }
            div { class: "lib-word-body",
                p { class: "lib-sentence", lang: "id", dir: "ltr", bdi { "{w.sentence}" } }
                match text {
                    Some(text) => rsx! {
                        button {
                            class: "tonal",
                            onclick: {
                                let db = db.clone();
                                let lexeme = w.lexeme.clone();
                                move |_| open_at(&db, st, text.clone(), &lexeme)
                            },
                            "{lt(lang, L::OpenInText)}"
                        }
                    },
                    None => rsx! {
                        p { class: "muted small", "{lt(lang, L::TextGone)}" }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_is_case_blind_and_empty_matches_all() {
        assert!(matches("", &["anything"]));
        assert!(matches("pagi", &["Selamat Pagi"]));
        assert!(!matches("malam", &["Selamat Pagi", ""]));
    }

    #[test]
    fn saved_lexemes_name_their_language() {
        assert_eq!(word_of("id:baca"), Some(("baca", WordLang::Indonesian)));
        assert_eq!(word_of("jv:mangan"), Some(("mangan", WordLang::Javanese)));
        assert_eq!(word_of("phrase:apa kabar"), None);
    }

    #[test]
    fn sources_show_a_host_or_a_name() {
        assert_eq!(source_name("https://www.kompas.com/read/1"), "kompas.com");
        assert_eq!(source_name("WhatsApp"), "WhatsApp");
    }
}
