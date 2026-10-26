//! Review sessions (SPEC §16) on top of the FSRS scheduler in [`crate::srs`]:
//! which cards to show, in what order, the stats on the Review home and the
//! end-of-session summary. Pure: callers pass the time in.

mod from_lesson;
mod seed;

use std::collections::{BTreeMap, HashSet, VecDeque};

pub use from_lesson::{lesson_card_id, lesson_cards};
use kabar_protocol::srs::{Card, CardKind, Rating, ReviewEvent, ReviewMode};
pub use seed::{seed_bridge_deck, seed_words};

use crate::srs::{CardState, Pile, Scheduler, card_wall_rating};

const DAY_MS: u64 = 86_400_000;
/// New cards introduced per learner day.
pub const NEW_PER_DAY: usize = 12;
/// Retention on the Review home looks back this far.
pub const RETENTION_WINDOW_DAYS: u64 = 30;
/// A Burst lasts one minute.
pub const BURST_MS: u64 = 60_000;

/// The learner's cards with their derived FSRS state.
#[derive(Clone, Debug, Default)]
pub struct Deck {
    cards: Vec<Card>,
    index: BTreeMap<String, usize>,
    states: BTreeMap<String, CardState>,
    log: Vec<ReviewEvent>,
}

/// Numbers for the Review home.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub due: usize,
    /// New cards a session would introduce now (daily limit applied).
    pub new_available: usize,
    pub reviews_today: usize,
    /// Share of reviews of already-learned cards that were not "Again",
    /// over the last [`RETENTION_WINDOW_DAYS`]. `None` until there are any.
    pub retention: Option<f32>,
}

