//! Word strength from outside the Review screen. A deck word answered in a
//! lesson exercise or tapped for its meaning while reading tells the
//! scheduler something real about that memory, so it becomes a review event
//! with its [`Evidence`] kind. The rules keep that evidence honest:
//!
//! - only cards that ask for a word's meaning (recognition, Arabic Bridge),
//!   for words the deck already holds and the learner has reviewed before
//!   (a new card waits for its first proper review);
//! - at most one event per card per learner day, and none on a day the card
//!   was already reviewed (same-day repeats say little about memory);
//! - a right answer counts only once the card is due, so seeing a word in a
//!   lesson the day after reviewing it doesn't inflate its interval;
//! - a miss counts only when the exercise tested that one word, so one wrong
//!   pick in a phrase doesn't mark every word in it as forgotten;
//! - a lookup is always "Again": the learner needed the meaning.

use std::collections::BTreeMap;

use kabar_protocol::lesson::{ExerciseKind, ExerciseResult, LessonContent, Outcome};
use kabar_protocol::srs::{CardKind, Evidence, Rating, StudyLanguage};

use super::Deck;
use crate::lesson::mistake_words;
use crate::srs::Scheduler;

/// What the learner showed about one word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Answered an exercise on it right the first time.
    Recalled,
    /// Missed an exercise that tested only this word.
    Forgot,
    /// Tapped it for its meaning while reading.
    LookedUp,
}

/// What a finished lesson's first tries say about the words they tested.
/// Retries don't count: the answer was shown in between.
pub fn lesson_signals(lesson: &LessonContent, results: &[ExerciseResult]) -> Vec<(String, Signal)> {
    let mut out = Vec::new();
    for r in results {
        let Some(ex) = lesson.exercises.iter().find(|e| e.id == r.id) else { continue };
        let words = match &ex.kind {
            ExerciseKind::MatchPairs { pairs } if ex.words.is_empty() => {
                pairs.iter().map(|p| p.text.clone()).collect()
            }
            _ => mistake_words(ex),
        };
        match r.first {
            Outcome::Correct | Outcome::Almost => {
                out.extend(words.into_iter().map(|w| (w, Signal::Recalled)))
            }
            // A matching board misses one pair, not every word on it.
            Outcome::Missed if matches!(ex.kind, ExerciseKind::MatchPairs { .. }) => {}
            Outcome::Missed if words.len() == 1 => out.extend(words.into_iter().map(|w| (w, Signal::Forgot))),
            _ => {}
        }
    }
    out
}

/// The review events the signals justify, as (card id, rating, evidence).
/// Several signals for one card collapse to one event, a miss winning.
pub fn evidence_reviews(
    scheduler: &Scheduler,
    deck: &Deck,
    language: StudyLanguage,
    signals: &[(String, Signal)],
    now_ms: u64,
) -> Vec<(String, Rating, Evidence)> {
    let today = scheduler.day.day(now_ms);
    let mut by_card: BTreeMap<&str, (Rating, Evidence)> = BTreeMap::new();
    for (word, signal) in signals {
        let word = word.trim();
        // The cards that ask for the word's meaning: recognition and the
        // Arabic Bridge. Producing the word from its meaning is another skill.
        let cards = deck.cards().iter().filter(|c| {
            matches!(c.kind, CardKind::Recognition | CardKind::Bridge)
                && c.language == language
                && c.word.eq_ignore_ascii_case(word)
        });
        for card in cards {
            let state = deck.state(&card.id);
            if state.is_new() || state.last_review_ms.is_some_and(|t| scheduler.day.day(t) == today) {
                continue;
            }
            let event = match signal {
                Signal::Recalled if state.due_ms.is_some_and(|due| due <= now_ms) => {
                    (Rating::Good, Evidence::Exercise)
                }
                Signal::Recalled => continue,
                Signal::Forgot => (Rating::Again, Evidence::Exercise),
                Signal::LookedUp => (Rating::Again, Evidence::Lookup),
            };
            by_card
                .entry(card.id.as_str())
                .and_modify(|e| {
                    if event.0 == Rating::Again {
                        *e = event;
                    }
                })
                .or_insert(event);
        }
    }
    by_card.into_iter().map(|(id, (rating, evidence))| (id.to_string(), rating, evidence)).collect()
}

