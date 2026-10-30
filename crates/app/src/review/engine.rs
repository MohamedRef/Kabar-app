//! Deck caching and review session transitions.

use super::*;

pub(super) fn read_deck(db: &Db, sched: &Scheduler) -> Deck {
    let store = db.0.borrow();
    let cards = store.cards().unwrap_or_else(|e| {
        crate::diag::report!("reading cards failed: {e}");
        Vec::new()
    });
    let log = store.review_log().unwrap_or_else(|e| {
        crate::diag::report!("reading the review log failed: {e}");
        Vec::new()
    });
    Deck::new(cards, log, sched)
}
/// The last deck built, with what it was built from. The weak handle keeps
/// a dropped store's address from being mistaken for a new one.
pub(super) struct CachedDeck {
    pub(super) db: Weak<RefCell<Store>>,
    pub(super) stamp: (u64, i64),
    /// Held so its address can't be reused by a different scheduler.
    pub(super) sched: Rc<Scheduler>,
    pub(super) deck: Rc<Deck>,
}
thread_local! {
    static DECK_CACHE: RefCell<Option<CachedDeck>> = const { RefCell::new(None) };
}
/// The deck, replayed from the store only when the store or the scheduler
/// changed since the last call (#116): Today, Kabar Pagi, the plan and
/// Review all ask for it, and parsing plus replaying a year of reviews is
/// not free on a phone. `sched` comes from [`crate::schedule`], which hands
/// out the same `Rc` while its inputs stay the same.
pub(crate) fn deck(db: &Db, sched: &Rc<Scheduler>) -> Rc<Deck> {
    let stamp = match db.0.borrow().change_stamp() {
        Ok(stamp) => stamp,
        Err(e) => {
            crate::diag::report!("reading the store's change stamp failed: {e}");
            return Rc::new(read_deck(db, sched));
        }
    };
    let hit = DECK_CACHE.with_borrow(|c| {
        c.as_ref()
            .filter(|c| {
                c.stamp == stamp
                    && Rc::ptr_eq(&c.sched, sched)
                    && c.db.upgrade().is_some_and(|s| Rc::ptr_eq(&s, &db.0))
            })
            .map(|c| c.deck.clone())
    });
    hit.unwrap_or_else(|| {
        let deck = Rc::new(read_deck(db, sched));
        DECK_CACHE.set(Some(CachedDeck {
            db: Rc::downgrade(&db.0),
            stamp,
            sched: sched.clone(),
            deck: deck.clone(),
        }));
        deck
    })
}
/// Turns what a lesson or the Reader showed about deck words into review
/// events (see `kabar_core::review::evidence` for the rules), so each word's
/// schedule follows how well the learner really knows it. Returns how many
/// events were recorded.
pub(crate) fn note_evidence(
    db: &Db,
    coach: &Coach,
    language: StudyLanguage,
    signals: &[(String, kabar_core::review::Signal)],
) -> usize {
    note_evidence_at(db, coach.offset(), language, signals, now_ms())
}
pub(super) fn note_evidence_at(
    db: &Db,
    utc_offset_min: i16,
    language: StudyLanguage,
    signals: &[(String, kabar_core::review::Signal)],
    now: u64,
) -> usize {
    if signals.is_empty() {
        return 0;
    }
    let settings = db.0.borrow().settings().unwrap_or_default();
    let sched = crate::schedule::scheduler_at(utc_offset_min, &settings);
    let events = kabar_core::review::evidence_reviews(&sched, &deck(db, &sched), language, signals, now);
    let mut store = db.0.borrow_mut();
    let mut recorded = 0;
    for (card, rating, evidence) in events {
        match store.record_evidence(&card, rating, now, evidence) {
            Ok(_) => recorded += 1,
            Err(e) => crate::diag::report!("recording {evidence:?} for {card} failed: {e}"),
        }
    }
    recorded
}
/// An owned deck for a review session to update as answers come in.
pub(crate) fn load_deck(db: &Db, sched: &Rc<Scheduler>) -> Deck {
    Deck::clone(&deck(db, sched))
}
/// Words of the cards due now, least likely to be remembered first, without
/// duplicates (a word can have several card kinds). Feeds the daily plan and
/// Kabar Pagi.
pub(crate) fn due_words(db: &Db, sched: &Rc<Scheduler>, now_ms: u64) -> Vec<String> {
    let deck = deck(db, sched);
    let mut due: Vec<(f32, &Card)> = deck
        .due(sched, now_ms)
        .into_iter()
        .map(|c| (sched.retrievability(&deck.state(&c.id), now_ms).unwrap_or(0.0), c))
        .collect();
    due.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let mut words: Vec<String> = Vec::new();
    for (_, c) in due {
        let w = c.word.to_lowercase();
        if !words.contains(&w) {
            words.push(w);
        }
    }
    words
}
pub(super) fn fits(mode: ReviewMode) -> fn(&Card) -> bool {
    match mode {
        ReviewMode::Classic => |_| true,
        ReviewMode::CardWall | ReviewMode::Burst => is_quick,
    }
}
impl Engine {
    pub(super) fn queue_len(&self, mode: ReviewMode, now: u64) -> usize {
        self.deck.read().queue(&self.sched, now, fits(mode)).len()
    }

    pub(super) fn start(&self, mode: ReviewMode) {
        self.start_scoped(mode, QueueScope::All);
    }

