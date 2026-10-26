//! Rewards: Pelita (the streak lamp, with freezes that are earned, never
//! bought), Koin for the Warung's cosmetic batik themes, Liga Pribadi (this
//! week against your own past weeks) and achievements named after proverbs.
//! All derived from the activity log, so nothing can double-count across
//! devices.
//!
//! Pelita rules (kind by design):
//! - A day is lit when the learner studied for at least [`LIT_MINUTES`]
//!   minutes or finished any plan item.
//! - The streak counts lit days in a row. Today only counts once it's lit, so
//!   the lamp never shows 0 in the morning.
//! - Every [`FREEZE_EVERY`] lit days in a streak earn a freeze; at most
//!   [`MAX_FREEZES`] are held.
//! - A single missed day between lit days is covered by a freeze
//!   automatically: the streak survives, but the frozen day adds nothing to the
//!   count. Yesterday missed (and today not lit yet) is covered provisionally;
//!   if today is missed too, the freeze is given back.
//! - A longer break restarts the streak quietly; `best` and freezes are kept.

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::coach::{ActivityEvent, ActivityKind, LeagueResult, LocalDate};

use super::history::{self, DayStats};
use super::plan::COMEBACK_AFTER_DAYS;

/// Minutes of study that light the Pelita.
pub const LIT_MINUTES: u32 = 5;
/// Lit days in a streak per earned freeze.
pub const FREEZE_EVERY: u32 = 7;
pub const MAX_FREEZES: u32 = 2;
/// XP per Koin, counted per day (each day's XP divided down separately).
pub const XP_PER_KOIN: u32 = 10;
/// Extra Koin for each day the Pelita is lit.
pub const LIT_DAY_KOIN: u32 = 5;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pelita {
    /// Days in a row, counting frozen days as kept.
    pub streak: u32,
    pub best: u32,
    /// Freezes in hand.
    pub freezes: u32,
    pub lit_today: bool,
    /// Days a freeze covered, newest first.
    pub frozen_days: Vec<LocalDate>,
}

/// Whether a day lights the Pelita.
pub fn is_lit(stats: &DayStats) -> bool {
    stats.studied() && (stats.minutes() >= LIT_MINUTES || !stats.items_done.is_empty())
}

pub fn pelita(events: &[ActivityEvent], today: LocalDate) -> Pelita {
    let days = history::by_day(events);
    let mut walk = Walk::default();
    let Some(first) = days.iter().find(|(d, s)| **d <= today && is_lit(s)).map(|(d, _)| *d) else {
        return Pelita::default();
    };
    for n in 0..=today.days_since(first) {
        let date = first.add_days(n);
        walk.step(date, date == today, days.get(&date).is_some_and(is_lit));
    }
    walk.finish(today)
}

/// The Pelita walked day by day.
#[derive(Default)]
struct Walk {
    run: u32,
    best: u32,
    freezes: u32,
    lit_today: bool,
    /// Unlit days in a row before the current day (today excluded).
    missed: i32,
    frozen: Vec<LocalDate>,
}

impl Walk {
    fn step(&mut self, date: LocalDate, is_today: bool, lit: bool) {
        if lit {
            if self.missed == 1 && self.freezes > 0 {
                self.freezes -= 1;
                self.frozen.push(date.add_days(-1));
            } else if self.missed > 0 {
                self.run = 0;
            }
            self.missed = 0;
            self.run += 1;
            self.best = self.best.max(self.run);
            if self.run.is_multiple_of(FREEZE_EVERY) && self.freezes < MAX_FREEZES {
                self.freezes += 1;
            }
            self.lit_today = is_today;
        } else if !is_today {
            self.missed += 1;
        }
    }

    fn finish(mut self, today: LocalDate) -> Pelita {
        if !self.lit_today {
            if self.missed == 1 && self.freezes > 0 {
                self.freezes -= 1;
                self.frozen.push(today.add_days(-1));
            } else if self.missed > 0 {
                self.run = 0;
            }
        }
        self.frozen.reverse();
        Pelita {
            streak: self.run,
            best: self.best,
            freezes: self.freezes,
            lit_today: self.lit_today,
            frozen_days: self.frozen,
        }
    }
}

