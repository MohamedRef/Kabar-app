# Kabar on the web

Kabar's web app is the same Rust code as the Mac and Android apps, compiled to
WebAssembly. It follows the shape of the ArtCraft "Crafting Apps" (PdfCraft,
WordCraft…): one Rust codebase for every platform, a thin web shell, a static
site any host can serve, and CI that keeps the wasm build compiling. Kabar
keeps its Dioxus screens (HTML/CSS, right-to-left) instead of their egui
toolkit, as Batna does (`MohamedRef/Batna-app` docs/WEB.md).

SPEC §23 row M2+. Phase 1 (this) runs everything that works offline; phase 2
brings Kabar Cloud into the browser.

## Shape

| Piece | Where | What it does |
|---|---|---|
| The app | `crates/app` with `--no-default-features --features web` | Every screen; content, fonts, art and sounds built in |
| Browser stand-ins | `crates/app/src/platform/web.rs`, `platform/time.rs` | Storage, downloads, timers, background work |
| Web shell | `web/index.html`, `web/manifest.webmanifest`, `web/_headers` | Loads `kabar.js`, installable as an app |
| Build | `scripts/web/build.sh` (`just web`, `just web-release`) | cargo → wasm-bindgen → wasm-opt → `target/web/kabar` |
| Host | `web/wrangler.toml`, `.github/workflows/web-deploy.yml` | A static-assets Worker, `kabar-web`, on the free plan, same account and token as Kabar Cloud |
| CI gate | `ci.yml` › "Web app compiles (wasm32)" | clippy for wasm32 on every PR |

## What differs in the browser

| Area | Mac / Android | Web (phase 1) |
|---|---|---|
| Database | SQLite file | The same SQLite, compiled to wasm (`sqlite-wasm-rs`), in memory; its bytes are saved to IndexedDB 1.5 s after a change and when the tab is hidden or closed |
| Two tabs | n/a | The newest tab owns the data; an older one stops saving and says so on every screen |
| Device prefs | In the database | The same (they're rows in the database) |
| Export everything | A file in Downloads / Save to | A browser download |
| Background work (glossary, FSRS fitting) | Threads | The browser's one thread, after the screen has painted |
| Read aloud | System voice / WebView | The browser's voice (Web Speech) |
| Updates and content bundles | Over the air | Each deploy is the newest build; reload to get it |
| Kabar Cloud: sign-in, sync, AI answers, cloud voices | Live | Phase 2: Settings › Kabar Cloud says so; AI screens say "connect to Kabar Cloud" as on an unconfigured build |
| Offline packs (audio per unit) | Download from the `packs` release | Phase 2 (served from the web app's own origin; no pack has audio yet) |
| Reader link import | Fetched natively | Paste the text (a browser can't read other sites' pages) |
| Screenshot Decoder (OCR) | Apple Vision / ML Kit | Phase 2 (AI vision through Kabar Cloud) |
| Reminders | Notifications | Not on the web; the phone or Mac rings |

Nothing is dropped: the rows marked phase 2 are planned below.

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

## Phase 2: Kabar Cloud from the browser

1. A transport for wasm: `cloud/api.rs` keeps its request/response types and
   gains a `fetch` implementation; the sync engine's socket becomes a browser
   `WebSocket`. The engine already runs as one async loop, so on the web it
   runs on the page's executor instead of its own thread.
2. Google sign-in by redirect with a **Web** OAuth client (Google Cloud
   console, free), the session kept in IndexedDB instead of the keychain.
3. CORS on the Worker (`crates/server`) for the web app's origin only.
4. AI and TTS then work unchanged (`cloud/link.rs` already routes every call
   through one place); cloud voices cache in IndexedDB.
5. Offline packs: the deploy copies the `packs` release next to the app (same
   origin, no CORS), and clips cache in IndexedDB.

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
- Network and file code that has no browser twin yet is `#[cfg(not(target_arch
  = "wasm32"))]` with a web stub that says why.
