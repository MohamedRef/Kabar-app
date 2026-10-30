//! Classic review cards, session controls and undo presentation.

use super::*;

// ---------------------------------------------------------------- card faces

/// The word, isolated from the surrounding UI direction.
#[component]
pub(super) fn Word(card: Card, big: bool) -> Element {
    let ui = use_context::<Ui>();
    let text = card.word.clone();
    let language = card.language;
    rsx! {
        div { class: "rev-word-row",
            bdi { class: if big { "rev-word big" } else { "rev-word" }, lang: language.tag(), dir: "ltr", "{card.word}" }
            button {
                class: "icon-btn",
                aria_label: ui.t(Msg::RevPlay),
                title: ui.t(Msg::RevPlay),
                onclick: move |e| {
                    e.stop_propagation();
                    speak(&text, language);
                },
                SpeakerIcon {}
            }
        }
    }
}
#[component]
pub(super) fn SpeakerIcon() -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            width: "22",
            height: "22",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: "M11 5 6 9H3v6h3l5 4zM15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13" }
        }
    }
}
#[component]
pub(super) fn Badges(card: Card) -> Element {
    let ui = use_context::<Ui>();
    let unverified = card.language == StudyLanguage::Jv && !card.verified;
    rsx! {
        if card.arabic.is_some() || unverified {
            div { class: "rev-badges",
                if card.arabic.is_some() {
                    span { class: "badge gold", "{ui.t(Msg::RevFromArabic)}" }
                }
                if unverified {
                    span { class: "badge warn", "{ui.t(Msg::RevUnverified)}" }
                }
            }
        }
    }
}
/// Javanese words spelled like an Indonesian word with another meaning
/// (awan "midday" ≠ Indonesian awan "cloud"), from the lessons'
/// `new_words[].false_friend` (#293): lowercase word → the Indonesian twin.
/// Read once, the first time a card asks.
pub(super) static JV_TWINS: std::sync::LazyLock<std::collections::HashMap<String, String>> =
    std::sync::LazyLock::new(|| {
        crate::embedded::all(crate::embedded::LESSONS)
            .filter(|(id, text)| id.starts_with('j') && text.contains("false_friend:"))
            .filter_map(|(id, text)| match kabar_core::lesson::parse_lesson(&text) {
                Ok(l) => Some(l),
                Err(e) => {
                    crate::diag::report!("reading {id} for false friends failed: {e}");
                    None
                }
            })
            .flat_map(|l| l.new_words)
            .filter_map(|w| Some((w.word.to_lowercase(), w.false_friend?)))
            .collect()
    });