/// Koin earned minus Koin spent: each day earns its XP / [`XP_PER_KOIN`]
/// (rounded down) plus [`LIT_DAY_KOIN`] if the Pelita was lit. Never negative.
pub fn koin_balance(events: &[ActivityEvent]) -> u32 {
    let (earned, spent) = history::by_day(events).values().fold((0u32, 0u32), |(earned, spent), s| {
        let bonus = if is_lit(s) { LIT_DAY_KOIN } else { 0 };
        (earned.saturating_add(s.xp / XP_PER_KOIN + bonus), spent.saturating_add(s.koin_spent))
    });
    earned.saturating_sub(spent)
}

/// This week's XP ranked against up to `past_weeks` earlier weeks. Weeks run
/// Monday to Sunday; earlier weeks before the learner's first XP aren't
/// compared. Ties go to this week.
pub fn league(events: &[ActivityEvent], week_start: LocalDate, past_weeks: u32) -> LeagueResult {
    let this = week_start.week_start();
    let mut weeks: BTreeMap<LocalDate, u32> = BTreeMap::new();
    let mut first: Option<LocalDate> = None;
    for (date, s) in history::by_day(events) {
        if s.xp > 0 {
            first = Some(first.map_or(date, |f| f.min(date)));
            *weeks.entry(date.week_start()).or_default() += s.xp;
        }
    }
    let xp = weeks.get(&this).copied().unwrap_or(0);
    let Some(first_week) = first.map(LocalDate::week_start) else {
        return LeagueResult { xp, rank: 1, of: 1 };
    };
    let earlier: Vec<u32> = (1..=past_weeks.min(520) as i32)
        .map(|n| this.add_days(-7 * n))
        .take_while(|w| *w >= first_week)
        .map(|w| weeks.get(&w).copied().unwrap_or(0))
        .collect();
    let rank = 1 + earlier.iter().filter(|&&w| w > xp).count() as u32;
    LeagueResult { xp, rank, of: 1 + earlier.len() as u32 }
}

/// A cosmetic Warung item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarungItem {
    pub id: &'static str,
    /// Indonesian name, e.g. "Kawung Senja".
    pub name: &'static str,
    pub koin: u32,
    /// Batik tile colour (CSS hex without `#`).
    pub color: &'static str,
}

const WARUNG: &[WarungItem] = &[
    WarungItem { id: "kawung-senja", name: "Kawung Senja", koin: 100, color: "c8573a" },
    WarungItem { id: "parang-fajar", name: "Parang Fajar", koin: 150, color: "8a6100" },
    WarungItem { id: "lereng-sawah", name: "Lereng Sawah", koin: 200, color: "2f6b3a" },
    WarungItem { id: "mega-mendung", name: "Mega Mendung", koin: 250, color: "2b3a70" },
    WarungItem { id: "truntum-malam", name: "Truntum Malam", koin: 300, color: "1f2a52" },
    WarungItem { id: "sidomukti", name: "Sidomukti", koin: 400, color: "6b4a2b" },
];

/// Everything on sale, cheapest first.
pub fn warung() -> &'static [WarungItem] {
    WARUNG
}

