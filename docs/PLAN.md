# Kabar — Plan (Milestone 0)

Status: **draft, waiting for approval.** Source of truth for features is `docs/SPEC.md`; verification details are in `docs/RESEARCH.md`. Nothing here drops a feature from the spec; hard things are phased.

## 1. Decisions that need your OK

| # | Decision | Recommendation |
|---|---|---|
| D1 | Add Cloudflare **Workers AI** as a fourth engine (`workers_ai`), mainly SEA-LION v4 27B | Yes. Free 10k Neurons/day, SEA-tuned, server-side, no extra key. |
| D2 | Local model shortlist (Mac has **8 GB**) | Only ~4B models fit next to macOS: Gemma-SEA-LION-v4-4B-VL (text + vision) and Qwen-SEA-LION-v4-4B-VL at Q4. 8–9B models (Apertus-SEA-LION-v4-8B, Sahabat-AI Gemma-2 9B) are tested once but expected to swap. Kabar Home becomes a helper for vision, Whisper (small) and NLLB-600M, not the main text engine. |
| D3 | Azure Speech via REST, not the `azure-speech` crate | Yes (crate last released Jul 2025; REST is small). |
| D4 | Server storage split | Per-account data (including the **device registry**, since M2) lives in the account's **Durable Object SQLite**; **D1** holds the shared AI cache, quota counters and content-pack index; the allowlist is a Worker secret. (Spec says D1 for storage; DO SQLite is free and co-located with the sync hub, so it removes a hop and a consistency problem.) |
| D5 | Javanese morphology — **you chose: most accurate** | Four layers, most trusted first: (1) a **build-time analysis table** for the top ~5,000 Javanese surface forms (root, affixes, level, gloss), generated with Claude Code, cross-checked against Wiktionary, reviewed by you; (2) our own rule-based decomposer (N-, di-, ka-, -ake/-ke, -i, -an, -e/-ne, -en, reduplication, krama/ngoko pairs) validated against Wiktionary lemmas + sastrawi-rs as one signal; (3) AI fallback, cached and badged **unverified**; (4) every new form you meet is queued for the next content batch so layer 1 keeps growing. |

Answers so far (26 Oct): Mac has 8 GB RAM; Claude Max 20x now, but not guaranteed every month; Daytona is available for tests; D5 = most accurate.

Consequences:
- **Claude is never a runtime dependency.** When the Max credit is there, use it for big *build-time* batches through the Message Batches API (half price), e.g. the 10k glosses. Content is committed, so it stays after the credit ends. Runtime `claude` stays an optional switch.
- **Runtime quality rests on build-time content + `workers_ai` + `gemini_free`**, with `local` for vision and speech.
- **Daytona** = optional extra build/test machine (see §14). Not part of the runtime.

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
- **i18n:** a typed message catalog in Rust (`crates/app/src/i18n.rs`) for `ar-EG`, `ar` (MSA) and `en`, so a missing translation fails to compile; switch to Fluent if plural-heavy strings arrive. RTL via `dir` + logical CSS properties; Latin inside Arabic is isolated with `<bdi>` or FSI/PDI.

## 5. Data model

Every synced row: `id` (UUIDv7), `account_id`, `device_id`, `hlc` (64-bit: 48-bit ms + 16-bit counter, plus device id as tiebreak), `deleted` (tombstone). Local-only tables omit them.

