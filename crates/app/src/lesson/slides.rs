//! Slide, dialogue and audio controls shared by lesson players.

use super::reading::Title;
use super::*;

/// A play button for target-language audio, with a slow option.
#[component]
pub(super) fn PlayButton(
    text: String,
    language: String,
    audio: Option<String>,
    missing: Signal<bool>,
    big: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let (t1, l1, a1) = (text.clone(), language.clone(), audio.clone());
    let banner = try_use_context::<NoSoundBanner>().is_some();
    if banner && !big && *missing.read() {
        return rsx! {};
    }
    rsx! {
        div { class: "row lp-play",
            button {
                class: if big { "lp-sound big" } else { "lp-sound" },
                aria_label: lt(lang, L::Listen),
                title: lt(lang, L::Listen),
                onclick: move |_| speak(&t1, &l1, false, a1.as_deref(), missing),
                {speaker_icon()}
            }
            if big {
                button {
                    class: "lp-sound slow",
                    aria_label: lt(lang, L::Slowly),
                    title: lt(lang, L::Slowly),
                    onclick: move |_| speak(&text, &language, true, audio.as_deref(), missing),
                    {speaker_icon()}
                    span { class: "lp-slow-label", "{lt(lang, L::Slowly)}" }
                }
            }
            if *missing.read() {
                span { class: "muted lp-small", "{lt(lang, no_audio_msg())}" }
            }
        }
    }
}
/// Why there's no sound: a missing Indonesian voice gets an install hint.
pub(crate) fn no_audio_msg() -> L {
    match platform::tts_status() {
        TtsStatus::MissingVoice | TtsStatus::Unsupported => L::NoVoiceData,
        _ => L::NoAudio,
    }
}
/// The circular arrow of "Try again".
pub(super) fn retry_icon() -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: "M20 12a8 8 0 1 1-2.34-5.66M20 4v4h-4" }
        }
    }
}
pub(crate) fn speaker_icon() -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: "M4 10v4h4l5 4V6L8 10zM16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11" }
        }
    }
}
// ---------------------------------------------------------------------------
// Slides, dialogue, summary
// ---------------------------------------------------------------------------

