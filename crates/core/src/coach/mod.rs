//! Kak Nur, the coach (spec §6): onboarding, the daily plan, the end-of-day
//! recap, the weekly report, account-aware reminders and rewards.
//!
//! Everything here is a pure function of synced data (the profile, the plan,
//! the append-only [`ActivityEvent`] log) plus "now", so every device and
//! Kabar Cloud's reminder cron reach the same answer. The data types live in
//! [`kabar_protocol::coach`]; Kak Nur's words live in `content/coach/` and are
//! rendered by [`voice`].
//!
//! [`ActivityEvent`]: kabar_protocol::coach::ActivityEvent

pub mod history;
pub mod onboarding;
pub mod plan;
pub mod recap;
pub mod reminders;
pub mod report;
pub mod rewards;
pub mod voice;

pub use kabar_protocol::coach::*;
