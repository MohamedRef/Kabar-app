//! Existing behavior regressions for the feature facade.

use kabar_protocol::srs::{Rating, ReviewMode};

use super::*;

#[test]
fn the_deck_is_rebuilt_only_after_the_store_changes() {
    let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
    let sched = Rc::new(Scheduler::default());
    db.0.borrow_mut().upsert_cards(&seed_bridge_deck()).unwrap();
    let first = deck(&db, &sched);
    assert!(Rc::ptr_eq(&first, &deck(&db, &sched)), "nothing changed: the cached deck");
    let card = seed_bridge_deck()[0].id.clone();
    db.0.borrow_mut()
        .record_review(&card, Rating::Good, 1_761_472_800_000, 2000, ReviewMode::Classic)
        .unwrap();
    let second = deck(&db, &sched);
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(second.state(&card).reps, 1);
    // Another store never gets this one's deck.
    let other = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
    assert!(deck(&other, &sched).is_empty());
    // Nor does a deck built with another scheduler.
    let travel = crate::schedule::scheduler_at(420, &Default::default());
    assert!(!Rc::ptr_eq(&deck(&db, &sched), &deck(&db, &travel)));
}

/// Undo puts everything back as it was: the card's schedule, the
/// session (card in hand again), and the log (the event tombstoned, no
/// second event). Undoing with nothing answered changes nothing.
#[test]
fn undo_restores_schedule_session_and_log() {
    let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
    let sched = Scheduler::default();
    db.0.borrow_mut().upsert_cards(&seed_bridge_deck()).unwrap();
    let now = 1_761_472_800_000;
    let mut deck = read_deck(&db, &sched);
    let queue = deck.queue(&sched, now, |_| true);
    let mut session = Session::new(ReviewMode::Classic, queue.clone(), now);
    let card = session.pull().unwrap();
    let before = deck.state(&card);
    let preview = sched.preview(&before, now);

    assert!(!take_back(&mut db.0.borrow_mut(), &mut session, &mut deck, &sched).unwrap());

    let event = db.0.borrow_mut().record_review(&card, Rating::Easy, now, 900, ReviewMode::Classic).unwrap();
    deck.record(event.clone(), &sched);
    session.answer(event.id.clone(), card.clone(), Rating::Easy, 900);
    assert_ne!(deck.state(&card), before);

    assert!(take_back(&mut db.0.borrow_mut(), &mut session, &mut deck, &sched).unwrap());
    assert_eq!(deck.state(&card), before);
    assert_eq!(sched.preview(&deck.state(&card), now), preview, "the buttons show the same intervals");
    assert!(session.answers().is_empty());
    assert_eq!(session.pull(), Some(card.clone()), "the card is back in hand");
    assert!(db.0.borrow().review_log().unwrap().is_empty());
    // The log and the in-memory deck agree after a full replay.
    assert_eq!(read_deck(&db, &sched).state(&card), before);
    // A second undo has nothing left to take back.
    assert!(!take_back(&mut db.0.borrow_mut(), &mut session, &mut deck, &sched).unwrap());
}

