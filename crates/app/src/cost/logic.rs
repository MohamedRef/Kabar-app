//! The cost dashboard's arithmetic and wording, kept free of UI so it can be
//! tested: percentages, warning levels, "$0.00", reset times in local time,
//! and the warnings Kak Nur's chrome shows before a free allowance runs out.

use kabar_core::locale::{UiLanguage, eastern_arabic_digits};
use kabar_protocol::ai::EngineId;
use kabar_protocol::cloud::{ServiceUsage, UsagePeriod, UsageService, UsageUnit};
use kabar_protocol::coach::{LocalDate, LocalTime};

use crate::i18n::{Msg, t};

/// Warn from this share of a free allowance on.
pub const WARN_PERCENT: u8 = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Plenty left.
    Calm,
    /// At least [`WARN_PERCENT`] used.
    Near,
    /// The hard stop is reached: paused until the reset.
    Paused,
}

impl Level {
    pub fn class(self) -> &'static str {
        match self {
            Self::Calm => "calm",
            Self::Near => "near",
            Self::Paused => "paused",
        }
    }
}

/// Share of `limit` used, 0..=100 (rounded down, so 100 means really full).
pub fn percent(used: u64, limit: u64) -> u8 {
    if limit == 0 {
        return if used == 0 { 0 } else { 100 };
    }
    (u128::from(used.min(limit)) * 100 / u128::from(limit)) as u8
}

pub fn level(used: u64, limit: u64) -> Level {
    if limit == 0 || used >= limit {
        Level::Paused
    } else if percent(used, limit) >= WARN_PERCENT {
        Level::Near
    } else {
        Level::Calm
    }
}

/// Whether a row deserves a warning sentence. A service that is switched off
/// or not set up can't be used, so its counter needs no warning.
pub fn warning_level(u: &ServiceUsage) -> Option<Level> {
    if !u.enabled || !u.configured {
        return None;
    }
    match level(u.used, u.limit) {
        Level::Calm => None,
        l => Some(l),
    }
}

/// "$0.00": US dollars from cents.
pub fn dollars(cents: u64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// A number with thousands separators, in the UI's digits.
pub fn grouped(n: u64, lang: UiLanguage, eastern: bool) -> String {
    let digits = n.to_string();
    let rtl_eastern = eastern && lang != UiLanguage::En;
    let sep = if rtl_eastern { '\u{066C}' } else { ',' };
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    if rtl_eastern { eastern_arabic_digits(&out) } else { out }
}

/// When a counter resets, seen from the learner's local day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reset {
    Today(LocalTime),
    Tomorrow(LocalTime),
    On(LocalDate),
}

pub fn reset(resets_at_ms: u64, now_ms: u64, utc_offset_min: i16) -> Reset {
    let today = LocalDate::from_unix_ms(now_ms, utc_offset_min);
    let day = LocalDate::from_unix_ms(resets_at_ms, utc_offset_min);
    let time = LocalTime::from_unix_ms(resets_at_ms, utc_offset_min);
    match day.days_since(today) {
        i32::MIN..=0 => Reset::Today(time),
        1 => Reset::Tomorrow(time),
        _ => Reset::On(day),
    }
}

/// Latin and digits inside Arabic text stay in one piece.
fn isolate(s: &str) -> String {
    format!("\u{2068}{s}\u{2069}")
}

fn digits(s: String, lang: UiLanguage, eastern: bool) -> String {
    if eastern && lang != UiLanguage::En { eastern_arabic_digits(&s) } else { s }
}

/// "at 02:00", "tomorrow at 02:00" or "on 2026-11-01".
pub fn when(r: Reset, lang: UiLanguage, eastern: bool) -> String {
    match r {
        Reset::Today(time) => {
            t(lang, Msg::CostAtTime).replace("{time}", &isolate(&digits(time.to_string(), lang, eastern)))
        }
        Reset::Tomorrow(time) => {
            t(lang, Msg::CostTomorrowAt).replace("{time}", &isolate(&digits(time.to_string(), lang, eastern)))
        }
        Reset::On(date) => {
            t(lang, Msg::CostOnDate).replace("{date}", &isolate(&digits(date.to_string(), lang, eastern)))
        }
    }
}

