# Patched dependencies

Kabar builds two Dioxus 0.7.10 crates from here instead of crates.io
(`[patch.crates-io]` in the root `Cargo.toml`). Both are the published 0.7.10
sources (MIT OR Apache-2.0, © DioxusLabs) with one fix each, marked
"Kabar patch" in the code. Drop this folder once a Dioxus release carries an
equivalent fix (upstream PR DioxusLabs/dioxus#5812 is the same idea).

## Why: the screen froze after the phone was locked (2026-11-06)

Dioxus sends every screen change to the WebView over a localhost WebSocket
and waits for the page to say "applied" before it renders again. When
Android freezes a locked app it destroys the app's TCP sockets, this one
included. Upstream 0.7.10 then loses the edit that was on its way (a broken
socket counted as "applied"), and a reconnect while the old socket's worker
was idle could be refused and its entry overwritten. Either way the next
render waits forever: taps still scroll and highlight, but nothing changes.

## What changed

- `dioxus-desktop/src/edits.rs`: each webview keeps its edits in an outbox
  until the page acknowledges them by sequence number. A new socket for the
  same webview takes over (a generation number retires the old worker) and
  is sent the oldest unacknowledged edit again. A socket that dies during
  the handshake no longer panics the accept loop. Test:
  `cargo test -p dioxus-desktop --lib reconnect`.
- `dioxus-interpreter-js/src/js/native.js` (and `src/ts/native.ts`, kept in
  step by hand): every edit frame starts with an 8-byte little-endian
  sequence number; the page skips a frame it already applied, acknowledges
  the highest one, survives an edit that throws, and only the current socket
  reconnects. `build.rs` was removed (it needs Bun to regenerate the JS);
  edit `native.js` directly.

Batna carries the same patch (`batna-app/vendor/`).