impl Deck {
    /// Builds a deck: card state comes from replaying `log`, never from storage.
    pub fn new(cards: Vec<Card>, log: Vec<ReviewEvent>, scheduler: &Scheduler) -> Self {
        let index = cards.iter().enumerate().map(|(i, c)| (c.id.clone(), i)).collect();
        let states = scheduler.replay_all(&log);
        Self { cards, index, states, log }
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Adds an event just written to the log and updates its card.
    pub fn record(&mut self, event: ReviewEvent, scheduler: &Scheduler) {
        let card_id = event.card_id.clone();
        let in_order = self.log.last().is_none_or(|last| last.hlc <= event.hlc);
        self.log.push(event);
        if !in_order {
            self.log.sort_by(|a, b| a.hlc.cmp(&b.hlc));
        }
        self.states.insert(card_id.clone(), scheduler.replay(&card_id, &self.log));
    }

    /// Removes an undone event and rebuilds its card.
    pub fn forget(&mut self, event_id: &str, scheduler: &Scheduler) {
        let Some(i) = self.log.iter().position(|e| e.id == event_id) else { return };
        let card_id = self.log.remove(i).card_id;
        if self.log.iter().any(|e| e.card_id == card_id) {
            self.states.insert(card_id.clone(), scheduler.replay(&card_id, &self.log));
        } else {
            self.states.remove(&card_id);
        }
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn card(&self, id: &str) -> Option<&Card> {
        self.index.get(id).map(|&i| &self.cards[i])
    }

    pub fn log(&self) -> &[ReviewEvent] {
        &self.log
    }

    /// The card's derived state (a fresh one for a card never reviewed).
    pub fn state(&self, id: &str) -> CardState {
        self.states.get(id).cloned().unwrap_or_else(|| CardState::new(id))
    }

    fn live_states(&self) -> impl Iterator<Item = &CardState> {
        self.states.values().filter(|s| self.index.contains_key(&s.card_id))
    }

    /// Cards due now, most-forgotten first.
    pub fn due(&self, scheduler: &Scheduler, now_ms: u64) -> Vec<&Card> {
        scheduler.due(self.live_states(), now_ms).into_iter().filter_map(|s| self.card(&s.card_id)).collect()
    }

    /// Cards never reviewed, in deck order.
    pub fn unseen(&self) -> impl Iterator<Item = &Card> {
        self.cards.iter().filter(|c| !self.states.contains_key(&c.id))
    }

    /// How many new cards were introduced on the learner's current day.
    pub fn introduced_today(&self, scheduler: &Scheduler, now_ms: u64) -> usize {
        let today = scheduler.day.day(now_ms);
        let mut first: BTreeMap<&str, &ReviewEvent> = BTreeMap::new();
        for e in &self.log {
            first.entry(&e.card_id).and_modify(|f| *f = if e.hlc < f.hlc { e } else { f }).or_insert(e);
        }
        first.values().filter(|e| scheduler.day.day(e.at_ms) == today).count()
    }

    /// A session's queue: due cards (most forgotten first), then new cards up
    /// to the daily limit, one sibling per word so "kabar" is not asked twice
    /// in a row in two forms. `fits` filters by kind (Card Wall and Burst skip
    /// cards that need typing or audio).
    pub fn queue(&self, scheduler: &Scheduler, now_ms: u64, fits: impl Fn(&Card) -> bool) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut words = HashSet::new();
        for c in self.due(scheduler, now_ms).into_iter().filter(|c| fits(c)) {
            words.insert(c.sibling_key());
            out.push(c.id.clone());
        }
        // A sibling reviewed today also buries its new siblings until tomorrow.
        let today = scheduler.day.day(now_ms);
        for e in self.log.iter().filter(|e| scheduler.day.day(e.at_ms) == today) {
            if let Some(c) = self.card(&e.card_id) {
                words.insert(c.sibling_key());
            }
        }
        let budget = NEW_PER_DAY.saturating_sub(self.introduced_today(scheduler, now_ms));
        let new = self.unseen().filter(|c| fits(c)).filter(|c| words.insert(c.sibling_key())).take(budget);
        out.extend(new.map(|c| c.id.clone()));
        out
    }

    pub fn forecast(&self, scheduler: &Scheduler, now_ms: u64, days: usize) -> Vec<u32> {
        scheduler.forecast(self.live_states(), now_ms, days)
    }

    /// When the next card falls due after `now_ms`, if any.
    pub fn next_due_ms(&self, now_ms: u64) -> Option<u64> {
        self.live_states().filter_map(|s| s.due_ms).filter(|&d| d > now_ms).min()
    }

    pub fn stats(&self, scheduler: &Scheduler, now_ms: u64) -> Stats {
        let today = scheduler.day.day(now_ms);
        let due = self.due(scheduler, now_ms).len();
        let new_available = self.queue(scheduler, now_ms, |_| true).len() - due;
        let reviews_today = self.log.iter().filter(|e| scheduler.day.day(e.at_ms) == today).count();
        let since = now_ms.saturating_sub(RETENTION_WINDOW_DAYS * DAY_MS);
        Stats { due, new_available, reviews_today, retention: retention(&self.log, since) }
    }
}

/// True retention: among reviews at or after `since_ms` of cards that had
/// been reviewed before, the share not answered "Again".
pub fn retention(log: &[ReviewEvent], since_ms: u64) -> Option<f32> {
    let mut sorted: Vec<&ReviewEvent> = log.iter().collect();
    sorted.sort_by(|a, b| a.hlc.cmp(&b.hlc));
    let mut seen_cards = HashSet::new();
    let mut seen_events = HashSet::new();
    let (mut total, mut passed) = (0u32, 0u32);
    for e in sorted.into_iter().filter(|e| seen_events.insert(e.id.as_str())) {
        let learned = !seen_cards.insert(e.card_id.as_str());
        if learned && e.at_ms >= since_ms {
            total += 1;
            passed += u32::from(e.rating != Rating::Again);
        }
    }
    (total > 0).then(|| passed as f32 / total as f32)
}

/// Card kinds that work as a quick glance-and-sort (Card Wall, Burst).
pub fn is_quick(card: &Card) -> bool {
    matches!(card.kind, CardKind::Recognition | CardKind::Bridge | CardKind::Cloze)
}

/// The Card Wall grade. Tapping a card to check its meaning first means
/// the learner was not sure, so a *tahu* after a flip counts at most Good.
pub fn wall_rating(pile: Pile, sort_ms: u32, flipped: bool) -> Rating {
    match card_wall_rating(pile, sort_ms) {
        Rating::Easy if flipped => Rating::Good,
        r => r,
    }
}

/// One answer given in a session.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub event_id: String,
    pub card_id: String,
    pub rating: Rating,
    pub duration_ms: u32,
    /// "Again" cards come back at the end of the session.
    pub requeued: bool,
}

