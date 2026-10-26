//! Hard stops for free tiers. Counters live in D1 (`quota_usage`); a call
//! that would cross a limit is refused before it is made, so the bill stays $0.

use crate::config::Limits;
use crate::time::{day_key, month_key, next_day_ms, next_month_ms};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    WorkersAi,
    Gemini,
    Claude,
    /// Azure neural TTS characters.
    Tts,
}

impl Service {
    pub fn name(self) -> &'static str {
        match self {
            Service::WorkersAi => "workers_ai",
            Service::Gemini => "gemini_free",
            Service::Claude => "claude",
            Service::Tts => "azure_tts",
        }
    }

    /// The counter's period, e.g. "2026-10-26" or "2026-10".
    pub fn period(self, now_ms: u64) -> String {
        match self {
            Service::WorkersAi | Service::Gemini => day_key(now_ms),
            Service::Claude | Service::Tts => month_key(now_ms),
        }
    }

    /// Seconds until the counter resets.
    pub fn retry_after_s(self, now_ms: u64) -> u64 {
        let reset = match self {
            Service::WorkersAi | Service::Gemini => next_day_ms(now_ms),
            Service::Claude | Service::Tts => next_month_ms(now_ms),
        };
        reset.saturating_sub(now_ms).div_ceil(1000)
    }

    pub fn limit(self, limits: &Limits) -> u64 {
        match self {
            Service::WorkersAi => limits.workers_ai_daily_requests,
            Service::Gemini => limits.gemini_daily_requests,
            Service::Claude => limits.claude_monthly_requests,
            Service::Tts => limits.tts_monthly_chars,
        }
    }
}

/// Whether spending `cost` more units stays within the limit.
pub fn allows(used: u64, cost: u64, limit: u64) -> bool {
    used.saturating_add(cost) <= limit
}

pub const SELECT_USED: &str = "SELECT used FROM quota_usage WHERE service = ?1 AND period = ?2";
pub const ADD_USED: &str = "INSERT INTO quota_usage (service, period, used) VALUES (?1, ?2, ?3)
    ON CONFLICT (service, period) DO UPDATE SET used = used + excluded.used";

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_793_011_856_000;

    #[test]
    fn periods_and_limits() {
        assert_eq!(Service::Gemini.period(NOW), "2026-10-26");
        assert_eq!(Service::Tts.period(NOW), "2026-10");
        let limits = Limits::default();
        assert_eq!(Service::Tts.limit(&limits), 450_000);
        assert!(allows(449_990, 10, 450_000));
        assert!(!allows(449_990, 11, 450_000));
        assert!(!allows(u64::MAX, 1, u64::MAX - 1));
        // 10:50:56 UTC → 13h 9m 4s to midnight.
        assert_eq!(Service::Gemini.retry_after_s(NOW), 13 * 3600 + 9 * 60 + 4);
    }

    #[test]
    fn d1_statements_are_valid_sqlite() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../migrations/0001_init.sql")).unwrap();
        db.execute(ADD_USED, rusqlite::params!["azure_tts", "2026-10", 120]).unwrap();
        db.execute(ADD_USED, rusqlite::params!["azure_tts", "2026-10", 30]).unwrap();
        let used: i64 =
            db.query_row(SELECT_USED, rusqlite::params!["azure_tts", "2026-10"], |r| r.get(0)).unwrap();
        assert_eq!(used, 150);
    }
}
