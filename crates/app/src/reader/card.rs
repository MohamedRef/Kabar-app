//! The word popover: a light peek (hover, pen hover, tap) and the full word
//! card (click, pen tap, long-press), both answered from local data.

use dioxus::prelude::*;
use kabar_core::grammar::word::parts_of_token;
use kabar_core::locale::UiLanguage;
use kabar_core::settings::PronMode;
use kabar_protocol::content::Lang;
use kabar_protocol::reader::WordStatus;
use kabar_protocol::srs::StudyLanguage;

use super::content::taught_info;
use super::doc::{Tag, Word};
use super::explain::ExplainCard;
use super::glossary::{Shift, SpeechLevel, WordInfo, WordLang};
use super::morph::MorphDetail;
use super::{Panel, Peek, ReaderState};
use crate::annotated::parts::{PartsLine, SayWord};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::platform::{self, SpeakError};
use crate::pron::{Pron, PronGuide};
use crate::sheet::Sheet;
use crate::teach::{TeachInbox, TeachSeed};
use crate::{Db, Screen, Ui};

fn tag_msg(tag: Tag) -> Msg {
    match tag {
        Tag::Indonesian => Msg::LangIndonesian,
        Tag::Javanese => Msg::LangJavanese,
        Tag::English => Msg::LangEnglish,
        Tag::Proper => Msg::LangProper,
        Tag::Unknown => Msg::LangUnknown,
    }
}

fn status_msg(s: WordStatus) -> Msg {
    match s {
        WordStatus::New => Msg::StatusNew,
        WordStatus::Learning => Msg::StatusLearning,
        WordStatus::Known => Msg::StatusKnown,
        WordStatus::Ignored => Msg::StatusIgnored,
    }
}

fn shift_msg(s: Shift) -> Msg {
    match s {
        Shift::Same => Msg::ShiftSame,
        Shift::Narrowed => Msg::ShiftNarrowed,
        Shift::Broadened => Msg::ShiftBroadened,
        Shift::Shifted => Msg::ShiftShifted,
        Shift::FalseFriend => Msg::FalseFriend,
    }
}

fn level_msg(l: SpeechLevel) -> Msg {
    match l {
        SpeechLevel::Ngoko => Msg::LevelNgoko,
        SpeechLevel::Madya => Msg::LevelMadya,
        SpeechLevel::Krama => Msg::LevelKrama,
        SpeechLevel::KramaInggil => Msg::LevelKramaInggil,
        SpeechLevel::KramaAndhap => Msg::LevelKramaAndhap,
    }
}

/// The floating peek next to a word.
#[component]
pub fn PeekView(peek: Peek) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let voice = *st.voice.read();
    let open = st.open.read();
    let Some(open) = open.as_ref() else { return rsx! {} };
    let Some(t) = open.doc.tokens.get(peek.i) else { return rsx! {} };
    let Some(w) = &t.word else { return rsx! {} };
    let surface = open.doc.text(t.start, t.end).to_string();
    let anchor = if peek.below { "inset-block-start" } else { "inset-block-end" };
    let style = format!("inset-inline-start: {}px; {anchor}: {}px", peek.start, peek.y);

    rsx! {
        div { class: "peek", role: "tooltip", style: "{style}",
            Headline { w: w.clone(), surface }
            match &w.shown() {
                Some(info) => rsx! {
                    p { class: "gloss", "{info.gloss.get(voice)}" }
                    if let Some(b) = &info.bridge {
                        p { class: if b.shift == Shift::FalseFriend { "bridge-line false" } else { "bridge-line" },
                            span { class: "ar", lang: "ar", "{b.arabic}" }
                            " · {ui.t(shift_msg(b.shift))}"
                        }
                    }
                },
                // The headline's English tag says it all.
                None if w.is_english_aside() => rsx! {},
                None => rsx! {
                    p { class: "muted small", "{ui.t(Msg::NoLocalGloss)}" }
                },
            }
        }
    }
}

/// Records the lookup once per word shown (see `review::note_evidence`).
#[component]
fn NoteLookup(language: StudyLanguage, words: Vec<String>) -> Element {
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    use_effect(use_reactive!(|(language, words)| {
        let signals: Vec<_> = words.into_iter().map(|w| (w, kabar_core::review::Signal::LookedUp)).collect();
        crate::review::note_evidence(&db, &coach, language, &signals);
    }));
    rsx! {}
}