    pub(super) fn start_scoped(&self, mode: ReviewMode, scope: QueueScope) {
        let now = now_ms();
        let queue = self.deck.read().queue_scoped(&self.sched, now, fits(mode), scope);
        let (mut session, mut view, mut error) = (self.session, self.view, self.error);
        session.set(Some(Session::new(mode, queue, now)));
        error.set(None);
        view.set(match mode {
            ReviewMode::Classic => View::Classic,
            ReviewMode::CardWall => View::Wall,
            ReviewMode::Burst => View::Burst,
        });
    }

    pub(super) fn pull(&self) -> Option<String> {
        let mut session = self.session;
        session.write().as_mut().and_then(Session::pull)
    }

    pub(super) fn put_back(&self, card_id: String) {
        let mut session = self.session;
        if let Some(s) = session.write().as_mut() {
            s.put_back(card_id);
        }
    }

    pub(super) fn mode(&self) -> ReviewMode {
        self.session.read().as_ref().map_or(ReviewMode::Classic, |s| s.mode)
    }

    /// Writes the answer to the log, then updates the deck and the session.
    pub(super) fn answer(&self, card_id: &str, rating: Rating, duration_ms: u64) -> bool {
        let mode = self.mode();
        let duration = duration_ms.min(u64::from(u32::MAX)) as u32;
        let recorded = self.db.0.borrow_mut().record_review(card_id, rating, now_ms(), duration, mode);
        let (mut deck, mut session, mut error) = (self.deck, self.session, self.error);
        match recorded {
            Ok(event) => {
                let id = event.id.clone();
                deck.write().record(event, &self.sched);
                if let Some(s) = session.write().as_mut() {
                    s.answer(id, card_id.to_string(), rating, duration);
                }
                error.set(None);
                true
            }
            Err(e) => {
                crate::diag::report!("saving a review failed: {e}");
                error.set(Some(Msg::RevSaveFailed));
                false
            }
        }
    }

    pub(super) fn can_undo(&self) -> bool {
        self.session.read().as_ref().is_some_and(|s| !s.answers().is_empty())
    }

    /// Takes back the last answer; its card goes to the front of the queue.
    pub(super) fn undo(&self) -> bool {
        let (mut session, mut deck) = (self.session, self.deck);
        let mut session = session.write();
        let Some(session) = session.as_mut() else { return false };
        let undone = take_back(&mut self.db.0.borrow_mut(), session, &mut deck.write(), &self.sched);
        undone.unwrap_or_else(|e| {
            crate::diag::report!("undo failed: {e}");
            false
        })
    }

    /// Ends the session (once: effects may ask twice before unmounting).
    pub(super) fn finish(&self) {
        let (mut session, mut summary, mut view) = (self.session, self.summary, self.view);
        let Some(done) = session.write().take() else { return };
        if done.answers().is_empty() {
            // Nothing answered: no summary to show, just go back.
            view.set(View::Home);
            return;
        }
        let s = done.summary(now_ms());
        // Kak Nur counts reviews toward today's goal and the weekly report.
        let cards = s.answers as u32;
        let correct = (s.answers - s.again) as u32;
        self.coach.record(&self.db, ActivityKind::Reviews { cards, correct }, (s.duration_ms / 1000) as u32);
        summary.set(Some(s));
        view.set(View::Done);
    }

    pub(super) fn home(&self) {
        let (mut session, mut view) = (self.session, self.view);
        session.set(None);
        view.set(View::Home);
    }

    pub(super) fn seed(&self) {
        let mut deck = self.deck;
        if let Err(e) = self.db.0.borrow_mut().upsert_cards(&seed_bridge_deck()) {
            crate::diag::report!("seeding the starter deck failed: {e}");
            let mut error = self.error;
            error.set(Some(Msg::RevSaveFailed));
            return;
        }
        deck.set(load_deck(&self.db, &self.sched));
    }

    /// The learner checked a card's content: it loses the unverified flag
    /// (its memory schedule is untouched).
    pub(super) fn verify_card(&self, card: &Card) {
        let checked = Card { verified: true, ..card.clone() };
        if let Err(e) = self.db.0.borrow_mut().upsert_card(&checked) {
            crate::diag::report!("saving the content check failed: {e}");
            let mut error = self.error;
            error.set(Some(Msg::RevSaveFailed));
            return;
        }
        let mut deck = self.deck;
        deck.set(load_deck(&self.db, &self.sched));
    }

    pub(super) fn card(&self, id: &str) -> Option<Card> {
        self.deck.read().card(id).cloned()
    }
}
/// Undo of the session's last answer, in this order so the three never
/// disagree: the event gets its tombstone in the log (the review log is
/// append-only; a tombstone is its supported correction, and it syncs),
/// then the session hands the card back and the deck forgets the event, so
/// the card's schedule is exactly what it was before the answer. Nothing
/// changes if the log write fails. Returns whether an answer was undone.
pub(super) fn take_back(
    store: &mut Store,
    session: &mut Session,
    deck: &mut Deck,
    sched: &Scheduler,
) -> Result<bool, crate::store::StoreError> {
    let Some(last) = session.answers().last().map(|a| a.event_id.clone()) else { return Ok(false) };
    store.undo_review(&last)?;
    session.undo();
    deck.forget(&last, sched);
    Ok(true)
}
