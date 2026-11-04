//! Annotated text as the learner sees it outside the Reader: rendered with
//! SSR in each translation setting, and the word sheet a tap opens.

use kabar_core::grammar::word::{Part, PartKind};
use kabar_core::locale::UiLanguage;
use kabar_core::settings::TranslationMode;

use super::*;
use crate::i18n::Msg;
use crate::reader::fixture::{Case, Screenful, msg, text_of};

const EN: UiLanguage = UiLanguage::En;

/// Renders `body` with the glossary parsed (as a moment after start-up).
fn ready(
    body: fn() -> Element,
    settings: fn(&mut kabar_core::settings::Settings),
    lang: UiLanguage,
) -> String {
    let mut case = Case::new(body);
    case.settings = settings;
    case.lang = lang;
    let mut s = Screenful::new(case);
    s.act(|_, _| {
        let mut words = consume_context::<Words>();
        words.ready.set(true);
    });
    s.settle()
}

fn greeting() -> Element {
    rsx! {
        Annotated {
            text: "Halo! Selamat pagi, Bu. Nama saya Gadis.",
            tag: "id",
            meaning: "Hello! Good morning, ma'am. My name is Gadis.",
            marked: vec!["nama".to_string()],
        }
    }
}

#[test]
fn every_word_is_tappable_with_its_meaning_under_it() {
    let html = ready(greeting, |_| {}, EN);
    let text = text_of(&html);
    assert!(html.contains("class=\"annot id\""), "{html}");
    assert!(html.contains("role=\"button\" tabindex=\"0\""), "words are buttons: {html}");
    // "Selamat pagi" is one greeting, its meaning under both words.
    assert!(html.contains("annot-phrase glossed"), "{html}");
    let phrase = &html[html.find("annot-phrase glossed").unwrap()..];
    assert!(text_of(phrase).contains("good morning"), "{phrase}");
    // A cast member's name says it is a name; the story's word is underlined.
    assert!(html.contains("annot-g annot-name") && text.contains(tt(EN, T::Name)), "{html}");
    assert!(html.contains("annot-w target glossed"), "Nama is marked: {html}");
    // The translation under the sentence, the speaker and the pronunciation.
    assert!(html.contains("annot-tr") && text.contains("My name is Gadis."), "{html}");
    assert!(html.contains("class=\"say\""), "{html}");
    assert!(html.contains("class=\"pron"), "{html}");
}

#[test]
fn word_parts_take_their_colours_inside_the_sentence() {
    let html = ready(|| rsx! { Annotated { text: "Rumahnya dibersihkan.", tag: "id" } }, |_| {}, EN);
    assert!(html.contains("wc-line wc-verb"), "{html}");
    assert!(html.contains("<span class=\"wp-prefix\">di</span><span class=\"wp-root\">bersih</span><span class=\"wp-suffix\">kan</span>"), "{html}");
}

#[test]
fn on_tap_the_translation_waits_behind_a_chip() {
    let html = ready(greeting, |s| s.translation = TranslationMode::Tap, EN);
    assert!(html.contains("annot-reveal") && text_of(&html).contains(tt(EN, T::ShowTranslation)), "{html}");
    assert!(!html.contains("annot-tr"), "{html}");
    assert!(!html.contains("annot-w target glossed"), "no meanings under the words on tap: {html}");
}

#[test]
fn translation_off_shows_the_sentence_alone() {
    let html = ready(greeting, |s| s.translation = TranslationMode::Off, EN);
    assert!(
        !html.contains("annot-tr") && !html.contains("annot-reveal") && !html.contains("annot-g"),
        "{html}"
    );
}

#[test]
fn inline_text_flows_in_the_sentence_without_a_line_below() {
    let html = ready(
        || rsx! { Annotated { text: "Selamat pagi", tag: "id", meaning: "Good morning", inline: true, say: false, pron: false } },
        |_| {},
        UiLanguage::ArEg,
    );
    assert!(html.contains("class=\"annot inline id\""), "{html}");
    assert!(
        !html.contains("annot-tr") && !html.contains("annot-g") && !html.contains("class=\"say\""),
        "{html}"
    );
}

