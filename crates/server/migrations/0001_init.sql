-- Kabar Cloud D1 schema. Per-account data (devices, sync log) lives in the
-- account's Durable Object; D1 holds what is shared: the AI cache and the
-- free-tier counters.

CREATE TABLE IF NOT EXISTS ai_cache (
    key TEXT PRIMARY KEY,          -- sha256 of (task, prompt version, engine, model, input)
    task TEXT NOT NULL,
    prompt_version INTEGER NOT NULL,
    engine TEXT NOT NULL,
    model TEXT NOT NULL,
    output TEXT NOT NULL,
    created_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS quota_usage (
    service TEXT NOT NULL,
    period TEXT NOT NULL,          -- "2026-10-26" (daily) or "2026-10" (monthly), UTC
    used INTEGER NOT NULL,
    PRIMARY KEY (service, period)
);
