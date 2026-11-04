//! Review screens driven through their real buttons: the starter deck,
//! classic cards with typed answers and undo, the Card Wall, bursts, the
//! session summary, and what each leaves in the review log.

use kabar_core::layout::WidthClass;
use kabar_protocol::lesson::{Example, Localized};
use kabar_protocol::srs::{ArabicOrigin, CardSource};

use super::*;
use crate::lesson::sim::{self, Node, Sim};

thread_local! {
    static WIDTH: std::cell::Cell<WidthClass> = const { std::cell::Cell::new(WidthClass::Compact) };
    static LANG: std::cell::Cell<UiLanguage> = const { std::cell::Cell::new(UiLanguage::En) };
    /// Cards stored before the screen opens.
    static CARDS: RefCell<Vec<Card>> = const { RefCell::new(Vec::new()) };
}

fn app() -> Element {
    let (ui, db) = sim::provide(WIDTH.get());
    use_hook(|| {
        let mut ui = ui;
        ui.settings.write().ui_language = LANG.get();
        ui.screen.set(Screen::Review);
        db.0.borrow_mut().upsert_cards(&CARDS.with_borrow(Clone::clone)).unwrap();
    });
    rsx! {
        if *ui.screen.read() == Screen::Review {
            ReviewScreen {}
        }
    }
}

fn open(width: WidthClass, lang: UiLanguage, cards: Vec<Card>) -> Sim {
    WIDTH.set(width);
    LANG.set(lang);
    CARDS.set(cards);
    Sim::new(app)
}

fn en(m: Msg) -> String {
    esc(crate::i18n::t(UiLanguage::En, m))
}

/// Text as SSR writes it.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('\'', "&#39;")
}

fn db(sim: &mut Sim) -> Db {
    sim.with(consume_context::<Db>)
}

fn log_len(sim: &mut Sim) -> usize {
    db(sim).0.borrow().review_log().unwrap().len()
}

fn button_text(text: String) -> impl Fn(&Node) -> bool {
    move |n| n.tag == "button" && n.text.contains(&text)
}

fn word(word: &str, kind: CardKind, language: StudyLanguage) -> Card {
    let l = |s: &str| Localized { ar_eg: s.into(), ar: s.into(), en: s.into() };
    Card {
        v: 1,
        id: format!("t.{word}.{kind:?}"),
        word: word.into(),
        language,
        kind,
        meaning: l(&format!("meaning of {word}")),
        note: Some(l("a \u{2068}note\u{2069} here")),
        example: Some(Example { text: format!("Saya {word} sekarang."), meaning: l("I do it now.") }),
        arabic: None,
        source: CardSource::Lesson,
        source_ref: None,
        verified: false,
    }
}

/// An empty deck invites the starter words; each opens its Bridge card,
/// and the button adds them all to the deck.
#[test]
fn an_empty_deck_offers_the_starter_words() {
    let mut sim = open(WidthClass::Compact, UiLanguage::En, vec![]);
    let html = sim.html();
    assert!(html.contains(&en(Msg::RevSeedTitle)), "{html}");
    let first = seed_words().next().unwrap();
    sim.click(|n| n.has_class("seed-word") && n.text == first);
    let html = sim.html();
    assert!(html.contains("id=\"seed-card\""), "{html}");
    assert!(html.contains("rev-bridge"), "the Bridge card shows the Arabic origin: {html}");
    assert!(sim.find(|n| n.has_class("seed-word") && n.attr("aria-pressed") == Some("true")).is_some());
    // A second tap closes it.
    sim.click(|n| n.has_class("seed-word") && n.text == first);
    assert!(!sim.html().contains("id=\"seed-card\""));
    sim.click(|n| n.tag == "button" && n.has_class("primary"));
    let n = db(&mut sim).0.borrow().cards().unwrap().len();
    assert_eq!(n, seed_bridge_deck().len());
    let html = sim.html();
    assert!(html.contains("rev-stats") && html.contains(&en(Msg::RevDue)), "{html}");
}

