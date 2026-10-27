//! Lesson notes: the supporting pane beside the Lesson Player on wide windows
//! (tablet landscape, Mac; docs/DESIGN.md). It shows where you are in the
//! lesson and its words, so the empty half of the screen helps the lesson.
//! Meanings hide while a question waits for an answer, so the pane never
//! gives one away.

use dioxus::prelude::*;
use kabar_core::lesson::{Step, pick};
use kabar_core::locale::{UiLanguage, format_number};
use kabar_protocol::lesson::{LessonContent, Outcome};

use super::text::{L, lt};
use super::{Rich, speak, target_tag};
use crate::Ui;
use crate::pron::Pron;

/// Where an exercise stands, for the practice dots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Todo,
    Current,
    Right,
    Missed,
    Skipped,
}

impl Mark {
    pub fn of(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Correct | Outcome::Almost | Outcome::Practiced => Self::Right,
            Outcome::Missed => Self::Missed,
            Outcome::Skipped => Self::Skipped,
        }
    }

    fn class(self) -> &'static str {
        match self {
            Self::Todo => "ln-dot",
            Self::Current => "ln-dot current",
            Self::Right => "ln-dot right",
            Self::Missed => "ln-dot missed",
            Self::Skipped => "ln-dot skipped",
        }
    }

    /// Text for screen readers and the tooltip: colour is never the only cue.
    fn label(self, lang: UiLanguage) -> &'static str {
        nt(
            lang,
            match self {
                Self::Todo => N::Todo,
                Self::Current => N::Now,
                Self::Right => N::Right,
                Self::Missed => N::Missed,
                Self::Skipped => N::SkippedOne,
            },
        )
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Right => "✓",
            Self::Missed => "↺",
            Self::Skipped => "–",
            Self::Todo | Self::Current => "",
        }
    }
}

/// The outline's sections, in lesson order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Learn,
    Practice,
    Dialogue,
    Summary,
}

fn part_of(step: Step) -> Part {
    match step {
        Step::Slide(_) => Part::Learn,
        Step::Exercise { .. } => Part::Practice,
        Step::Dialogue => Part::Dialogue,
        Step::Summary => Part::Summary,
    }
}

fn state(part: Part, now: Part) -> &'static str {
    use std::cmp::Ordering::*;
    let rank = |p: Part| p as u8;
    match rank(part).cmp(&rank(now)) {
        Less => "ln-part done",
        Equal => "ln-part current",
        Greater => "ln-part",
    }
}