/// A review session: a queue of card ids and the answers given so far.
/// The UI pulls cards into its hand (one for Classic, several on the Card
/// Wall) and reports answers by card id.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub mode: ReviewMode,
    pub started_ms: u64,
    queue: VecDeque<String>,
    answers: Vec<Answer>,
}

/// The end-of-session numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub answers: usize,
    pub words: usize,
    pub again: usize,
    /// Share of answers that were not "Again".
    pub retention: Option<f32>,
    /// The card remembered best (Easy, then Good, fastest first).
    pub strongest: Option<String>,
    /// Cards to meet again soon, in the order they were missed.
    pub revisit: Vec<String>,
    pub duration_ms: u64,
}

impl Session {
    pub fn new(mode: ReviewMode, queue: Vec<String>, now_ms: u64) -> Self {
        Self { mode, started_ms: now_ms, queue: queue.into(), answers: Vec::new() }
    }

    /// Takes the next card into the hand.
    pub fn pull(&mut self) -> Option<String> {
        self.queue.pop_front()
    }

    /// Returns an unanswered card from the hand to the front of the queue.
    pub fn put_back(&mut self, card_id: String) {
        self.queue.push_front(card_id);
    }

    pub fn remaining(&self) -> usize {
        self.queue.len()
    }

    pub fn answers(&self) -> &[Answer] {
        &self.answers
    }

    /// Records an answer for a card in hand. "Again" sends it back to the
    /// end of the queue so it is seen once more before the session ends.
    pub fn answer(&mut self, event_id: String, card_id: String, rating: Rating, duration_ms: u32) {
        let requeued = rating == Rating::Again;
        if requeued {
            self.queue.push_back(card_id.clone());
        }
        self.answers.push(Answer { event_id, card_id, rating, duration_ms, requeued });
    }

    /// Takes back the last answer. Its card goes to the front of the queue;
    /// the caller removes the event from the log.
    pub fn undo(&mut self) -> Option<Answer> {
        let last = self.answers.pop()?;
        if last.requeued
            && let Some(i) = self.queue.iter().rposition(|id| *id == last.card_id)
        {
            self.queue.remove(i);
        }
        self.queue.push_front(last.card_id.clone());
        Some(last)
    }

    pub fn summary(&self, now_ms: u64) -> Summary {
        let answers = self.answers.len();
        let again = self.answers.iter().filter(|a| a.rating == Rating::Again).count();
        let words = self.answers.iter().map(|a| a.card_id.as_str()).collect::<HashSet<_>>().len();
        let strongest = self
            .answers
            .iter()
            .filter(|a| matches!(a.rating, Rating::Good | Rating::Easy))
            .max_by(|a, b| (a.rating as u8).cmp(&(b.rating as u8)).then(b.duration_ms.cmp(&a.duration_ms)))
            .map(|a| a.card_id.clone());
        let mut revisit: Vec<String> = Vec::new();
        for a in self.answers.iter().filter(|a| a.rating == Rating::Again) {
            if !revisit.contains(&a.card_id) {
                revisit.push(a.card_id.clone());
            }
        }
        Summary {
            answers,
            words,
            again,
            retention: (answers > 0).then(|| (answers - again) as f32 / answers as f32),
            strongest,
            revisit,
            duration_ms: now_ms.saturating_sub(self.started_ms),
        }
    }
}

