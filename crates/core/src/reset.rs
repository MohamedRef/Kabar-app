//! «ابدأ من الأول»: restarting progress for one level or the whole course
//! without deleting anything (ported from Batna #1341, adapted from
//! batna-app main: crates/core/src/reset.rs).
//!
//! A reset is one synced last-writer-wins setting, `reset.<level>` (`bipa1`,
//! `j3`) or `reset.all`. Its row's own HLC is the cutoff: readers leave out
//! progress in that scope stamped at or before it.
//!
//! - Review answers by HLC, so an offline device's older answers that sync
//!   later stay out, and its answers after the reset count. A card belongs
//!   to a level when its id is `lesson.<level>.…` (cards a lesson added);
//!   every other card (the Arabic Bridge seed, Reader words, taught words)
//!   restarts only with `reset.all`.
//! - Finished lessons and level tests (`completed` lesson events) and the
//!   Lanjut position, by the HLC of their row.
//!
//! Nothing is deleted or tombstoned: undo removes the marker (a tombstone on
//! that one setting) and everything reads as before. Lesson checks
//! (verified/unverified), saved and known words, the personal dictionary,
//! highlights, XP, streaks, the journal and settings are not progress, so no
//! reader here touches them.

use std::collections::BTreeMap;

use kabar_protocol::Hlc;
use kabar_protocol::srs::ReviewEvent;

/// Synced settings keys start with this.
pub const PREFIX: &str = "reset.";
/// The scope id of the whole course (`reset.all`).
pub const ALL: &str = "all";

/// What a reset covers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// Every level and every card.
    All,
    /// One level, by id (`bipa1`, `j3`).
    Level(String),
}

impl Scope {
    /// The synced setting's key: `reset.all`, `reset.bipa1`.
    pub fn key(&self) -> String {
        format!("{PREFIX}{}", self.id())
    }

    pub fn id(&self) -> &str {
        match self {
            Scope::All => ALL,
            Scope::Level(l) => l,
        }
    }

    pub fn from_key(key: &str) -> Option<Scope> {
        match key.strip_prefix(PREFIX)? {
            "" => None,
            ALL => Some(Scope::All),
            l => Some(Scope::Level(l.to_owned())),
        }
    }
}

/// The level a lesson id belongs to: `bipa1.u01.l01` and `bipa1.test` →
/// `bipa1`.
pub fn level_of_lesson(lesson_id: &str) -> &str {
    lesson_id.split('.').next().unwrap_or(lesson_id)
}

/// The level of a card a lesson added (`lesson.bipa1.u01.l01.pagi.recognition`
/// → `bipa1`); other cards belong to no level.
pub fn level_of_card(card_id: &str) -> Option<&str> {
    card_id.strip_prefix("lesson.").map(level_of_lesson)
}

/// Every reset in force, by the HLC of its setting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resets {
    pub all: Option<Hlc>,
    pub levels: BTreeMap<String, Hlc>,
}

