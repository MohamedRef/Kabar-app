//! `GET /v1/usage`: the cost dashboard's numbers. Built from the hard-stop
//! counters in D1 (`quota_usage`) plus Cloudflare's Free-plan limits, which
//! are listed for information only: the Free plan has no card, so Cloudflare
//! stops serving at a limit instead of billing. Nothing here calls
//! Cloudflare's API.

use kabar_protocol::cloud::{PlatformLimit, ServiceUsage, UsagePeriod, UsageReport, UsageService, UsageUnit};

use crate::config::Config;
use crate::quota::Service;
use crate::time::{next_day_ms, next_month_ms};

/// Every metered service, in the order the dashboard shows them.
pub const SERVICES: [Service; 4] = [Service::Gemini, Service::WorkersAi, Service::Tts, Service::Claude];

/// Cloudflare Workers Free plan limits (developers.cloudflare.com, 2026).
pub fn platform_limits() -> Vec<PlatformLimit> {
    let limit = |name: &str, unit, limit| PlatformLimit {
        name: name.to_owned(),
        unit,
        limit,
        period: UsagePeriod::Day,
    };
    vec![
        limit("Workers", UsageUnit::Requests, 100_000),
        limit("Durable Objects", UsageUnit::Requests, 100_000),
        limit("D1", UsageUnit::RowsRead, 5_000_000),
        limit("D1", UsageUnit::RowsWritten, 100_000),
        limit("Workers AI", UsageUnit::Neurons, 10_000),
    ]
}

fn describe(service: Service, cfg: &Config) -> (UsageService, UsageUnit, UsagePeriod, bool, bool) {
    match service {
        Service::WorkersAi => {
            (UsageService::WorkersAi, UsageUnit::Requests, UsagePeriod::Day, true, cfg.workers_ai_bound)
        }
        Service::Gemini => {
            (UsageService::GeminiFree, UsageUnit::Requests, UsagePeriod::Day, true, cfg.gemini_key.is_some())
        }
        Service::Claude => (
            UsageService::Claude,
            UsageUnit::Requests,
            UsagePeriod::Month,
            cfg.claude_enabled,
            cfg.claude_key.is_some(),
        ),
        Service::Tts => {
            (UsageService::AzureTts, UsageUnit::Characters, UsagePeriod::Month, true, cfg.tts_ready())
        }
    }
}

/// The report for `now_ms`; `used` reads a service's counter for its
/// current period (D1 on the Worker, a map in tests).
pub fn report(cfg: &Config, now_ms: u64, used: impl Fn(Service) -> u64) -> UsageReport {
    let services = SERVICES
        .iter()
        .map(|&s| {
            let (service, unit, period, enabled, configured) = describe(s, cfg);
            let resets_at_ms = match period {
                UsagePeriod::Month => next_month_ms(now_ms),
                _ => next_day_ms(now_ms),
            };
            ServiceUsage {
                service,
                unit,
                used: used(s),
                limit: s.limit(&cfg.limits),
                period,
                resets_at_ms,
                enabled,
                configured,
            }
        })
        .collect();
    UsageReport {
        services,
        platform: platform_limits(),
        // Every paid path is behind a hard stop that refuses before calling,
        // and Claude is off by default: nothing is ever billed.
        spend_usd_cents: 0,
        hard_stops: true,
        generated_at_ms: now_ms,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::quota::{ADD_USED, SELECT_USED};

    const NOW: u64 = 1_793_011_856_000; // 2026-10-26T10:50:56Z

    #[test]
    fn report_reads_each_service_for_its_own_period() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../migrations/0001_init.sql")).unwrap();
        db.execute(ADD_USED, rusqlite::params!["gemini_free", "2026-10-26", 812]).unwrap();
        db.execute(ADD_USED, rusqlite::params!["gemini_free", "2026-10-25", 999]).unwrap();
        db.execute(ADD_USED, rusqlite::params!["azure_tts", "2026-10", 1200]).unwrap();
        let read = |s: Service| -> u64 {
            db.query_row(SELECT_USED, rusqlite::params![s.name(), s.period(NOW)], |r| r.get::<_, i64>(0))
                .map(|v| v as u64)
                .unwrap_or(0)
        };

        let cfg = Config::from_lookup(|k| (k == "GEMINI_API_KEY").then(|| "test-not-a-key".to_owned()), true)
            .unwrap();
        let r = report(&cfg, NOW, read);

        assert_eq!(r.spend_usd_cents, 0);
        assert!(r.hard_stops);
        assert_eq!(r.generated_at_ms, NOW);
        let by: HashMap<_, _> = r.services.iter().map(|s| (s.service, s)).collect();
        let gemini = by[&UsageService::GeminiFree];
        assert_eq!((gemini.used, gemini.limit), (812, 1_000), "yesterday's counter doesn't count");
        assert!(gemini.configured && gemini.enabled);
        assert_eq!(gemini.period, UsagePeriod::Day);
        assert_eq!(gemini.resets_at_ms, 1_793_059_200_000); // 2026-10-27T00:00Z
        let tts = by[&UsageService::AzureTts];
        assert_eq!((tts.used, tts.unit, tts.period), (1200, UsageUnit::Characters, UsagePeriod::Month));
        assert!(!tts.configured);
        assert_eq!(crate::time::day_key(tts.resets_at_ms), "2026-11-01");
        let claude = by[&UsageService::Claude];
        assert!(!claude.enabled && !claude.configured, "Claude stays off by default");
        assert_eq!(by[&UsageService::WorkersAi].used, 0);
        assert!(r.platform.iter().any(|p| p.name == "Workers" && p.limit == 100_000));
    }

    #[test]
    fn report_never_carries_keys() {
        let cfg = Config::from_lookup(
            |k| match k {
                "GEMINI_API_KEY" => Some("AIzaSecretValue".into()),
                "ANTHROPIC_API_KEY" => Some("sk-ant-secret".into()),
                _ => None,
            },
            true,
        )
        .unwrap();
        let json = serde_json::to_string(&report(&cfg, NOW, |_| 0)).unwrap();
        assert!(!json.contains("AIzaSecretValue") && !json.contains("sk-ant-secret"));
    }
}
