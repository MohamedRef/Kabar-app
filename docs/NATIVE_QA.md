# Native QA matrix

Engineering audit F23 (2026-10-29): green CI doesn't prove a full native
journey. What CI covers, and what only a real device can, is listed here.
Record each manual run against the **exact commit** it was made on (never as
a pass for a different head). The Mac workflow stays nightly/by hand; no PR
waits for any of this.

## Covered by automated tests

| Journey | Where |
|---|---|
| First launch → grade → restart → restore (backup on a new phone) → two-device sync | `store/journey_tests.rs` |
| Kabar Cloud with fake Google and two devices (local Worker) | CI job "Kabar Cloud (wasm build + local end-to-end)", main only |
| Refused sync ops held, sent again | `store/held.rs`, `settings/tests.rs` |
| A UI write while sync holds the lock (bounded wait, then retry state) | `store/contention.rs` |
| Lesson finish all-or-nothing; read failures announced | `store/tests.rs`, `persistence/reads.rs` |
| A restored database takes its own device id | `store/install.rs` |
| Dependency advisories (RustSec) | `.github/workflows/audit.yml`, weekly |

## Manual, on devices (Mac, S24 Ultra, Tab S11)

Copy the table into a dated section below for each run.

| Check | How | Result |
|---|---|---|
| First launch on a clean install | Uninstall, install the build, finish onboarding | |
| Lesson → answer → kill the app (recents swipe) → reopen | Lands on the same step; answers kept | |
| Process death mid-lesson | Developer options › Don't keep activities, leave and return | |
| Background / resume | Home, wait 1 min, return; sync reconnects, nothing lost | |
| Offline | Airplane mode, review, back online; the outbox empties | |
| Permission denied | Deny notifications and microphone; Kak Nur and speaking say so, nothing crashes | |
| Two devices | Review on the phone, see it on the tablet within a minute | |
| TalkBack / VoiceOver | Today, a lesson step, Settings › Account read in order | |
| Keyboard focus (Mac, tablet keyboard) | Tab through Settings and a review card; focus visible | |
| 200% text | Android font size max; nothing clipped on Today and Review | |
| RTL with mixed script | Indonesian/Javanese in Arabic UI; `<bdi>` keeps order | |
| Release-build feel | 20 reviews quickly; no visible lag | |

## Runs

(none recorded yet)