/// A time span for the UI, before localization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    Minutes(u64),
    Hours(u64),
    Days(u64),
    Months(u64),
    Years(u64),
}

impl Span {
    /// The interval an answer button leads to, as the scheduler applies it
    /// (under a day: at least 10 minutes).
    pub fn from_days(days: f32) -> Self {
        if days < 1.0 {
            let minutes = (days * 1440.0).round().max(10.0) as u64;
            if minutes < 60 {
                Self::Minutes(minutes)
            } else {
                Self::Hours((minutes as f32 / 60.0).round() as u64)
            }
        } else if days < 30.0 {
            Self::Days(days.round() as u64)
        } else if days < 365.0 {
            Self::Months((days / 30.4).round().max(1.0) as u64)
        } else {
            Self::Years((days / 365.0).round().max(1.0) as u64)
        }
    }

    /// The wait until `due_ms`, counting whole learner days once it is a day
    /// or more away (so "tomorrow" means the learner's tomorrow).
    pub fn until(scheduler: &Scheduler, now_ms: u64, due_ms: u64) -> Self {
        let wait = due_ms.saturating_sub(now_ms);
        let days = scheduler.day.day(due_ms) - scheduler.day.day(now_ms);
        if wait < DAY_MS && days < 1 {
            let minutes = wait.div_ceil(60_000).max(1);
            if minutes < 60 { Self::Minutes(minutes) } else { Self::Hours(minutes.div_ceil(60)) }
        } else {
            Self::from_days(days.max(1) as f32)
        }
    }
}

/// How a typed (or handwritten) production answer compares to the word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerMatch {
    Exact,
    /// One letter off: a typo or a near miss.
    Close,
    Wrong,
}

impl AnswerMatch {
    /// The rating the UI pre-selects; the learner still decides.
    pub fn suggested(self) -> Rating {
        match self {
            Self::Exact => Rating::Good,
            Self::Close => Rating::Hard,
            Self::Wrong => Rating::Again,
        }
    }
}

fn normalize(s: &str) -> String {
    let lower = s.to_lowercase();
    let kept: String = lower.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { ' ' }).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Compares an answer ignoring case, punctuation and extra spaces.
pub fn compare_answer(given: &str, expected: &str) -> AnswerMatch {
    let (g, e) = (normalize(given), normalize(expected));
    if g.is_empty() {
        return AnswerMatch::Wrong;
    }
    if g == e {
        return AnswerMatch::Exact;
    }
    let (gc, ec): (Vec<char>, Vec<char>) = (g.chars().collect(), e.chars().collect());
    if ec.len() >= 4 && edit_distance(&gc, &ec) == 1 { AnswerMatch::Close } else { AnswerMatch::Wrong }
}