impl Resets {
    /// From the reset settings' keys and row stamps; other keys are skipped.
    pub fn from_rows(rows: impl IntoIterator<Item = (String, Hlc)>) -> Resets {
        let mut out = Resets::default();
        for (key, hlc) in rows {
            match Scope::from_key(&key) {
                Some(Scope::All) => out.all = Some(hlc),
                Some(Scope::Level(l)) => {
                    out.levels.insert(l, hlc);
                }
                None => {}
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_none() && self.levels.is_empty()
    }

    pub fn get(&self, scope: &Scope) -> Option<&Hlc> {
        match scope {
            Scope::All => self.all.as_ref(),
            Scope::Level(l) => self.levels.get(l),
        }
    }

    /// Every reset in force, the whole course first.
    pub fn scopes(&self) -> Vec<Scope> {
        self.all
            .iter()
            .map(|_| Scope::All)
            .chain(self.levels.keys().map(|l| Scope::Level(l.clone())))
            .collect()
    }

    fn cutoff(&self, level: Option<&str>) -> Option<&Hlc> {
        let lvl = level.and_then(|l| self.levels.get(l));
        match (self.all.as_ref(), lvl) {
            (Some(a), Some(l)) => Some(a.max(l)),
            (a, l) => a.or(l),
        }
    }

    /// Whether an answer still counts as progress.
    pub fn counts(&self, e: &ReviewEvent) -> bool {
        self.cutoff(level_of_card(&e.card_id)).is_none_or(|c| e.hlc > *c)
    }

    /// Whether a lesson's row stamped `hlc` (a finish, a Lanjut position)
    /// still counts.
    pub fn lesson_counts(&self, lesson_id: &str, hlc: &Hlc) -> bool {
        self.cutoff(Some(level_of_lesson(lesson_id))).is_none_or(|c| hlc > c)
    }

    /// The answers that still count, in the log's order.
    pub fn progress(&self, log: Vec<ReviewEvent>) -> Vec<ReviewEvent> {
        if self.is_empty() {
            return log;
        }
        log.into_iter().filter(|e| self.counts(e)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabar_protocol::DeviceId;
    use kabar_protocol::srs::{Rating, ReviewMode};

    fn hlc(wall_ms: u64, device: &str) -> Hlc {
        Hlc { wall_ms, counter: 0, device: DeviceId(device.into()) }
    }

    fn answer(card: &str, wall_ms: u64) -> ReviewEvent {
        ReviewEvent {
            id: format!("{card}@{wall_ms}"),
            card_id: card.into(),
            hlc: hlc(wall_ms, "tablet"),
            at_ms: wall_ms,
            rating: Rating::Good,
            duration_ms: 1000,
            mode: ReviewMode::Classic,
            evidence: Default::default(),
        }
    }

    fn resets(rows: &[(&str, u64)]) -> Resets {
        Resets::from_rows(rows.iter().map(|(k, at)| (k.to_string(), hlc(*at, "phone"))))
    }

    #[test]
    fn keys_and_ids() {
        assert_eq!(Scope::Level("bipa1".into()).key(), "reset.bipa1");
        assert_eq!(Scope::from_key("reset.all"), Some(Scope::All));
        assert_eq!(Scope::from_key("reset."), None);
        assert_eq!(Scope::from_key("theme"), None);
        assert_eq!(level_of_lesson("bipa1.u01.l01"), "bipa1");
        assert_eq!(level_of_lesson("j3.test"), "j3");
        assert_eq!(level_of_card("lesson.bipa1.u01.l01.pagi.recognition"), Some("bipa1"));
        assert_eq!(level_of_card("seed.bridge.kabar.meaning"), None);
        assert_eq!(level_of_card("reader.id.makan.recognition"), None);
    }

    #[test]
    fn a_level_reset_leaves_out_only_that_levels_older_answers() {
        let r = resets(&[("reset.bipa1", 100)]);
        assert!(!r.counts(&answer("lesson.bipa1.u01.l01.pagi.recognition", 99)));
        assert!(r.counts(&answer("lesson.bipa1.u01.l01.pagi.recognition", 101)));
        assert!(r.counts(&answer("lesson.bipa2.u01.l01.rumah.recognition", 50)));
        assert!(r.counts(&answer("seed.bridge.kabar.meaning", 50)), "no level: only the whole course");
    }

    #[test]
    fn the_whole_course_covers_every_card_and_the_later_marker_wins() {
        let r = resets(&[("reset.all", 100), ("reset.bipa1", 200)]);
        assert!(!r.counts(&answer("seed.bridge.kabar.meaning", 50)));
        assert!(r.counts(&answer("lesson.bipa2.u01.l01.rumah.recognition", 150)));
        assert!(!r.counts(&answer("lesson.bipa1.u01.l01.pagi.recognition", 150)));
        assert!(!r.lesson_counts("bipa1.u02.l03", &hlc(150, "tablet")));
        assert!(r.lesson_counts("bipa1.u02.l03", &hlc(201, "tablet")));
        assert!(r.lesson_counts("j1.u01.l01", &hlc(101, "tablet")));
    }

    #[test]
    fn progress_keeps_order_and_no_reset_keeps_everything() {
        let log = vec![answer("lesson.bipa1.a.b.x.y", 10), answer("lesson.bipa2.a.b.x.y", 20)];
        assert_eq!(Resets::default().progress(log.clone()), log);
        let kept = resets(&[("reset.bipa1", 15)]).progress(log);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].hlc.wall_ms, 20);
    }
}
