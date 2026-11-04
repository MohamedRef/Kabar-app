//! While a page, a picture or Kabar's write-up is on its way, offer one word
//! the learner saved in the Reader to recall (Speaker UX item 7): the word
//! first, its meaning and the sentence it was met in a tap away. Only real
//! saved words and the offline glossary, no AI call; nothing shows when
//! nothing is saved.

use dioxus::prelude::*;

use super::glossary::WordLang;
use super::library::word_of;
use crate::i18n::Msg;
use crate::store::SavedWord;
use crate::{Db, Ui};

/// The newest saves the pick rotates through.
const RECENT: usize = 30;

/// One saved word (phrases have no single meaning to recall), rotating with
/// `at`; None when nothing is saved.
fn pick(words: Vec<SavedWord>, at: usize) -> Option<SavedWord> {
    let words: Vec<_> = words.into_iter().filter(|w| word_of(&w.lexeme).is_some()).collect();
    (!words.is_empty()).then(|| words[at % words.len()].clone())
}

/// The saved word's meaning in the learner's voice, from the offline glossary.
fn meaning(
    word: &str,
    lang: WordLang,
    surface: &str,
    voice: kabar_core::locale::UiLanguage,
) -> Option<String> {
    super::glossary()
        .lookup(Some(lang), &[word, &surface.to_lowercase()])
        .map(|i| i.gloss.get(voice).to_string())
}

#[component]
pub fn WaitWord() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    // One pick per wait, so the word doesn't change under the learner.
    let pick = use_hook(|| {
        let words = db.0.borrow().saved_words(RECENT).unwrap_or_default();
        pick(words, (crate::store::now_ms() / 1000) as usize)
    });
    let mut open = use_signal(|| false);
    let Some(w) = pick else { return rsx! {} };
    let (word, wl) = word_of(&w.lexeme).unwrap_or((&w.surface, WordLang::Indonesian));
    let lang = if wl == WordLang::Javanese { "jv" } else { "id" };
    // The glossary is looked up on reveal, so a screen that never opened
    // the Reader doesn't parse it just to show the word.
    let gloss = open().then(|| meaning(word, wl, &w.surface, ui.lang()));
    rsx! {
        details { class: "wait-word", ontoggle: move |_| open.toggle(),
            summary {
                span { class: "muted small", "{ui.t(Msg::WaitWordPrompt)}" }
                bdi { class: "wait-word-surface", lang, dir: "ltr", "{w.surface}" }
            }
            div { class: "wait-word-body",
                if let Some(Some(g)) = &gloss {
                    p { class: "wait-word-gloss", "{g}" }
                }
                p { class: "wait-word-sentence", lang, dir: "ltr", bdi { "{w.sentence}" } }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(lexeme: &str, surface: &str) -> SavedWord {
        SavedWord {
            lexeme: lexeme.into(),
            surface: surface.into(),
            sentence: "Saya sedang baca buku.".into(),
            text_id: None,
        }
    }

    #[test]
    fn the_revealed_meaning_is_in_the_learners_voice() {
        use kabar_core::locale::UiLanguage;
        assert_eq!(
            meaning("membaca", WordLang::Indonesian, "Membaca", UiLanguage::En).as_deref(),
            Some("to read")
        );
        let arz = meaning("mangan", WordLang::Javanese, "mangan", UiLanguage::ArEg).unwrap();
        assert!(arz.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)), "{arz}");
        assert_eq!(meaning("zxqvbn", WordLang::Indonesian, "zxqvbn", UiLanguage::En), None);
    }

    #[test]
    fn picks_only_saved_words_and_nothing_when_none() {
        assert_eq!(pick(vec![], 7), None);
        assert_eq!(pick(vec![saved("phrase:baca buku", "baca buku")], 7), None);
        let words = vec![
            saved("phrase:baca buku", "baca buku"),
            saved("id:baca", "baca"),
            saved("jv:mangan", "mangan"),
        ];
        assert_eq!(pick(words.clone(), 0).unwrap().surface, "baca");
        assert_eq!(pick(words, 1).unwrap().surface, "mangan");
    }
}
