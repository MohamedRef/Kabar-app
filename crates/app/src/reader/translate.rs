//! The translation under each line (spec §11): every paragraph or chat bubble
//! of the open text, translated in context by the `translate_text` task in
//! the learner's voice before any word is tapped. Without an engine (offline,
//! no Kabar Cloud, quota used up) each line shows its words' meanings one by
//! one from the local dictionary instead, and says why.

use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::ai::FriendlyReason;
use kabar_core::ai::tasks::{LineTranslation, TranslateText};
use kabar_core::lang::Lang;
use kabar_core::locale::UiLanguage;
use kabar_core::reader::translate::{self, MAX_CHARS, MAX_LINES};
use kabar_protocol::ai::Task;

use super::ReaderState;
use super::doc::{ReadDoc, SegKind, Tag};
use crate::Ui;
use crate::account::Cloud;
use crate::i18n::Msg;
use crate::teach::Ai;

/// Where the translation of the open text stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Progress {
    #[default]
    Working,
    Done,
    /// No full translation: lines fall back to word by word.
    Failed(Msg),
}

/// The open text's translations, by segment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Translated {
    /// The text these belong to, so a late answer never lands on another.
    pub text_id: String,
    pub lines: Vec<Option<LineTranslation>>,
    pub progress: Progress,
    /// Javanese, or a route that hasn't passed the bake-off.
    pub unverified: bool,
    /// Each segment word by word in every voice, made once per text.
    pub fallback: Rc<[[String; 3]]>,
}

impl Translated {
    pub fn line(&self, seg: usize) -> Option<&LineTranslation> {
        self.lines.get(seg).and_then(Option::as_ref)
    }
}

fn voice_index(voice: UiLanguage) -> usize {
    match voice {
        UiLanguage::ArEg => 0,
        UiLanguage::Ar => 1,
        UiLanguage::En => 2,
    }
}

/// What the line under one segment shows.
#[derive(Clone, Debug, PartialEq)]
enum Under {
    Line(String),
    Pending,
    WordByWord(String),
}

/// One line's text in the voice the learner picked.
pub fn in_voice(l: &LineTranslation, voice: UiLanguage) -> &str {
    match voice {
        UiLanguage::ArEg => &l.arz,
        UiLanguage::Ar => &l.ar,
        UiLanguage::En => &l.en,
    }
}

/// Segments worth translating: text with at least one word, not chrome.
pub fn lines(doc: &ReadDoc) -> Vec<usize> {
    doc.segments
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind != SegKind::Chrome && s.tokens.clone().any(|i| doc.tokens[i].word.is_some()))
        .map(|(i, _)| i)
        .collect()
}

/// Javanese when most of the text's tagged words are.
pub fn text_lang(doc: &ReadDoc) -> Lang {
    let count =
        |tag: Tag| doc.tokens.iter().filter(|t| t.word.as_ref().is_some_and(|w| w.tag == tag)).count();
    if count(Tag::Javanese) > count(Tag::Indonesian) { Lang::Javanese } else { Lang::Indonesian }
}

/// The first sense of a gloss, without its hints: "going to, will · to
/// want" → "going to"; "أنا (قبل الفعل)" → "أنا".
fn first_sense(gloss: &str) -> String {
    let sense =
        gloss.split([',', '،', '·', ';', '/']).map(str::trim).find(|s| !s.is_empty()).unwrap_or(gloss);
    let mut out = String::new();
    let mut depth = 0;
    for c in sense.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.trim().trim_end_matches(['…', '.']).trim();
    if out.is_empty() { sense.to_string() } else { out.to_string() }
}