/// The service's display name (a brand, so the same in every language).
pub fn service_name(s: UsageService) -> Option<&'static str> {
    match s {
        UsageService::GeminiFree => Some("Gemini"),
        UsageService::WorkersAi => Some("Workers AI"),
        UsageService::AzureTts => Some("Azure TTS"),
        UsageService::Claude => Some("Claude"),
        UsageService::Other => None,
    }
}

pub fn unit_msg(u: UsageUnit) -> Option<Msg> {
    match u {
        UsageUnit::Requests => Some(Msg::CostUnitRequests),
        UsageUnit::Characters => Some(Msg::CostUnitCharacters),
        UsageUnit::Neurons => Some(Msg::CostUnitNeurons),
        UsageUnit::RowsRead => Some(Msg::CostUnitRowsRead),
        UsageUnit::RowsWritten => Some(Msg::CostUnitRowsWritten),
        UsageUnit::Other => None,
    }
}

pub fn period_msg(p: UsagePeriod) -> Option<Msg> {
    match p {
        UsagePeriod::Day => Some(Msg::CostPerDay),
        UsagePeriod::Month => Some(Msg::CostPerMonth),
        UsagePeriod::Other => None,
    }
}

/// "Gemini's free allowance for today is almost used. At the limit Kabar
/// pauses it rather than pay; it's back at 02:00." `None` when all is calm.
pub fn warning(
    u: &ServiceUsage,
    now_ms: u64,
    offset: i16,
    lang: UiLanguage,
    eastern: bool,
) -> Option<String> {
    let name = service_name(u.service)?;
    let monthly = u.period == UsagePeriod::Month;
    let msg = match (warning_level(u)?, monthly) {
        (Level::Near, false) => Msg::CostNearDay,
        (Level::Near, true) => Msg::CostNearMonth,
        (_, false) => Msg::CostPausedDay,
        (_, true) => Msg::CostPausedMonth,
    };
    let back = when(reset(u.resets_at_ms, now_ms, offset), lang, eastern);
    Some(t(lang, msg).replace("{service}", &isolate(name)).replace("{when}", &back))
}