/// Item ids already bought, in the order first bought, without duplicates.
pub fn owned(events: &[ActivityEvent]) -> Vec<String> {
    let mut seen_events = BTreeSet::new();
    let mut purchases: Vec<(LocalDate, u64, &str)> = events
        .iter()
        .filter(|e| seen_events.insert(e.id.as_str()))
        .filter_map(|e| match &e.kind {
            ActivityKind::Purchase { item_id, .. } => Some((e.date, e.at_ms, item_id.as_str())),
            _ => None,
        })
        .collect();
    purchases.sort();
    let mut seen = BTreeSet::new();
    purchases.into_iter().filter(|(_, _, id)| seen.insert(*id)).map(|(_, _, id)| id.to_string()).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Achievement {
    pub id: &'static str,
    /// The Indonesian proverb it's named after.
    pub proverb: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unlocked {
    pub achievement: Achievement,
    pub on: LocalDate,
}

const ACHIEVEMENTS: &[Achievement] = &[
    // The first day of study.
    Achievement { id: "seribu-langkah", proverb: "Perjalanan seribu mil dimulai dengan satu langkah" },
    // A Pelita of 7 days.
    Achievement { id: "sedikit-demi-sedikit", proverb: "Sedikit demi sedikit, lama-lama menjadi bukit" },
    // Back to study after a break of more than three days.
    Achievement { id: "ada-kemauan-ada-jalan", proverb: "Di mana ada kemauan, di situ ada jalan" },
    // 100 reviews in all.
    Achievement { id: "rajin-pangkal-pandai", proverb: "Rajin pangkal pandai" },
    // 50 different new words.
    Achievement { id: "tak-kenal-maka-tak-sayang", proverb: "Tak kenal maka tak sayang" },
    // 50 mistakes made and learned from: effort, not perfection.
    Achievement { id: "tak-ada-gading-yang-tak-retak", proverb: "Tak ada gading yang tak retak" },
    // One day with 30 minutes or more.
    Achievement { id: "berakit-rakit-ke-hulu", proverb: "Berakit-rakit ke hulu, berenang-renang ke tepian" },
    // 30 study days in all, in a row or not.
    Achievement { id: "alah-bisa-karena-biasa", proverb: "Alah bisa karena biasa" },
    // The first Warung purchase.
    Achievement { id: "hemat-pangkal-kaya", proverb: "Hemat pangkal kaya" },
];

pub fn achievements() -> &'static [Achievement] {
    ACHIEVEMENTS
}

/// Achievements earned up to `today`, oldest first (ties in [`achievements`]
/// order).
pub fn unlocked(events: &[ActivityEvent], today: LocalDate) -> Vec<Unlocked> {
    let days = history::by_day(events);
    let mut on: BTreeMap<&'static str, LocalDate> = BTreeMap::new();
    let mut earn = |id: &'static str, date: LocalDate| {
        on.entry(id).or_insert(date);
    };
    let (mut reviews, mut mistakes, mut study_days) = (0u32, 0u32, 0u32);
    let mut words: BTreeSet<&str> = BTreeSet::new();
    let mut last_study: Option<LocalDate> = None;
    let mut walk = Walk::default();
    let mut walked: Option<LocalDate> = None;
    for (&date, s) in days.range(..=today) {
        // Walk the Pelita through any days without events, too.
        let from = walked.map_or(date, |w| w.add_days(1));
        for n in 0..=date.days_since(from) {
            let d = from.add_days(n);
            let lit = d == date && is_lit(s);
            if walked.is_some() || lit {
                walk.step(d, d == today, lit);
                walked = Some(d);
            }
        }
        if walk.best >= 7 {
            earn("sedikit-demi-sedikit", date);
        }
        if s.studied() {
            earn("seribu-langkah", date);
            if last_study.is_some_and(|l| date.days_since(l) > COMEBACK_AFTER_DAYS) {
                earn("ada-kemauan-ada-jalan", date);
            }
            last_study = Some(date);
            study_days += 1;
        }
        reviews += s.reviews;
        mistakes += s.reviews - s.reviews_correct;
        mistakes += s.practice.iter().map(|p| p.total - p.correct).sum::<u32>();
        words.extend(s.new_words.iter().map(String::as_str));
        let checks = [
            ("rajin-pangkal-pandai", reviews >= 100),
            ("tak-kenal-maka-tak-sayang", words.len() >= 50),
            ("tak-ada-gading-yang-tak-retak", mistakes >= 50),
            ("berakit-rakit-ke-hulu", s.minutes() >= 30),
            ("alah-bisa-karena-biasa", study_days >= 30),
            ("hemat-pangkal-kaya", s.koin_spent > 0),
        ];
        for (id, met) in checks {
            if met {
                earn(id, date);
            }
        }
    }
    let mut out: Vec<(LocalDate, usize, Achievement)> = ACHIEVEMENTS
        .iter()
        .enumerate()
        .filter_map(|(i, a)| on.get(a.id).map(|&date| (date, i, *a)))
        .collect();
    out.sort_by_key(|&(date, i, _)| (date, i));
    out.into_iter().map(|(on, _, achievement)| Unlocked { achievement, on }).collect()
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::{ActivityKind, Skill};

    use super::*;
    use crate::coach::history::tests::{d, event};

    fn study(id: &str, date: LocalDate, minutes: u32) -> ActivityEvent {
        event(id, date, minutes * 60, ActivityKind::Reviews { cards: 10, correct: 8 })
    }

    /// One 10-minute session on each listed day (days ago from `today`).
    fn days_ago(today: LocalDate, ago: &[i32]) -> Vec<ActivityEvent> {
        ago.iter().map(|&n| study(&format!("s{n}"), today.add_days(-n), 10)).collect()
    }

    fn range(today: LocalDate, from_ago: i32, to_ago: i32) -> Vec<ActivityEvent> {
        days_ago(today, &(to_ago..=from_ago).collect::<Vec<_>>())
    }

    #[test]
    fn what_lights_the_lamp() {
        let today = d(2026, 10, 26);
        assert!(is_lit(&history::day(&[study("a", today, 5)], today)));
        assert!(!is_lit(&history::day(&[study("a", today, 4)], today)));
        let item = event("i", today, 30, ActivityKind::PlanItemDone { item_id: "review".into() });
        assert!(is_lit(&history::day(&[item], today)));
        let buy = event("p", today, 0, ActivityKind::Purchase { item_id: "x".into(), koin: 5 });
        assert!(!is_lit(&history::day(&[buy], today)));
    }

    #[test]
    fn empty_log_is_an_unlit_lamp() {
        assert_eq!(pelita(&[], d(2026, 10, 26)), Pelita::default());
    }

    #[test]
    fn streak_counts_today_or_yesterday() {
        let today = d(2026, 10, 26);
        // Three days ending yesterday; not studied yet today: still 3, not 0.
        let p = pelita(&range(today, 3, 1), today);
        assert_eq!((p.streak, p.best, p.lit_today), (3, 3, false));
        let p = pelita(&range(today, 3, 0), today);
        assert_eq!((p.streak, p.best, p.lit_today), (4, 4, true));
        // A short day today doesn't light it but doesn't break anything.
        let mut events = range(today, 3, 1);
        events.push(study("short", today, 2));
        assert_eq!(pelita(&events, today).streak, 3);
    }

    #[test]
    fn freezes_are_earned_every_seven_days_up_to_two() {
        let today = d(2026, 10, 26);
        assert_eq!(pelita(&range(today, 5, 0), today).freezes, 0);
        assert_eq!(pelita(&range(today, 6, 0), today).freezes, 1);
        assert_eq!(pelita(&range(today, 13, 0), today).freezes, 2);
        let p = pelita(&range(today, 29, 0), today);
        assert_eq!((p.streak, p.freezes), (30, 2));
    }

    #[test]
    fn a_single_missed_day_is_frozen() {
        let today = d(2026, 10, 26);
        // 7 lit days, a miss 2 days ago, lit yesterday and today.
        let mut ago: Vec<i32> = (3..=9).collect();
        ago.extend([1, 0]);
        let p = pelita(&days_ago(today, &ago), today);
        assert_eq!(p.frozen_days, [today.add_days(-2)]);
        assert_eq!((p.streak, p.freezes, p.best), (9, 0, 9));
    }

    #[test]
    fn missing_yesterday_is_covered_provisionally() {
        let today = d(2026, 10, 26);
        let events = range(today, 8, 2);
        let p = pelita(&events, today);
        assert_eq!((p.streak, p.freezes, p.frozen_days.clone()), (7, 0, vec![today.add_days(-1)]));
        // Missing today too gives the freeze back and restarts quietly.
        let p = pelita(&events, today.add_days(1));
        assert_eq!((p.streak, p.best, p.freezes), (0, 7, 1));
        assert!(p.frozen_days.is_empty());
    }

    #[test]
    fn without_a_freeze_a_miss_restarts_and_best_is_kept() {
        let today = d(2026, 10, 26);
        let p = pelita(&days_ago(today, &[5, 4, 3, 1, 0]), today);
        assert_eq!((p.streak, p.best, p.freezes), (2, 3, 0));
        assert!(p.frozen_days.is_empty());
        let p = pelita(&days_ago(today, &[5, 4, 3]), today);
        assert_eq!((p.streak, p.best), (0, 3));
    }

    #[test]
    fn two_freezes_cover_two_separate_misses() {
        let today = d(2026, 10, 26);
        let mut ago: Vec<i32> = (6..=19).collect(); // 14 lit days: two freezes
        ago.extend([4, 3, 1, 0]);
        let p = pelita(&days_ago(today, &ago), today);
        assert_eq!(p.frozen_days, [today.add_days(-2), today.add_days(-5)]);
        assert_eq!((p.streak, p.freezes), (18, 0));
    }

    #[test]
    fn future_events_are_ignored() {
        let today = d(2026, 10, 26);
        let p = pelita(&days_ago(today, &[-1, -2]), today);
        assert_eq!(p, Pelita::default());
    }

    #[test]
    fn koin_from_xp_and_lit_days_minus_purchases() {
        let today = d(2026, 10, 26);
        // 10 cards / 8 correct = 18 XP → 1 Koin, plus 5 for a lit day.
        let mut events = vec![study("a", today, 10), study("b", today.add_days(-1), 10)];
        assert_eq!(koin_balance(&events), 12);
        // Per-day floors: 18 + 18 XP on one day is 3 Koin, not 2.
        events.push(study("c", today, 1));
        assert_eq!(koin_balance(&events), 3 + 5 + 1 + 5);
        events.push(event("p", today, 0, ActivityKind::Purchase { item_id: "x".into(), koin: 10 }));
        assert_eq!(koin_balance(&events), 4);
        // Replayed purchase counts once; overspending floors at zero.
        events.push(event("p", today, 0, ActivityKind::Purchase { item_id: "x".into(), koin: 10 }));
        assert_eq!(koin_balance(&events), 4);
        events.push(event("q", today, 0, ActivityKind::Purchase { item_id: "y".into(), koin: 99 }));
        assert_eq!(koin_balance(&events), 0);
    }

    #[test]
    fn league_against_past_weeks() {
        let monday = d(2026, 10, 26);
        let xp = |id: &str, date: LocalDate, cards: u32| {
            event(id, date, 60, ActivityKind::Reviews { cards, correct: cards })
        };
        let events = vec![
            xp("a", monday.add_days(-21), 50), // 100 XP, 3 weeks ago
            xp("b", monday.add_days(-12), 10), // 20 XP, 2 weeks ago
            xp("c", monday.add_days(-1), 30),  // 60 XP, last Sunday
            xp("d", monday.add_days(2), 30),   // 60 XP this week
        ];
        let r = league(&events, monday.add_days(3), 8);
        // Weeks before the first activity aren't compared; ties favour this week.
        assert_eq!(r, LeagueResult { xp: 60, rank: 2, of: 4 });
        assert_eq!(league(&events, monday, 1), LeagueResult { xp: 60, rank: 1, of: 2 });
        assert_eq!(league(&events, monday, 0), LeagueResult { xp: 60, rank: 1, of: 1 });
        // A week not started yet: every earlier week is ahead, kindly shown as a rank.
        let r = league(&events, monday.add_days(7), 8);
        assert_eq!(r, LeagueResult { xp: 0, rank: 5, of: 5 });
        assert_eq!(league(&[], monday, 4), LeagueResult { xp: 0, rank: 1, of: 1 });
    }

    #[test]
    fn warung_is_cosmetic_and_well_formed() {
        let items = warung();
        assert_eq!(items.len(), 6);
        let ids: BTreeSet<&str> = items.iter().map(|i| i.id).collect();
        assert_eq!(ids.len(), items.len());
        for i in items {
            assert!((100..=400).contains(&i.koin), "{}", i.id);
            assert!(i.color.len() == 6 && i.color.chars().all(|c| c.is_ascii_hexdigit()), "{}", i.id);
            assert!(i.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'), "{}", i.id);
        }
        assert!(items.windows(2).all(|w| w[0].koin <= w[1].koin));
    }

    #[test]
    fn owned_in_purchase_order_without_duplicates() {
        let today = d(2026, 10, 26);
        let buy = |id: &str, date: LocalDate, item: &str| {
            event(id, date, 0, ActivityKind::Purchase { item_id: item.into(), koin: 100 })
        };
        let events = vec![
            buy("2", today, "mega-mendung"),
            buy("1", today.add_days(-3), "kawung-senja"),
            buy("3", today, "kawung-senja"),
            study("s", today, 10),
        ];
        assert_eq!(owned(&events), ["kawung-senja", "mega-mendung"]);
        assert!(owned(&[]).is_empty());
    }

    #[test]
    fn achievements_are_proverbs_with_unique_kebab_ids() {
        let all = achievements();
        assert!(all.len() >= 8);
        let ids: BTreeSet<&str> = all.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), all.len());
        for a in all {
            assert!(a.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'), "{}", a.id);
            assert!(!a.proverb.is_empty());
        }
    }

    fn ids(u: &[Unlocked]) -> Vec<(&'static str, LocalDate)> {
        u.iter().map(|u| (u.achievement.id, u.on)).collect()
    }

    #[test]
    fn unlocks_effort_and_comebacks() {
        let today = d(2026, 10, 26);
        let start = today.add_days(-20);
        let mut events: Vec<ActivityEvent> =
            (0..7).map(|n| study(&format!("s{n}"), start.add_days(n), 10)).collect();
        // A break, then back after five days with a long session.
        let back = ActivityKind::Reviews { cards: 30, correct: 20 };
        events.push(event("back", start.add_days(11), 30 * 60, back));
        let words: Vec<String> = (0..50).map(|n| format!("kata{n}")).collect();
        events.push(event("w", start.add_days(12), 0, ActivityKind::NewWords { words }));
        events.push(event(
            "p",
            today,
            0,
            ActivityKind::Purchase { item_id: "kawung-senja".into(), koin: 100 },
        ));
        let u = unlocked(&events, today);
        assert_eq!(
            ids(&u),
            [
                ("seribu-langkah", start),
                ("sedikit-demi-sedikit", start.add_days(6)),
                ("ada-kemauan-ada-jalan", start.add_days(11)),
                ("rajin-pangkal-pandai", start.add_days(11)),
                ("berakit-rakit-ke-hulu", start.add_days(11)),
                ("tak-kenal-maka-tak-sayang", start.add_days(12)),
                ("hemat-pangkal-kaya", today),
            ]
        );
        // Nothing from the future, and deterministic.
        assert_eq!(ids(&unlocked(&events, start.add_days(5))), [("seribu-langkah", start)]);
        assert_eq!(unlocked(&events, today), u);
    }

    #[test]
    fn mistakes_and_steady_days_count_too() {
        let today = d(2026, 10, 26);
        let mut events: Vec<ActivityEvent> =
            (0..30).map(|n| study(&format!("s{n}"), today.add_days(-60 + 2 * n), 6)).collect();
        let practice =
            ActivityKind::Practice { skill: Skill::Writing, topic: "-kan".into(), correct: 0, total: 50 };
        events.push(event("pr", today.add_days(-59), 0, practice));
        let u = ids(&unlocked(&events, today));
        // Every other day: no 7-day Pelita, but 30 days studied in all.
        assert!(!u.iter().any(|(id, _)| *id == "sedikit-demi-sedikit"));
        assert!(u.contains(&("alah-bisa-karena-biasa", today.add_days(-2))));
        assert!(u.contains(&("tak-ada-gading-yang-tak-retak", today.add_days(-59))));
        // 300 reviews were done over the 30 days; 100 reached on the 10th.
        assert!(u.contains(&("rajin-pangkal-pandai", today.add_days(-60 + 18))));
    }

    #[test]
    fn a_long_break_after_a_frozen_day_restarts_but_keeps_achievements() {
        let today = d(2026, 10, 26);
        // 7 lit days earn a freeze; a miss is frozen; then the run continues.
        let mut ago: Vec<i32> = (12..=18).collect();
        ago.extend(9..=10);
        let events = days_ago(today, &ago);
        let u = ids(&unlocked(&events, today));
        assert!(u.contains(&("sedikit-demi-sedikit", today.add_days(-12))));
        assert_eq!(pelita(&events, today).streak, 0);
    }
}