/// `marks` has one entry per exercise. `answering` is true while a question
/// is on screen and not yet checked: meanings hide then.
#[component]
pub fn LessonNotes(
    lesson: LessonContent,
    step: Step,
    marks: Vec<Mark>,
    answering: bool,
    missing: Signal<bool>,
    on_hide: EventHandler,
) -> Element {
    let ui = use_context::<Ui>();
    let settings = ui.settings.read().clone();
    let lang = settings.ui_language;
    let num = |n: usize| format_number(n as u64, lang, settings.eastern_digits);
    let target = target_tag(&lesson.language);
    let now = part_of(step);
    let right = marks.iter().filter(|m| **m == Mark::Right).count();
    let language = lesson.language.clone();

    rsx! {
        aside { class: "lp-notes", aria_label: nt(lang, N::Notes),
            header { class: "ln-head",
                div {
                    p { class: "ln-kicker", "{nt(lang, N::ThisLesson)}" }
                    h2 { class: "ln-title", bdi { lang: target, dir: "ltr", "{lesson.title}" } }
                }
                button {
                    class: "ghost ln-hide",
                    aria_label: nt(lang, N::Hide),
                    title: nt(lang, N::Hide),
                    onclick: move |_| on_hide.call(()),
                    "×"
                }
            }
            p { class: "ln-cando muted", Rich { text: pick(&lesson.can_do, lang).to_string(), target } }

            ol { class: "ln-outline",
                li { class: state(Part::Learn, now), aria_current: if now == Part::Learn { "step" } else { "false" },
                    span { class: "ln-part-name", "{nt(lang, N::Learn)}" }
                    ol { class: "ln-slides",
                        for (i, s) in lesson.slides.iter().enumerate() {
                            li {
                                key: "{i}",
                                class: match step {
                                    Step::Slide(j) if j == i => "current",
                                    Step::Slide(j) if j < i => "",
                                    _ => "done",
                                },
                                if let Some(t) = &s.title {
                                    bdi { "{t}" }
                                } else {
                                    "{nt(lang, N::Slide)} {num(i + 1)}"
                                }
                            }
                        }
                    }
                }
                if !marks.is_empty() {
                    li { class: state(Part::Practice, now), aria_current: if now == Part::Practice { "step" } else { "false" },
                        span { class: "ln-part-name", "{nt(lang, N::Practice)}" }
                        span { class: "ln-count muted", "{num(right)} / {num(marks.len())}" }
                        ol { class: "ln-dots",
                            for (i, m) in marks.iter().enumerate() {
                                li { key: "{i}", class: m.class(), title: m.label(lang),
                                    span { aria_hidden: "true", "{m.glyph()}" }
                                    span { class: "visually-hidden", "{num(i + 1)}: {m.label(lang)}" }
                                }
                            }
                        }
                    }
                }
                if lesson.dialogue.is_some() {
                    li { class: state(Part::Dialogue, now), aria_current: if now == Part::Dialogue { "step" } else { "false" },
                        span { class: "ln-part-name", "{lt(lang, L::Dialogue)}" }
                    }
                }
                li { class: state(Part::Summary, now), aria_current: if now == Part::Summary { "step" } else { "false" },
                    span { class: "ln-part-name", "{lt(lang, L::Done)}" }
                }
            }

            if !lesson.new_words.is_empty() {
                section { class: "ln-words",
                    h3 { "{lt(lang, L::NewWords)}" }
                    if answering {
                        p { class: "ln-hidden-note muted", "{nt(lang, N::MeaningsHidden)}" }
                    }
                    ul {
                        for w in lesson.new_words.iter() {
                            li { key: "{w.word}",
                                button {
                                    class: "ln-word",
                                    title: lt(lang, L::TapToHear),
                                    onclick: {
                                        let (t, l) = (w.word.clone(), language.clone());
                                        move |_| speak(&t, &l, false, None, missing)
                                    },
                                    bdi { lang: target, dir: "ltr", class: if w.arabic.is_some() { "latin bridge-word" } else { "latin" }, "{w.word}" }
                                    Pron { text: w.word.clone(), tag: target, inline: true }
                                }
                                if !answering {
                                    span { class: "ln-meaning muted", Rich { text: pick(&w.meaning, lang).to_string(), target } }
                                    if let Some(ar) = &w.arabic {
                                        span { class: "ln-bridge", title: lt(lang, L::FromArabic), lang: "ar", "{ar}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The button that brings the pane back after it was hidden.
#[component]
pub fn ShowNotes(on_show: EventHandler) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    rsx! {
        button { class: "ghost lp-notes-show", onclick: move |_| on_show.call(()), "{nt(lang, N::Show)}" }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum N {
    Notes,
    ThisLesson,
    Hide,
    Show,
    Learn,
    Slide,
    Practice,
    MeaningsHidden,
    Todo,
    Now,
    Right,
    Missed,
    SkippedOne,
}

fn nt(lang: UiLanguage, msg: N) -> &'static str {
    use N::*;
    use UiLanguage::*;
    match (msg, lang) {
        (Notes, ArEg) => "ملاحظات الدرس",
        (Notes, Ar) => "ملاحظات الدرس",
        (Notes, En) => "Lesson notes",
        (ThisLesson, ArEg) => "الدرس ده",
        (ThisLesson, Ar) => "هذا الدرس",
        (ThisLesson, En) => "This lesson",
        (Hide, ArEg) => "خبّي الملاحظات",
        (Hide, Ar) => "أخفِ الملاحظات",
        (Hide, En) => "Hide notes",
        (Show, ArEg) => "ملاحظات الدرس",
        (Show, Ar) => "ملاحظات الدرس",
        (Show, En) => "Lesson notes",
        (Learn, ArEg) => "نتعلّم",
        (Learn, Ar) => "نتعلّم",
        (Learn, En) => "Learn",
        (Slide, ArEg | Ar) => "شريحة",
        (Slide, En) => "Slide",
        (Practice, ArEg) => "نتمرّن",
        (Practice, Ar) => "نتدرّب",
        (Practice, En) => "Practice",
        (MeaningsHidden, ArEg) => "المعاني مستخبية لحد ما تجاوب، عشان تفتكر لوحدك.",
        (MeaningsHidden, Ar) => "المعاني مخفية حتى تجيب، لتتذكّر بنفسك.",
        (MeaningsHidden, En) => "Meanings are hidden until you answer, so you can recall them yourself.",
        (Todo, ArEg) => "لسه",
        (Todo, Ar) => "لم يبدأ",
        (Todo, En) => "Not yet",
        (Now, ArEg) => "دلوقتي",
        (Now, Ar) => "الآن",
        (Now, En) => "Now",
        (Right, ArEg) => "صح",
        (Right, Ar) => "صحيح",
        (Right, En) => "Right",
        (Missed, ArEg) => "هترجعلها تاني",
        (Missed, Ar) => "ستعود إليه",
        (Missed, En) => "Coming back to it",
        (SkippedOne, ArEg) => "اتعدّت",
        (SkippedOne, Ar) => "تم تخطيه",
        (SkippedOne, En) => "Skipped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_outcome_has_a_non_colour_cue() {
        for o in [Outcome::Correct, Outcome::Almost, Outcome::Missed, Outcome::Practiced, Outcome::Skipped] {
            assert!(!Mark::of(o).glyph().is_empty(), "{o:?} needs a glyph, not only a colour");
        }
    }

    #[test]
    fn outline_marks_earlier_parts_done() {
        assert_eq!(state(Part::Learn, Part::Practice), "ln-part done");
        assert_eq!(state(Part::Practice, Part::Practice), "ln-part current");
        assert_eq!(state(Part::Summary, Part::Practice), "ln-part");
    }
}
