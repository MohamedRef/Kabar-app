//! Sortable review wall and native drag interactions.

use super::*;

// ---------------------------------------------------------------- card wall

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Slot {
    pub(super) card_id: String,
    pub(super) dealt_ms: u64,
    pub(super) flipped: bool,
}
#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub(super) enum FromWall {
    Flip { id: String },
    Drop { id: String, pile: Pile },
}
/// A small, stable tilt per card so the table looks dealt by hand.
pub(super) fn tilt(card_id: &str) -> i32 {
    let h = card_id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    (h % 7) as i32 - 3
}
#[component]
pub(super) fn CardWall() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let seats = match *ui.width.peek() {
        WidthClass::Compact => 1,
        WidthClass::Medium => 4,
        _ => 6,
    };
    let mut slots = use_signal({
        let engine = engine.clone();
        move || {
            let now = now_ms();
            (0..seats)
                .map(|_| engine.pull().map(|card_id| Slot { card_id, dealt_ms: now, flipped: false }))
                .collect::<Vec<_>>()
        }
    });
    let mut last_sort = use_signal(now_ms);
    let mut freed = use_signal(Vec::<usize>::new);

    let sort = use_callback({
        let engine = engine.clone();
        move |(card_id, pile): (String, Pile)| {
            let Some(i) = slots.peek().iter().position(|s| s.as_ref().is_some_and(|s| s.card_id == card_id))
            else {
                return;
            };
            let slot = slots.peek()[i].clone().expect("slot is filled");
            let now = now_ms();
            // Sorting time: from when this card could get attention (dealt,
            // or the previous card left the table) to the drop.
            let since = slot.dealt_ms.max(*last_sort.peek());
            let sort_ms = now.saturating_sub(since).min(u64::from(u32::MAX)) as u32;
            let rating = wall_rating(pile, sort_ms, slot.flipped);
            if engine.answer(&card_id, rating, now.saturating_sub(since)) {
                last_sort.set(now);
                freed.write().push(i);
                slots.write()[i] =
                    engine.pull().map(|card_id| Slot { card_id, dealt_ms: now, flipped: false });
                if slots.peek().iter().all(Option::is_none) {
                    engine.finish();
                }
            }
        }
    });
    let flip = use_callback(move |card_id: String| {
        if let Some(s) = slots.write().iter_mut().flatten().find(|s| s.card_id == card_id) {
            s.flipped = !s.flipped;
        }
    });

    use_future(move || async move {
        let mut eval = document::eval(WALL_JS);
        while let Ok(msg) = eval.recv::<FromWall>().await {
            match msg {
                FromWall::Flip { id } => flip.call(id),
                FromWall::Drop { id, pile } => sort.call((id, pile)),
            }
        }
    });

    let undo = {
        let engine = engine.clone();
        move |_| {
            let Some(i) = freed.write().pop() else { return };
            if let Some(Some(current)) = slots.peek().get(i).cloned() {
                engine.put_back(current.card_id);
            }
            if engine.undo() {
                slots.write()[i] =
                    engine.pull().map(|card_id| Slot { card_id, dealt_ms: now_ms(), flipped: false });
            }
        }
    };

    let (tahu, belum) = engine.session.read().as_ref().map_or((0, 0), |s| {
        let belum = s.answers().iter().filter(|a| a.rating == Rating::Again).count();
        (s.answers().len() - belum, belum)
    });
    let left = engine.session.read().as_ref().map_or(0, Session::remaining);
    let seat_view: Vec<(String, Option<Slot>)> = slots
        .read()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                s.as_ref().map_or_else(|| format!("empty-{i}"), |s| format!("{}@{}", s.card_id, s.dealt_ms)),
                s.clone(),
            )
        })
        .collect();

    rsx! {
        div { class: "wall",
            p { class: "muted small wall-help", "{ui.t(Msg::RevWallHelp)}" }
            div { id: "card-wall", class: "wall-table", "data-seats": "{seats}",
                // Seats are keyed by their card and deal time, so a new card,
                // or a "belum" card dealt again, gets a fresh element (no
                // leftover drag transform from the last throw).
                for (seat_key, slot) in seat_view {
                    div { key: "{seat_key}", class: "seat",
                        if let Some(slot) = slot {
                            if let Some(card) = engine.card(&slot.card_id) {
                                WallCard { card, flipped: slot.flipped, sort }
                            }
                        }
                    }
                }
            }
            div { class: "piles",
                div { class: "pile belum", "data-pile": "belum",
                    span { class: "pile-word latin", lang: "id", "belum" }
                    span { class: "pile-label", "{ui.t(Msg::RevBelum)}" }
                    span { class: "pile-count", "{ui.num(belum as u64)}" }
                }
                div { class: "pile tahu", "data-pile": "tahu",
                    span { class: "pile-word latin", lang: "id", "tahu" }
                    span { class: "pile-label", "{ui.t(Msg::RevTahu)}" }
                    span { class: "pile-count", "{ui.num(tahu as u64)}" }
                }
            }
            div { class: "rev-foot",
                span { class: "muted small", {ui.t(Msg::RevLeft).replace("{n}", &ui.num(left as u64))} }
                SessionActions { on_undo: undo }
            }
        }
    }
}
#[component]
pub(super) fn WallCard(card: Card, flipped: bool, sort: Callback<(String, Pile)>) -> Element {
    let ui = use_context::<Ui>();
    let id = card.id.clone();
    let front = match card.kind {
        CardKind::Bridge => card.arabic.as_ref().map(|a| a.word.clone()).unwrap_or_default(),
        _ => card.word.clone(),
    };
    let front_lang = if card.kind == CardKind::Bridge { "ar" } else { card.language.tag() };
    let label = if flipped { localized(&ui, &card.meaning).to_string() } else { front.clone() };
    let false_friend = card.arabic.as_ref().is_some_and(|a| a.shift == MeaningShift::FalseFriend);
    rsx! {
        div {
            class: if flipped { "wall-card flipped" } else { "wall-card" },
            "data-id": "{id}",
            role: "group",
            aria_label: "{label}",
            style: "--tilt: {tilt(&id)}deg",
            tabindex: "0",
            div { class: "face front",
                if card.arabic.is_some() {
                    span { class: "dot-gold", aria_hidden: "true" }
                }
                span { class: if front_lang == "ar" { "wall-word ar" } else { "wall-word latin" }, lang: front_lang,
                    bdi { "{front}" }
                }
            }
            div { class: "face back",
                if card.kind == CardKind::Bridge {
                    bdi { class: "wall-word small latin", lang: card.language.tag(), dir: "ltr", "{card.word}" }
                    Say { text: card.word.clone(), tag: card.language.tag() }
                }
                Pron { text: card.word.clone(), tag: card.language.tag() }
                span { class: "wall-meaning", "{localized(&ui, &card.meaning)}" }
                if false_friend {
                    span { class: "badge alarm", "{ui.t(Msg::RevFalseFriend)}" }
                }
                if let Some(twin) = indonesian_twin(&card) {
                    TwinWarning { twin }
                }
                if card.language == StudyLanguage::Jv && !card.verified {
                    span { class: "badge warn", "{ui.t(Msg::RevUnverified)}" }
                }
            }
            // Keyboard and screen-reader path; shown on focus.
            div { class: "wall-keys",
                button {
                    onclick: {
                        let id = id.clone();
                        move |_| sort.call((id.clone(), Pile::Belum))
                    },
                    "{ui.t(Msg::RevBelum)}"
                }
                button {
                    onclick: {
                        let id = id.clone();
                        move |_| sort.call((id.clone(), Pile::Tahu))
                    },
                    "{ui.t(Msg::RevTahu)}"
                }
            }
        }
    }
}
