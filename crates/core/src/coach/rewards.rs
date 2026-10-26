//! Rewards: Pelita (the streak lamp, with freezes that are earned, never
//! bought), Koin for the Warung's cosmetic batik themes, Liga Pribadi (this
//! week against your own past weeks) and achievements named after proverbs.
//! All derived from the activity log, so nothing can double-count across
//! devices.

use kabar_protocol::coach::{ActivityEvent, LeagueResult, LocalDate};

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

pub fn pelita(events: &[ActivityEvent], today: LocalDate) -> Pelita {
    let _ = (events, today);
    todo!("coach agent B")
}

/// Koin earned minus Koin spent.
pub fn koin_balance(events: &[ActivityEvent]) -> u32 {
    let _ = events;
    todo!("coach agent B")
}

/// This week's XP ranked against up to `past_weeks` earlier weeks.
pub fn league(events: &[ActivityEvent], week_start: LocalDate, past_weeks: u32) -> LeagueResult {
    let _ = (events, week_start, past_weeks);
    todo!("coach agent B")
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

pub fn warung() -> &'static [WarungItem] {
    todo!("coach agent B")
}

/// Item ids already bought.
pub fn owned(events: &[ActivityEvent]) -> Vec<String> {
    let _ = events;
    todo!("coach agent B")
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

pub fn achievements() -> &'static [Achievement] {
    todo!("coach agent B")
}

/// Achievements earned up to `today`, oldest first.
pub fn unlocked(events: &[ActivityEvent], today: LocalDate) -> Vec<Unlocked> {
    let _ = (events, today);
    todo!("coach agent B")
}