/// The language of a table column: an "Indonesia" column in a Javanese
/// lesson holds Indonesian words; every other column the lesson's own.
pub(super) fn column_tag(head: Option<&str>, target: &'static str) -> &'static str {
    let indonesian = head.is_some_and(|h| {
        let h = h.trim().to_lowercase();
        h == "indonesia" || h == "indonesian" || h == "bahasa indonesia"
    });
    if indonesian { "id" } else { target }
}
/// One table cell. A cell that is a word or phrase always shows how it's
/// said, in its column's language, so a table follows one rule (#381):
/// Javanese "awan" is said even though Indonesian spells it the same.
#[component]
pub(super) fn TableCell(text: String, tag: &'static str) -> Element {
    let ui = use_context::<Ui>();
    let say = ui.settings.read().ui_language != UiLanguage::En;
    let word = latin_runs(&text).len() == 1 && is_target_run(&text) && !text.contains("**");
    if !word {
        return rsx! { Rich { text, target: tag, pron: true } };
    }
    rsx! {
        // Word over its pronunciation, so a four-column table fits a phone.
        span { class: "lp-cell-word",
            bdi { lang: tag, dir: "ltr", class: "latin", "{text}" }
            Say { text: text.clone(), tag }
            if say {
                Pron { text: text.clone(), tag, inline: true }
            }
        }
    }
}
#[component]
pub(super) fn SlideView(lesson: LessonContent, index: usize, missing: Signal<bool>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&lesson.language);
    // Passed earlier with a reasonable score: the learner approved it.
    let db = use_context::<Db>();
    let approved = use_hook(|| db.0.borrow().approved_lessons().is_ok_and(|ids| ids.contains(&lesson.id)));
    let Some(slide) = lesson.slides.get(index).cloned() else { return rsx! {} };
    let language = lesson.language.clone();
    rsx! {
        article { class: "lp-slide lp-enter",
            if index == 0 {
                p { class: "lp-lesson-title",
                    bdi { lang: target, dir: "ltr", "{lesson.title}" }
                    if !lesson.review.verified && !approved {
                        span { class: "pill lp-draft", "{lt(lang, L::NotReviewed)}" }
                    }
                }
                p { class: "muted", Rich { text: pick(&lesson.can_do, lang).to_string(), target } }
            }
            if let Some(title) = &slide.title {
                h2 { class: "lp-slide-title", Title { text: title.clone(), target } }
            }
            p { class: "lp-slide-body pre", Rich { text: pick(&slide.body, lang).trim().to_string(), target, pron: true } }
            if let Some(table) = &slide.table {
                div { class: "lp-table-wrap",
                    table { class: "lp-table",
                        thead {
                            tr {
                                for (i, h) in table.head.iter().enumerate() {
                                    th { key: "{i}", bdi { lang: target, dir: "ltr", "{h}" } }
                                }
                                th { "{lt(lang, L::TableMeaning)}" }
                            }
                        }
                        tbody {
                            for (i, row) in table.rows.iter().enumerate() {
                                tr { key: "{i}",
                                    for (j, cell) in row.cells.iter().enumerate() {
                                        {
                                            let tag = column_tag(table.head.get(j).map(String::as_str), target);
                                            rsx! {
                                                td { key: "{j}", lang: tag, TableCell { text: cell.clone(), tag } }
                                            }
                                        }
                                    }
                                    td { class: "muted", "{pick(&row.meaning, lang)}" }
                                }
                            }
                        }
                    }
                }
            }
            if !slide.examples.is_empty() {
                p { class: "muted lp-small", "{lt(lang, L::Examples)}" }
                ul { class: "lp-examples",
                    for (i, ex) in slide.examples.iter().enumerate() {
                        li { key: "{i}",
                            button {
                                class: "lp-example",
                                onclick: {
                                    let (t, l) = (ex.text.clone(), language.clone());
                                    move |_| speak(&t, &l, false, None, missing)
                                },
                                span { class: "lp-example-icon", {speaker_icon()} }
                                span { class: "lp-example-text",
                                    Annotated { text: ex.text.clone(), tag: target, meaning: pick(&ex.meaning, lang).to_string(), class: "lp-target", say: false }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
#[component]
pub(super) fn DialogueView(
    dialogue: Dialogue,
    language: String,
    shown: usize,
    missing: Signal<bool>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&language);
    let first = dialogue.lines.first().map(|l| l.speaker.clone()).unwrap_or_default();
    rsx! {
        article { class: "lp-dialogue",
            p { class: "lp-instruction", "{lt(lang, L::Dialogue)}" }
            if let Some(t) = &dialogue.title {
                h2 { class: "lp-slide-title", bdi { lang: target, dir: "ltr", "{t}" } }
            }
            p { class: "muted lp-small", "{lt(lang, L::TapToHear)}" }
            ol { class: "lp-lines",
                for (i, line) in dialogue.lines.iter().take(shown).enumerate() {
                    li {
                        key: "{i}",
                        class: if line.speaker == first { "lp-line a lp-enter" } else { "lp-line b lp-enter" },
                        button {
                            class: "lp-bubble",
                            onclick: {
                                let (d, l) = (dialogue.clone(), language.clone());
                                move |_| speak_line(&d, i, &l, missing)
                            },
                            span { class: "lp-speaker",
                                crate::portraits::SpeakerFace { speaker: line.speaker.clone(), class: "lp-face" }
                                bdi { "{line.speaker}" }
                            }
                            Annotated { text: line.text.clone(), tag: target, meaning: pick(&line.meaning, lang).to_string(), class: "lp-target", say: false }
                        }
                    }
                }
            }
        }
    }
}
