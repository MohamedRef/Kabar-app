# Kabar — Plan (Milestone 0)

Status: **draft, waiting for approval.** Source of truth for features is `docs/SPEC.md`; verification details are in `docs/RESEARCH.md`. Nothing here drops a feature from the spec; hard things are phased.

## 1. Decisions that need your OK

| # | Decision | Recommendation |
|---|---|---|
| D1 | Add Cloudflare **Workers AI** as a fourth engine (`workers_ai`), mainly SEA-LION v4 27B | Yes. Free 10k Neurons/day, SEA-tuned, server-side, no extra key. |
| D2 | Local model shortlist | Benchmark Apertus-SEA-LION-v4-8B, Sahabat-AI v1 (Gemma-2 9B), and Gemma-SEA-LION-v4-27B only if the Mac has ≥24 GB RAM. |
| D3 | Azure Speech via REST, not the `azure-speech` crate | Yes (crate last released Jul 2025; REST is small). |
| D4 | Server storage split | Per-account data lives in the account's **Durable Object SQLite**; **D1** holds the shared AI/TTS cache, content-pack index, device registry and allowlist. (Spec says D1 for storage; DO SQLite is free and co-located with the sync hub, so it removes a hop and a consistency problem.) |
| D5 | Javanese morphology | Own affix decomposer + Wiktionary lemmas; sastrawi-rs Javanese stemmer as a hint only (it missed diculke, tukokke, ditukoni). |

Need from you: **MacBook Air RAM** (8 / 16 / 24 GB) and whether you're on **Claude Max** (5x or 20x) for the `claude` engine credit.

## 2. Architecture

```
 ┌──────────── devices ─────────────┐        ┌──────────── Kabar Cloud (Cloudflare Free) ────────────┐
 │ Dioxus app (Mac / S24U / Tab S11) │  HTTPS │ Worker (workers-rs): auth, routing, AI/TTS proxy,     │
 │  ├ crates/core (logic, router)    │◀──────▶│   key vault (secrets), 1 cron dispatcher              │
 │  ├ SQLite (rusqlite, FTS5)        │   WS   │ Durable Object per account: WebSocket hub (hibernate),│
 │  ├ ink module (TS in WebView)     │◀──────▶│   event log + LWW state in DO SQLite, push fan-out    │
 │  └ android/ Kotlin glue (JNI)     │        │ D1: ai_cache, audio index, content_packs, devices     │
 └───────────────┬───────────────────┘        │ Cache API: hot audio (see §8)                         │
                 │ LAN / Tailscale            │ Workers AI: SEA-LION v4 27B, vision, Whisper          │
        ┌────────▼────────┐                   └──────┬────────────┬────────────┬─────────────────────┘
        │ Kabar Home (Mac)│                          │            │            │
        │ Ollama, NLLB,   │                     Gemini free   Azure TTS     FCM / Anthropic (opt-in)
        │ whisper.cpp     │
        └─────────────────┘
```

Principles: local-first (the app is complete offline with content packs), the server is thin (10 ms CPU per request), and every AI result is cached and shared.

## 3. Workspace layout

```
kabar/
├─ Cargo.toml                 # workspace, edition 2024, shared lints
├─ justfile                   # setup-mac, android-build, install-s24, install-tab, deploy-cloud, bakeoff …
├─ crates/
│  ├─ core/        # tokenizer, normalizer, morphology, lang-id, dictionary, SRS (fsrs), curriculum,
│  │               # coach logic, AI router + engine trait, TTS client, cache, sync engine (HLC, merge)
│  ├─ protocol/    # serde types shared by app/server/home; builds for native + wasm32
│  ├─ app/         # Dioxus UI (desktop + android + web); assets/ink/ holds the TS ink module
│  ├─ server/      # Kabar Cloud: Worker + Durable Object (workers-rs), wrangler.toml
│  ├─ home/        # Kabar Home: small HTTP service fronting Ollama / NLLB / whisper.cpp
│  └─ tools/       # content pipeline, dictionary import, audio pre-gen, bake-off runner, backups
├─ android/        # Kotlin: Credential Manager, FCM, share target, ML Kit (OCR + ink), widgets, pen extras
├─ prompts/        # versioned prompt templates (task@vN.md)
├─ content/        # build-time content (JSON/YAML, schema-validated), packed into content packs
├─ curriculum/     # indonesian.yaml, javanese.yaml
├─ schemas/        # JSON Schemas for content and AI outputs
├─ fixtures/       # recorded AI responses for tests
└─ docs/           # SPEC, PLAN, RESEARCH, BAKEOFF, run/install guides
```

`core` has no I/O traits baked in: storage, HTTP and clock are injected, so the same logic runs in the app, the Worker (wasm32) and tests.

## 4. Client stack