/// Turning this engine on could cost money, so the UI asks first (default No).
pub fn needs_paid_confirm(engine: EngineId) -> bool {
    matches!(engine, EngineId::Claude)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-26T10:50:56Z
    const NOW: u64 = 1_793_011_856_000;
    /// 2026-10-27T00:00Z
    const MIDNIGHT: u64 = 1_793_059_200_000;
    const CAIRO: i16 = 120;
    const JAKARTA: i16 = 420;

    fn usage(service: UsageService, used: u64, limit: u64, period: UsagePeriod) -> ServiceUsage {
        ServiceUsage {
            service,
            unit: UsageUnit::Requests,
            used,
            limit,
            period,
            resets_at_ms: MIDNIGHT,
            enabled: true,
            configured: true,
        }
    }

    #[test]
    fn percentages_and_levels() {
        assert_eq!(percent(0, 1000), 0);
        assert_eq!(percent(799, 1000), 79);
        assert_eq!(percent(800, 1000), 80);
        assert_eq!(percent(1500, 1000), 100);
        assert_eq!(percent(u64::MAX, u64::MAX), 100);
        assert_eq!(percent(0, 0), 0);
        assert_eq!(level(799, 1000), Level::Calm);
        assert_eq!(level(800, 1000), Level::Near);
        assert_eq!(level(999, 1000), Level::Near);
        assert_eq!(level(1000, 1000), Level::Paused);
        assert_eq!(level(0, 0), Level::Paused);
    }

    #[test]
    fn money_is_dollars_and_cents() {
        assert_eq!(dollars(0), "$0.00");
        assert_eq!(dollars(5), "$0.05");
        assert_eq!(dollars(12_345), "$123.45");
    }

    #[test]
    fn numbers_group_in_the_ui_digits() {
        assert_eq!(grouped(450_000, UiLanguage::En, true), "450,000");
        assert_eq!(grouped(999, UiLanguage::ArEg, false), "999");
        assert_eq!(grouped(1_000, UiLanguage::ArEg, false), "1,000");
        assert_eq!(grouped(5_000_000, UiLanguage::Ar, true), "٥٬٠٠٠٬٠٠٠");
    }

    #[test]
    fn resets_are_in_local_time() {
        // UTC midnight is 02:00 tomorrow in Cairo and 07:00 tomorrow in Jakarta.
        assert_eq!(reset(MIDNIGHT, NOW, CAIRO), Reset::Tomorrow(LocalTime::hm(2, 0)));
        assert_eq!(reset(MIDNIGHT, NOW, JAKARTA), Reset::Tomorrow(LocalTime::hm(7, 0)));
        // Late evening in Jakarta (UTC 20:00 = 03:00 local next day): the reset is today.
        let late = MIDNIGHT - 4 * 3_600_000;
        assert_eq!(reset(MIDNIGHT, late, JAKARTA), Reset::Today(LocalTime::hm(7, 0)));
        let month = 1_793_491_200_000; // 2026-11-01T00:00Z
        assert_eq!(reset(month, NOW, CAIRO), Reset::On(LocalDate::from_ymd(2026, 11, 1).unwrap()));
        assert_eq!(
            when(Reset::Today(LocalTime::hm(7, 0)), UiLanguage::En, false),
            "at \u{2068}07:00\u{2069}"
        );
        assert_eq!(
            when(Reset::Tomorrow(LocalTime::hm(2, 0)), UiLanguage::ArEg, true),
            "بكرة الساعة \u{2068}٠٢:٠٠\u{2069}"
        );
    }

    #[test]
    fn warnings_start_at_eighty_percent_and_never_offer_to_pay() {
        let calm = usage(UsageService::GeminiFree, 500, 1000, UsagePeriod::Day);
        assert_eq!(warning(&calm, NOW, CAIRO, UiLanguage::En, false), None);

        let near = usage(UsageService::GeminiFree, 812, 1000, UsagePeriod::Day);
        assert_eq!(
            warning(&near, NOW, CAIRO, UiLanguage::En, false).unwrap(),
            "\u{2068}Gemini\u{2069}'s free allowance for today is almost used. At the limit Kabar pauses \
             it rather than pay; it's back tomorrow at \u{2068}02:00\u{2069}."
        );
        let ar = warning(&near, NOW, CAIRO, UiLanguage::ArEg, false).unwrap();
        assert!(ar.contains("\u{2068}Gemini\u{2069}") && ar.contains("02:00"), "{ar}");

        let mut full = usage(UsageService::AzureTts, 450_000, 450_000, UsagePeriod::Month);
        full.resets_at_ms = 1_793_491_200_000;
        let msg = warning(&full, NOW, CAIRO, UiLanguage::En, false).unwrap();
        assert!(msg.contains("paused") && msg.contains("2026-11-01"), "{msg}");

        // Off or not set up: nothing can be spent, so no warning.
        let off =
            ServiceUsage { enabled: false, ..usage(UsageService::Claude, 300, 300, UsagePeriod::Month) };
        assert_eq!(warning(&off, NOW, CAIRO, UiLanguage::En, false), None);
        let unknown = usage(UsageService::Other, 10, 10, UsagePeriod::Day);
        assert_eq!(warning(&unknown, NOW, CAIRO, UiLanguage::En, false), None);
    }

    #[test]
    fn only_claude_needs_a_paid_confirm() {
        assert!(needs_paid_confirm(EngineId::Claude));
        for e in [EngineId::Deterministic, EngineId::WorkersAi, EngineId::Local, EngineId::GeminiFree] {
            assert!(!needs_paid_confirm(e));
        }
    }
}