/// Show answer, rate, undo, finish: each tap changes the log and the
/// session the way the learner expects.
#[test]
fn a_classic_round_rates_undoes_and_finishes_with_a_summary() {
    let mut sim = open(WidthClass::Compact, UiLanguage::En, seed_bridge_deck());
    let html = sim.html();
    assert!(html.contains(&en(Msg::RevNew)), "{html}");
    // The "new" number starts a classic round of new cards.
    sim.click(|n| n.has_class("rev-stat") && n.text.contains(crate::i18n::t(UiLanguage::En, Msg::RevNew)));
    let html = sim.html();
    assert!(html.contains("id=\"review-classic\""), "{html}");
    assert!(sim.find(|n| n.has_class("rev-undo")).unwrap().disabled(), "nothing to undo yet");
    sim.click(|n| n.has_class("reveal-btn"));
    let html = sim.html();
    assert!(html.contains("rev-back") && html.contains(&en(Msg::RevHowWell)), "{html}");
    assert_eq!(sim.count(|n| n.has_class("rating")), 4);
    sim.click(|n| n.has_class("rating") && n.has_class("good"));
    assert_eq!(log_len(&mut sim), 1);
    assert!(sim.html().contains(">2 / "), "the next card is in hand");
    // Undo takes it back: the log entry goes, the counter steps back.
    sim.click(|n| n.has_class("rev-undo"));
    assert_eq!(log_len(&mut sim), 0);
    assert!(sim.html().contains(">1 / "));
    sim.click(|n| n.has_class("reveal-btn"));
    sim.click(|n| n.has_class("rating") && n.has_class("again"));
    sim.click(|n| n.has_class("reveal-btn"));
    sim.click(|n| n.has_class("rating") && n.has_class("easy"));
    assert_eq!(log_len(&mut sim), 2);
    sim.click(button_text(crate::i18n::t(UiLanguage::En, Msg::RevFinish).into()));
    let html = sim.html();
    assert!(html.contains(&en(Msg::RevDoneTitle)), "{html}");
    assert!(html.contains(&en(Msg::RevRevisit)), "the Again card is listed to revisit: {html}");
    assert!(html.contains(&en(Msg::RevStrongest)), "{html}");
    sim.click(|n| n.tag == "button" && n.has_class("primary"));
    assert!(sim.html().contains("rev-modes"), "back home");
}

/// Finishing with nothing answered just goes back home.
#[test]
fn finishing_before_answering_goes_home() {
    let mut sim = open(WidthClass::Compact, UiLanguage::ArEg, seed_bridge_deck());
    sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
    assert!(sim.html().contains("id=\"review-classic\""));
    sim.click(|n| n.attr("data-back") == Some("true"));
    assert!(sim.html().contains("rev-modes"));
    assert_eq!(log_len(&mut sim), 0);
}

/// A production card is typed; the check says how close it was and
/// suggests a rating.
#[test]
fn production_cards_check_the_typed_word() {
    let cards = vec![word("makan", CardKind::Production, StudyLanguage::Id)];
    for (typed, verdict) in [("makan", Msg::RevExact), ("makon", Msg::RevClose), ("tidur", Msg::RevWrong)] {
        let mut sim = open(WidthClass::Compact, UiLanguage::En, cards.clone());
        sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
        let html = sim.html();
        assert!(html.contains(&en(Msg::RevPromptProduce)), "{html}");
        assert!(html.contains("meaning of makan"), "the front shows the meaning");
        sim.input(|n| n.tag == "input" && n.attr("type") == Some("text"), typed);
        if typed == "makon" {
            sim.submit(|n| n.has_class("rev-answer"));
        } else {
            sim.click(|n| n.has_class("reveal-btn"));
        }
        let html = sim.html();
        assert!(html.contains(&en(verdict)), "{typed}: {html}");
        assert!(sim.find(|n| n.has_class("suggested")).is_some(), "{typed}: a suggested rating");
        if verdict != Msg::RevExact {
            assert!(html.contains(&format!("class=\"struck\">{typed}<")), "{html}");
        }
        sim.click(|n| n.has_class("rating") && n.has_class("hard"));
        assert_eq!(log_len(&mut sim), 1);
        // The last card done: the round ends by itself.
        assert!(sim.html().contains(&en(Msg::RevDoneTitle)));
    }
}