/// Splits `sentence` around the first whole-word occurrence of `word`
/// (case-insensitive), for a cloze card: `(before, the word as written, after)`.
pub fn cloze<'a>(sentence: &'a str, word: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let lower = sentence.to_lowercase();
    let needle = word.to_lowercase();
    if lower.len() != sentence.len() || needle.is_empty() {
        return None; // lowercasing changed byte offsets; don't guess
    }
    let is_word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&needle) {
        let start = from + pos;
        let end = start + needle.len();
        if !is_word(lower[..start].chars().next_back()) && !is_word(lower[end..].chars().next()) {
            return Some((&sentence[..start], &sentence[start..end], &sentence[end..]));
        }
        from = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use kabar_protocol::{DeviceId, Hlc};

    use super::*;

    const T0: u64 = 1_761_472_800_000; // 26 Oct 2026, 10:00 UTC

    fn ev(id: &str, card: &str, at: u64, rating: Rating) -> ReviewEvent {
        ReviewEvent {
            id: id.into(),
            card_id: card.into(),
            hlc: Hlc { wall_ms: at, counter: 0, device: DeviceId("s24u".into()) },
            at_ms: at,
            rating,
            duration_ms: 2000,
            mode: ReviewMode::Classic,
        }
    }

    #[test]
    fn first_session_introduces_one_sibling_per_word() {
        let s = Scheduler::default();
        let deck = Deck::new(seed_bridge_deck(), vec![], &s);
        let q = deck.queue(&s, T0, |_| true);
        assert_eq!(q.len(), 11, "one card per word, under the daily limit");
        assert!(q.iter().all(|id| id.ends_with(".recognition")));
        assert_eq!(q[0], "seed.bridge.kabar.recognition");
        let stats = deck.stats(&s, T0);
        assert_eq!((stats.due, stats.new_available, stats.reviews_today), (0, 11, 0));
        assert_eq!(stats.retention, None);
    }

    #[test]
    fn daily_new_limit_and_due_cards() {
        let s = Scheduler::default();
        let log: Vec<ReviewEvent> = seed_bridge_deck()
            .iter()
            .filter(|c| c.kind == CardKind::Recognition)
            .enumerate()
            .map(|(i, c)| ev(&format!("e{i}"), &c.id, T0 + i as u64, Rating::Good))
            .collect();
        let deck = Deck::new(seed_bridge_deck(), log, &s);
        assert_eq!(deck.introduced_today(&s, T0 + 60_000), 11);
        // Same day: one new left in the budget, but every word's sibling was reviewed today.
        assert!(deck.queue(&s, T0 + 60_000, |_| true).is_empty());
        // A week later: the recognition cards are due, the bridge cards are new.
        let later = T0 + 7 * DAY_MS;
        let q = deck.queue(&s, later, |_| true);
        assert_eq!(q.iter().filter(|id| id.ends_with(".recognition")).count(), 11);
        assert_eq!(q.iter().filter(|id| id.ends_with(".bridge")).count(), 0, "siblings of due cards wait");
        assert_eq!(deck.forecast(&s, T0, 7).iter().sum::<u32>(), 11);
        assert!(deck.next_due_ms(T0).is_some());
    }

    #[test]
    fn record_and_forget_match_a_full_replay() {
        let s = Scheduler::default();
        let log = vec![ev("1", "seed.bridge.kabar.recognition", T0, Rating::Good)];
        let mut deck = Deck::new(seed_bridge_deck(), log.clone(), &s);
        let later = ev("2", "seed.bridge.kabar.recognition", T0 + 3 * DAY_MS, Rating::Hard);
        let earlier = ev("0", "seed.bridge.waktu.recognition", T0 - DAY_MS, Rating::Again);
        deck.record(later.clone(), &s);
        deck.record(earlier.clone(), &s);
        let full = Deck::new(seed_bridge_deck(), vec![earlier.clone(), log[0].clone(), later], &s);
        assert_eq!(deck.log(), full.log());
        assert_eq!(deck.state("seed.bridge.kabar.recognition"), full.state("seed.bridge.kabar.recognition"));
        deck.forget("2", &s);
        deck.forget("0", &s);
        assert_eq!(deck.state("seed.bridge.kabar.recognition").reps, 1);
        assert!(deck.state("seed.bridge.waktu.recognition").is_new());
        assert_eq!(deck.unseen().count(), 21);
    }

    #[test]
    fn retention_ignores_first_reviews() {
        let log = [
            ev("1", "a", T0, Rating::Again),
            ev("2", "a", T0 + DAY_MS, Rating::Good),
            ev("3", "a", T0 + 3 * DAY_MS, Rating::Again),
            ev("4", "b", T0, Rating::Good),
        ];
        assert_eq!(retention(&log, 0), Some(0.5));
        assert_eq!(retention(&log, T0 + 2 * DAY_MS), Some(0.0));
        assert_eq!(retention(&log[3..], 0), None);
    }

    #[test]
    fn session_requeues_again_and_undo_restores() {
        let mut session = Session::new(ReviewMode::Classic, vec!["a".into(), "b".into()], T0);
        let a = session.pull().unwrap();
        session.answer("e1".into(), a, Rating::Again, 4000);
        assert_eq!(session.remaining(), 2, "a comes back at the end");
        let undone = session.undo().unwrap();
        assert_eq!(undone.event_id, "e1");
        assert_eq!(session.remaining(), 2);
        assert_eq!(session.pull().as_deref(), Some("a"));
        session.answer("e2".into(), "a".into(), Rating::Easy, 900);
        let b = session.pull().unwrap();
        session.answer("e3".into(), b, Rating::Again, 5000);
        let b = session.pull().unwrap();
        session.answer("e4".into(), b, Rating::Good, 3000);
        assert!(session.pull().is_none());
        let sum = session.summary(T0 + 60_000);
        assert_eq!((sum.answers, sum.words, sum.again), (3, 2, 1));
        assert_eq!(sum.strongest.as_deref(), Some("a"));
        assert_eq!(sum.revisit, ["b"]);
        assert_eq!(sum.duration_ms, 60_000);
    }

    #[test]
    fn wall_flip_caps_easy() {
        assert_eq!(wall_rating(Pile::Tahu, 1_000, false), Rating::Easy);
        assert_eq!(wall_rating(Pile::Tahu, 1_000, true), Rating::Good);
        assert_eq!(wall_rating(Pile::Belum, 1_000, true), Rating::Again);
    }

    #[test]
    fn spans_for_buttons_and_waits() {
        assert_eq!(Span::from_days(0.001), Span::Minutes(10));
        assert_eq!(Span::from_days(0.25), Span::Hours(6));
        assert_eq!(Span::from_days(3.4), Span::Days(3));
        assert_eq!(Span::from_days(60.0), Span::Months(2));
        assert_eq!(Span::from_days(800.0), Span::Years(2));
        let s = Scheduler::default();
        assert_eq!(Span::until(&s, T0, T0 + 10 * 60_000), Span::Minutes(10));
        let tomorrow = s.day.day_start_ms(s.day.day(T0) + 1);
        assert_eq!(Span::until(&s, T0, tomorrow), Span::Days(1));
    }

    #[test]
    fn button_previews_for_a_new_card_are_readable() {
        let s = Scheduler::default();
        let spans: Vec<Span> = s
            .preview(&CardState::new("kabar"), T0)
            .iter()
            .map(|p| Span::from_days(p.interval_days))
            .collect();
        assert!(matches!(spans[0], Span::Minutes(_) | Span::Hours(_) | Span::Days(1)), "{spans:?}");
        assert!(matches!(spans[3], Span::Days(_)), "{spans:?}");
    }

    #[test]
    fn answers_compare_kindly() {
        assert_eq!(compare_answer(" Kabar! ", "kabar"), AnswerMatch::Exact);
        assert_eq!(compare_answer("kabr", "kabar"), AnswerMatch::Close);
        assert_eq!(compare_answer("waktu", "kabar"), AnswerMatch::Wrong);
        assert_eq!(compare_answer("", "kabar"), AnswerMatch::Wrong);
        assert_eq!(compare_answer("anak anak", "anak-anak"), AnswerMatch::Close);
        assert_eq!(AnswerMatch::Close.suggested(), Rating::Hard);
    }

    #[test]
    fn cloze_finds_whole_words() {
        assert_eq!(cloze("Apa kabar?", "kabar"), Some(("Apa ", "kabar", "?")));
        assert_eq!(
            cloze("Kalimat ini panjang sekali.", "kalimat"),
            Some(("", "Kalimat", " ini panjang sekali."))
        );
        assert_eq!(cloze("Makanannya lezat sekali.", "makan"), None);
        assert_eq!(cloze("Saya membaca buku.", "baca"), None);
    }
}
