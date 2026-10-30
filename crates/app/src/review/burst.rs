//! Timed burst review presentation.

use super::*;

// ---------------------------------------------------------------- burst

#[component]
pub(super) fn Burst() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let started = engine.session.peek().as_ref().map_or_else(now_ms, |s| s.started_ms);
    let mut hand = use_signal({
        let engine = engine.clone();
        move || engine.pull().map(|id| (id, now_ms()))
    });
    let mut revealed = use_signal(|| false);
    let mut tick = use_signal(now_ms);

    use_future(move || async move {
        let mut eval = document::eval(
            r#"for (let i = 0; i < 400; i++) {
                 await new Promise((r) => setTimeout(r, 250));
                 dioxus.send(i);
               }"#,
        );
        while eval.recv::<u32>().await.is_ok() {
            tick.set(now_ms());
        }
    });
    use_effect({
        let engine = engine.clone();
        move || {
            if tick().saturating_sub(started) >= BURST_MS || hand.read().is_none() {
                engine.finish();
            }
        }
    });

    let Some((card_id, shown_ms)) = hand() else { return rsx! {} };
    let Some(card) = engine.card(&card_id) else { return rsx! {} };
    let left_ms = BURST_MS.saturating_sub(tick().saturating_sub(started));
    let secs = left_ms.div_ceil(1000);

    let answer = {
        let (engine, card_id) = (engine.clone(), card_id.clone());
        move |r: Rating| {
            if engine.answer(&card_id, r, now_ms().saturating_sub(shown_ms)) {
                hand.set(engine.pull().map(|id| (id, now_ms())));
                revealed.set(false);
            }
        }
    };
    let undo = {
        let (engine, card_id) = (engine.clone(), card_id.clone());
        move |_| {
            engine.put_back(card_id.clone());
            engine.undo();
            hand.set(engine.pull().map(|id| (id, now_ms())));
            revealed.set(false);
        }
    };
    let front = match card.kind {
        CardKind::Bridge => card.arabic.as_ref().map(|a| a.word.clone()).unwrap_or_default(),
        _ => card.word.clone(),
    };
    let front_ar = card.kind == CardKind::Bridge;

    rsx! {
        div { class: "burst",
            div { class: "burst-timer", role: "timer",
                span { class: "burst-secs", "{ui.num(secs)}" }
                span { class: "muted small", "{ui.t(Msg::RevSecondsLeft)}" }
                div { class: "progress",
                    span { class: "progress-fill", style: "inline-size: {left_ms * 100 / BURST_MS}%" }
                }
            }
            crate::ui::CardButton {
                key: "{card_id}",
                class: if revealed() { "burst-card revealed" } else { "burst-card" },
                aria_expanded: if revealed() { "true" } else { "false" },
                onclick: move |_| revealed.set(true),
                span { class: if front_ar { "burst-word ar" } else { "burst-word latin" },
                    lang: if front_ar { "ar" } else { card.language.tag() },
                    bdi { "{front}" }
                }
                if revealed() {
                    if front_ar {
                        bdi { class: "burst-sub latin", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                    }
                    span { class: "burst-meaning", "{localized(&ui, &card.meaning)}" }
                    if card.language == StudyLanguage::Jv && !card.verified {
                        span { class: "badge warn", "{ui.t(Msg::RevUnverified)}" }
                    }
                } else {
                    span { class: "muted small", "{ui.t(Msg::RevTapToReveal)}" }
                }
            }
            div { class: "burst-answers",
                button {
                    class: "rate again",
                    onclick: {
                        let mut answer = answer.clone();
                        move |_| answer(Rating::Again)
                    },
                    "{ui.t(Msg::RevNotYet)}"
                }
                button {
                    class: "rate good",
                    onclick: {
                        let mut answer = answer.clone();
                        move |_| answer(Rating::Good)
                    },
                    "{ui.t(Msg::RevKnow)}"
                }
            }
            div { class: "rev-foot",
                span {}
                SessionActions { on_undo: undo }
            }
        }
    }
}