#[cfg(test)]
mod tests {
    use kabar_protocol::Hlc;
    use kabar_protocol::srs::{ReviewEvent, ReviewMode};

    use super::*;
    use crate::lesson::parse_lesson;
    use crate::review::lesson_cards;

    const FIXTURE: &str = include_str!("../../../../fixtures/lessons/bipa1.u01.l01.yaml");
    const DAY: u64 = 86_400_000;
    // 2026-10-20 12:00 Cairo.
    const T0: u64 = 1_760_950_800_000;

    fn review(card: &str, at_ms: u64) -> ReviewEvent {
        ReviewEvent {
            id: format!("{card}@{at_ms}"),
            card_id: card.into(),
            hlc: Hlc { wall_ms: at_ms, counter: 0, device: kabar_protocol::DeviceId("m".into()) },
            at_ms,
            rating: Rating::Good,
            duration_ms: 1000,
            mode: ReviewMode::Classic,
            evidence: Evidence::Review,
        }
    }

    fn result(id: &str, first: Outcome) -> ExerciseResult {
        ExerciseResult { id: id.into(), first, retry: None, mistakes: Vec::new() }
    }

    #[test]
    fn first_tries_become_signals() {
        let lesson = parse_lesson(FIXTURE).unwrap();
        let signals = lesson_signals(
            &lesson,
            &[
                result("e01", Outcome::Correct),
                result("e02", Outcome::Missed),
                result("nope", Outcome::Correct),
            ],
        );
        assert_eq!(
            signals,
            [("selamat".to_string(), Signal::Recalled), ("pagi".to_string(), Signal::Recalled)]
        );
        // A miss on a two-word exercise blames neither word.
        assert!(lesson_signals(&lesson, &[result("e01", Outcome::Missed)]).is_empty());
    }

    #[test]
    fn evidence_only_where_it_says_something() {
        let lesson = parse_lesson(FIXTURE).unwrap();
        let cards = lesson_cards(&lesson, &[]);
        let id = |w: &str| cards.iter().find(|c| c.word == w).unwrap().id.clone();
        let (pagi, siang, sore) = (id("pagi"), id("siang"), id("sore"));
        let s = Scheduler::default();
        // pagi and siang were learned ten days ago; sore yesterday; malam never.
        let log = vec![review(&pagi, T0 - 10 * DAY), review(&siang, T0 - 10 * DAY), review(&sore, T0 - DAY)];
        let deck = Deck::new(cards.clone(), log, &s);
        assert!(deck.state(&pagi).due_ms.unwrap() <= T0, "pagi is due");
        let signals: Vec<(String, Signal)> = [
            ("Pagi", Signal::Recalled),
            ("siang", Signal::Recalled),
            ("siang", Signal::LookedUp),
            ("sore", Signal::Recalled),
            ("malam", Signal::Forgot),
            ("kucing", Signal::LookedUp),
        ]
        .into_iter()
        .map(|(w, s)| (w.to_string(), s))
        .collect();
        let mut got = evidence_reviews(&s, &deck, StudyLanguage::Id, &signals, T0);
        got.sort_by(|a, b| a.0.cmp(&b.0));
        let mut want =
            vec![(pagi.clone(), Rating::Good, Evidence::Exercise), (siang, Rating::Again, Evidence::Lookup)];
        want.sort_by(|a, b| a.0.cmp(&b.0));
        // sore isn't due yet, malam is new, kucing isn't in the deck.
        assert_eq!(got, want);

        // Javanese cards are a different deck; a card reviewed today gets nothing more.
        assert!(evidence_reviews(&s, &deck, StudyLanguage::Jv, &signals, T0).is_empty());
        let reviewed_today =
            Deck::new(cards, vec![review(&pagi, T0 - 10 * DAY), review(&pagi, T0 - 3_600_000)], &s);
        assert!(evidence_reviews(&s, &reviewed_today, StudyLanguage::Id, &signals[..1], T0).is_empty());
    }
}
