//! Reusable exercise inputs and answer feedback.

use super::*;

// ---------------------------------------------------------------------------
// Feedback
// ---------------------------------------------------------------------------

/// Spoken-text exercises also teach the meaning in the feedback.
pub(super) fn meaning_of(ex: &Exercise) -> Option<&Localized> {
    match &ex.kind {
        ExerciseKind::Listen { meaning, .. } => meaning.as_ref(),
        ExerciseKind::Dictation { meaning, .. }
        | ExerciseKind::Speaking { meaning, .. }
        | ExerciseKind::FillBlank { meaning, .. } => Some(meaning),
        _ => None,
    }
}
#[component]
pub(super) fn Feedback(
    ex: Exercise,
    grade: Grade,
    retry: bool,
    target: &'static str,
    on_assess: EventHandler<bool>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    // Vary the praise, but the same exercise always gets the same words.
    let praise = [L::Correct1, L::Correct2, L::Correct3][ex.id.bytes().map(usize::from).sum::<usize>() % 3];
    let word = ex.words.first().cloned();
    let meaning = meaning_of(&ex).map(|m| pick(m, lang).to_string());
    // Comprehension: the answer is about the audio, so also show what was said.
    let transcript = match &ex.kind {
        ExerciseKind::Listen { question: Some(_), text, .. } => Some(text.clone()),
        _ => None,
    };
    let explain = ex.explain.as_ref().map(|e| pick(e, lang).to_string());
    let answer = display_answer(&ex, lang);
    let answer_tag = match &ex.kind {
        ExerciseKind::MultipleChoice { options_meaning, .. } if !options_meaning.is_empty() => lang.tag(),
        ExerciseKind::Translate { direction: Direction::FromTarget, .. } => lang.tag(),
        _ => target,
    };
    rsx! {
        div { class: "lp-feedback lp-enter", role: "status", aria_live: "polite",
            match &grade {
                Grade::Correct => rsx! {
                    p { class: "lp-verdict", "{lt(lang, praise)}" }
                    if let Some(w) = &word {
                        p {
                            Rich { text: lt(lang, L::CorrectWord).replace("{w}", w), target }
                            Say { text: w.clone(), tag: target }
                        }
                        Pron { text: w.clone(), tag: target }
                    }
                },
                Grade::Practiced => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::Practiced)}" }
                },
                Grade::Almost { expected } => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::Almost)}" }
                    p { Annotated { text: expected.clone(), tag: target, class: "lp-answer" } }
                },
                Grade::Wrong => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::NotYet)}" }
                    if answer_tag == target {
                        p { Annotated { text: answer.clone(), tag: target, class: "lp-answer" } }
                    } else {
                        p { bdi { lang: answer_tag, dir: "auto", class: "lp-answer", "{answer}" } }
                    }
                    if !retry {
                        p { class: "muted lp-small", "{lt(lang, L::AgainAtEnd)}" }
                    }
                },
                Grade::NeedsReview => rsx! {
                    p { class: "lp-verdict", "{lt(lang, L::NeedsReview)}" }
                    ul { class: "lp-refs",
                        for r in references(&ex, lang).into_iter().take(3) {
                            li { key: "{r}", bdi { lang: answer_tag, dir: if answer_tag == target { "ltr" } else { "auto" }, class: "lp-answer", "{r}" } }
                        }
                    }
                    p { "{lt(lang, L::SameMeaning)}" }
                    div { class: "row",
                        button { class: "primary", onclick: move |_| on_assess.call(true), "{lt(lang, L::YesSame)}" }
                        button { onclick: move |_| on_assess.call(false), "{lt(lang, L::NotQuite)}" }
                    }
                },
            }
            if let Some(t) = transcript {
                p { class: "lp-small",
                    span { class: "muted", "{lt(lang, L::Transcript)} " }
                    Annotated { text: t.clone(), tag: target, class: "lp-target" }
                }
            }
            if let Some(m) = meaning {
                p { class: "muted", Rich { text: m, target } }
            }
            // Keep optional teaching notes discoverable after a correct answer,
            // while showing them automatically when a learner needs help.
            if let Some(e) = explain {
                details {
                    class: "lp-why",
                    open: !matches!(grade, Grade::Correct | Grade::Practiced),
                    summary { "{lt(lang, L::Why)}" }
                    p { Rich { text: e, target } }
                }
            }
        }
    }
}
// ---------------------------------------------------------------------------
// Exercises (standalone: also usable by level tests)
// ---------------------------------------------------------------------------