#[test]
fn a_prompt_shows_the_latin_text_only() {
    let html =
        ready(|| rsx! { Annotated { text: "Apa kabar?", tag: "jv", plain: true, class: "big" } }, |_| {}, EN);
    assert_eq!(html, "<bdi class=\"latin annot plain big\" lang=\"jv\" dir=\"ltr\">Apa kabar?</bdi>");
}

#[test]
fn before_the_glossary_is_ready_the_sentence_already_reads() {
    let html = Screenful::new(Case::new(greeting)).html();
    let text = text_of(&html);
    assert!(text.contains("Halo! Selamat pagi, Bu. Nama saya Gadis."), "{text}");
    assert!(!html.contains("annot-w"), "no word runs yet: {html}");
    assert!(text.contains("My name is Gadis."), "{text}");
}

#[test]
fn a_tap_opens_a_word_and_a_second_tap_closes_it() {
    let mut s = Screenful::new(Case::new(|| rsx! {}));
    s.act(|_, _| {
        let open = Signal::new(None);
        toggle(open, 3);
        assert_eq!(*open.peek(), Some(3));
        toggle(open, 5);
        assert_eq!(*open.peek(), Some(5));
        toggle(open, 5);
        assert_eq!(*open.peek(), None);
    });
}

#[test]
fn taught_words_are_read_again_after_teach_saves() {
    let mut s = Screenful::new(Case::new(|| rsx! {}));
    s.act(|db, _| {
        let words = consume_context::<Words>();
        assert!(words.taught.peek().approved().next().is_none());
        let mut t: kabar_protocol::learn::TaughtEntry =
            serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
        t.entry.status = kabar_protocol::content::ReviewStatus::Approved;
        db.0.borrow_mut().save_taught(&t).unwrap();
        words.refresh(db);
        assert_eq!(words.taught.peek().approved().count(), 1);
    });
}

// ---- The word sheet --------------------------------------------------------

