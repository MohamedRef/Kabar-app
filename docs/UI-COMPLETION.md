# Shared UI completion: Kabar

Updated: 2026-10-30. Branch: `improve/shared-ui-completion` (PR #483).

## Implementation checkpoint

- [x] Follow-up: migrated 11 more static content surfaces across Studio, Diagnostics, Pen Lab, Chat, Reader and Screens (9 files). The follow-up brings the explicitly counted static-card migrations from 67 to 78; controls and original div/section tags are preserved.
- [x] Added scanner regression coverage for shared Card/DivCard classes on the next source line.
- [x] Incorporated merged preference-recovery and Android Back changes from `main`.
- [x] Added semantic shared content surfaces: `ui::Card` retains `section`, `ui::DivCard` retains `div`. Existing `ui::ActionRow` keeps its accessible native button.
- [x] Migrated a first set of static cards in Lesson, Review, Journey-related Today, Play, Listen, Reader, Stats, Teach, and other eligible views without flattening child controls. Interactive cards and elements with special native attributes remain in place.
- [x] Extracted daily Journey route helpers to `journey/today.rs` and lesson decisions to `lesson/decision.rs`, retaining existing public call sites.
- [x] Added VirtualDom smoke coverage exercising actual shared components with button, label, text input, and action row.
- [x] Added stylesheet contract assertions for responsive width classes, appearance, high-contrast black theme, reduced motion/transparency, and shared surface tokens.
- [ ] Complete migration of every remaining eligible surface; do not change an interactive button, `li`, ARIA attributes, keyed node, or custom element into a generic card.
- [ ] Further split large independent workflows (`lesson.rs`, `review.rs`, `store.rs`, `update/mod.rs`) in small tested changes.
- [ ] Add true browser/WebView layout, focus and keyboard tests on rendered production views. VirtualDom and source CSS tests do **not** prove pixel layouts or keyboard navigation.
- [ ] Verify latest-head CI and do real device QA before marking ready.

## Quality and rollout

Current automated coverage includes strict Rust formatting, Clippy, tests, content validation, Android arm64 compile and Android renderer-loss regression (workflow outcomes are published on the PR). The CSS contract test checks production selectors, not computed layout.

Still to verify on actual WebViews: 384/600/840dp and wider, all supported themes, Calm/Glossy, Egyptian Arabic RTL and English LTR, maximum text size, keyboard Tab/Escape, screen reader labels, nested sheets and Android Back. No real-device test has been claimed.

Kabar and Batna maintain independent sources and release lifecycles; copy proven primitives deliberately and never add a shared runtime dependency.

## Next change boundaries

Keep the PR in draft until tests, target coverage and native verification are complete. Preserve saved data formats, cloud behavior, preferences, existing events and content approvals. Do not merge or deploy as part of this checkpoint.
