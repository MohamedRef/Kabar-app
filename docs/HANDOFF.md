# Kabar handoff: everything a new agent needs

This file lets any new agent, account or chat continue Kabar from where the
last one stopped, with no other history. Read it top to bottom once, then
`CLAUDE.md` (working conventions) and the doc for the area you touch.

**Keep it current.** Every PR that changes behaviour, process, commands,
secrets, deploys or the state of the work updates this file in the same PR
(usually § Current state and § Log). See `AGENTS.md`.

Last full refresh: 2026-11-09, main at `3318e37` (latest release `v2026.11.1`).

**Contents:** 1 What Kabar is · 2 Architecture · 3 Build, test, run · 4 CI ·
5 Merge routine · 6 Releases, secrets, URLs · 7 Content and pedagogy ·
8 Testing · 9 Current state and open work · 10 Resume message · 11 Log

---

## 1. What Kabar is

- **Kabar**: a personal **Indonesian + Javanese** learning app for one
  learner, **Mohamed Refaat** (GitHub `MohamedRef`): Egyptian Arabic native,
  reads English. Javanese is **Yogyakarta + Madiun (Mataraman) only**, in
  street speech, not textbook.
- Full brief: [SPEC.md](SPEC.md) (v4 wins); agreed design: [PLAN.md](PLAN.md).
- Devices: **Galaxy S24 Ultra** (~384dp), **Galaxy Tab S11** (753dp portrait /
  1205×680dp landscape), **MacBook Air M2**, plus a **web** build.
- Personal only: never published to a store. Releases are sideloaded APKs, a
  self-signed Mac app and a web site on Cloudflare. $0/month to run.
- **Batna** (`MohamedRef/Batna-app`) is his separate medical study app with the
  same architecture. Kabar work never goes there and vice versa.

### Talking to Mohamed

- Reply in the language he writes (usually **Egyptian Arabic**), short.
  Code, comments, commits, docs: English.
- He wants **status and statistics as Markdown tables**.
- "Do everything yourself": open PRs and squash-merge them yourself once CI
  is green. **Ask before anything that costs money** or changes the stack,
  and before anything that can't be undone outside the repo.
- Treat asks as seeds and expand them sensibly. He prefers the agent does
  console/browser setup itself and only asks him to sign in.
- A bare "." or "?" from him is not an approval.
- PRs he opens from ChatGPT/Codex under his login ("No merge requested"):
  review the whole diff critically, fix and improve on the branch, then merge
  (standing rule, 2026-10-30). Typical noise to drop: `changes/*-checkpoint.yaml`
  holding `[]`, stray lines in `changes/retro/titles.yaml`, work logs in `docs/UI-PROGRESS.md`.
- New alpha releases only when he says «طلّع ألفا» (§ 6).
- Image/portrait prompts for Codex must be fully self-contained (style block,
  rules, sizes, output paths in each prompt): [VISUAL-ASSETS.md](VISUAL-ASSETS.md).
  SVG is drawn in code; painterly art comes from Codex as zips.

### Files outside git

