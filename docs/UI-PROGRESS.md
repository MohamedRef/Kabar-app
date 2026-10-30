# UI follow-up progress

Updated: 2026-10-30. This file is a resumable checkpoint for the preference
recovery and maintainability follow-up. Save each coherent change in a commit
on this PR branch; update this checkpoint with evidence and the next step.

## Baseline and concurrent changes

- Started from main `3de3c1a6f26718db598765c326a0b36c8c57a0d4`.
- Merged work already present: #477 (shared Settings), #478 (WebView guard), #479 (audit hardening).
- Open #480 adds Android WebView renderer regression tests, injectable
  recovery hooks and cleanup of destroyed renderers. Its changed files are
  Android Kotlin/test harness/workflow files and its own release note. This
  follow-up does not edit those files. Preserve the existing startup guard.
- Previous UI PR is merged; this is a new follow-up PR, not a replacement.

## Scope

1. Make failed preference saves visible, with an explicit retry action.
2. Keep the newest failed choice per setting; retry must preserve unrelated
   newer changes and must never replay append-only study events.
3. Separate reusable save state, app state and device file IO from rendering.
4. Test real write failures, recovery and reactive notice updates. Preserve
   audit hardening, existing features and synced data formats.

## Verification and next step

- Repository conventions and concurrent PR file lists reviewed.
- Implemented the shared latest-choice queue, visible notice, device and collection
  preference recovery; extracted `state.rs`. Batna file publication now preserves
  the previous preference file on failure.
- Added queue, real IO recovery and reactive Dioxus observer tests.
- Formatting and whitespace checked. App tests are running in a fresh local
  Rust environment; full validation is pending for this checkpoint.
- Next: integrate synced per-key preferences and review choices without
  restoring stale snapshots. Complete strict Clippy, workspace tests and content
  gates, and record exact results here and in the PR before marking ready.
- Native Mac/Android visual QA remains outstanding; DOM tests cannot prove
  native layout or renderer recovery on a physical device.