/// Every card kind has its own front, and the back shows note, example and
/// the Arabic Bridge with its false-friend warning.
#[test]
fn each_card_kind_has_its_front_and_a_full_back() {
    let mut bridge = word("kabar", CardKind::Bridge, StudyLanguage::Id);
    bridge.arabic = Some(ArabicOrigin {
        word: "خبر".into(),
        root: Some("خ ب ر".into()),
        shift: MeaningShift::FalseFriend,
        note: Some(Localized { ar_eg: "x".into(), ar: "x".into(), en: "news, not only".into() }),
    });
    let cases = [
        (word("air", CardKind::Recognition, StudyLanguage::Id), Msg::RevPromptMeaning, "rev-word big"),
        (word("dengar", CardKind::AudioMeaning, StudyLanguage::Id), Msg::RevPromptListen, "rev-listen"),
        (word("baca", CardKind::Cloze, StudyLanguage::Id), Msg::RevPromptCloze, "class=\"gap\""),
        (bridge, Msg::RevPromptBridge, "rev-arabic"),
    ];
    for (card, prompt, front) in cases {
        let kind = card.kind;
        let mut sim = open(WidthClass::Compact, UiLanguage::En, vec![card]);
        sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
        let html = sim.html();
        assert!(html.contains(&en(prompt)) && html.contains(front), "{kind:?}: {html}");
        sim.click(|n| n.has_class("reveal-btn"));
        let html = sim.html();
        assert!(
            html.contains("<span>a </span><bdi class=\"latin\" dir=\"ltr\">note</bdi><span> here</span>"),
            "{kind:?}: {html}"
        );
        assert!(html.contains("rev-example"), "{kind:?}");
        if kind == CardKind::Bridge {
            assert!(html.contains("rev-bridge alarm") && html.contains(&en(Msg::RevFalseFriend)), "{html}");
            assert!(html.contains("(خ ب ر)") && html.contains("news, not only"));
            assert!(html.contains(&en(Msg::RevFromArabic)));
        }
        // The play buttons speak (no voice here: nothing breaks).
        sim.click(|n| n.has_class("icon-btn") || n.has_class("rev-listen"));
    }
}

/// A Javanese card asks separately whether its content is right; saying so
/// marks it checked without touching its schedule.
#[test]
fn javanese_cards_can_be_checked_apart_from_the_rating() {
    let card = word("awan", CardKind::Recognition, StudyLanguage::Jv);
    let mut sim = open(WidthClass::Compact, UiLanguage::En, vec![card.clone()]);
    sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
    sim.click(|n| n.has_class("reveal-btn"));
    let html = sim.html();
    assert!(html.contains(&en(Msg::RevUnverified)) && html.contains(&en(Msg::RevContentUnchecked)), "{html}");
    assert!(html.contains("badge twin"), "awan has an Indonesian twin: {html}");
    sim.click(|n| n.has_class("rev-content-ok"));
    let stored = db(&mut sim).0.borrow().cards().unwrap();
    assert!(stored.iter().find(|c| c.id == card.id).unwrap().verified);
    assert_eq!(log_len(&mut sim), 0, "checking content isn't a review");
    assert!(sim.html().contains(&en(Msg::RevContentChecked)));
}

/// Wide windows list the session beside the card, with undo on the latest.
#[test]
fn wide_windows_show_the_session_pane() {
    let mut sim = open(WidthClass::Expanded, UiLanguage::ArEg, seed_bridge_deck());
    sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
    let html = sim.html();
    assert!(
        html.contains("rev-pane") && html.contains(&esc(crate::i18n::t(UiLanguage::ArEg, Msg::RevNoneYet)))
    );
    sim.click(|n| n.has_class("reveal-btn"));
    sim.click(|n| n.has_class("rating") && n.has_class("good"));
    let html = sim.html();
    assert!(html.contains("rev-recent"), "{html}");
    sim.click(|n| n.has_class("rev-recent-undo"));
    assert_eq!(log_len(&mut sim), 0);
}