Everything that lived only in the old project's shared folder
(`/mnt/project-files/`: art zips and batches, screenshots, curriculum tool
specs, critiques, Kabar's pre-purge git bundle, uploads) and the project's
memory notes are archived in the **private repo `MohamedRef/Batna-sources`**
(`project-files/`, `project-memory/`; files over 100 MB split in
`project-files/big/`). Despite its name it holds both apps' files. Clone it
when a task needs an old asset or note. New large working files that matter
beyond one task go there too, never into this repo if they are big or private.

## 2. Architecture

Rust stable (edition 2024, `rust-version` 1.90), one Cargo workspace,
Dioxus 0.7 (HTML/CSS UI in a WebView), SQLite, minimal Kotlin for Android.

| Crate | Package | What |
|---|---|---|
| `crates/core` | `kabar-core` | Pure logic: language engine, FSRS, coach (Kak Nur), journey, lessons, pacing, AI router, update checks. Builds for native and wasm32. |
| `crates/protocol` | `kabar-protocol` | Every type that crosses a process boundary (sync, AI `EngineCall/Reply/Error`, update feed, content bundle). Versioned serde. |
| `crates/content` | `kabar-content` | Content model, loaders and validation (`cargo test -p kabar-content` is the content gate). |
| `crates/app` | `kabar-app` | The Dioxus app for Mac, Android and web (`--no-default-features --features web`): journey, lesson, review, reader, teach, listen, chat, play, studio, stats, settings, desk (⌘K quick search, resizable panes, review focus), update, cloud, store, platform, `i18n.rs` (shared UI strings; a feature's own strings live in its `text.rs` / `msg.rs`). |
| `crates/server` | `kabar-server` | **Kabar Cloud**: Cloudflare Worker (workers-rs) + Durable Object `Account` per account (devices + sync hub) + `Login` DO per username; D1 `kabar` holds only `ai_cache` and `quota_usage`. `/v1/ai` proxies free AI engines; TTS via Azure F0; FCM push. |
| `crates/tools` | `kabar-tools` | Build-time content pipeline: glosses, Bridge, bake-off, packs, dialect guard, explains, README numbers, update keygen/manifest/sign/verify (`cargo run -p kabar-tools -- --help`). |

Other paths: `curriculum/` (BIPA 1–7, J1–J6 level → unit → lesson),
`content/` (lessons, tests, grammar, glosses, Bridge, stories, radio, games,
personas, cast, packs), `prompts/` (versioned AI prompts + `routes.toml`),
`schemas/`, `fixtures/` (recorded AI replies), `assets/`, `android/`, `web/`,
`scripts/` (incl. `scripts/curriculum/` lesson toolkit), `changes/`, `vendor/`
(Dioxus 0.7.10 `dioxus-desktop` + `dioxus-interpreter-js` patched so the
screen survives Android destroying its edit socket; see `vendor/README.md`,
and re-check it before any Dioxus upgrade).

Key design rules (also in `CLAUDE.md`):
- Synced tables carry `account_id`, `device_id`, HLC, tombstone. Reviews,
  attempts and XP are append-only events; statuses/settings LWW by HLC.
- **A new synced table needs a protocol `TABLES` entry**, writes via
  `write_synced` / `append_event`, and a `Typed` arm in `store.rs materialize`.
- AI goes through the router in `crates/core`, prompts versioned in
  `prompts/`, cached by (task, input hash, engine, prompt version), validated
  with serde (retry once, then a friendly message). `routes.toml` lists
  engines per task, best first: `workers_ai`, `gemini_free`, `local`
  (Ollama on the Mac), `claude` (off: paid). **Nothing personal goes to
  gemini_free** (it trains on inputs); personal tasks are never cached.
- Layout by window width (compact <600dp, medium <840dp, expanded ≥840dp),
  never by device. Logical CSS only (RTL); Latin snippets in `<bdi>`.
- App code must compile for wasm: no `std::time::Instant/SystemTime`, no
  `std::thread::spawn` (use `web_time`, `platform::time`).
- Robustness rules from the 2026-10-29 engineering audit:
  - Cloud quotas are **reserved** before a provider call in one D1
    statement (`quota::RESERVE`, conditional upsert + `RETURNING`), never
    read-then-add; a call that never ran (AI transport error, Azure non-200)
    gets its units back (`quota::REFUND`).
  - Content bundles: `bundle::decode` refuses more than `MAX_ENTRIES` files or
    a key over `MAX_KEY_BYTES`; every inflate is bounded by
    `MAX_FILE_UNPACKED` (download check, and `embedded::unpack_bundled` at
    read time).
  - `Store::export` reads every table inside one read transaction (one
    snapshot).
  - Finishing a lesson (`completed` event, new cards, cleared position) is
    one write: `Store::atomically` (a savepoint; `write_synced`,
    `append_event` and `upsert_cards` use savepoints so they nest). A refusal
    keeps the summary with «احفظ تاني» and doesn't count the lesson.
  - Store reads in the UI go through `persistence::read_or`/`read_failed`,
    never a silent `unwrap_or_default()`; `ReadNotice` says so above every
    screen and «اقرأ تاني» remounts the screen (`READ_RETRY`). Evidence is
    skipped, not scheduled with default settings, when settings can't be read.
  - Sync ops Kabar Cloud refuses for good, and outbox rows this build can't
    read, move to the local `held_ops` table (migration 10) with their
    reason instead of vanishing or blocking the queue (`store/held.rs`;
    the engine settles a push with `Store::settle_outbox`). Settings ›
    Account shows a card (`account/held.rs`) with «ابعتها تاني» and
    «سيبها» (asks first). Newest 1000 kept. Batna has the same design.
  - A lesson's "the content is right" check is bound to the content it was
    made on: `LessonVerified.fingerprint` (optional, older builds ignore it)
    from `kabar_core::fingerprint::lesson` (policy v1: layout, comments, key
    order, whitespace runs and the `review` block don't count; any other
    change does). A check on older content reads `Verification::Changed`,
    doesn't count (`Verification::counts`) and asks again in the summary;
    checks from before 2026-11-11 read `CheckedBefore` and still count.
  - Two connections, bounded waits (F16, `store/contention.rs`): the UI's
    connection waits at most `UI_BUSY` (750 ms) for the sync engine's
    write lock (was 5 s, a frozen screen), then fails into the screen's
    saving/retry state; the engine's waits `SYNC_BUSY` (5 s). Writes take
    the lock up front (`BEGIN IMMEDIATE` via `atomically` and the
    connection's default transaction behaviour), because a read that turns
    into a write fails at once instead of waiting. The export's snapshot
    stays a deferred read.
  - Study days (FSRS `DayClock`): each moment is read with the offset in
    force then in the device's zone (`zone::history`, chrono, built once
    per run; `DayClock::with_history`), so DST changes don't move past
    answers across the 4 am line. After travel the new zone's rules apply
    to the past (Kabar can't know where the learner was); a history that
    disagrees with the profile's offset now is ignored. Past moments on
    the Stats and Journal screens (review days, journal times, level-test
    days) read the same history through `kabar_core::coach::zone::Zone`
    (`Coach::zone`). Activity events keep the day stored when they
    happened. Reminder timing stays on the profile's offset alone, on
    purpose: Kabar Cloud's cron has no zone history and must reach the
    same answer as the device.

## 3. Build, test, run

`just` drives everything (`brew install just`; in a cloud container it is
usually missing, so run the recipe lines by hand). `.env` (git-ignored, from
`.env.example`) holds `KABAR_CLOUD_URL`, `KABAR_GOOGLE_CLIENT_ID`,
`KABAR_GOOGLE_CLIENT_SECRET`, and optional Azure/Cloudflare/Gemini keys for
`just packs-audio` and the bake-off on the Mac.

| Task | Command |
|---|---|
| Full gate before every commit | `just check` = `scripts/check-tracked-files.sh`, `scripts/check-file-sizes.sh` (warns only), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |
| Content gate | `just content-check`; after lesson changes `cargo run -p kabar-tools -- packs build` (CI runs `--check`) |
| Mac setup / dev | `just setup-mac` (once), `just run-mac`, `just run-phone-preview` (412×915) |
| Open one lesson | `KABAR_OPEN_LESSON=bipa1.u01.l01 just run-mac` |
| Android | `just android-keystore` (once), `just apk`, `just devices`, install ([RUN.md](RUN.md) § 3) |
| Mac app bundle | `just bundle-mac` (self-signed; first install `xattr -dr com.apple.quarantine /Applications/Kabar.app`) |
| Web | `just web` (dev), `just web-release` → `target/web/kabar` (must stay under Cloudflare's 25 MiB/file) |
| Cloud local | `just cloud-dev`, `just cloud-e2e` (workerd, fake Google, two devices) |
| Coverage | `just coverage` (floor in `scripts/coverage-floor`, now **93**%) |
| README numbers | `just readme` (optional; CI only warns) |

Container gotchas:
- Linux desktop builds need `apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libxdo-dev`.
- Two target dirs (Kabar + Batna) fill a 40 GB allowance; delete one first.
  With a shared `CARGO_TARGET_DIR` across worktrees, `kabar-tools` and tests
  that bake `CARGO_MANIFEST_DIR` can read the other tree: pass `--root <tree>`
  and `touch crates/content/tests/*.rs crates/content/src/*.rs crates/app/build.rs`.
- kaikki.org, dumps.wikimedia.org, Hugging Face and Leipzig are blocked from
  the cloud container: Wiktionary import and Javanese frequency run on the Mac.
- `*.workers.dev` is blocked by the cloud proxy: verify deploys via Actions.
- `gh` works for REST (`gh api repos/...`), not GraphQL; secrets/job logs 403.

More: [RUN.md](RUN.md), [MAC-TODO.md](MAC-TODO.md), [TESTING.md](TESTING.md),
[WEB.md](WEB.md), [CLOUD.md](CLOUD.md), [CONTENT.md](CONTENT.md).

## 4. CI and workflows (what runs when)

Mohamed (2026-11-04, firm): **pull requests run only the fast checks**; heavy
jobs are periodic or manual. Never re-add PR/push triggers to heavy workflows.

| Workflow | Runs on | What |
|---|---|---|
| `ci.yml` | PR + push to main (newer run cancels older) | PR: `check` (fmt, clippy, tests, content, ids, What's new file; lesson demand per level (`kabar-tools demand --check`, #633); advisory: README numbers, reading coverage, explanations; the Javanese dialect check fails, since 2026-11-13) + Android arm64 clippy. Main only: wasm32 core/protocol, web build, Cloud Worker build + local e2e. |
| `main-clock.yml` | every push to main | GitHub cron runs late here, so this waits until 4 h after the last preview, then dispatches preview APK, coverage and Settings browser. Newer merge restarts it. |
| `android-preview.yml` | cron every 4 h (backup) + manual + main-clock | signed preview APK → `preview` release + feed (skips if main unchanged) |
| `coverage.yml` | cron/main-clock + manual | line-coverage floor (93%). Red → fix in its own PR. |
| `settings-browser.yml` | cron/main-clock + manual; PR only when its test changes | Playwright check of Settings on the web build; `scripts/ci-apt-install.sh` first points apt at an Ubuntu mirror that resolves (Azure, archive.ubuntu.com, kernel.org, MIT), since `--with-deps` calls apt |
| `android-webview-regression.yml` | `android/**` changes + manual | Kotlin/JVM tests |
| `mac.yml` | nightly + manual, **never on PRs** | macOS build. Never wait on it to merge. |
| `web-deploy.yml` | push to main touching app/core/protocol/content/web | deploys Worker `kabar-web` |
| `cloud-deploy.yml` | push to main touching server/protocol + manual | deploys Worker `kabar-cloud` (finds/creates D1, migrations, signing key, allowlist) |
| `release.yml` | manual only | alpha/beta/stable release (§ 6) |
| `audit.yml` | weekly (Mon 05:17 UTC) + manual, **never on PRs** | `cargo audit` against RustSec (audit F23). A finding → fix in its own PR. |

Gotcha: `cloud-deploy.yml` finishes **green but skipped** when
`CLOUDFLARE_API_TOKEN` is missing. Check that the step "Deploy" succeeded.
macOS-15 runners are sometimes not acquired: rerun failed jobs later.

## 5. Merge routine

1. Branch from `origin/main`; PR to `main`.
2. Add `changes/<topic>.yaml` (every PR; CI fails without one):
   `- {kind: new|improved|fixed, ar_eg: "...", en: "..."}` (optional `ar`),
   one line per change the learner can see; quote lines containing `:`.
   Housekeeping PRs add a file holding `[]`; a README-only PR needs none.
3. Update this file if behaviour, process, commands or state changed; update
   README's hand-written feature/roadmap/run tables when they change.
4. Before merging: merge latest `origin/main`, run the checks, wait for green
   CI on that commit, then **squash-merge yourself**. Green and slightly
   behind main with no overlap is fine. Never force-push `main`; never
   `git add -A` with a target dir inside the checkout. If main is red, look
   for an open fix PR first.
5. A session may only push its own designated branch.

## 6. Releases and updates

- **Numbering (Mohamed, 2026-11-08):** calendar versions `YEAR.MONTH.N`
  (`2026.11.1` = first release of November 2026) and one build number for
  every build, preview or release (commits on `main`). Android versionCode
  = 20000000 + build; Mac CFBundleVersion = build. `scripts/version.sh`
  works it out; `Cargo.toml` keeps `2026.11.0` and is never bumped by hand.
  Details: `docs/RELEASE.md` §3. First calendar release: `v2026.11.1`
  (2026-11-08); the last old-style one was `v1.0.0-alpha.3` (2026-11-06).
- Channels: `preview` (CI), `update-alpha`, `update-beta`, `update-stable`
  (a beta also goes to alpha; stable to every channel; feed tags never
  change, installed apps read them). Human release tags: `v<version>`
  (`v2026.11.1`), with "What's new", `Kabar-<v>-android.apk`,
  `Kabar-<v>-mac.zip` and `SHA256SUMS.txt`.
- **Alpha only when Mohamed says so** («طلّع ألفا»), together with Batna
  unless he says otherwise. Steps: Actions › Release › Run workflow
  (channel alpha; no bump PR) → verify `update.json` with
  `kabar-tools update verify` against `crates/app/update-key.pub`.
- Preview builds are signed with the release keys, so any build can switch
  channel in Settings › Updates.
- Over the air: signed `update.json` (+ `.sig`) per channel; a separate signed
  **content bundle** (lessons, stories, radio, glosses, CSS) applies at the
  next launch without reinstalling; CI skips the APK when only
  content/changes/docs/CSS changed. Downloads resume. Offline audio packs per
  unit live in the `packs` release. [RELEASE.md](RELEASE.md),
  [COMPATIBILITY.md](COMPATIBILITY.md).
- What's new comes from `changes/*.yaml` (Morphe-style sheet before/after an update).
- A check shows the app's answer as soon as the feed is read; a newer
  content bundle downloads after it. Every update request fails after 20 s
  without progress (lookup, connect, each read), so a connection Android cut
  while the phone was locked doesn't hold the check for minutes.

### Secrets (names only, values never in git)

| Where | Name | What |
|---|---|---|
| GitHub Actions | `KABAR_UPDATE_SECRET` | Ed25519 feed signing key |
| | `KABAR_ANDROID_KEYSTORE`, `KABAR_ANDROID_KEYSTORE_PASSWORD` | APK release keystore |
| | `KABAR_MAC_CERT_P12`, `KABAR_MAC_CERT_PASSWORD` | self-signed code-signing identity |
| | `KABAR_GOOGLE_CLIENT_SECRET` | Google Desktop client secret (same value as Batna's) |
| | `KABAR_GOOGLE_SERVICES_JSON` | Firebase config for pushed reminders (**not set yet**) |
| | `CLOUDFLARE_API_TOKEN` | deploys kabar-cloud and kabar-web |
| | `KABAR_ALLOWED_EMAILS` | who may sign in |
| | `GEMINI_API_KEY`, `AZURE_SPEECH_KEY`, `FCM_SERVICE_ACCOUNT` | optional Cloud keys (**not set as of 2026-11-02**) |
| Mac Keychain | `kabar-update`, `kabar-android`, `kabar-codesign` | originals, from `scripts/release-secrets.sh` (also `~/.kabar/`) |
| Mac `.env` | `KABAR_CLOUD_URL`, `KABAR_GOOGLE_CLIENT_ID`, `KABAR_GOOGLE_CLIENT_SECRET`, `AZURE_SPEECH_KEY`, bake-off keys | local builds and pipeline |

Public (not secret): `GOOGLE_CLIENT_IDS` in `crates/server/wrangler.toml`
(Kabar still borrows Batna's Desktop client).

### Live URLs

| What | URL |
|---|---|
| Kabar Cloud | https://kabar-cloud.hamorefaat85.workers.dev |
| Kabar web | https://kabar-web.hamorefaat85.workers.dev |
| Releases | https://github.com/MohamedRef/Kabar-app/releases |

Sign-in: Google (PKCE loopback) **or username + password** (Argon2id on
device, `Login` DO; changing `PASSWORD_PARAMS` or the salt prefix invalidates
every password). On macOS the session lives in `<data dir>/cloud-session.json`,
not the Keychain (self-signed apps re-prompt). [CLOUD.md](CLOUD.md) § 4.1.

## 7. Content and pedagogy

Numbers (generated in README): BIPA 1–7 (70 units, ~800 lessons, ~4,150 new
words), J1–J6 (37 units, ~420 lessons, ~1,500 words), 10k Indonesian + 3k
Javanese glosses, ~630 Arabic Bridge cards, 107 Sinetron + 107 Radio
episodes, 10 personas, 20 golden sets.

Rules (Mohamed's decisions; all in force):
1. **Everything is unverified until Mohamed approves it.** Passing a lesson
   shows "finished ✓"; the unverified badge clears only when he taps
   «المحتوى سليم». Javanese and AI content are never auto-promoted by a score.
2. **Javanese = Yogyakarta + Madiun (Mataraman)**, street speech: contracted
   ngoko (ra, wis, sik), particles lho/to/ta; shop/neighbour krama. No Arek or
   Semarangan forms (`kabar-tools dialect`, list in
   `content/javanese/dialect-guard.yaml`; CI fails on a hit, deliberate
   uses go in its `allow:` list with a reason). The only Arek taught is recognition:
   grammar `jv.dialect.arek` is taught hear-only in `j1.u05.l06` (hear gak /
   onok / koen, answer in Jogja ngoko), listed in the guard's `allow:`. Plain e, never é/è (except aksara
   items about the vowel). Madiun-only words stay unverified until sourced.
   [curriculum/JAVANESE-FOCUS.md](curriculum/JAVANESE-FOCUS.md).
3. **Indonesian street speech** in dialogues (nggak, udah, aja…), formal in
   reading/official scenes; slides show both; `new_words` keep the dictionary
   form; closed exercises accept both via `alternatives:`.
   [curriculum/INDONESIAN-STREET.md](curriculum/INDONESIAN-STREET.md).
4. **Lesson shape** (validator-enforced): 1 slide with 3 examples, 10
   exercises of ≥4 types, a 4–6 line dialogue, 8 new words, ~6–7 min; each new
   word in ≥3 exercises of ≥2 types; word lessons go before the unit's
   practice lesson; the unit review retrieves a word from every learn lesson;
   at most 3 (hard limit 4) words of one tight semantic set per lesson;
   listening texts over 12 words and dictations over 10 add time to the
   estimate; task demand per BIPA level may not slip back
   (`content/demand-limits.tsv`, #633); every
   text has `ar_eg`, `ar`, `en`; BIPA 3+/J4+ closed items need `explain:`.
   Toolkit and full rules: [scripts/curriculum/README.md](../scripts/curriculum/README.md),
   [PEDAGOGY.md](PEDAGOGY.md), [AUTHORING.md](AUTHORING.md).
5. **Arabic Bridge**: every new word with `arabic:` leads to a card in
   `content/bridge/id.jsonl` naming the same source (`bridge:` override).
6. **Cast**: speakers only in their registered role (`content/cast.yaml`,
   [CAST.md](CAST.md)). Every asset, character, story or game is linked to a
   unit/lesson and shown in the app.
7. **Arabic number agreement** in UI: 1 singular, 2 dual, 3–10 plural, 0 and
   11+ singular. Templates use `{n|one|two|few[|many]}` filled by
   `kabar_core::coach::voice::fill` / `Ui::fill`; never `.replace("{n}", …)`.
8. **No dead UI**: anything that looks like a card/tile/row/chip opens what it
   shows (`.tap` classes, `card-action`; `dead_ends.rs` enforces it).
9. Glosses: never reorder `content/glosses/id/plan.tsv`; extend with
   `gloss plan --count N` + new batches. Lemma convention: chat forms take the
   dictionary form as lemma.
10. Every dataset, corpus, font and sound goes in `ATTRIBUTION.md`.
11. Use the test texts in [SPEC.md](SPEC.md) § 24 in tests and demos; tests
    never call live APIs (recorded fixtures in `fixtures/`).

## 8. Testing

- ~94% line coverage (floor 93, ratchet = one point under main). Run
  `just coverage` before merging a PR that adds code.
- Harnesses: FakeCloud hub, click-through Dioxus DOM tests (`test_dom.rs`,
  `testkit.rs`, `test_screens.rs`, `screen_kit.rs`; merging them is open
  work), scripted AI, workerd e2e, Playwright `settings-browser.cjs`.
- Android Back: `system-back.js` keeps one history trap; `BackKey.kt` routes
  the Back key through the page (every Back = one step; exit only from Today).
  Tests use `history.system()`, not `history.back()`.
- Android crash reports start from Settings › Storage diagnostics
  (WebViewGuard records renderer loss and exit reasons).

- Journey test (audit F23): `store/journey_tests.rs` runs first launch →
  grade → restart → restore (backup on a new phone) → two-device sync on
  one review state. What only a device can show is the manual matrix in
  `docs/NATIVE_QA.md`; record runs there against their exact commit.
- Shared plumbing with Batna: `docs/INFRA_PARITY.md` (audit F24, same file
  in both repos). A fix there says in its PR whether Batna needs it too.

## 9. Current state (2026-11-11)

| Milestone | State |
|---|---|
| M0 bake-off | code done; run on the Mac with real keys |
| M1 shell, M3 engine, M4 pipeline, M5 reader, M6 review, M7 coach, M8 journey, M10 games, M11 Javanese, M12 personas/Sinetron/Radio | done |
| M2 Kabar Cloud | deployed 2026-11-02; Gemini/Azure/FCM secrets unset; own Google client still to make |
| Web | phases 1–2 live (sync, AI, voices, password sign-in); phase 3 offline packs + OCR |
| M9 Studio | mind maps only; pen writing paused |
| M13 | content done; performance pass open; aksara and widgets parked |
| Releases | `v2026.11.1` published 2026-11-08 (alpha channel, first calendar version); previews every 4 h |
| Critic loop | final pass 10 done 2026-11-04; loop closed. Don't start another unless asked. |
| Open PRs / issues | None on 2026-11-09 (#608, Android Google sign-in, merged 2026-11-05) |
| Mac desk tools | ⌘K quick search, drag-to-resize sidebar and Reader word panel, review focus mode (`crates/app/src/desk/`, 2026-11-11, ported from Batna) |
| Brand | K2 since 2026-11-08: kawung launcher icon on batik indigo; the same mark beside the name (`brand.rs`) |
| Coverage | 94.42% when last measured 2026-11-04 (floor 93) |

### Open work and known gaps

- #633 (BIPA 5–7 task demand): content batches, half a level per PR (BIPA 5 u01–u05, u06–u10, then 6, then 7): Indonesian comprehension options with Arabic in `explain:`, longer listening texts, one multi-clause item per lesson. Each batch tightens `content/demand-limits.tsv`; target: Arabic-only listen options under BIPA 4's 32%, multiple choice near 0%. Drop the README "(aim)" label (`readme.rs cefr_label`) once all three levels are done.
- Mohamed's steps: Cloud keys (`GEMINI_API_KEY`, `AZURE_SPEECH_KEY`,
  `FCM_SERVICE_ACCOUNT`, `KABAR_GOOGLE_SERVICES_JSON`) so AI, cloud voices and
  pushed reminders work in release builds; bake-off on the Mac.
- Learner review of all content (glosses, Bridge, Javanese).
- Pen writing features (Notebook, handwritten answers, aksara calligraphy)
  paused; keep S Pen hover. Widgets and aksara (M13) parked.
- Not yet verified on a real device: Back-key routing, update notifications,
  the lock/unlock freeze fix (`vendor/`, tested on desktop sockets only).
- Test debt: merge duplicate click-test helpers.
- Big files: two Rust files are over 800 lines (`update/view.rs`, `test_dom.rs`;
  `scripts/check-file-sizes.sh` lists them); split each when a PR already changes it a lot (AGENTS.md § Small files).
  About 410 strings still sit in the shared `Msg` (some used by several
  features, some typed as `Msg` in helpers); move them as features change.

## 10. Resume message

Mohamed can paste this into any new chat or account to continue:

> Continue work on my apps Kabar (`MohamedRef/Kabar-app`) and Batna
> (`MohamedRef/Batna-app`). Before anything else, read `AGENTS.md` and
> `docs/HANDOFF.md` in each repo you touch and follow them. Reply to me in
> Egyptian Arabic. Keep `docs/HANDOFF.md` updated in every PR that changes
> behaviour, process or state.

## 11. Log

Newest first. One line per PR that changes behaviour, process or state.

- 2026-11-13: review #635: J3 krama lines use `legi` for a sweet taste (`j3.u02.l03`, `j3.u05.l08`), not Indonesian manis.
- 2026-11-13: #633 batch BIPA 6 u06–u10: 108 listen/choice items now have Indonesian options with Arabic `explain:`, listening texts 3–4 sentences, one compound item per lesson; `content/demand-limits.tsv` bipa6 tightened to 50.0 / 25.0 / 15.6.
- 2026-11-13: review #633 (part 1, tooling): `kabar-tools demand` reports task demand per BIPA level and CI fails a level worse than `content/demand-limits.tsv`; long listening/dictation texts count in the 7-minute estimate; README shows BIPA 5–7 CEFR as "(aim)". Content batches follow: half a level per PR (Mohamed approved the plan 2026-11-13).
- 2026-11-13: review #636: `curriculum/README.md` rewritten from `javanese.yaml` (Jogja/Madiun route, no stale counts); the Javanese dialect check now fails CI, with every current deliberate use in the guard's `allow:` list (§ 4, § 7).
- 2026-11-13: review #634: the «ra (Jogja)» slide in `j1.u05.l01` is tagged `jv.negation.ora-dudu`; `jv.dialect.arek` moves to `j1.u05.l06` as a hear-only slide + one closed item (guard `allow:` entry, `gak` in `jv.core-exclude.tsv`) (§ 7).
- 2026-11-11: Mac desk tools ported from Batna (`crates/app/src/desk/`, `assets/desk.css`): ⌘K / Ctrl+K quick search over screens, Settings sections, games and every lesson of both routes (also Go › Quick Search… in the Mac menu, a sidebar row and a phone top-bar button); drag-to-resize sidebar and Reader word panel for mouse pointers (`pref.nav_width`, `pref.panel_width`, tenths of a rem, device-only); review-session focus mode on tablet and Mac (`pref.study_focus`, `data-study-focus`). Lessons already cover the sidebar, so they need no toggle.
- 2026-11-11: Settings browser runs point apt at the first Ubuntu mirror that resolves (`scripts/ci-apt-install.sh`, from Batna #798) after ubuntu.com DNS failures broke `playwright install --with-deps`.
- 2026-11-11: engineering audit F19 (rest): Stats and Journal read past moments with the offset in force then, so a summer review keeps its day and hour (§ 2).
- 2026-11-11: engineering audit F23/F24: journey test, native QA matrix, weekly cargo audit, shared `docs/INFRA_PARITY.md` (§ 4, § 8).
- 2026-11-11: engineering audit F16: UI writes wait at most 750 ms for the sync lock (was 5 s) and take it up front (§ 2).
- 2026-11-11: engineering audit F19: study days read each answer with the time zone offset in force then, so DST changes don't move past answers (§ 2).
- 2026-11-11: engineering audit F25: a lesson's content check is bound to a content fingerprint; a lesson changed since asks to be checked again (§ 2).
- 2026-11-11: engineering audit F12: sync ops the hub refuses (or the device can't read) are held with their reason, shown in Settings › Account with send-again / let-go (§ 2).
- 2026-11-11: engineering audit F09/F10: a lesson's finish is one transaction with a retry; failed reads are announced above every screen with «اقرأ تاني» (§ 2).
- 2026-11-11: engineering audit F14/F18/F21: export is one snapshot, bundle decode and every inflate are bounded, Cloud quotas are reserved atomically (§ 2).
- 2026-11-09: a release with no new `changes/` lines says «تصليحات صغيرة وتنضيف.» on its page instead of repeating the previous release's lines (`scripts/release-notes.sh` reads the feed's newest history entry only when it is this version's).
- 2026-11-09: the GitHub release page is written in Egyptian Arabic only (`scripts/release-notes.sh`); a `changes/` line without Arabic text is left out (Mohamed).
- 2026-11-09: handoff and README reviewed against main: release `v2026.11.1`, Kabar Cloud deployed, open PRs, contents line.
- 2026-11-08: `v2026.11.1` released (first calendar version), the same day as Batna's.
- 2026-11-08: rebrand K2 (Mohamed's pick from the proposals): the launcher icon is now a drawn kawung (four cream petals and a soga seed on batik indigo `#23305e`), replacing the painted rice-terrace bubble. Sources `assets/src/icon/svg/*.svg` → `scripts/assets/render_icon_svgs.py` → PNG masters → `export_icons.py`. The same mark sits beside "Kabar" (with «خبر» under it) in the sidebar and phone top bar (`crates/app/src/brand.rs`, petals in `--indigo`; a test keeps it equal to the icon).
- 2026-11-08: release numbering switched to calendar versions (`2026.11.N`) and one build counter (commits on main) for previews and releases, on Mohamed's ask; `scripts/version.sh`, versionCode 20000000 + build, channel picked in the Release workflow, cleaner release page (what's new, uniform file names, SHA256SUMS), no bump PR (`docs/RELEASE.md` §3).
- 2026-11-06: 1.0.0-alpha.3 released on Mohamed's ask, so the Mac app (alpha feed only; previews carry no Mac build) gets the lock/unlock and update-check fixes.
- 2026-11-06: Android screen froze after lock/unlock (seen in Batna, same code here): Dioxus's edit socket patched in `vendor/` (edits resent until acknowledged); update check shows the app result before the content bundle downloads, requests time out after 20 s of no progress.
- 2026-11-05: Mac nightly green again (#611): update tests expect the Mac's own button and install message; menu test skipped on macOS (muda needs the main thread).
- 2026-11-05: feature strings moved out of the shared `Msg` into `<feature>/msg.rs` (i18n `Text` trait); `main.rs` split into `screen.rs`, `app_hooks.rs`, `shell.rs`; AGENTS.md § Small files; `check-file-sizes.sh` warns in CI.
- 2026-11-05: project folder and memory notes archived in the private repo `MohamedRef/Batna-sources` (§ 1).
- 2026-11-05: handoff docs added (`AGENTS.md`, this file, curriculum toolkit in `scripts/curriculum/`).
- 2026-11-04: 1.0.0-alpha.2 released (#607); final critic pass closed.
