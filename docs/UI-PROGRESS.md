# UI follow-up progress

Updated: 2026-10-30. PR: https://github.com/MohamedRef/Kabar-app/pull/481
Branch: `improve/preference-save-recovery`.

## Baseline and concurrent changes

- Started from main `3de3c1a6f26718db598765c326a0b36c8c57a0d4` after the shared Settings UI, audit
  hardening and Android WebView guard had already merged.
- Renderer regression PR #480 merged during this work. Integrated main
  `21c1655` and verified the merge tree. Android Kotlin, test harness and
  workflow changes are retained intact; the existing startup guard is preserved.
- Each coherent change was pushed promptly. Both applications own their code;
  Batna does not depend on Kabar crates. No deployment or merge is requested.

## Completed implementation

- Shared reactive preference-save service and UI-independent queue. Only the
  newest failed action per stable key is retained; unrelated successful writes
  cannot hide a failure. Retry removes only successful current generations.
- Accessible failure notice in Settings, or on the main surface when Settings
  closes. The notice states that closing the app may lose unsaved choices.
- Device and collection preferences use the shared recovery path. App-wide
  state is extracted into `state.rs` with stable root re-exports.
- Kabar keeps atomic initial settings writes and retries only changed fields,
  preserving unrelated newer settings and updating the shared reactive state.
- Batna centralizes device changes through `Ui::update_prefs`, atomically
  publishes its existing preference file, and preserves the old file on failure.
  Synced settings use per-key retries; review commands change one field in
  current backend settings, preserving other choices and fitted parameters.
- `docs/UI-MAINTENANCE.md` describes ownership and safe extension points.

## Verification

- Local workspace tests with `--no-default-features --locked`: **1,037
  passed, 4 existing ignored**; no failures. App suite: 316 passed.
- Strict workspace/all-target Clippy via `clippy-driver`, warnings denied,
  passed without desktop features in both apps.
- Full Rust formatting, whitespace and equivalent tracked-source gates passed.
- Content validation and localized release-note validation passed. Batna's
  offline packs also built and round-tripped successfully.
- Tests exercise real blocked writes and recovery, latest-choice replacement,
  unrelated failures, repeated retries and a Dioxus reactive observer. Kabar
  includes a SQLite read-only/recovery test with an intervening synced setting.
- Full desktop, Android and Worker checks run on the PR. The merged Kotlin/JVM
  renderer regression workflow is included. The PR's latest-head checks are
  the source of truth for CI status, including after documentation commits.

## Remaining work

- Before review/merge, inspect the latest PR head's complete CI results. Keep
  the PR open for review; no merge or release was requested in this task.
- Native Mac/Android visual QA remains outstanding: compact/wide layouts,
  maximum interface size, RTL/LTR, themes, keyboard, Back and nested sheets.
  Virtual DOM/JVM tests do not prove physical-device rendering or crash recovery.
- Domain-specific writes (reviews, content edits, exports) retain their existing
  error paths. Never blindly replay append-only study events through this queue.
- Continue gradual extraction of large workflows and adoption of Card/ActionRow,
  following the remaining priorities in UI-MAINTENANCE.md.