/// The Indonesian word a Javanese card's word is a false friend of (#379).
pub(super) fn indonesian_twin(card: &Card) -> Option<&'static str> {
    (card.language == StudyLanguage::Jv)
        .then(|| JV_TWINS.get(&card.word.trim().to_lowercase()).map(String::as_str))
        .flatten()
}
/// «≠ not like Indonesian awan»: the meaning memory will offer is the wrong one.
#[component]
pub(super) fn TwinWarning(twin: &'static str) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        span { class: "badge twin",
            span { "≠ {ui.t(Msg::RevNotIndonesian)} " }
            bdi { lang: "id", dir: "ltr", class: "latin", "{twin}" }
        }
    }
}
/// What the learner sees before answering.
#[component]
pub(super) fn Front(card: Card) -> Element {
    let ui = use_context::<Ui>();
    let prompt = match card.kind {
        CardKind::Recognition => Msg::RevPromptMeaning,
        CardKind::Production => Msg::RevPromptProduce,
        CardKind::AudioMeaning => Msg::RevPromptListen,
        CardKind::Cloze => Msg::RevPromptCloze,
        CardKind::Bridge => Msg::RevPromptBridge,
    };
    let gap = card.example.as_ref().and_then(|ex| {
        cloze(&ex.text, &card.word)
            .map(|(a, _, b)| (a.to_string(), b.to_string(), localized(&ui, &ex.meaning).to_string()))
    });
    let (text, language) = (card.word.clone(), card.language);
    rsx! {
        p { class: "rev-prompt", "{ui.t(prompt)}" }
        match card.kind {
            CardKind::Recognition => rsx! { Word { card: card.clone(), big: true } },
            CardKind::Production => rsx! {
                p { class: "rev-meaning big", "{localized(&ui, &card.meaning)}" }
            },
            CardKind::AudioMeaning => rsx! {
                button {
                    class: "rev-listen",
                    aria_label: ui.t(Msg::RevPlay),
                    onclick: move |e| {
                        e.stop_propagation();
                        speak(&text, language);
                    },
                    SpeakerIcon {}
                }
            },
            CardKind::Cloze => match gap {
                Some((before, after, meaning)) => rsx! {
                    p { class: "rev-cloze", lang: card.language.tag(), dir: "ltr",
                        "{before}"
                        span { class: "gap", aria_label: "…", "＿＿＿" }
                        "{after}"
                    }
                    p { class: "muted", "{meaning}" }
                },
                None => rsx! { Word { card: card.clone(), big: true } },
            },
            CardKind::Bridge => rsx! {
                p { class: "rev-arabic", lang: "ar",
                    {card.arabic.as_ref().map(|a| a.word.clone()).unwrap_or_default()}
                }
            },
        }
    }
}
/// The answer side: word, meaning, note, example and the Arabic Bridge.
#[component]
pub(super) fn Back(card: Card) -> Element {
    let ui = use_context::<Ui>();
    // Arrow from the Arabic origin to the word, mirrored with the UI.
    let arrow = if ui.settings.read().ui_language == UiLanguage::En { "←" } else { "→" };
    rsx! {
        div { class: "rev-back",
            // A recognition card already shows the word on its front.
            if card.kind != CardKind::Recognition {
                Word { card: card.clone(), big: false }
            }
            // Only on the answer side: on the front it would hint the answer.
            Pron { text: card.word.clone(), tag: card.language.tag() }
            p { class: "rev-meaning", "{localized(&ui, &card.meaning)}" }
            if let Some(twin) = indonesian_twin(&card) {
                TwinWarning { twin }
            }
            if let Some(note) = &card.note {
                p { class: "muted rev-note", Isolated { text: localized(&ui, note).to_string() } }
            }
            if let Some(ex) = &card.example {
                div { class: "rev-example",
                    Annotated { text: ex.text.clone(), tag: card.language.tag(), meaning: localized(&ui, &ex.meaning).to_string(), class: "ex-text" }
                }
            }
            if let Some(a) = &card.arabic {
                div { class: if a.shift == MeaningShift::FalseFriend { "rev-bridge alarm" } else { "rev-bridge" },
                    div { class: "bridge-line",
                        bdi { class: "latin bridge-word", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                        span { aria_hidden: "true", "{arrow}" }
                        span { class: "ar", lang: "ar", "{a.word}" }
                        if let Some(root) = &a.root {
                            span { class: "root", lang: "ar", "({root})" }
                        }
                    }
                    match a.shift {
                        MeaningShift::FalseFriend => rsx! { p { class: "shift", "{ui.t(Msg::RevFalseFriend)}" } },
                        MeaningShift::Shifted => rsx! { p { class: "shift", "{ui.t(Msg::RevShifted)}" } },
                        MeaningShift::Same => rsx! {},
                    }
                    if let Some(note) = &a.note {
                        p { class: "bridge-note", Isolated { text: localized(&ui, note).to_string() } }
                    }
                }
            }
            Badges { card: card.clone() }
        }
    }
}
// ---------------------------------------------------------------- classic

#[component]
pub(super) fn Classic() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let mut hand = use_signal({
        let engine = engine.clone();
        move || engine.pull().map(|id| (id, now_ms()))
    });
    let mut revealed = use_signal(|| false);
    let mut typed = use_signal(String::new);
    let mut checked = use_signal(|| None::<AnswerMatch>);

    use_effect({
        let engine = engine.clone();
        move || {
            if hand.read().is_none() {
                engine.finish();
            }
        }
    });
    // Audio cards speak as soon as they are shown.
    use_effect({
        let engine = engine.clone();
        move || {
            if let Some((id, _)) = hand.read().as_ref()
                && let Some(card) = engine.deck.peek().card(id)
                && card.kind == CardKind::AudioMeaning
            {
                speak(&card.word, card.language);
            }
        }
    });

    // Hand the next card over with a clean slate.
    let mut advance = move |engine: &Engine| {
        hand.set(engine.pull().map(|id| (id, now_ms())));
        revealed.set(false);
        typed.set(String::new());
        checked.set(None);
    };
    let rate = use_callback({
        let engine = engine.clone();
        move |r: Rating| {
            // Only a revealed card is rated: a quick second tap must not
            // rate the card that just replaced it.
            if !revealed() {
                return;
            }
            let Some((card_id, shown_ms)) = hand() else { return };
            if engine.answer(&card_id, r, now_ms().saturating_sub(shown_ms)) {
                advance(&engine);
            }
        }
    });
    let undo = use_callback({
        let engine = engine.clone();
        move |()| {
            if let Some((card_id, _)) = hand() {
                engine.put_back(card_id);
            }
            engine.undo();
            advance(&engine);
        }
    });
    // Keys come from a document-level listener, so they work wherever focus
    // is (a button that just became disabled drops it to the body).
    let on_key = use_callback(move |key: String| match key.as_str() {
        " " if !revealed() => revealed.set(true),
        "1" | "2" | "3" | "4" if revealed() => {
            let i = key.parse::<usize>().unwrap_or(1) - 1;
            rate.call(Rating::ALL[i]);
        }
        "undo" => undo.call(()),
        _ => {}
    });
    use_future(move || async move {
        let mut eval = document::eval(KEYS_JS);
        while let Ok(key) = eval.recv::<String>().await {
            on_key.call(key);
        }
    });

    let Some((card_id, _)) = hand() else { return rsx! {} };
    let Some(card) = engine.card(&card_id) else { return rsx! {} };
    let now = now_ms();
    let previews = engine.sched.preview(&engine.deck.read().state(&card_id), now);
    let left = engine.session.read().as_ref().map_or(0, Session::remaining);
    let is_production = card.kind == CardKind::Production;
    let suggested = checked().map(AnswerMatch::suggested);
    // Expanded+ puts the session's progress beside the card (docs/DESIGN.md).
    let wide = *ui.width.read() >= WidthClass::Expanded;

    let check = {
        let word = card.word.clone();
        move || {
            checked.set(Some(compare_answer(&typed.read(), &word)));
            revealed.set(true);
        }
    };

    rsx! {
        div { id: "review-classic", class: if wide { "rev-session with-pane" } else { "rev-session" },
            SessionBar { left }
            crate::ui::Card { element: crate::ui::CardElement::Article,
                key: "{card_id}",
                class: "rev-card",
                aria_live: "polite",
                Front { card: card.clone() }
                if is_production {
                    form {
                        class: "rev-answer",
                        onsubmit: {
                            let mut check = check.clone();
                            move |e: FormEvent| {
                                e.prevent_default();
                                if !revealed() {
                                    check();
                                }
                            }
                        },
                        input {
                            r#type: "text",
                            lang: card.language.tag(),
                            dir: "ltr",
                            autocomplete: "off",
                            autocapitalize: "off",
                            spellcheck: "false",
                            readonly: revealed(),
                            placeholder: ui.t(Msg::RevAnswerPlaceholder),
                            aria_label: ui.t(Msg::RevPromptProduce),
                            value: "{typed}",
                            oninput: move |e: FormEvent| typed.set(e.value()),
                            onmounted: move |e: MountedEvent| async move {
                                let _ = e.set_focus(true).await;
                            },
                        }
                    }
                    if let Some(m) = checked() {
                        p { class: if m == AnswerMatch::Exact { "rev-verdict ok" } else { "rev-verdict" },
                            {
                                match m {
                                    AnswerMatch::Exact => ui.t(Msg::RevExact),
                                    AnswerMatch::Close => ui.t(Msg::RevClose),
                                    AnswerMatch::Wrong => ui.t(Msg::RevWrong),
                                }
                            }
                        }
                        if m != AnswerMatch::Exact && !typed.read().trim().is_empty() {
                            p { class: "muted",
                                "{ui.t(Msg::RevYouWrote)}: "
                                bdi { lang: card.language.tag(), dir: "ltr", class: "struck", "{typed}" }
                            }
                        }
                    }
                }
                if revealed() {
                    Back { card: card.clone() }
                }
            }
            // Whether the card's content is right is its own question, kept
            // apart from the memory rating below.
            if revealed() && card.language == StudyLanguage::Jv {
                ContentRow { card: card.clone() }
            }
            // The footer keeps its place: one primary action before the
            // answer shows, the four ratings (with their real next
            // intervals) after.
            div { class: "rev-footer",
            if revealed() {
                p { class: "rev-ask", id: "rev-ask",
                    strong { "{ui.t(Msg::RevHowWell)}" }
                    span { class: "muted small", " {ui.t(Msg::RevRateHint)}" }
                }
                div { class: "rate-row rating-row", role: "group", aria_labelledby: "rev-ask",
                    for (i, p) in previews.iter().enumerate() {
                        {
                            let r = p.rating;
                            let span = span_short(&ui, Span::from_days(p.interval_days));
                            let hint = if suggested == Some(r) { " suggested" } else { "" };
                            let class = format!("rating {}{hint}", rate_class(r));
                            rsx! {
                                button {
                                    key: "{i}",
                                    class,
                                    "aria-keyshortcuts": "{i + 1}",
                                    onclick: move |_| rate.call(r),
                                    span { class: "rate-label", "{ui.t(rating_msg(r))}" }
                                    small { class: "rate-span", "{span}" }
                                }
                            }
                        }
                    }
                }
            } else if is_production {
                div { class: "rate-row",
                    button {
                        class: "primary reveal-btn",
                        onclick: {
                            let mut check = check.clone();
                            move |_| check()
                        },
                        "{ui.t(Msg::RevCheck)}"
                    }
                }
            } else {
                div { class: "rate-row",
                    button {
                        class: "primary reveal-btn",
                        "aria-keyshortcuts": "Space",
                        onclick: move |_| revealed.set(true),
                        "{ui.t(Msg::RevShowAnswer)}"
                    }
                }
            }
            }
            div { class: "rev-foot",
                span { class: "muted small kbd-hint", {ui.digits(ui.t(Msg::RevKeysHint))} }
                SessionActions { on_undo: move |_| undo.call(()) }
            }
            if wide {
                SessionPane { left, on_undo: move |_| undo.call(()) }
            }
        }
    }
}
/// A Javanese card's content check, separate from how well it was
/// remembered: unchecked until the learner says the card is right.
#[component]
pub(super) fn ContentRow(card: Card) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    rsx! {
        div { class: "rev-content", role: "status",
            if card.verified {
                span { class: "muted small", "{ui.t(Msg::RevContentChecked)}" }
            } else {
                span { class: "muted small", "{ui.t(Msg::RevContentUnchecked)}" }
                button {
                    class: "ghost rev-content-ok",
                    onclick: move |_| engine.verify_card(&card),
                    "{ui.t(Msg::RevContentOk)}"
                }
            }
        }
    }
}
/// How many recent answers the side pane lists.
pub(super) const RECENT: usize = 8;
/// The side pane on wide windows: answer counts per rating and the words just
/// reviewed, newest first, with undo on the latest. Fills the margin with the
/// session instead of leaving it empty.
#[component]
pub(super) fn SessionPane(left: usize, on_undo: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let session = engine.session.read();
    let answers = session.as_ref().map_or(&[][..], Session::answers);
    let count = |r: Rating| answers.iter().filter(|a| a.rating == r).count() as u64;
    let recent: Vec<(String, Card, Rating)> = answers
        .iter()
        .rev()
        .take(RECENT)
        .filter_map(|a| engine.card(&a.card_id).map(|c| (a.event_id.clone(), c, a.rating)))
        .collect();
    let can_undo = engine.can_undo();
    rsx! {
        aside { class: "rev-pane", aria_label: ui.t(Msg::RevThisSession),
            h2 { class: "rev-pane-title", "{ui.t(Msg::RevThisSession)}" }
            div { class: "rev-pane-counts",
                div { class: "rev-pane-big",
                    span { class: "rev-stat-value", "{ui.num(answers.len() as u64)}" }
                    span { class: "rev-stat-label", "{ui.t(Msg::RevAnswered)}" }
                }
                div { class: "rev-pane-big",
                    span { class: "rev-stat-value", "{ui.num((left + 1) as u64)}" }
                    span { class: "rev-stat-label", "{ui.t(Msg::RevRemaining)}" }
                }
            }
            ul { class: "rev-pane-ratings",
                for r in Rating::ALL {
                    li { key: "{r:?}", class: rate_class(r),
                        span { class: "rev-pane-n", "{ui.num(count(r))}" }
                        span { "{ui.t(rating_msg(r))}" }
                    }
                }
            }
            h3 { class: "rev-h3", "{ui.t(Msg::RevJustReviewed)}" }
            if recent.is_empty() {
                p { class: "muted small", "{ui.t(Msg::RevNoneYet)}" }
            } else {
                ol { class: "rev-recent",
                    for (i, (key, card, r)) in recent.into_iter().enumerate() {
                        li { key: "{key}",
                            bdi { class: "latin rev-recent-word", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                            span { class: "pill {rate_class(r)}", "{ui.t(rating_msg(r))}" }
                            if i == 0 && can_undo {
                                button {
                                    class: "ghost rev-recent-undo",
                                    title: ui.t(Msg::RevUndo),
                                    aria_label: ui.t(Msg::RevUndo),
                                    "aria-keyshortcuts": "Control+Z Meta+Z",
                                    onclick: move |_| on_undo.call(()),
                                    "↶"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
/// The rating's colour class, shared by the answer buttons and the pane.
pub(super) fn rate_class(r: Rating) -> &'static str {
    match r {
        Rating::Again => "again",
        Rating::Hard => "hard",
        Rating::Good => "good",
        Rating::Easy => "easy",
    }
}
#[component]
pub(super) fn SessionBar(left: usize) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let done = engine.session.read().as_ref().map_or(0, |s| s.answers().len());
    let total = done + left + 1;
    rsx! {
        div { class: "session-bar",
            div {
                class: "progress progress-line",
                role: "progressbar",
                aria_valuemin: "0",
                aria_valuemax: "{total}",
                aria_valuenow: "{done}",
                span { class: "progress-fill fill", style: "inline-size: {done * 100 / total.max(1)}%" }
            }
            // The card in hand out of the session, read left to right.
            span { class: "muted small ltr-count", dir: "ltr", "{ui.num((done + 1) as u64)} / {ui.num(total as u64)}" }
            span { class: "muted small", {ui.t(Msg::RevLeft).replace("{n}", &ui.num((left + 1) as u64))} }
        }
    }
}
#[component]
pub(super) fn SessionActions(on_undo: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let can_undo = engine.can_undo();
    rsx! {
        div { class: "row",
            button { class: "rev-undo", disabled: !can_undo, onclick: move |_| on_undo.call(()), "{ui.t(Msg::RevUndo)}" }
            // Android Back ends the session here (its answers are already saved).
            button { "data-back": "true", onclick: move |_| engine.finish(), "{ui.t(Msg::RevFinish)}" }
        }
    }
}
