//! The learner's FSRS scheduler (#118), built in one place: the coach's UTC
//! offset (so "due today", the 4 am rollover and per-day counts follow the
//! same day as Kak Nur), the desired retention and the personal parameters
//! from the synced settings. Once there are enough reviews, the parameters
//! are re-fitted to the learner's own log in the background every week
//! (SPEC §16: "optimized on my own review log once there's enough data").

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::settings::{RetentionPct, Settings};
use kabar_core::srs::{FsrsParams, MIN_REVIEWS_TO_OPTIMIZE, Scheduler};

use crate::coach::Coach;
use crate::store::Store;
use crate::{Db, Ui};

const DAY_MS: u64 = 86_400_000;
/// Personal parameters are re-fitted at most this often.
pub const OPTIMIZE_EVERY_MS: u64 = 7 * DAY_MS;
/// After a check that found too little history (or a failed fit), wait this
/// long before looking again.
const RETRY_MS: u64 = DAY_MS;

/// Everything a scheduler is built from.
#[derive(Clone, PartialEq, Eq)]
struct Key {
    offset: i16,
    retention: RetentionPct,
    fsrs: Option<FsrsParams>,
}

thread_local! {
    static CACHE: RefCell<Option<(Key, Rc<Scheduler>)>> = const { RefCell::new(None) };
    /// When the optimizer last looked at the log (UTC ms).
    static LAST_TRY_MS: Cell<u64> = const { Cell::new(0) };
    static FITTING: Cell<bool> = const { Cell::new(false) };
}

/// The scheduler for a time zone and settings. The same `Rc` comes back
/// while neither changes, so data derived from it (the deck cache) can be
/// keyed on it, and building FSRS is not repeated per screen.
pub fn scheduler_at(utc_offset_min: i16, settings: &Settings) -> Rc<Scheduler> {
    let key =
        Key { offset: utc_offset_min, retention: settings.desired_retention, fsrs: settings.fsrs.clone() };
    if let Some(hit) = CACHE.with_borrow(|c| c.as_ref().filter(|(k, _)| *k == key).map(|(_, s)| s.clone())) {
        return hit;
    }
    let sched = Rc::new(Scheduler::from_settings(settings, utc_offset_min));
    CACHE.set(Some((key, sched.clone())));
    sched
}

/// The learner's scheduler: the coach's offset and the stored settings.
/// Review, Stats, the level check, Today and Kabar Pagi all use this.
pub fn scheduler(coach: &Coach, store: &Store) -> Rc<Scheduler> {
    let settings = store.settings().unwrap_or_else(|e| {
        crate::diag::report!("reading settings for the scheduler failed: {e}");
        Settings::default()
    });
    scheduler_at(coach.offset(), &settings)
}

/// Whether to look at the log for a new fit now.
fn time_to_fit(settings: &Settings, now_ms: u64, last_try_ms: u64) -> bool {
    now_ms.saturating_sub(last_try_ms) >= RETRY_MS
        && settings.fsrs.as_ref().is_none_or(|p| now_ms.saturating_sub(p.trained_at_ms) >= OPTIMIZE_EVERY_MS)
}