| Group | Tables (local SQLite) | Sync kind |
|---|---|---|
| Language | lexemes, senses, glosses, examples, forms, etymology_arabic, abbreviations, speech_levels, grammar_points | **content pack** (read-only, versioned) |
| Personal dictionary | taught_entries (Teach Kabar, §15) | LWW per `{lang}:{word}` |
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
- **Server:** verifies the ID token (Google JWKS, cached), checks the email against the allowlist (a Worker secret), issues a 15-min access token (Ed25519-signed JWT, verify is cheap) and a 60-day rotating refresh token (hash stored in the account's Durable Object, next to the sync hub).
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
| 8 GB Mac limits local models | weak `local` engine; slow builds | small models only; heavy text tasks on `workers_ai`; Daytona for heavy builds/bake-off |
| Max credit not every month | slower content pipeline | batch big generations into months with credit; pipeline is resumable |
| Region of Worker egress for Gemini | blocked calls from unsupported colo | retry via Kabar Home or device; monitor errors |
| Content pipeline volume vs Claude plan limits | slow M4/M13 | resumable, batched, idempotent scripts |

## 13. What runs where (for you on the Mac)

Tracked in `docs/MAC-TODO.md`.

## 14. Daytona (optional)

Daytona sandboxes are billed per use (≈ $0.05/vCPU-h + $0.016/GiB-h; your account's credits cover it). Uses that fit Kabar:
- `cargo test` / `clippy` / Android APK builds on a 4–8 vCPU Linux box, to spare the 8 GB Mac.
- The bake-off runner for the cloud engines, and a one-off run of 8–27B local candidates on a bigger box, only to learn whether they'd be worth it (they can't be hosted there at runtime for $0).

Not needed for: device tests and Pen Lab (need the Galaxy devices), macOS builds (need the Mac). To let me drive it from here, add a `DAYTONA_API_KEY` as an environment secret — never in the repo. I'll keep each run short and tell you the expected cost before starting.

## 15. Teach Kabar (spec §26)

The learner teaches Kabar words it doesn't know. Phase T1 ships with M5.

```
 popover / Teach screen ──▶ kabar_core::learn::prepare ──▶ Router (task learn_entry) ──▶ draft TaughtEntry
   word, lang, sentence        scrub e-mails/phones,          prompt learn_entry@v1,          review screen:
   sources: paste | link |     HTML → text, trim to 6k,       serde + checks, retry once,     approve / edit /
   screenshot (OCR) |          morphology + Bridge hints,     cache by input hash            👎 regenerate / discard
   Wiktionary lookup           personal flag for screenshots                                       │
                                                                                                    ▼
                                         taught_entries (SQLite, synced LWW) ◀── PersonalDictionary ──▶ popover layer
```

- **Types.** `kabar_protocol::learn::TaughtEntry` wraps the content pack's `GlossEntry` (so popovers, lessons and the next content batch read one shape) plus an optional Bridge card, Javanese speech levels, the sources used, the sentence, the engine and whether the learner edited it. `entry.status` is the review state (`unverified` = draft, `approved`), `entry.provenance.generator = "learner"` marks "taught by you", `rank = 0`.
- **Task.** `learn_entry` (`prompts/learn_entry@v1.md`). Not a personal task in itself: what is sent is learning text (the word, the sentence, what the learner pasted from the web). Screenshot sources set `AiRequest::personal`, so they never reach `gemini_free`. E-mail addresses and phone numbers are scrubbed from every source first. Route: `workers_ai`, `gemini_free`, `local`, `claude`, provisional until the bake-off has a `learn_entry` golden set (seeded in `content/golden/seed.yaml`).
- **Validation.** Word matches the request; 1–4 senses with non-empty glosses in all three voices; `arz`/`ar` in Arabic script; 1–3 examples that contain the word (or its lemma/root); `false_friend` needs `arabic_origin`; Bridge `arabic` in Arabic script; root/affixes consistent. The deterministic morphology is sent as a hint and differences are surfaced in the review screen, not silently fixed.
- **Storage and sync.** `taught_entries` local table with the usual `account_id`, `device_id`, HLC and tombstone columns; row id `{lang}:{word}`; added to `kabar_protocol::sync::TABLES` as LWW so the hub accepts it. Discarding a draft or deleting an entry is a tombstone.
- **Popover.** The Reader looks the word up in `PersonalDictionary` first (`lookup_forms`, same form order as the prebuilt glossary). Approved entries show above the prebuilt gloss with a "taught by you" badge; drafts only show a "review your draft" chip. The Reader owns the button in the popover card; it fills `TeachSeed` and opens `Screen::Teach`.
- **Web lookup (free only).** "Search Google" opens `https://www.google.com/search?q=<word>+artinya` (Javanese: `+tegese`) in the browser; Kabar never scrapes Google. Wiktionary's REST definition API (`https://en.wiktionary.org/api/rest_v1/page/definition/<word>`, free, no key, CORS-enabled) is fetched from the WebView and parsed in core. Pasted links are fetched from the WebView too; when a site blocks that (CORS), the screen asks the learner to paste the text (T2 moves this fetch to Kabar Cloud).
- **Later (T2, M13).** Cloud-side link fetch, vision for screenshots without OCR text, a "taught by you" review deck, and exporting approved entries into `content/glosses/` batches.

## 16. Personas, Sinetron, Kabar Radio (M12, app side)

Done when (spec §23): a 5-minute Kabar Radio episode plays offline on the phone. The curriculum thread writes the content: `content/personas/`, `content/stories/sinetron/` and `content/radio/`, with schemas `kabar.persona/v1`, `kabar.story/v1` and `kabar.radio/v1`. This milestone is the runtime that plays that content and talks with it.

```
 content/*.yaml ──(include_str, parsed at start)──▶ kabar_protocol::story types
                                                         │
   Persona chat ──▶ Router(roleplay_turn) ──▶ CloudEngine ──▶ POST /v1/ai (Workers AI first)
        │
        └─ every line, Sinetron, Radio ──▶ AudioCache ──miss──▶ POST /v1/tts (Azure F0) ──▶ <data>/audio/<sha>.mp3
                                              └─ quota / offline ──▶ system TTS (id) or "audio later" (jv)
```

**Phase A: plumbing (no content dependency).**
- **`CloudEngine`** in `crates/app/src/cloud`:
  - Implements `kabar_core::ai::Engine` over `POST /v1/ai`, using the signed-in session.
  - Registered in `ai_router()`, which has no engines today.
  - One engine for each id the proxy serves (`workers_ai`, `gemini_free`, `claude`). The router still chooses by `prompts/routes.toml`, the $0 hard stop and the personal flag.
  - The router cache is backed by SQLite (`ai_cache`, local, not synced).
- **`AudioCache`**:
  - Fetches from `POST /v1/tts` and keeps each MP3 forever under the app data dir (`audio/`), keyed by sha256(voice, pitch, rate, text). The files themselves are the index: "free up space" and download progress read the directory.
  - Lessons and reviews already use it: with Kabar Cloud live (or the line cached), a line plays in Gadis's (id) or Siti's (jv) neural voice.
  - Missing audio never blocks anything. Indonesian falls back to the system voice (the existing `speak` path). Javanese shows "audio comes later" until the quota resets.
- **`TtsSegment` gains an optional `pitch_percent`** (−20..=20, default 0); the server adds `<prosody pitch>`. Azure has only two voices per language, so the Sinetron cast has to share them, and pitch tells the characters apart. The change is additive and versioned, and old clients send nothing.

**Phase B: Sinetron and Kabar Radio (needs content PR #25).**
- **Types:** Rust structs for the three formats live in `kabar_protocol::story`: `Persona`, `Story`, `RadioEpisode` and `Line { speaker, text, ar_eg, ar, en, note, correction }`. A test parses every committed file.
- **Sinetron player:**
  - One episode unlocks per finished unit (`unlock_after`, read from the Journey's completed lessons).
  - It plays scene by scene and line by line, with audio.
  - The translation, in the chosen voice (arz, ar or en), can be shown or hidden per line. Notes are tappable chips.
  - New words and the episode's questions come at the end, reusing the lesson exercise components.
  - The speaker → voice map lives in one place (e.g. Pak Budi = Ardi −10 % pitch, Sari = Gadis).
- **Kabar Radio:**
  - Plays the unit's two-host episode segment by segment: intro, dialogue, word spotlight, recap, outro.
  - A transcript follows the audio.
  - **Download for offline** fetches every line into `AudioCache` and writes one joined MP3 per episode, so it plays as a single track. The lines share one encoding, so the MP3 frames simply concatenate.
  - Downloads are per unit, and a "free up space" button removes them.
- **Background playback** with the screen off (earbuds on the way out) needs an Android `MediaSession` foreground service in Kotlin. It is part of M12, but it lands last, after in-app playback works.
- **Daily episodes "from my words"** (spec §14) use the `radio_script` task. They switch on once that task passes its golden set in the bake-off, which runs on the Mac. Until then Radio plays the prebuilt unit episodes, and the gap is reported, not hidden.

**Phase C: persona chats (needs #25 and the Reader, PR #14).**
- **Prompt:** each of the four personas has a style guide, and they all feed `prompts/roleplay_turn@v1.md`:
  - The persona block is filled from the YAML.
  - The learner's level comes from the Journey.
  - The example dialogues are used as few-shot examples.
- **Output:** `{reply, corrections[{text, fix, why_en, why_ar}], register}`.
  - It is validated with serde plus checks: each correction must quote the learner's text, and the Javanese register must fit the persona.
  - On failure it retries once, then shows a friendly message.
- **Chat screen:**
  - Pick a persona and a scenario (from the YAML openers and dialogues).
  - A register switch (ngoko/krama, casual/standard) is sent to the model as a hint.
  - Corrections appear inline, under the learner's message.
  - Every message is hoverable through the Reader's popover, and every line has audio.
  - Replies can be typed, or handwritten through the existing handwriting input.
  - A "replay as dialogue" button plays the whole chat.
- **Storage:** the synced `conversations` table holds one row per conversation and one per message, so rows stay small (LWW). Javanese replies carry the **unverified** badge.
- **Privacy:**
  - What the learner types in a roleplay counts as learning text. It is routed like `learn_entry`: Workers AI first, `gemini_free` allowed.
  - E-mail addresses and phone numbers are scrubbed before sending.
  - The chat shows one line saying that messages go to a free AI engine.
  - A persona never asks for real personal details.

**Cost:** $0.
- Workers AI's free daily allowance and Azure F0 (0.5M characters a month) cover it. One Radio episode is about 4–5k characters.
- When a quota runs out, a warm "come back tomorrow" (or "next month") message shows, and cached audio keeps working.

**Order and PRs:**
1. Phase A, one PR.
2. Phase B, one PR, after #25 merges.
3. Phase C, one PR, after #14 merges.
4. Background playback.

Each PR keeps `just check` green and ships its run/install steps in `docs/RUN.md`.

## 17. Tablet Studio (M9, spec §4)

M9 builds on the Pen Lab ink module (M1). One ink engine serves every surface: the Notebook, Gambar!, the Listening Companion, mind maps and write-mode lessons. The Study Desk and Card Wall are already done (M5/M6). Aksara Jawa calligraphy (`check_strokes`) stays in M13.

```
 WebView ink.js (pointer events, coalesced + predicted, desynchronized canvas, pan/zoom)
      │ finished strokes (quantized points)        ▲ strokes / items to draw
      ▼                                            │
 kabar_core::ink  ── stroke model, simplify, bbox, eraser hit-test, grouping into lines/words,
      │              spatial query, notebook search, letter diff, spelling suggestions,
      │              Gambar! rounds, mind-map model + layout
      ▼
 SQLite: notebooks · ink_documents (LWW) · ink_strokes (immutable, delete = tombstone)
         · notebook_items (LWW) · ink_transcripts (LWW)  ──▶ synced via Kabar Cloud
```

- **Data.**
  - `InkStroke` holds points quantized to 1/10 px with pressure (0–255) and time offsets (ms). There is one immutable row per stroke; erasing tombstones it, and strokes over 2,000 points are split.
  - `InkDocument` is one page of a notebook, or a Gambar! / Listening / mind-map / lesson canvas.
  - `NotebookItem` is a word card, text box, arrow or mind-map node with a position. These items are LWW, so moving a card is one upsert.
  - `InkTranscript` is the recognized text of a stroke group, with its source and confidence. Notebook search reads transcripts plus card and text items.
- **Recognition, best first.**
  1. **ML Kit Digital Ink Recognition** (Android): on-device, free, private, with `id` and `jv` models. It works on stroke data, not images. It arrives with the Android Kotlin glue; the bridge is `platform::recognize_ink(strokes, lang)`.
  2. **The `read_handwriting` vision task** through the router, for the Mac and as a fallback. It needs the Cloud blob upload.
  3. **Typed correction** by the learner, which always wins.

  Until the first two are connected, the pads offer "type what you wrote" so nothing is blocked.
- **Notebook.** One notebook per unit (`bipa1.u01` …) plus free notebooks. Each page is an infinite canvas with pan/zoom (two fingers, or a mouse wheel on the Mac/DeX). The pen writes, and the eraser button or the eraser tool erases. Word cards show the gloss (personal dictionary → prebuilt glosses), play audio and peek on hover. Search covers every page.
- **Gambar!** Kabar picks a drawable word from the learner's words (curriculum nouns first). The learner sketches for 60 s, and the canvas goes as a PNG to `guess_sketch`. The guess is scored against the target and its alternatives, and every round logs an `ActivityEvent`.
- **Listening Companion.** A narrow window or pane meant for split screen next to YouTube. The learner scribbles a word as they think it's spelled. Kabar suggests real words with deterministic fuzzy matching: edit distance with Indonesian/Javanese sound folding (old spelling dj→j, tj→c, oe→u, ch→c, double letters, e/é, final k/glottal). Each suggestion shows its gloss and audio, and one tap saves it to Review or Teach Kabar.
- **Mind-map generator.** "Make me a map of *di pasar*" goes to the `mind_map` task (topic, level, known words, lang). The answer is validated, laid out radially, and saved as notebook items, which the learner extends with the pen.
- **Write-mode lessons.** `InkPad` plus `ink::answer::letter_diff`; the Lesson Player (Journey thread) hosts it and grades the recognized text with its existing grader. The ink feedback circles wrong letters and writes the correct form underneath.
- **Free tier.** Recognition is on-device. Vision calls use Workers AI or Gemini free (`guess_sketch` sends only a sketch, which is not personal). Each ink row is a few KB, so a heavy day is ~500 strokes ≈ 1 MB of DO storage.