- **Dioxus 0.7.x** for macOS (aarch64-apple-darwin) and Android (arm64-v8a); web build as a bonus.
- **SQLite** via `rusqlite` (bundled, FTS5) with embedded migrations (`rusqlite_migration` or hand-rolled numbered SQL).
- **Ink:** a small TypeScript module inside the WebView: Pointer Events (`pointerType="pen"`), `getCoalescedEvents()`, `getPredictedEvents()`, `desynchronized: true` 2D canvas. Points batched to Rust per stroke (points + pressure + tilt + t).
- **Native bridges (Android):** JNI from Rust (`jni` crate) using `ndk_context` for the JavaVM and the activity's class loader to reach our Kotlin classes. Dioxus has no official Kotlin interop yet, so M1 proves this with one call (e.g., read the S Pen "in range" state) before anything depends on it. Fallback: a custom Gradle template via `dx` bundling.
- **macOS:** Keychain via `keyring`; global hotkey via Dioxus desktop's global-shortcut support; Apple Vision OCR through a tiny Swift/objc2 shim.
- **Layout:** width classes per Android guidance: compact <600dp, medium 600–840, expanded ≥840 (large ≥1200 gets the three-pane Study Desk with wider panes).
- **i18n:** Fluent (`fluent-bundle`) with `ar-EG`, `ar` (MSA) and `en`; RTL via `dir` + logical CSS properties.

## 5. Data model

Every synced row: `id` (UUIDv7), `account_id`, `device_id`, `hlc` (64-bit: 48-bit ms + 16-bit counter, plus device id as tiebreak), `deleted` (tombstone). Local-only tables omit them.

| Group | Tables (local SQLite) | Sync kind |
|---|---|---|
| Language | lexemes, senses, glosses, examples, forms, etymology_arabic, abbreviations, speech_levels, grammar_points | **content pack** (read-only, versioned) |
| Reading | texts, text_tokens, word_status, highlights | texts/highlights: LWW · word_status: LWW per (lexeme) |
| Learning | cards, review_log, curriculum_nodes, lessons, exercises, attempts, conversations, stories, souvenirs | review_log/attempts/xp_events: **append-only** · cards: derived from review_log · lessons/exercises: content pack |
| Ink | notebooks, ink_documents, ink_strokes | strokes **immutable** (delete = event) · documents: LWW |
| Coach | goals, daily_plans, reminders, reports, journal_entries, coach_memory, skill_scores | LWW (plans keyed by date) · journal: append-only |
| Account | accounts, devices, sessions (Lanjut), push_tokens, sync_events | server-owned; sessions LWW |
| Quality | engine_runs, quality_feedback, golden_items | append-only |
| System | content_packs, ai_cache, audio_cache, quota_usage, settings, cost_log, outbox | settings LWW · ai_cache shared via D1 · outbox local |

FSRS card state is **derived**: replaying `review_log` (ordered by HLC) through `fsrs` gives the same state on every device, so concurrent reviews never conflict. A cached `cards` row is a materialized view.

## 6. Auth

- **Android:** Credential Manager → Google ID token (Kotlin) → `POST /auth/google`.
- **Mac:** system browser OAuth 2.0 + PKCE with a loopback redirect → ID token → same endpoint.
- **Server:** verifies the ID token (Google JWKS, cached), checks the email against the allowlist (a Worker secret), issues a 15-min access token (Ed25519-signed JWT, verify is cheap) and a 60-day rotating refresh token (hash stored in D1 `devices`).
- **Storage:** Android Keystore (EncryptedSharedPreferences/Keystore via Kotlin), macOS Keychain.
- **Devices screen:** lists `devices` (name, model, last sync); "sign out" revokes the refresh token and pushes a `revoked` message over the WebSocket.

## 7. Sync design

1. Every local write produces an **op** `{table, row_id, hlc, kind: upsert|append|delete, payload}` in the local `outbox` in the same transaction.
2. The app holds one WebSocket to the account's Durable Object (hibernation API). On connect it sends its last-seen server sequence; the DO streams missing ops.
3. Sending: the app batches outbox ops (≤ 50 per message, every ≤ 500 ms while active). The DO validates, applies merge rules, assigns a monotonically increasing `seq`, stores to its SQLite `ops` log + current-state tables, acks, and broadcasts to the other sockets.
4. Merge rules (in `core`, shared with the DO): append-only → insert if new id; LWW → keep the higher `(hlc, device_id)`; immutable → insert once, deletion is a tombstone op.
5. Offline: ops queue in the outbox and replay on reconnect; idempotent by op id.
6. Lanjut: `sessions` row `{device, screen, lesson_id, exercise_index, updated_hlc}` written on every exercise change; other devices show the prompt if it's newer than theirs and < 24 h old.
7. Blobs (audio, screenshots): content-addressed by SHA-256, fetched on demand, cached per device.
8. Compaction: the DO snapshots state monthly and trims the op log older than the oldest device's ack (devices unseen for 90 days resync from snapshot).

Target: an edit appears on other online devices in < 5 s (normally < 1 s).