/// Surface word, normalized form and language tag.
#[component]
fn Headline(w: Word, surface: String) -> Element {
    let ui = use_context::<Ui>();
    let normalized =
        w.analyzed.normalized.as_ref().filter(|n| n.full.to_lowercase() != surface.to_lowercase()).map(|n| {
            match &n.alt {
                Some(alt) => format!("{} / {alt}", n.full),
                None => n.full.clone(),
            }
        });
    // Say the full form (arp → arep); English asides keep their own sound.
    let javanese = w.tag == Tag::Javanese || w.info.as_ref().is_some_and(|i| i.lang == WordLang::Javanese);
    let pron = (w.tag != Tag::English).then(|| {
        let text = w.analyzed.normalized.as_ref().map_or(surface.clone(), |n| n.full.clone());
        (text, if javanese { "jv" } else { "id" })
    });
    rsx! {
        div { class: "headline",
            bdi { class: "latin surface", lang: "id", "{surface}" }
            if let Some((text, tag)) = pron {
                Pron { text, tag }
            }
            if let Some(n) = normalized {
                span { class: "muted", " → " }
                bdi { class: "latin", lang: "id", "{n}" }
            }
            span { class: "tag tag-{w.tag:?}", "{ui.t(tag_msg(w.tag))}" }
            if w.slang {
                span { class: "tag", "{ui.t(Msg::LangSlang)}" }
            }
            if w.taught_approved().is_some() {
                span { class: "tag taught", "{ui.t(Msg::TaughtByYou)}" }
            } else if w.info.as_ref().is_some_and(|i| !i.verified) {
                span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
            }
        }
    }
}

/// The word sheet: a side column on wide windows, a bottom sheet the
/// learner resizes on phones and tablets (`crate::sheet`).
#[component]
pub fn PanelView() -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let panel = st.panel.read().clone();
    let text_id = st.open.read().as_ref().map(|o| o.text.id.clone()).unwrap_or_default();
    rsx! {
        Sheet {
            label: ui.t(Msg::WordSheet).to_string(),
            side: true,
            on_close: move |_| {
                let mut p = st.panel;
                p.set(None);
            },
            div { class: "panel", aria_live: "polite",
                match panel {
                    Some(Panel::Word(i)) => rsx! {
                        WordCard { i }
                    },
                    Some(Panel::Phrase { tokens, save_first }) => rsx! {
                        PhraseCard { tokens, save_first }
                    },
                    Some(Panel::Explain(i)) => rsx! {
                        ExplainCard { key: "{text_id}-{i}", token: i }
                    },
                    None => rsx! {},
                }
            }
        }
    }
}