/// A save that fails says so and keeps the card in hand.
#[test]
fn a_failed_save_shows_an_alert_and_keeps_the_card() {
    let mut sim = open(WidthClass::Compact, UiLanguage::En, seed_bridge_deck());
    sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
    sim.click(|n| n.has_class("reveal-btn"));
    db(&mut sim).0.borrow().refuse_writes();
    sim.click(|n| n.has_class("rating") && n.has_class("good"));
    let html = sim.html();
    assert!(html.contains("role=\"alert\"") && html.contains(&en(Msg::RevSaveFailed)), "{html}");
    assert!(html.contains(">1 / "), "the same card is still in hand");
    db(&mut sim).0.borrow().allow_writes();
    sim.click(|n| n.has_class("rating") && n.has_class("good"));
    assert_eq!(log_len(&mut sim), 1);
    assert!(!sim.html().contains("role=\"alert\""), "a good save clears the alert");
}

/// A burst flips cards by tap and rates them with two buttons.
#[test]
fn a_burst_flips_and_rates_with_two_buttons() {
    let mut sim = open(WidthClass::Compact, UiLanguage::En, seed_bridge_deck());
    sim.click(|n| n.has_class("rev-mode") && n.has_class("mode-burst") && n.has_class("card-action"));
    let html = sim.html();
    assert!(html.contains("role=\"timer\"") && html.contains(&en(Msg::RevTapToReveal)), "{html}");
    sim.click(|n| n.has_class("burst-card"));
    let html = sim.html();
    assert!(html.contains("burst-card revealed") && html.contains("burst-meaning"), "{html}");
    sim.click(|n| n.has_class("rate") && n.has_class("good"));
    sim.click(|n| n.has_class("rate") && n.has_class("again"));
    let log = db(&mut sim).0.borrow().review_log().unwrap();
    assert_eq!(log.iter().map(|e| e.rating).collect::<Vec<_>>(), vec![Rating::Good, Rating::Again]);
    assert!(log.iter().all(|e| e.mode == ReviewMode::Burst));
    sim.click(|n| n.has_class("rev-undo"));
    assert_eq!(log_len(&mut sim), 1);
    sim.click(|n| n.attr("data-back") == Some("true"));
    assert!(sim.html().contains(&en(Msg::RevDoneTitle)));
}

/// The Card Wall deals one card per seat; sorting by the keyboard path
/// rates it, undo puts it back, and the last card ends the round.
#[test]
fn the_card_wall_sorts_into_piles_and_undoes() {
    // Three different words, both card kinds each: siblings wait their turn.
    let cards: Vec<Card> = seed_bridge_deck().into_iter().take(6).collect();
    let mut sim = open(WidthClass::Medium, UiLanguage::En, cards);
    sim.click(|n| n.has_class("rev-mode") && n.text.contains(crate::i18n::t(UiLanguage::En, Msg::RevWall)));
    let html = sim.html();
    assert!(html.contains("id=\"card-wall\"") && html.contains("data-seats=\"4\""), "{html}");
    let dealt = sim.count(|n| n.has_class("wall-card"));
    assert!((2..=4).contains(&dealt), "{dealt} cards dealt");
    let tahu = crate::i18n::t(UiLanguage::En, Msg::RevTahu).to_string();
    let belum = crate::i18n::t(UiLanguage::En, Msg::RevBelum).to_string();
    sim.click({
        let t = tahu.clone();
        move |n| n.inside("wall-keys") && n.text == t
    });
    sim.click({
        let b = belum.clone();
        move |n| n.inside("wall-keys") && n.text == b
    });
    assert_eq!(log_len(&mut sim), 2);
    let html = sim.html();
    assert!(html.contains("class=\"pile-count\">1<"), "one card on each pile: {html}");
    sim.click(|n| n.has_class("rev-undo"));
    assert_eq!(log_len(&mut sim), 1);
    // Sort everything left: the round ends by itself.
    while sim.find(|n| n.inside("wall-keys")).is_some() {
        let t = tahu.clone();
        sim.click(move |n| n.inside("wall-keys") && n.text == t);
    }
    assert!(sim.html().contains(&en(Msg::RevDoneTitle)));
    let log = db(&mut sim).0.borrow().review_log().unwrap();
    assert!(log.iter().all(|e| e.mode == ReviewMode::CardWall));
}