## 8. Free-tier budget (single user, generous estimates)

| Service | Free limit | Kabar's expected daily use | Headroom |
|---|---|---|---|
| Workers requests | 100k/day | ~2k (API calls + AI proxy) | 50× |
| Worker CPU | 10 ms/request | auth + routing ≈ 1–3 ms; JWT verify Ed25519 | ok; nothing heavy in Worker |
| Cron Triggers | 5/account | 1 (every 15 min dispatcher) | 4 spare |
| DO requests | 100k/day | ~5k WS msgs ÷ 20 + connects ≈ 500 | 200× |
| DO duration | 13,000 GB-s/day | hibernating sockets ≈ near 0 | large |
| DO rows written | 100k/day | ~3k (reviews, strokes, statuses) | 30× |
| DO storage | 5 GB | < 200 MB/year incl. ink | years |
| D1 | 5 GB, 500 MB/db | ai_cache + audio index < 100 MB | ok |
| Workers AI | 10k Neurons/day | ≈ 100–200k tokens/day available | covers routine AI |
| Gemini free | ≈1,500 RPD, ≈10 RPM (unofficial) | < 200 req/day; queue + backoff | ok |
| Azure TTS F0 | 0.5M chars/month | pre-gen current + next unit ≈ 150k chars/month; Radio ≈ 100k | ~2× |
| Azure STT F0 | 5 h/month | later; Whisper on Home first | ok |
| FCM | no-cost | < 20/day | — |
| Anthropic | $0 unless on Max (credit $100/$200/mo) | off by default; hard stop | — |

Audio blobs: stored in **D1 as chunks is a bad fit**; plan is the Workers **Cache API** for hot audio + devices as the durable cache, and the content pipeline commits pre-generated core audio into content packs hosted on **GitHub Releases** (free). R2 has a free tier (10 GB) but needs a card on file; ask before using it.

Hard stop: `quota_usage` counters per service in D1; the router refuses a call that would exceed a free limit and the cost dashboard shows $0.

## 9. AI router

```rust
trait Engine { fn id(&self) -> EngineId; async fn run(&self, task: &TaskRequest) -> Result<TaskOutput>; fn is_free(&self) -> bool; }
```

Engines: `deterministic` (always first), `workers_ai`, `local` (Kabar Home), `gemini_free`, `claude` (off by default). The route table is data (`prompts/routes.toml`), written from bake-off results: per task, an ordered list of engines that **passed** the golden set. 👎 → next engine up, cache replaced, case added to golden set. Privacy rule enforced in code: tasks tagged `personal` (coach, daily plan, recap, reports, journal-aware tasks) can never route to `gemini_free`.

## 10. Engine bake-off (M0)

See `docs/BAKEOFF.md`. In the cloud sandbox I can't reach the engines (no keys, no Ollama, kaikki/AI endpoints blocked), so M0 delivers the golden **seed set**, rubrics and the runner design; you run it on the Mac and I judge the outputs with you.

## 11. Milestone notes (phasing, nothing dropped)

M1 starts with the riskiest spikes, each a day or less: (a) Dioxus Android build on S24U + Tab S11, (b) JNI call into Kotlin, (c) Pen Lab in the WebView, (d) Arabic shaping + RTL in the WebView and on Mac. If (b) fails, Kotlin features (sign-in, FCM, ML Kit, share target) move to a thin Kotlin activity hosting the same WebView; that's a stack change and I'll ask first.

## 12. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| No official Kotlin interop in Dioxus | sign-in, push, ML Kit, share target | spike in M1; JNI + activity class loader; fallback Gradle template |
| Android WebView pen events (hover, tilt, eraser, barrel button) incomplete | pen features | Pen Lab measures; native `MotionEvent` bridge for what's missing |
| Ink latency in WebView | "feels attached to the pen" | desynchronized canvas, predicted events; measure in Pen Lab; native front-buffer layer as last resort |
| Worker 10 ms CPU | timeouts | thin Worker, heavy work in DO / devices; Ed25519 JWT |
| Gemini free tier trains on inputs and limits shift | privacy, availability | learning text only; `personal` tasks blocked by code; Workers AI + local as alternatives |
| Javanese quality from free models | wrong teaching | build-time content, dictionary cross-check, "unverified" badge, your review |
| Javanese stemming weak | wrong roots in popovers | own decomposer + lemma list; tests from §24 |
| Azure TTS quota | missing new audio | just-in-time pre-gen, cache forever, system TTS fallback |
| sastrawi-rs tiny user base | abandonment | MIT, small; vendor/fork if it stalls |
| Region of Worker egress for Gemini | blocked calls from unsupported colo | retry via Kabar Home or device; monitor errors |
| Content pipeline volume vs Claude plan limits | slow M4/M13 | resumable, batched, idempotent scripts |

## 13. What runs where (for you on the Mac)

Tracked in `docs/MAC-TODO.md`.
