# Shared UI completion: Kabar

Updated: 2026-10-30. Current branch: `improve/workflow-modules-20261030`.
Previous checkpoint: merged PR #486; subsequent merged UX changes are retained.

## Item 1: interactive and dynamic cards (2026-10-30)

- [x] Migrated 63 remaining native base-card surfaces, including dynamic
  classes, clickable buttons, status/alert regions, labelled sections and keyed
  list items. No native `.card` markup remains outside the shared factory.
- [x] Added `CardElement` for the original native tags. Global HTML/ARIA/data
  attributes pass through; typed click and drag handlers retain their callbacks.
- [x] `DivCard` delegates to the same Card implementation. `CardButton` retains
  native keyboard, disabled and form-button behavior with one explicit type.
- [x] Added rendered-production-component tests for semantic roots, attributes,
  dynamic status, event registration, keyed input retention and form type.
- [x] Kept shared components visible to the dead-end scanner and added a guard
  against copying base card markup back into individual features.
- [x] Local strict workspace/all-target Clippy, full workspace tests, content
  validation and production-script Node tests passed without desktop features.
  Batna packs also build and round-trip. Full native/target CI runs on the PR.

The existing physical-device and browser-layout gaps below remain separate
numbered items. Passing these component tests does not certify pixel layout.

## Item 2: workflow ownership (2026-10-30)

- [x] Split rendering stages, update state and orchestration, persistence
  responsibilities and existing tests into focused sibling modules.
- [x] Preserve the public feature facades and original function bodies. Child
  modules keep parent-only visibility for implementation helpers; no new runtime
  dependency, persistence format, scheduler or content approval change is involved.
- [x] Retain each original behavior test once, rebasing physical fixture paths.
  A function inventory confirms that no function was lost or duplicated.
- Validation results and full native/target CI are recorded on this item's PR.
  Local checks omit desktop features because this runner has no WebView libraries.

| Feature facade | Before (lines) | After (lines) |
|---|---:|---:|
| `lesson.rs` | 2,816 | 529 |
| `review.rs` | 1,909 | 448 |
| `journey.rs` | 1,590 | 746 |
| `store.rs` | 1,830 | 880 |
| `update/mod.rs` | 1,878 | 385 |

The remaining facade code owns integration and shared state. Tests live in
`tests.rs`; they are not replaced with new tests of the extraction itself.

## Item 3: rendered Settings browser checks (2026-10-30)

- [x] Export actual Settings/Look components and production styles through a
  test-only SSR renderer with synthetic preferences and an in-memory store.
- [x] Add pinned Chromium regression coverage for computed layout, focus and
  keyboard behavior across widths, themes, appearances, text sizes and RTL/LTR.
- The phone's normal 15px density is intentional; the suite verifies it along
  with 16px on wider layouts and the 125% larger-text setting.
- Browser run results and screenshots belong to this item's CI; see
  [SETTINGS-BROWSER-QA.md](SETTINGS-BROWSER-QA.md) for exact limitations.
- [ ] Real-device and broader-screen verification remains separate. SSR
  callbacks are not hydrated and physical-device QA is not claimed.

## Implementation checkpoint

- [x] Follow-up: migrated 11 more static content surfaces across Studio, Diagnostics, Pen Lab, Chat, Reader and Screens (9 files). The follow-up brings the explicitly counted static-card migrations from 67 to 78; controls and original div/section tags are preserved.
- [x] Added scanner regression coverage for shared Card/DivCard classes on the next source line.
- [x] Incorporated merged preference-recovery and Android Back changes from `main`.
- [x] Added semantic shared content surfaces: `ui::Card` retains `section`, `ui::DivCard` retains `div`. Existing `ui::ActionRow` keeps its accessible native button.
- [x] Migrated a first set of static cards in Lesson, Review, Journey-related Today, Play, Listen, Reader, Stats, Teach, and other eligible views without flattening child controls. Interactive cards and elements with special native attributes remain in place.
- [x] Extracted daily Journey route helpers to `journey/today.rs` and lesson decisions to `lesson/decision.rs`, retaining existing public call sites.
- [x] Added VirtualDom smoke coverage exercising actual shared components with button, label, text input, and action row.
- [x] Added stylesheet contract assertions for responsive width classes, appearance, high-contrast black theme, reduced motion/transparency, and shared surface tokens.
- [x] Completed the remaining base-card migration using explicit native semantics; interactive, keyed, labelled and list surfaces keep their original role.
- [x] Split the targeted large workflows behind their existing feature APIs (see item 2).
- [ ] Add true browser/WebView layout, focus and keyboard tests on rendered production views. VirtualDom and source CSS tests do **not** prove pixel layouts or keyboard navigation.
- [ ] Complete the separate physical-device matrix below. Current-item CI status is recorded on its PR.

## Quality and rollout

Current automated coverage includes strict Rust formatting, Clippy, tests, content validation, Android arm64 compile and Android renderer-loss regression (workflow outcomes are published on the PR). The CSS contract test checks production selectors, not computed layout.

Still to verify on actual WebViews: 384/600/840dp and wider, all supported themes, Calm/Glossy, Egyptian Arabic RTL and English LTR, maximum text size, keyboard Tab/Escape, screen reader labels, nested sheets and Android Back. No real-device test has been claimed.

Kabar and Batna maintain independent sources and release lifecycles; copy proven primitives deliberately and never add a shared runtime dependency.

## Next change boundaries

Each numbered item is checked, committed and uploaded separately. The current
item covers the shared card contract; later workflow, browser, device and write
feedback work remains explicitly tracked. CI status belongs to the current PR
head. No merge, release or deployment is implied by uploading a checkpoint.
