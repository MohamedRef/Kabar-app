# Shared UI completion: Kabar

Updated: 2026-10-30. Current branch: `improve/interactive-cards-20261030`.
Previous checkpoint: merged PR #483.

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
- [ ] Further split large independent workflows (`lesson.rs`, `review.rs`, `store.rs`, `update/mod.rs`) in small tested changes.
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