/// Re-fits the FSRS parameters in the background: checked every minute
/// with the coach's clock, run at most weekly, only with at least
/// [`MIN_REVIEWS_TO_OPTIMIZE`] training reviews, and never on the UI
/// thread. The result is a synced setting, so every device schedules with
/// it and the scheduler (and the deck built with it) is rebuilt.
pub fn use_optimizer(db: Db, ui: Ui, coach: Coach) {
    use_effect(move || {
        let now = coach.now();
        let offset = coach.offset();
        let settings = ui.settings.peek().clone();
        if FITTING.get() || !time_to_fit(&settings, now, LAST_TRY_MS.get()) {
            return;
        }
        LAST_TRY_MS.set(now);
        let deck = crate::review::deck(&db, &scheduler_at(offset, &settings));
        // Every training item is a logged review, so fewer events can't be enough.
        if deck.log().len() < MIN_REVIEWS_TO_OPTIMIZE {
            return;
        }
        let log = deck.log().to_vec();
        FITTING.set(true);
        let db = db.clone();
        spawn(async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            std::thread::spawn(move || {
                let sched = Scheduler::from_settings(&settings, offset);
                let reviews = sched.training_items(&log).len();
                let _ = tx.send(sched.optimize(&log).map(|weights| (weights, reviews)));
            });
            let fitted = rx.await.ok().flatten();
            FITTING.set(false);
            match fitted {
                Some((weights, reviews)) => {
                    let params = FsrsParams {
                        weights,
                        reviews: u32::try_from(reviews).unwrap_or(u32::MAX),
                        trained_at_ms: crate::store::now_ms(),
                    };
                    ui.update_settings(&db, |s| s.fsrs = Some(params));
                }
                None => {
                    crate::diag::report!("FSRS parameters not fitted (too little history or a fit error)")
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use kabar_core::review::{Deck, seed_bridge_deck};
    use kabar_core::stats::day_totals;
    use kabar_protocol::coach::{LocalDate, LocalTime, to_unix_ms};
    use kabar_protocol::srs::{Rating, ReviewEvent, ReviewMode};
    use kabar_protocol::{DeviceId, Hlc};

    use super::*;
    use crate::coach::clock;

    fn review(id: &str, card: &str, at_ms: u64) -> ReviewEvent {
        ReviewEvent {
            id: id.into(),
            card_id: card.into(),
            hlc: Hlc { wall_ms: at_ms, counter: 0, device: DeviceId("t".into()) },
            at_ms,
            rating: Rating::Good,
            duration_ms: 3000,
            mode: ReviewMode::Classic,
            evidence: Default::default(),
        }
    }

    /// Cairo in winter and summer, and Jakarta.
    const OFFSETS: [i16; 3] = [120, 180, 420];

    #[test]
    fn a_review_at_0130_lands_on_the_same_day_everywhere() {
        let date = LocalDate::from_ymd(2026, 11, 3).unwrap();
        let local = |d: LocalDate, h: u16, m: u16, off| to_unix_ms(d, LocalTime::hm(h, m), off);
        for off in OFFSETS {
            let sched = scheduler_at(off, &Settings::default());
            let at = local(date, 1, 30, off);
            // Stats' heatmap and Kak Nur put it on the same calendar date.
            let totals = day_totals(&[], &[review("r", "c", at)], off);
            assert_eq!(totals.keys().copied().collect::<Vec<_>>(), [date], "offset {off}");
            assert_eq!(clock::today(at, off), date, "offset {off}");
            // The scheduler rolls over at 4 am *local*: 01:30 belongs to the
            // evening before, 05:00 to the new day, in every time zone.
            let study_day = sched.day.day(at);
            assert_eq!(sched.day.day(local(date.add_days(-1), 22, 0, off)), study_day, "offset {off}");
            assert_eq!(sched.day.day(local(date, 5, 0, off)), study_day + 1, "offset {off}");
            // And Review counts it among "today's" reviews until 4 am.
            let cards = seed_bridge_deck();
            let deck = Deck::new(cards.clone(), vec![review("r", &cards[0].id, at)], &sched);
            assert_eq!(deck.stats(&sched, local(date, 3, 0, off)).reviews_today, 1, "offset {off}");
            assert_eq!(deck.stats(&sched, local(date, 5, 0, off)).reviews_today, 0, "offset {off}");
        }
    }

    #[test]
    fn the_scheduler_is_shared_until_its_inputs_change() {
        let settings = Settings::default();
        let a = scheduler_at(120, &settings);
        assert!(Rc::ptr_eq(&a, &scheduler_at(120, &settings)));
        let travel = scheduler_at(420, &settings);
        assert!(!Rc::ptr_eq(&a, &travel));
        assert_eq!(travel.day.utc_offset_minutes, 420);
        let fitted = Settings {
            fsrs: Some(FsrsParams { weights: vec![0.4, 1.2, 3.1], reviews: 600, trained_at_ms: 1 }),
            ..settings
        };
        assert!(!Rc::ptr_eq(&travel, &scheduler_at(420, &fitted)));
    }

    #[test]
    fn fits_weekly_and_retries_daily() {
        let now = 1_761_472_800_000;
        let mut s = Settings::default();
        assert!(time_to_fit(&s, now, 0), "never fitted");
        assert!(!time_to_fit(&s, now, now - 3_600_000), "looked an hour ago");
        s.fsrs = Some(FsrsParams { weights: Vec::new(), reviews: 500, trained_at_ms: now - 3 * DAY_MS });
        assert!(!time_to_fit(&s, now, 0), "fitted this week");
        s.fsrs.as_mut().unwrap().trained_at_ms = now - OPTIMIZE_EVERY_MS;
        assert!(time_to_fit(&s, now, 0), "a week old");
    }
}