#[test]
fn failed_undo_notifies_without_changing_the_review_then_recovers() {
    fn app() -> Element {
        let db = use_context_provider(|| {
            let mut store = Store::open_in_memory().unwrap();
            store.upsert_cards(&seed_bridge_deck()).unwrap();
            Db(Rc::new(RefCell::new(store)))
        });
        use_context_provider(|| {
            let sched = Rc::new(Scheduler::default());
            let deck = read_deck(&db, &sched);
            let session = Session::new(ReviewMode::Classic, deck.queue(&sched, now_ms(), |_| true), now_ms());
            Engine {
                coach: Coach::load(&db),
                db,
                sched,
                deck: Signal::new(deck),
                session: Signal::new(Some(session)),
                view: Signal::new(View::Classic),
                summary: Signal::new(None),
                error: Signal::new(None),
            }
        });
        let engine = use_context::<Engine>();
        let error = *engine.error.read();
        rsx! { if let Some(msg) = error { p { role: "alert", {crate::i18n::t(UiLanguage::En, msg)} } } }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dom.in_scope(ScopeId::APP, || {
        let engine = consume_context::<Engine>();
        let card = engine.pull().unwrap();
        assert!(engine.answer(&card, Rating::Good, 1200));
        let before = engine.deck.peek().state(&card);
        engine.db.0.borrow().refuse_writes();
        assert!(!engine.undo());
        assert_eq!(*engine.error.peek(), Some(Msg::RevUndoFailed));
        assert_eq!(engine.deck.peek().state(&card), before);
        assert_eq!(engine.session.peek().as_ref().unwrap().answers().len(), 1);
        assert_eq!(engine.db.0.borrow().review_log().unwrap().len(), 1);
    });
    let _ = dom.render_immediate_to_vec();
    let html = dioxus::ssr::render(&dom);
    assert!(html.contains("role=\"alert\"") && html.contains("It is still saved"), "{html}");
    dom.in_scope(ScopeId::APP, || {
        let engine = consume_context::<Engine>();
        engine.db.0.borrow().allow_writes();
        assert!(engine.undo());
        assert_eq!(*engine.error.peek(), None);
        assert!(engine.db.0.borrow().review_log().unwrap().is_empty());
        assert!(engine.session.peek().as_ref().unwrap().answers().is_empty());
    });
}

#[test]
fn javanese_false_friends_reach_review_cards() {
    let card = |word: &str, language| Card {
        v: 1,
        id: format!("{word}#r"),
        word: word.into(),
        language,
        kind: CardKind::Recognition,
        meaning: Default::default(),
        note: None,
        example: None,
        arabic: None,
        source: Default::default(),
        source_ref: None,
        verified: false,
    };
    assert_eq!(indonesian_twin(&card("awan", StudyLanguage::Jv)), Some("awan"));
    assert_eq!(indonesian_twin(&card("Mau", StudyLanguage::Jv)), Some("mau"));
    // Indonesian awan is just a cloud; ordinary Javanese words carry nothing.
    assert_eq!(indonesian_twin(&card("awan", StudyLanguage::Id)), None);
    assert_eq!(indonesian_twin(&card("enjing", StudyLanguage::Jv)), None);
    assert!(JV_TWINS.len() >= 10, "{:?}", JV_TWINS.keys());
}

#[test]
fn lessons_and_lookups_move_deck_words() {
    use kabar_core::review::Signal as S;
    use kabar_protocol::srs::Evidence;
    const DAY: u64 = 86_400_000;
    let t0 = 1_761_472_800_000;
    let db = Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())));
    let cards = seed_bridge_deck();
    db.0.borrow_mut().upsert_cards(&cards).unwrap();
    let mut recognition = cards.iter().filter(|c| c.kind == CardKind::Recognition);
    let (a, b) = (recognition.next().unwrap(), recognition.next().unwrap());
    for c in [a, b] {
        db.0.borrow_mut().record_review(&c.id, Rating::Good, t0, 2000, ReviewMode::Classic).unwrap();
    }
    let signals = [(a.word.to_uppercase(), S::Recalled), (b.word.clone(), S::LookedUp)];
    let lang = a.language;
    // b's Bridge card was never reviewed, so only its recognition card moves.
    assert_eq!(note_evidence_at(&db, 180, lang, &signals, t0 + 30 * DAY), 2);
    // Once a day per card.
    assert_eq!(note_evidence_at(&db, 180, lang, &signals, t0 + 30 * DAY + 60_000), 0);
    let log = db.0.borrow().review_log().unwrap();
    let evidence: Vec<(&str, Rating, Evidence)> =
        log[2..].iter().map(|e| (e.card_id.as_str(), e.rating, e.evidence)).collect();
    assert!(evidence.contains(&(a.id.as_str(), Rating::Good, Evidence::Exercise)), "{evidence:?}");
    assert!(evidence.contains(&(b.id.as_str(), Rating::Again, Evidence::Lookup)), "{evidence:?}");
    assert!(log[2..].iter().all(|e| e.duration_ms == 0));
    let sched = Rc::new(Scheduler::default());
    let deck = deck(&db, &sched);
    assert_eq!(deck.state(&b.id).lapses, 1);
    assert_eq!(deck.stats(&sched, t0 + 30 * DAY).reviews_today, 0, "evidence isn't a review");
}