/// A segment's words in the voice, one by one, from local data. Words with
/// no gloss (names, unknown spellings) stay as written.
pub fn word_by_word(doc: &ReadDoc, seg: usize, voice: UiLanguage) -> String {
    let Some(s) = doc.segments.get(seg) else { return String::new() };
    s.tokens
        .clone()
        .filter_map(|i| {
            let t = &doc.tokens[i];
            let w = t.word.as_ref()?;
            let surface = doc.text(t.start, t.end);
            Some(match w.shown() {
                Some(info) if !w.is_english_aside() => first_sense(info.gloss.get(voice)),
                _ => surface.to_string(),
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn failure(r: FriendlyReason) -> Msg {
    match r {
        FriendlyReason::ResetsLater => Msg::TranslationResetsLater,
        FriendlyReason::Unreachable => Msg::TranslationUnreachable,
        FriendlyReason::NotAvailable => Msg::TranslationNeedsCloud,
        FriendlyReason::TryAgain => Msg::TranslationTryAgain,
    }
}

/// Translates the open text in chunks, filling `st.translated` line by line
/// as answers arrive. Lives as long as the Reader view; restarts when another
/// text opens (or on "try again"), and answers come from the router's cache
/// the second time, offline too.
pub fn use_translation(st: ReaderState) {
    let retry = st.retry;
    let ai = try_consume_context::<Ai>();
    let cloud = try_consume_context::<Cloud>();
    // Only a different text (or a retry) starts over, not a re-read of this one.
    let key = use_memo(move || {
        let open = st.open.read();
        open.as_ref().map(|o| (o.text.id.clone(), o.doc.body.len(), *retry.read()))
    });
    use_resource(move || {
        let ai = ai.clone();
        let mut translated = st.translated;
        async move {
            let Some((text_id, _, _)) = key.read().clone() else { return };
            let Some(doc) = st.open.peek().as_ref().map(|o| o.doc.clone()) else { return };
            let segs = lines(&doc);
            let lang = text_lang(&doc);
            let fallback = (0..doc.segments.len())
                .map(|seg| UiLanguage::ALL.map(|v| word_by_word(&doc, seg, v)))
                .collect::<Vec<_>>()
                .into();
            translated.set(Translated {
                text_id: text_id.clone(),
                lines: vec![None; doc.segments.len()],
                progress: Progress::Working,
                unverified: lang == Lang::Javanese,
                fallback,
            });
            let Some(ai) = ai else {
                translated.write().progress = Progress::Failed(Msg::TranslationNeedsCloud);
                return;
            };
            // Not set up or signed out: say so, rather than "can't reach".
            let signed_in = cloud.is_some_and(|c| {
                let status = c.status.peek();
                status.configured && status.email.is_some()
            });
            if !signed_in || ai.0.plan(Task::TranslateText, true).is_err() {
                translated.write().progress = Progress::Failed(Msg::TranslationNeedsCloud);
                return;
            }
            let texts: Vec<&str> =
                segs.iter().map(|&s| doc.text(doc.segments[s].start, doc.segments[s].end)).collect();
            for range in translate::chunks(&texts, MAX_LINES, MAX_CHARS) {
                let req = translate::request(lang, &texts[range.clone()]);
                let answer = ai.0.run::<TranslateText>(&req).await;
                // Another text opened meanwhile: drop this answer.
                if translated.peek().text_id != text_id {
                    return;
                }
                match answer {
                    Ok(a) => {
                        let mut t = translated.write();
                        t.unverified |= a.needs_review || a.provisional;
                        for l in a.value.lines {
                            if let Some(&seg) = segs.get(range.start + l.i) {
                                t.lines[seg] = Some(l);
                            }
                        }
                    }
                    Err(e) => {
                        crate::diag::report!("translate_text failed: {e:?}");
                        translated.write().progress = Progress::Failed(failure(e.friendly()));
                        return;
                    }
                }
            }
            translated.write().progress = Progress::Done;
        }
    });
}

/// The status line over the text: what the lines under it are, and why.
#[component]
pub fn TranslationStatus() -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let (msg, busy, failed, unverified) = {
        let t = st.translated.read();
        let (msg, busy, failed) = match &t.progress {
            Progress::Working => (Msg::TranslationWorking, true, false),
            Progress::Done => (Msg::TranslationByAi, false, false),
            Progress::Failed(m) => (*m, false, true),
        };
        (msg, busy, failed, t.unverified)
    };
    rsx! {
        div { class: "tr-status", role: "status", aria_busy: if busy { "true" } else { "false" },
            span { class: "tr-badge", "{ui.t(Msg::TranslationTitle)}" }
            span { class: "muted small", "{ui.t(msg)}" }
            if unverified && !failed {
                span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
            }
            if failed && msg != Msg::TranslationNeedsCloud {
                button {
                    class: "ghost small",
                    onclick: move |_| {
                        let mut r = st.retry;
                        r += 1;
                    },
                    "↻ {ui.t(Msg::TranslationRetry)}"
                }
            }
        }
    }
}

/// The line under one segment: its translation, a placeholder while it's
/// coming, or the word-by-word fallback.
#[component]
pub fn LineUnder(seg: usize) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    // Each line re-renders only when what it shows changes, not on every
    // chunk that lands elsewhere in the text.
    let under = use_memo(move || {
        let voice = *st.voice.read();
        let t = st.translated.read();
        match (t.line(seg), &t.progress) {
            (Some(l), _) => Under::Line(in_voice(l, voice).to_string()),
            (None, Progress::Working) => Under::Pending,
            (None, _) => Under::WordByWord(
                t.fallback.get(seg).map(|f| f[voice_index(voice)].clone()).unwrap_or_default(),
            ),
        }
    });
    let lang = st.voice.read().tag();
    match under() {
        Under::Line(text) => rsx! {
            p { class: "tr-line", lang, dir: "auto", "{text}" }
        },
        Under::Pending => rsx! {
            p { class: "tr-line pending", aria_hidden: "true", "…" }
        },
        Under::WordByWord(words) => rsx! {
            p {
                class: "tr-line word-by-word",
                lang,
                dir: "auto",
                title: "{ui.t(Msg::TranslationWordByWord)}",
                "{words}"
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::doc::Lexicon;
    use crate::reader::glossary::SeedGlossary;
    use kabar_core::learn::PersonalDictionary;

    fn doc(text: &str) -> ReadDoc {
        ReadDoc::from_text(
            text,
            Lexicon { glossary: &SeedGlossary::default(), taught: &PersonalDictionary::default() },
        )
    }

    #[test]
    fn javanese_chat_lines_are_javanese_and_glossed_word_by_word() {
        let d = doc("Sek tak lapor pusat\n\narp dicul piro");
        assert_eq!(lines(&d), vec![0, 1]);
        assert_eq!(text_lang(&d), Lang::Javanese);
        let en = word_by_word(&d, 1, UiLanguage::En);
        assert_eq!(en, "want to let go how much");
        let first = word_by_word(&d, 0, UiLanguage::ArEg);
        assert!(first.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)), "{first}");
    }

    #[test]
    fn first_sense_takes_the_first_meaning() {
        assert_eq!(first_sense("going to, will · to want"), "going to");
        assert_eq!(first_sense("لـ / على"), "لـ");
        assert_eq!(first_sense("book"), "book");
        assert_eq!(first_sense("أنا (قبل الفعل) · خليني"), "أنا");
        assert_eq!(first_sense("أنا هـ… (أنا اللي هعمل)"), "أنا هـ");
        assert_eq!(first_sense("(prefix)"), "(prefix)");
    }

    #[test]
    fn a_fixture_answer_fills_the_lines_in_order() {
        let fixture = include_str!("../../../../fixtures/ai/translate_text.arp_dicul_piro.workers_ai.json");
        let a: TranslateText = serde_json::from_str(fixture).unwrap();
        assert_eq!(in_voice(&a.lines[0], UiLanguage::ArEg), "هيتباع بكام؟");
        assert_eq!(in_voice(&a.lines[1], UiLanguage::En), "Hold on, I'll report to HQ.");
    }
}
