# Kabar — working conventions

Kabar is a personal Indonesian + Javanese learning app in Rust for one learner (Egyptian Arabic native, reads English). Private, never published. The full brief is `docs/SPEC.md` (v4 wins over anything older); the agreed design is `docs/PLAN.md`.

## Non-negotiables
- **$0/month to run.** Ask before adding anything that could cost money or changing a stack decision.
- **No quality compromise.** Quality-critical content is generated at build time (`crates/tools`, `content/`) and reviewed by the user; runtime AI goes to the best *free* engine that passes the task's golden set. Never silently degrade; report gaps.
- **No secrets in the repo. Ever.** Keys live in Worker secrets, Android Keystore, macOS Keychain, or a git-ignored `.env`.
- **Never drop a feature from the spec.** If it's hard, phase it.
- Nothing personal is ever sent to free-tier AI engines (Gemini free tier trains on inputs). Only learning text.

## Language
- Reply to the user in the language they write in (usually Egyptian Arabic).
- Code, comments, commit messages and docs are in English.

## Workflow
- Go milestone by milestone (`docs/SPEC.md` §23). Don't start app code for a milestone until its plan is approved.
- Before every commit: `just check` (= `scripts/check-tracked-files.sh`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`). All green. CI also clippy-checks the Android arm64 build.
- A PR the learner would notice adds `changes/<topic>.yaml` (one line per change: `kind` new/improved/fixed, `ar_eg`, `en`); the app's "What's new" is built from these (`docs/RELEASE.md` §5). Housekeeping PRs add `[]` or nothing.
- Run/install instructions live in `docs/RUN.md`; keep them current.
- `README.md` shows the current state. Its content numbers are generated: the daily README routine refreshes them, and running `just readme` in a content PR is welcome but never required (CI only warns, so parallel PRs can't turn main red). Update its feature, roadmap and run tables by hand when a PR changes them.
- After each milestone: tell the user exactly how to run it on the Mac and install it on the S24 Ultra and Tab S11, summarize changes, commit with a clear message.
- Tests never call live APIs. AI parsing is tested against recorded JSON fixtures in `fixtures/`.
- Use the test texts in `docs/SPEC.md` §24 in unit tests, golden sets and demos.

## Code
- Rust stable, edition 2024, one Cargo workspace (`crates/core`, `protocol`, `app`, `server`, `home`, `tools`) plus minimal Kotlin in `android/`.
- `crates/core` is pure logic: no UI, no network, no filesystem assumptions. It must build for native and wasm32.
- `crates/protocol` holds every type that crosses a process boundary (serde). Version it; never break old clients silently.
- Every synced table has `account_id`, `device_id`, an HLC timestamp and a tombstone flag. Reviews/attempts/XP are append-only events; statuses/settings are LWW by HLC; ink strokes are immutable.
- UI: Dioxus + HTML/CSS. Logical CSS properties only (RTL), isolate Latin snippets with `<bdi>`. Layout by window width (compact <600dp, medium <840dp, expanded ≥840dp), never by device name.
- AI calls go through the router in `crates/core` with versioned prompts in `prompts/`; results are cached by (task, input hash, engine, prompt version) and validated with serde (retry once, then a friendly message).
- Javanese content and AI answers stay flagged **unverified** until the user approves them.
- Every dataset, corpus, font and sound goes in `ATTRIBUTION.md` with its license.

## Tone (user-facing text)
Warm, specific, encouraging. No guilt, fear, fake urgency or dark patterns.