#[component]
pub fn ExerciseView(
    ex: Exercise,
    language: String,
    draft: Signal<Draft>,
    locked: bool,
    tiles: Vec<String>,
    pair_order: Vec<usize>,
    missing: Signal<bool>,
    on_matched: EventHandler<Vec<String>>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let target = target_tag(&language);
    use ExerciseKind as K;
    match ex.kind.clone() {
        K::MultipleChoice { question, question_meaning, options, options_meaning, answer } => {
            let (labels, tag) = if options_meaning.is_empty() {
                (options, target)
            } else {
                (options_meaning.iter().map(|m| pick(m, lang).to_string()).collect(), lang.tag())
            };
            rsx! {
                div { class: "lp-prompt",
                    if !question.trim().is_empty() {
                        div { class: "row",
                            PlayButton { text: question.clone(), language: language.clone(), audio: None, missing, big: false }
                            p { class: "lp-question pre", bdi { lang: target, dir: "ltr", class: "latin", "{question}" } }
                        }
                    }
                    if let Some(q) = question_meaning {
                        p { class: "lp-question-meaning", Rich { text: pick(&q, lang).to_string(), target } }
                    }
                }
                Choices { labels, tag, draft, locked, answer }
            }
        }
        K::MatchPairs { pairs } => rsx! {
            MatchPairs { pairs: pairs.iter().map(|p| (p.text.clone(), pick(&p.meaning, lang).to_string())).collect::<Vec<_>>(), order: pair_order, target, draft, locked, on_matched }
        },
        K::Translate { direction, source_meaning, source_text, .. } => {
            let (answer_tag, prompt) = match direction {
                Direction::ToTarget => (target, None),
                Direction::FromTarget => (lang.tag(), source_text.clone()),
            };
            rsx! {
                div { class: "lp-prompt",
                    if let Some(m) = &source_meaning {
                        p { class: "lp-question-meaning", Rich { text: pick(m, lang).to_string(), target } }
                    }
                    if let Some(t) = prompt {
                        div { class: "row",
                            PlayButton { text: t.clone(), language: language.clone(), audio: None, missing, big: false }
                            p { class: "lp-question", bdi { lang: target, dir: "ltr", class: "latin", "{t}" } }
                        }
                    }
                }
                TextAnswer { tag: answer_tag, draft, locked }
            }
        }
        K::BuildSentence { source, .. } => rsx! {
            p { class: "lp-question-meaning", Rich { text: pick(&source, lang).to_string(), target } }
            Tiles { tiles, target, draft, locked }
        },
        K::Listen { text, audio, options, answer, question, .. } => rsx! {
            div { class: "lp-prompt center",
                PlayButton { text, language: language.clone(), audio, missing, big: true }
            }
            if let Some(q) = question {
                p { class: "lp-question-meaning", Rich { text: pick(&q, lang).to_string(), target } }
            }
            if options.is_empty() {
                TextAnswer { tag: target, draft, locked }
            } else {
                Choices { labels: options, tag: target, draft, locked, answer: answer.unwrap_or(usize::MAX) }
            }
        },
        K::FillBlank { sentence, meaning, options, answers } => {
            let (before, after) = sentence.split_once("___").unwrap_or((&sentence, ""));
            let (before, after) = (before.to_string(), after.to_string());
            let chosen = draft.read().choice.and_then(|i| options.get(i).cloned());
            let right = options.iter().position(|o| answers.iter().any(|a| a.eq_ignore_ascii_case(o)));
            // "Selamat ___, Pak!": the blank hugs the comma, no gap before it.
            let hug = if blank_hugs_next(&after) { " hug-end" } else { "" };
            rsx! {
                p { class: "lp-question lp-blank-line", lang: target, dir: "ltr",
                    "{before}"
                    if options.is_empty() {
                        input {
                            class: "lp-blank-input{hug}",
                            r#type: "text",
                            lang: target,
                            autocomplete: "off",
                            autocapitalize: "off",
                            spellcheck: "false",
                            disabled: locked,
                            size: "8",
                            value: "{draft.read().text}",
                            oninput: move |e: FormEvent| draft.write().text = e.value(),
                        }
                    } else {
                        span { class: if chosen.is_some() { "lp-blank filled{hug}" } else { "lp-blank{hug}" },
                            "{chosen.clone().unwrap_or_default()}"
                        }
                    }
                    "{after}"
                }
                p { class: "muted", Rich { text: pick(&meaning, lang).to_string(), target } }
                if !options.is_empty() {
                    Choices { labels: options.clone(), tag: target, draft, locked, answer: right.unwrap_or(usize::MAX), chips: true }
                }
            }
        }
        K::Dictation { text, audio, .. } => rsx! {
            div { class: "lp-prompt center",
                PlayButton { text, language: language.clone(), audio, missing, big: true }
            }
            TextAnswer { tag: target, draft, locked }
        },
        K::Speaking { text, meaning } => rsx! {
            div { class: "lp-prompt center",
                p { class: "lp-question big", bdi { lang: target, dir: "ltr", class: "latin", "{text}" } }
                p { class: "muted", Rich { text: pick(&meaning, lang).to_string(), target } }
                PlayButton { text: text.clone(), language: language.clone(), audio: None, missing, big: true }
                p { class: "muted lp-small", "{lt(lang, L::SpeakingLater)}" }
            }
        },
    }
}
/// Option buttons and fill-in chips (keys 1–9 pick them too). After checking, the right one is
/// marked and a wrong pick is shown gently.
#[component]
pub(super) fn Choices(
    labels: Vec<String>,
    tag: &'static str,
    draft: Signal<Draft>,
    locked: bool,
    answer: usize,
    #[props(default)] chips: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let chosen = draft.read().choice;
    rsx! {
        div { class: if chips { "lp-choices chips" } else { "lp-choices" }, role: "group",
            for (i, label) in labels.iter().enumerate() {
                {
                    let mark = choice_mark(locked, i == answer, chosen == Some(i));
                    let aria = mark.map(|m| format!("{label} — {}", lt(lang, m.says())));
                    rsx! {
                        button {
                            key: "{i}",
                            class: match mark {
                                Some(ChoiceMark::Right | ChoiceMark::YoursRight) => "lp-choice right",
                                Some(ChoiceMark::YoursWrong) => "lp-choice wrong",
                                None => "lp-choice",
                            },
                            aria_pressed: if chosen == Some(i) { "true" } else { "false" },
                            aria_label: aria,
                            disabled: locked,
                            onclick: move |_| draft.write().choice = Some(i),
                            if let Some(m) = mark {
                                span { class: "lp-key lp-mark", aria_hidden: "true", "{m.glyph()}" }
                            } else if i < 9 {
                                span { class: "lp-key", aria_hidden: "true", "{i + 1}" }
                            }
                            bdi { lang: tag, dir: "auto", "{label}" }
                        }
                    }
                }
            }
        }
    }
}
// Stateless choice and voice-note decisions are in decision.rs.