thread_local! {
    static SHEET: std::cell::RefCell<(&'static str, &'static str, &'static str)> =
        const { std::cell::RefCell::new(("", "", "id")) };
}

/// The sheet for `word` in `text`, as a tap on it opens it.
fn sheet(text: &'static str, word: &'static str, tag: &'static str, lang: UiLanguage) -> String {
    SHEET.with(|s| *s.borrow_mut() = (text, word, tag));
    let mut case = Case::new(|| {
        let (text, word, tag) = SHEET.with(|s| *s.borrow());
        let words = use_context::<Words>();
        let doc = use_hook(|| analyze(text, text_lang(tag), &words.taught.peek(), &[]).0);
        let i = doc.tokens.iter().position(|t| doc.text(t.start, t.end) == word).unwrap();
        rsx! { WordSheet { doc, i, tag, on_close: |_| {} } }
    });
    case.lang = lang;
    crate::reader::fixture::render(case)
}

#[test]
fn the_sheet_breaks_a_word_into_its_coloured_parts() {
    let html = sheet("Rumahnya dibersihkan kemarin.", "dibersihkan", "id", EN);
    let text = text_of(&html);
    assert!(text.contains(tt(EN, T::Breakdown)), "{text}");
    assert!(text.contains("di-passive: it is done (to the subject)"), "what di- does: {text}");
    assert!(html.contains("class-chip wc-verb"), "{html}");
    assert!(html.contains("piece wp-prefix") && html.contains("piece wp-suffix"), "{html}");
    assert!(html.contains("annot-note"), "a line on what di- and -kan do: {html}");
    assert!(text.contains(tt(EN, T::Legend)), "{text}");
    assert!(text.contains("clean"), "its meaning: {text}");
}

#[test]
fn the_sheet_of_a_doubled_word_says_what_doubling_does() {
    let html = sheet("Anak-anak bermain di taman.", "Anak-anak", "id", EN);
    assert!(html.contains("annot-note"), "{html}");
    assert!(text_of(&html).contains(tt(EN, T::Breakdown)), "{html}");
}

#[test]
fn the_sheet_of_a_javanese_word_flags_unreviewed_notes() {
    let html = sheet("Bukune ning meja.", "Bukune", "jv", UiLanguage::ArEg);
    let text = text_of(&html);
    assert!(text.contains(tt(UiLanguage::ArEg, T::Breakdown)), "{text}");
    assert!(
        text.contains(tt(UiLanguage::ArEg, T::Unverified)),
        "Javanese affix notes aren't reviewed: {text}"
    );
    assert!(text.contains("Bukune ← buku"), "the word and its lemma: {text}");
}

#[test]
fn an_unknown_word_offers_to_teach_kabar() {
    let html = sheet("Zxqvbn banget.", "Zxqvbn", "id", EN);
    let text = text_of(&html);
    assert!(text.contains(tt(EN, T::NotInDictionary)), "{text}");
    assert!(text.contains(tt(EN, T::TeachIt)), "{text}");
}

#[test]
fn the_sheet_lists_the_rules_the_word_is_in() {
    let html = sheet("Saya sedang membaca buku.", "sedang", "id", EN);
    let text = text_of(&html);
    assert!(text.contains(msg(EN, Msg::GrammarForWord)), "{text}");
    assert!(text.contains("Time without tenses"), "{text}");
}

#[test]
fn a_function_word_with_no_parts_shows_no_breakdown() {
    let html = sheet("Ini buku.", ".", "id", EN);
    assert_eq!(html, "", "punctuation has no sheet");
}

// ---- Shared pieces ---------------------------------------------------------

fn part(kind: PartKind, form: &str, label: &str) -> Part {
    Part { kind, form: form.into(), label: label.into() }
}

#[test]
fn pieces_show_which_side_they_join() {
    assert_eq!(parts::piece_text(&part(PartKind::Prefix, "mem", "meN-")), "mem-");
    assert_eq!(parts::piece_text(&part(PartKind::Suffix, "kan", "-kan")), "-kan");
    assert_eq!(parts::piece_text(&part(PartKind::Clitic, "nya", "-nya")), "-nya");
    assert_eq!(parts::piece_text(&part(PartKind::Confix, "ke", "ke-an")), "ke-");
    assert_eq!(parts::piece_text(&part(PartKind::Confix, "an", "ke-an")), "-an");
    assert_eq!(parts::piece_text(&part(PartKind::Root, "baca", "baca")), "baca");
}

#[test]
fn the_parts_line_joins_pieces_with_plus_and_names_each() {
    let html = crate::reader::fixture::render(Case::new(|| {
        let parts = vec![
            part(PartKind::Prefix, "di", "di-"),
            part(PartKind::Root, "bersih", "bersih"),
            part(PartKind::Suffix, "kan", "-kan"),
        ];
        rsx! {
            parts::PartsLine { parts, class: Some(WordClass::Verb), labels: true, voice: UiLanguage::En }
            parts::ClassChip { class: WordClass::Noun, voice: UiLanguage::ArEg }
            parts::SayWord { text: "dibersihkan", tag: "jv" }
            parts::Legend { voice: UiLanguage::En }
        }
    }));
    assert!(html.starts_with("<span class=\"parts-line wc-verb\" dir=\"ltr\">"), "{html}");
    assert_eq!(html.matches("class=\"plus\"").count(), 2, "{html}");
    assert_eq!(html.matches("class=\"piece-kind\"").count(), 3, "{html}");
    assert!(html.contains("class-chip wc-noun"), "{html}");
    assert!(html.contains("say-word") && html.contains("class=\"pron"), "letters by default: {html}");
    assert!(html.contains("colour-legend") && html.matches("legend-item").count() >= 4, "{html}");
}

#[test]
fn every_annotated_string_exists_in_all_three_voices() {
    for t in
        [T::ShowTranslation, T::Breakdown, T::NotInDictionary, T::TeachIt, T::Legend, T::Unverified, T::Name]
    {
        for lang in UiLanguage::ALL {
            let s = tt(lang, t);
            let arabic = s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
            assert_eq!(arabic, lang != UiLanguage::En, "{t:?} {lang:?}: {s}");
        }
        assert_ne!(tt(UiLanguage::ArEg, t), tt(UiLanguage::En, t));
    }
}
