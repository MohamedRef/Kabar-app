# Kabar on the web

Kabar's web app is the same Rust code as the Mac and Android apps, compiled to
WebAssembly. It follows the shape of the ArtCraft "Crafting Apps" (PdfCraft,
WordCraft…): one Rust codebase for every platform, a thin web shell, a static
site any host can serve, and CI that keeps the wasm build compiling. Kabar
keeps its Dioxus screens (HTML/CSS, right-to-left) instead of their egui
toolkit, as Batna does (`MohamedRef/Batna-app` docs/WEB.md).

SPEC §23 row M2+. Phase 1 runs everything that works offline; phase 2 brings
Kabar Cloud into the browser (sync, AI answers, cloud voices).

## Shape

| Piece | Where | What it does |
|---|---|---|
| The app | `crates/app` with `--no-default-features --features web` | Every screen; content, fonts, art and sounds built in |
| Browser stand-ins | `crates/app/src/platform/web.rs`, `platform/time.rs`, `platform/files.rs` | Storage, downloads, timers, background work, cached audio |
| Kabar Cloud client | `crates/app/src/cloud/api.rs` (`fetch`), `cloud/socket.rs` (`WebSocket`), `cloud/session.rs` (`WebVault`) | The same async client and sync engine as the apps, on browser transports |
| Kabar Cloud side | `crates/server/src/web.rs` | CORS for the web app's origin, the WebSocket's token as a subprotocol |
| Web shell | `web/index.html`, `web/manifest.webmanifest`, `web/_headers` | Loads `kabar.js`, installable as an app |
| Build | `scripts/web/build.sh` (`just web`, `just web-release`) | cargo → wasm-bindgen → wasm-opt → `target/web/kabar` |
| Host | `web/wrangler.toml`, `.github/workflows/web-deploy.yml` | A static-assets Worker, `kabar-web`, on the free plan, same account and token as Kabar Cloud |
| CI gate | `ci.yml` › "Web app compiles (wasm32)" | clippy for wasm32 on every PR |

## What differs in the browser

| Area | Mac / Android | Web |
|---|---|---|
| Database | SQLite file | The same SQLite, compiled to wasm (`sqlite-wasm-rs`), in memory; its bytes are saved to IndexedDB 1.5 s after a change and when the tab is hidden or closed |
| Two tabs | n/a | The newest tab owns the data; an older one stops saving and says so on every screen |
| Device prefs | In the database | The same (they're rows in the database) |
| Export everything | A file in Downloads / Save to | A browser download |
| Background work (glossary, FSRS fitting) | Threads | The browser's one thread, after the screen has painted |
| Read aloud | System voice / WebView | The browser's voice (Web Speech) |
| Updates and content bundles | Over the air | Each deploy downloads in the background (service worker) and is used from the next open |
| Kabar Cloud sign-in | Google (browser + loopback) | Username and password (the login work, `POST /v1/auth/password`); Google's desktop flow needs a loopback server a page can't run |
| Session | Keystore / private file | `localStorage`, or `sessionStorage` for "not my computer" (`WebVault`) |
| Sync | Its own thread and database connection, tokio-tungstenite | A task on the page's thread sharing the UI's database, the browser's `WebSocket` (token as the `kabar.bearer.<token>` subprotocol, since a browser can't set `Authorization` there) |
| AI answers, cloud voices | Kabar Cloud | The same calls through `fetch`; voices and downloaded episodes kept in IndexedDB (`platform/files.rs`) |
| Offline packs (audio per unit) | Download from the `packs` release | Phase 3: once packs carry audio (every pack has `audio_bytes: 0` today), the deploy copies them next to the app (same origin) and clips cache like cloud voices |
| Reader link import | Fetched natively | Paste the text (a browser can't read other sites' pages) |
| Screenshot Decoder (OCR) | Apple Vision / ML Kit | Phase 3 (AI vision through Kabar Cloud) |
| Reminders | Notifications | Not on the web; the phone or Mac rings |

Nothing is dropped: the rows marked phase 2 are planned below.

## Opening fast

`web/sw.js` (adapted from Batna's) keeps the newest build on the device.
`scripts/web/build.sh` fills in a build id (a hash of every file) and the file
list; each build has its own cache (`kabar-<id>`), so `kabar.js` and its wasm
always match. It answers this origin's GETs from that cache (every navigation
is `./`); Kabar Cloud never goes through it. A new deploy installs in the
background and the old cache goes once it activates, so after the first visit
opening doesn't wait for the 20+ MB download: it shows on the open after the
one that fetched it.

## Size

Cloudflare's static assets take files up to 25 MiB. `kabar_bg.wasm` holds the
code (roughly half) and everything built in: lesson content (about 4 MB
compressed), souvenir cards, portraits, map art, sounds and fonts. The
`web-release` profile (`opt-level = "z"`, fat LTO) and `wasm-opt -Oz` bring it
to about 23 MB (November 2026); `scripts/web/build.sh` prints the share of the
limit and fails past it.

When content outgrows that: the web build fetches the content bundle the
update feed already uses (`kabar_protocol::bundle`, `embedded::use_bundle`) as
its own file next to the app, and `build.rs` leaves the content out of the
wasm for `target_arch = "wasm32"`. Images can move out the same way.

## Kabar Cloud from the browser (phase 2)

- **One client.** `cloud/api.rs` is async on every target: `Api::send` runs
  ureq on a blocking thread on the Mac and Android and the browser's `fetch`
  on the web. The sync engine awaits it directly; its socket is
  tokio-tungstenite or the browser's `WebSocket` (`cloud/socket.rs`).
- **CORS** (`crates/server/src/web.rs`): Kabar Cloud answers preflights and
  adds `Access-Control-Allow-Origin` for `https://kabar-web.<account>.workers.dev`,
  `http://localhost:8080`, `http://127.0.0.1:8080` and any origin in the
  Worker's `WEB_ORIGINS` variable (comma-separated). The API takes bearer
  tokens, never cookies, so CORS guards nothing secret.
- **The socket's token.** `/v1/sync` also accepts
  `Sec-WebSocket-Protocol: kabar.v1, kabar.bearer.<token>` and answers with
  `kabar.v1`. A browser can't tell why a socket failed to open, so the web
  engine refreshes its token before the next try.
- **Which server.** `KABAR_CLOUD_URL` at build time (`.env` for `just web`;
  the deploy workflow uses the repo variable or the default Kabar Cloud).
- **Sign-in** is username and password (see the table); the form lives in
  Settings › Kabar Cloud, shared with the apps.

## Run it

```sh
cargo install wasm-bindgen-cli@<version in Cargo.lock> --locked   # once
just web          # dev build, http://localhost:8080
just web-release  # what Cloudflare serves (wasm-opt runs if installed)
```

Deploying: Actions › "Deploy Kabar web" › Run workflow (it also runs on every
push to main that changes the app). It needs the `CLOUDFLARE_API_TOKEN` repo
secret Kabar Cloud uses; the run summary prints the address
(`https://kabar-web.<account>.workers.dev`).

## Rules for app code

- Never call `std::time::{Instant, SystemTime}` (they panic in wasm): use
  `web_time`. Never `tokio::time`: use `platform::time::sleep`.
- Never `std::thread::spawn` in app code: use `platform::time::spawn` (a
  thread natively, the next event-loop turn on the web).
- Files outside the database go through `platform::files` (IndexedDB on the
  web). Network calls to Kabar Cloud go through `cloud::api::Api`.
- Network and file code that has no browser twin yet is `#[cfg(not(target_arch
  = "wasm32"))]` with a web stub that says why.