#[component]
fn WordCard(i: usize) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let st = use_context::<ReaderState>();
    let mut thanks = use_signal(|| false);
    let mut added = use_signal(|| false);
    let voice = *st.voice.read();
    let open = st.open.read().clone();
    let Some(open) = open else { return rsx! {} };
    let Some(t) = open.doc.tokens.get(i).cloned() else { return rsx! {} };
    let Some(w) = t.word.clone() else { return rsx! {} };
    let surface = open.doc.text(t.start, t.end).to_string();
    let sentence = open.doc.sentence_of(i).to_string();
    let key = w.key.clone();
    let status = key.as_deref().map(|k| st.status(k));
    let breakdown = w.breakdown().map(|_| (parts_of_token(&w.analyzed), w.class));
    let speak_text = w.analyzed.normalized.as_ref().map_or(surface.clone(), |n| n.full.clone());
    let taught = w.taught_approved().and_then(taught_info);
    let speech_tag = match w.tag {
        Tag::Javanese => "jv-ID",
        Tag::English => "en-US",
        _ => "id-ID",
    };
    let teach = use_context::<TeachInbox>();
    // Names and words with a draft waiting don't offer teaching.
    let teach_seed = (w.key.is_some() && w.taught_draft().is_none()).then(|| TeachSeed {
        lang: if w.info.as_ref().map(|i| i.lang) == Some(WordLang::Javanese) || w.tag == Tag::Javanese {
            Lang::Jv
        } else {
            Lang::Id
        },
        word: w.info.as_ref().map_or_else(|| w.analyzed.lookup_form(), |i| i.lemma.clone()),
        context: Some(sentence.clone()),
    });

    // Opening a deck word's card to read its meaning is evidence for its review.
    let lookup = (w.key.is_some() && w.tag != Tag::English).then(|| {
        let javanese = w.info.as_ref().map(|i| i.lang) == Some(WordLang::Javanese) || w.tag == Tag::Javanese;
        let words =
            [w.info.as_ref().map(|i| i.lemma.clone()), Some(w.analyzed.lookup_form()), Some(surface.clone())];
        (
            if javanese { StudyLanguage::Jv } else { StudyLanguage::Id },
            words.into_iter().flatten().collect::<Vec<_>>(),
        )
    });
    let word_tag = if w.tag == Tag::Javanese { "jv" } else { "id" }.to_string();

    rsx! {
        div { class: "word-card",
            Headline { w: w.clone(), surface: surface.clone() }
            if let Some((language, words)) = lookup {
                NoteLookup { language, words }
            }

            if let Some((parts, class)) = breakdown {
                div { class: "field",
                    span { class: "label", "{ui.t(Msg::Breakdown)}" }
                    div { class: "parts-row",
                        PartsLine { parts, class, labels: false, voice }
                        SayWord { text: speak_text.clone(), tag: word_tag, letters: false }
                    }
                }
            }
            MorphDetail { i }

            if let Some(t) = taught {
                div { class: "taught-block", aria_label: "{ui.t(Msg::TaughtByYou)}",
                    InfoView { info: t, voice }
                }
            }
            match w.info.clone() {
                Some(info) => rsx! {
                    if w.taught_approved().is_some() {
                        span { class: "label", "{ui.t(Msg::KabarDictionary)}" }
                    }
                    InfoView { info, voice }
                },
                None if w.is_english_aside() => rsx! {
                    p { class: "muted", "{ui.t(Msg::EnglishInChat)}" }
                },
                None if w.taught_approved().is_none() => rsx! {
                    p { class: "muted", "{ui.t(Msg::NoLocalGloss)}" }
                },
                None => rsx! {},
            }
            if w.taught_draft().is_some() {
                button {
                    class: "chip draft",
                    onclick: move |_| {
                        let mut screen = ui.screen;
                        screen.set(Screen::Teach);
                    },
                    "✎ {ui.t(Msg::DraftWaiting)}"
                }
            }

            super::grammar::WordGrammar { i }

            div { class: "field",
                span { class: "label", "{ui.t(Msg::InThisSentence)}" }
                p { class: "sentence", dir: "ltr", lang: "id", bdi { "{sentence}" } }
                Pron { text: sentence.clone(), tag: if w.tag == Tag::Javanese { "jv" } else { "id" } }
                button {
                    class: "ghost small",
                    onclick: move |_| {
                        let mut p = st.panel;
                        p.set(Some(Panel::Explain(i)));
                    },
                    "💡 {ui.t(Msg::ExplainWord)}"
                }
            }

            if ui.settings.read().pronunciation != PronMode::Off && w.tag != Tag::English {
                details { class: "pron-details",
                    summary { "{ui.t(Msg::PronHowToRead)}" }
                    PronGuide {}
                }
            }

            div { class: "row",
                if let Some(seed) = teach_seed {
                    button {
                        class: if w.has_gloss() { "" } else { "primary" },
                        title: "{ui.t(Msg::TeachKabarHint)}",
                        onclick: move |_| {
                            let mut slot = teach.0;
                            slot.set(Some(seed.clone()));
                            let mut screen = ui.screen;
                            screen.set(Screen::Teach);
                        },
                        "✍ {ui.t(Msg::TeachKabar)}"
                    }
                }
                button {
                    onclick: move |_| speak(&speak_text, speech_tag),
                    "🔊 {ui.t(Msg::Listen)}"
                }
                if let Some(k) = key.clone() {
                    button {
                        class: "primary",
                        disabled: *added.read(),
                        onclick: {
                            let db = db.clone();
                            let surface = surface.clone();
                            let sentence = sentence.clone();
                            move |_| {
                                let saved = super::add_to_review(
                                    &mut db.0.borrow_mut(),
                                    &k,
                                    &surface,
                                    &sentence,
                                    &open.text.id,
                                );
                                match saved {
                                    Ok(promoted) => {
                                        if promoted {
                                            let mut statuses = st.statuses;
                                            statuses.write().insert(k.clone(), WordStatus::Learning);
                                        }
                                        added.set(true);
                                    }
                                    // The button stays ready for another try.
                                    Err(e) => st.write_failed("adding to review", &e),
                                }
                            }
                        },
                        if *added.read() { "{ui.t(Msg::AddedToReview)}" } else { "{ui.t(Msg::AddToReview)}" }
                    }
                }
            }

            if let (Some(k), Some(current)) = (key.clone(), status) {
                div { class: "row status-row", role: "group",
                    for s in WordStatus::ALL {
                        button {
                            key: "{s.as_str()}",
                            class: "status-btn st-{s.as_str()}",
                            aria_pressed: if s == current { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                let k = k.clone();
                                move |_| st.set_status(&db, &k, s)
                            },
                            "{ui.t(status_msg(s))}"
                        }
                    }
                }
            }

            if let Some(k) = key {
                div { class: "row feedback",
                    if *thanks.read() {
                        span { class: "muted", "{ui.t(Msg::ThanksFeedback)}" }
                    } else {
                        for (up , label , icon) in [(true, Msg::Helpful, "👍"), (false, Msg::NotHelpful, "👎")] {
                            button {
                                key: "{up}",
                                class: "ghost",
                                aria_label: "{ui.t(label)}",
                                title: "{ui.t(label)}",
                                onclick: {
                                    let db = db.clone();
                                    let k = k.clone();
                                    move |_| {
                                        let sent = db.0.borrow_mut().add_feedback("gloss", &k, up);
                                        match sent {
                                            Ok(()) => thanks.set(true),
                                            Err(e) => st.write_failed("saving feedback", &e),
                                        }
                                    }
                                },
                                "{icon}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One gloss: meaning, note, example, speech levels and the Bridge card.
#[component]
pub(crate) fn InfoView(info: WordInfo, voice: UiLanguage) -> Element {
    let ui = use_context::<Ui>();
    let lang = if info.lang == WordLang::Javanese { "jv" } else { "id" };
    rsx! {
        div { class: "field",
            if let Some(pos) = &info.pos {
                span { class: "label", "{crate::i18n::part_of_speech(ui.lang(), pos)}" }
            }
            p { class: "gloss big", "{info.gloss.get(voice)}" }
            if voice != UiLanguage::En {
                p { class: "muted", lang: "en", dir: "ltr", "{info.gloss.en}" }
            }
        }
        if let Some(ind) = &info.indonesian {
            div { class: "field",
                span { class: "label", "{ui.t(Msg::InIndonesian)}" }
                bdi { class: "latin", lang: "id", "{ind}" }
                Pron { text: ind.clone(), tag: "id" }
            }
        }
        if let Some(note) = &info.note {
            p { class: "note", "{note.get(voice)}" }
        }
        if let Some((text, tr)) = &info.example {
            div { class: "field",
                span { class: "label", "{ui.t(Msg::Example)}" }
                p { class: "sentence", dir: "ltr", lang, "{text}" }
                Pron { text: text.clone(), tag: lang }
                p { class: "muted", "{tr.get(voice)}" }
            }
        }
        if let Some(l) = &info.levels {
            div { class: "field levels",
                span { class: "label",
                    "{ui.t(Msg::SpeechLevels)}"
                    if let Some(level) = l.level {
                        " · {ui.t(level_msg(level))}"
                    }
                }
                if l.rows().next().is_some() {
                    dl {
                        for (level , forms) in l.rows() {
                            dt { key: "{level:?}", class: if l.level == Some(level) { "current" } else { "" },
                                "{ui.t(level_msg(level))}"
                            }
                            dd { class: if l.level == Some(level) { "current" } else { "" },
                                bdi { class: "latin", lang: "jv", "{forms.join(\" / \")}" }
                            }
                        }
                    }
                }
                if let Some(note) = &l.note {
                    p { class: "note", "{note.get(voice)}" }
                }
            }
        }
        if let Some(b) = &info.bridge {
            div { class: if b.shift == Shift::FalseFriend { "bridge-card false" } else { "bridge-card" },
                span { class: "label", "{ui.t(Msg::BridgeTitle)}" }
                div { class: "bridge",
                    span { class: "bridge-word latin", lang, "{info.lemma}" }
                    span { class: "ar", lang: "ar", "{b.arabic}" }
                }
                if let Some(root) = &b.root {
                    p { class: "muted", lang: "ar", "{root}" }
                }
                p { strong { "{ui.t(shift_msg(b.shift))}" } }
                if let Some(note) = &b.note {
                    p { "{note.get(voice)}" }
                }
            }
        }
    }
}

#[component]
fn PhraseCard(tokens: Vec<usize>, save_first: bool) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let st = use_context::<ReaderState>();
    let voice = *st.voice.read();
    let open = st.open.read().clone();
    let Some(open) = open else { return rsx! {} };
    let Some((start, end)) = open.doc.span_of(&tokens) else { return rsx! {} };
    let phrase = open.doc.text(start, end).to_string();
    let existing =
        tokens.first().and_then(|&i| st.highlight_at(i)).filter(|h| h.start == start && h.end == end);
    let words: Vec<(usize, String, Option<String>)> = tokens
        .iter()
        .filter_map(|&i| {
            let t = open.doc.tokens.get(i)?;
            let w = t.word.as_ref()?;
            Some((
                i,
                open.doc.text(t.start, t.end).to_string(),
                w.shown().map(|x| x.gloss.get(voice).to_string()),
            ))
        })
        .collect();

    let save = {
        let db = db.clone();
        let tokens = tokens.clone();
        move |_| st.save_phrase(&db, &tokens)
    };

    rsx! {
        div { class: "word-card",
            p { class: "phrase", dir: "ltr", lang: "id", bdi { "{phrase}" } }
            div { class: "row",
                match existing {
                    Some(h) => rsx! {
                        button {
                            onclick: {
                                let db = db.clone();
                                move |_| {
                                    let removed = db.0.borrow_mut().remove_highlight(&h.id);
                                    match removed {
                                        Ok(()) => {
                                            let mut hl = st.highlights;
                                            hl.write().retain(|x| x.id != h.id);
                                        }
                                        Err(e) => st.write_failed("removing a highlight", &e),
                                    }
                                }
                            },
                            "{ui.t(Msg::RemoveHighlight)}"
                        }
                    },
                    None => rsx! {
                        button { class: if save_first { "primary" } else { "" }, onclick: save, "{ui.t(Msg::SavePhrase)}" }
                    },
                }
                button { onclick: move |_| speak(&phrase, "id-ID"), "🔊 {ui.t(Msg::Listen)}" }
                button {
                    onclick: {
                        let first = tokens.first().copied();
                        move |_| {
                            if let Some(i) = first {
                                let mut p = st.panel;
                                p.set(Some(Panel::Explain(i)));
                            }
                        }
                    },
                    "{ui.t(Msg::ExplainSentence)}"
                }
            }
            ul { class: "phrase-words",
                for (i , surface , gloss) in words {
                    li { key: "{i}",
                        button {
                            class: "ghost",
                            onclick: move |_| {
                                let mut p = st.panel;
                                p.set(Some(Panel::Word(i)));
                            },
                            bdi { class: "latin", lang: "id", "{surface}" }
                        }
                        span { " {gloss.as_deref().unwrap_or(\"…\")}" }
                    }
                }
            }
        }
    }
}

/// The device's own voice until the Azure voices arrive through Kabar Cloud:
/// Android's TextToSpeech engine, the WebView's `speechSynthesis` elsewhere.
pub(crate) fn speak(text: &str, lang_tag: &str) {
    match platform::speak(text, lang_tag, false) {
        Err(SpeakError::UseWebView) => {}
        Ok(()) => return,
        Err(e) => {
            crate::diag::report!("no reader audio: {e:?}");
            return;
        }
    }
    let payload = serde_json::json!({ "text": text, "lang": lang_tag });
    document::eval(&format!(
        r#"const p = {payload};
        if ("speechSynthesis" in window) {{
            const u = new SpeechSynthesisUtterance(p.text);
            u.lang = p.lang;
            u.rate = 0.9;
            speechSynthesis.cancel();
            speechSynthesis.speak(u);
        }}"#
    ));
}