/// The wall's own tilt and messages from its script.
#[test]
fn wall_cards_tilt_a_little_and_parse_script_messages() {
    for id in ["a", "seed.bridge.kabar.recognition", "x.y.z"] {
        assert!((-3..=3).contains(&tilt(id)));
        assert_eq!(tilt(id), tilt(id), "stable");
    }
    let flip: FromWall = serde_json::from_str(r#"{"t":"flip","id":"c1"}"#).unwrap();
    assert!(matches!(flip, FromWall::Flip { id } if id == "c1"));
    let drop: FromWall = serde_json::from_str(r#"{"t":"drop","id":"c1","pile":"tahu"}"#).unwrap();
    assert!(matches!(drop, FromWall::Drop { pile: Pile::Tahu, .. }));
}

/// With nothing due the home rests: no nudge, just an option to learn new
/// words, and the forecast opens Stats.
#[test]
fn nothing_due_rests_and_offers_new_words() {
    let mut sim = open(WidthClass::Compact, UiLanguage::En, seed_bridge_deck());
    // Answer every card so nothing is due today.
    sim.with(|| {
        let db = consume_context::<Db>();
        let now = now_ms();
        for c in seed_bridge_deck() {
            db.0.borrow_mut().record_review(&c.id, Rating::Easy, now, 1000, ReviewMode::Classic).unwrap();
        }
    });
    // Reopen the screen so it reads the store again.
    sim.with(|| consume_context::<Ui>().screen.set(Screen::Today));
    sim.with(|| consume_context::<Ui>().screen.set(Screen::Review));
    let html = sim.html();
    assert!(html.contains(&en(Msg::RevAllDone)), "{html}");
    assert!(html.contains(&en(Msg::RevNothingHere)));
    assert!(html.contains("class=\"forecast empty\""), "nothing due draws flat baselines");
    assert!(html.contains(&en(Msg::RevForecastEmpty)));
    sim.click(|n| n.has_class("forecast-tap"));
    assert_eq!(*sim.with(consume_context::<Ui>).screen.peek(), Screen::Stats);
    sim.with(|| consume_context::<Ui>().screen.set(Screen::Review));
    sim.click(|n| n.has_class("rev-learn-new"));
    assert_eq!(*sim.with(consume_context::<Ui>).screen.peek(), Screen::Journey);
}

/// Spans read naturally in every language: Arabic counts take 1 / 2 /
/// 3–10 / 11+ forms, MSA duals change after «بعد».
#[test]
fn spans_follow_each_languages_number_rules() {
    fn app() -> Element {
        sim::provide(WidthClass::Compact);
        rsx! {}
    }
    let mut sim = Sim::new(app);
    sim.with(|| {
        let mut ui = consume_context::<Ui>();
        let say = |ui: &Ui, s: Span, p: bool| span_text(ui, s, p);
        ui.settings.write().ui_language = UiLanguage::En;
        assert_eq!(say(&ui, Span::Minutes(1), false), "1 minute");
        assert_eq!(say(&ui, Span::Days(3), false), "3 days");
        assert_eq!(span_short(&ui, Span::Days(3)), "3d");
        assert_eq!(span_short(&ui, Span::Months(2)), "2mo");
        assert_eq!(span_short(&ui, Span::Years(1)), "1y");
        assert_eq!(span_short(&ui, Span::Hours(5)), "5h");
        assert_eq!(span_short(&ui, Span::Minutes(10)), "10m");
        assert_eq!(mm_ss(&ui, 75_000), "1:15");
        ui.settings.write().ui_language = UiLanguage::ArEg;
        assert_eq!(say(&ui, Span::Days(1), false), "يوم");
        assert_eq!(say(&ui, Span::Days(2), false), "يومين");
        assert!(say(&ui, Span::Days(3), false).ends_with(" أيام"));
        assert!(say(&ui, Span::Days(11), false).ends_with(" يوم"));
        assert_eq!(span_short(&ui, Span::Hours(2)), "ساعتين");
        ui.settings.write().ui_language = UiLanguage::Ar;
        assert_eq!(say(&ui, Span::Years(2), false), "سنتان");
        assert_eq!(say(&ui, Span::Years(2), true), "سنتين");
        assert!(say(&ui, Span::Months(12), true).ends_with(" شهرًا"));
        assert!(say(&ui, Span::Minutes(5), true).ends_with(" دقائق"));
        // Day 0 was a Thursday; the week wraps.
        ui.settings.write().ui_language = UiLanguage::En;
        assert_eq!(weekday(&ui, 0), crate::i18n::t(UiLanguage::En, Msg::ThuShort));
        assert_eq!(weekday(&ui, 3), crate::i18n::t(UiLanguage::En, Msg::SunShort));
        assert_eq!(weekday(&ui, -1), crate::i18n::t(UiLanguage::En, Msg::WedShort));
        let sched = Scheduler::default();
        let now = 1_761_472_800_000;
        assert_eq!(
            when_text(&ui, &sched, now, now + 86_400_000),
            crate::i18n::t(UiLanguage::En, Msg::RevTomorrow)
        );
        assert!(when_text(&ui, &sched, now, now + 600_000).contains("10 minutes"));
        for r in Rating::ALL {
            assert!(!ui.t(rating_msg(r)).is_empty());
            assert!(!rate_class(r).is_empty());
        }
    });
}

/// The isolate marks in catalog text become real `<bdi>` runs.
#[test]
fn isolated_text_wraps_latin_runs_in_bdi() {
    let html =
        dioxus::ssr::render_element(rsx! { Isolated { text: "قول \u{2068}Kabar\u{2069} كده".to_string() } });
    assert_eq!(html, "<span>قول </span><bdi class=\"latin\" dir=\"ltr\">Kabar</bdi><span> كده</span>");
}

/// Due words for the daily plan: due cards only, one entry per word.
#[test]
fn due_words_list_each_due_word_once() {
    let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
    let sched = Rc::new(Scheduler::default());
    db.0.borrow_mut().upsert_cards(&seed_bridge_deck()).unwrap();
    let t0 = 1_761_472_800_000;
    let c = &seed_bridge_deck()[0];
    db.0.borrow_mut().record_review(&c.id, Rating::Again, t0, 1000, ReviewMode::Classic).unwrap();
    let later = t0 + 30 * 86_400_000;
    let words = due_words(&db, &sched, later);
    assert_eq!(words.iter().filter(|w| **w == c.word.to_lowercase()).count(), 1, "{words:?}");
    let mut sorted = words.clone();
    sorted.dedup();
    assert_eq!(sorted.len(), words.len(), "no duplicates");
}

/// Space reveals, 1–4 rate, Ctrl+Z undoes, from anywhere on the page; a
/// rating key before the answer shows does nothing.
#[test]
fn classic_keys_reveal_rate_and_undo() {
    sim::live("__kabarKeys");
    let mut sim = open(WidthClass::Compact, UiLanguage::En, seed_bridge_deck());
    sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
    let key = |sim: &mut Sim, k: &str| sim.page_sends("__kabarKeys", serde_json::json!(k));
    key(&mut sim, "3");
    assert_eq!(log_len(&mut sim), 0, "nothing rated before the answer shows");
    key(&mut sim, " ");
    assert!(sim.html().contains("rev-back"));
    key(&mut sim, "3");
    let log = db(&mut sim).0.borrow().review_log().unwrap();
    assert_eq!(log.iter().map(|e| e.rating).collect::<Vec<_>>(), vec![Rating::Good]);
    key(&mut sim, "undo");
    assert_eq!(log_len(&mut sim), 0);
    key(&mut sim, "x");
    assert!(!sim.html().contains("rev-back"), "a fresh card, answer hidden");
}

/// The wall's script reports flips and drops; a flipped card sorted to
/// tahu counts at most Good.
#[test]
fn wall_flips_and_drops_from_the_page_are_rated() {
    sim::live("Card Wall (SPEC");
    let cards: Vec<Card> = seed_bridge_deck().into_iter().take(4).collect();
    let mut sim = open(WidthClass::Expanded, UiLanguage::Ar, cards);
    sim.click(|n| {
        n.has_class("rev-mode")
            && n.text.contains(crate::i18n::t(UiLanguage::Ar, Msg::RevWall))
            && n.has_class("card-action")
    });
    let id = sim.find(|n| n.has_class("wall-card")).unwrap().attr("data-id").unwrap().to_string();
    sim.page_sends("Card Wall (SPEC", serde_json::json!({ "t": "flip", "id": id }));
    assert!(sim.find(|n| n.has_class("wall-card") && n.has_class("flipped")).is_some());
    sim.page_sends("Card Wall (SPEC", serde_json::json!({ "t": "drop", "id": id, "pile": "tahu" }));
    let log = db(&mut sim).0.borrow().review_log().unwrap();
    assert_eq!(log.len(), 1);
    assert!(matches!(log[0].rating, Rating::Good | Rating::Hard), "{:?}", log[0].rating);
    // A drop for a card not on the table changes nothing.
    sim.page_sends("Card Wall (SPEC", serde_json::json!({ "t": "drop", "id": "nope", "pile": "belum" }));
    assert_eq!(log_len(&mut sim), 1);
}

/// The burst's clock ticks from the page; the timer counts down from a minute.
#[test]
fn the_burst_clock_ticks() {
    sim::live("i < 400");
    let mut sim = open(WidthClass::Compact, UiLanguage::En, seed_bridge_deck());
    sim.click(|n| n.has_class("mode-burst") && n.has_class("card-action"));
    sim.page_sends("i < 400", serde_json::json!(1));
    let secs = sim.find(|n| n.has_class("burst-secs")).unwrap().text;
    let secs: u64 = secs.parse().unwrap();
    assert!((1..=BURST_MS / 1000).contains(&secs), "{secs}");
}

/// Numbers that can't start a round open Stats instead.
#[test]
fn counts_without_a_round_open_stats() {
    let mut sim = open(WidthClass::Compact, UiLanguage::En, seed_bridge_deck());
    let today = crate::i18n::t(UiLanguage::En, Msg::RevReviewedToday);
    sim.click(move |n| n.has_class("rev-stat") && n.text.contains(today));
    assert_eq!(*sim.with(consume_context::<Ui>).screen.peek(), Screen::Stats);
}

/// When the store refuses writes, seeding and content checks say so and
/// change nothing.
#[test]
fn refused_writes_are_reported_not_lost_silently() {
    let mut sim = open(WidthClass::Compact, UiLanguage::En, vec![]);
    db(&mut sim).0.borrow().refuse_writes();
    sim.click(|n| n.tag == "button" && n.has_class("primary"));
    assert!(sim.html().contains(&en(Msg::RevSaveFailed)));
    assert!(db(&mut sim).0.borrow().cards().unwrap().is_empty());

    let card = word("awan", CardKind::Recognition, StudyLanguage::Jv);
    let mut sim = open(WidthClass::Compact, UiLanguage::En, vec![card.clone()]);
    sim.click(|n| n.has_class("rev-mode") && n.has_class("card-action"));
    sim.click(|n| n.has_class("reveal-btn"));
    db(&mut sim).0.borrow().refuse_writes();
    sim.click(|n| n.has_class("rev-content-ok"));
    assert!(sim.html().contains(&en(Msg::RevSaveFailed)));
    assert!(!db(&mut sim).0.borrow().cards().unwrap()[0].verified);
}

/// No signals, no events.
#[test]
fn no_evidence_records_nothing() {
    let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
    assert_eq!(note_evidence_at(&db, 0, StudyLanguage::Id, &[], 1_761_472_800_000), 0);
    assert!(db.0.borrow().review_log().unwrap().is_empty());
}