/// Free text: typed, Samsung handwriting keyboard, or DeX keyboard. `lang`
/// tells the IME which language to expect. Enter checks (the player's key
/// listener); Shift+Enter is a
/// new line.
#[component]
pub(crate) fn TextAnswer(tag: &'static str, draft: Signal<Draft>, locked: bool) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let dir = if tag.starts_with("ar") { "rtl" } else { "ltr" };
    rsx! {
        textarea {
            class: "lp-text",
            lang: tag,
            dir,
            rows: "3",
            autocomplete: "off",
            autocapitalize: "sentences",
            spellcheck: "false",
            placeholder: lt(lang, L::AnswerPlaceholder),
            disabled: locked,
            value: "{draft.read().text}",
            oninput: move |e: FormEvent| draft.write().text = e.value(),
        }
    }
}
/// Build a sentence: tap a tile in the bank to add it, tap it in the answer
/// to send it back.
#[component]
pub(super) fn Tiles(tiles: Vec<String>, target: &'static str, draft: Signal<Draft>, locked: bool) -> Element {
    let picked = draft.read().picked.clone();
    rsx! {
        div { class: "lp-answer-line", lang: target, dir: "auto",
            for (pos, &i) in picked.iter().enumerate() {
                button {
                    key: "a{i}",
                    class: "lp-tile",
                    disabled: locked,
                    onclick: move |_| {
                        draft.write().picked.remove(pos);
                    },
                    "{tiles[i]}"
                }
            }
        }
        div { class: "lp-bank", lang: target, dir: "auto",
            for (i, t) in tiles.iter().enumerate() {
                button {
                    key: "b{i}",
                    class: if picked.contains(&i) { "lp-tile ghost" } else { "lp-tile" },
                    disabled: locked || picked.contains(&i),
                    onclick: move |_| draft.write().picked.push(i),
                    "{t}"
                }
            }
        }
    }
}
/// Tap a word, then its meaning (either order). Works the same with the pen.
#[component]
pub(super) fn MatchPairs(
    pairs: Vec<(String, String)>,
    order: Vec<usize>,
    target: &'static str,
    draft: Signal<Draft>,
    locked: bool,
    on_matched: EventHandler<Vec<String>>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.settings.read().ui_language;
    let d = draft.read().clone();
    let words: Vec<String> = pairs.iter().map(|p| p.0.clone()).collect();
    let state = |sel: Option<usize>, i: usize, wrong: Option<usize>| pair_class(&d.matched, sel, i, wrong);
    let done_label = |i: usize, text: &str| {
        d.matched.contains(&i).then(|| format!("{text} — {}", lt(lang, L::PairMatched)))
    };
    rsx! {
        div { class: "lp-pairs",
            div { class: "lp-pair-col",
                for (i, (word, _)) in pairs.iter().enumerate() {
                    button {
                        key: "l{i}",
                        class: state(d.left, i, d.wrong.map(|w| w.0)),
                        disabled: locked || d.matched.contains(&i),
                        aria_pressed: if d.left == Some(i) { "true" } else { "false" },
                        aria_label: done_label(i, word),
                        onclick: {
                            let words = words.clone();
                            move |_| {
                                draft.write().left = Some(i);
                                try_match(draft, &words, on_matched);
                            }
                        },
                        if let Some(n) = couple_number(&d.matched, i) {
                            span { class: "lp-pair-tick", aria_hidden: "true", "✓{n}" }
                        }
                        bdi { lang: target, dir: "ltr", class: "latin", "{word}" }
                    }
                }
            }
            div { class: "lp-pair-col",
                for &i in order.iter() {
                    button {
                        key: "r{i}",
                        class: state(d.right, i, d.wrong.map(|w| w.1)),
                        disabled: locked || d.matched.contains(&i),
                        aria_pressed: if d.right == Some(i) { "true" } else { "false" },
                        aria_label: done_label(i, &pairs[i].1),
                        onclick: {
                            let words = words.clone();
                            move |_| {
                                draft.write().right = Some(i);
                                try_match(draft, &words, on_matched);
                            }
                        },
                        if let Some(n) = couple_number(&d.matched, i) {
                            span { class: "lp-pair-tick", aria_hidden: "true", "✓{n}" }
                        }
                        Rich { text: pairs[i].1.clone(), target }
                    }
                }
            }
        }
    }
}
/// Couple tints in lesson.css; a board has at most six pairs.
pub(super) const COUPLE_TINTS: usize = 6;
/// The class of one pair button. A matched couple reads as done, not
/// disabled: both halves share a tint (by the order they were matched), so
/// the learner can see which word went with which meaning.
pub(super) fn pair_class(matched: &[usize], sel: Option<usize>, i: usize, wrong: Option<usize>) -> String {
    if let Some(k) = matched.iter().position(|&m| m == i) {
        format!("lp-pair done couple-{}", k % COUPLE_TINTS)
    } else if wrong == Some(i) {
        "lp-pair wrong".into()
    } else if sel == Some(i) {
        "lp-pair selected".into()
    } else {
        "lp-pair".into()
    }
}
/// The number both halves of a matched couple wear (1 for the first match),
/// so colour is never the only cue (#311).
pub(super) fn couple_number(matched: &[usize], i: usize) -> Option<usize> {
    matched.iter().position(|&m| m == i).map(|k| k + 1)
}
/// Once both sides are selected: a match sticks, a mismatch is remembered as
/// a word to practise. All matched → `on_matched`.
pub(super) fn try_match(mut draft: Signal<Draft>, words: &[String], on_matched: EventHandler<Vec<String>>) {
    let (Some(l), Some(r)) = ({
        let d = draft.peek();
        (d.left, d.right)
    }) else {
        return;
    };
    let done = {
        let mut d = draft.write();
        d.left = None;
        d.right = None;
        if l == r {
            d.matched.push(l);
            d.wrong = None;
        } else {
            d.wrong = Some((l, r));
            if !d.missed.contains(&words[l]) {
                d.missed.push(words[l].clone());
            }
        }
        (d.matched.len() == words.len()).then(|| d.missed.clone())
    };
    if let Some(missed) = done {
        on_matched.call(missed);
    }
}
