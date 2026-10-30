# Shared UI completion checkpoint

Updated: 2026-10-30. Branch: `improve/shared-ui-completion`.

Complete the original request: minimal presentation with all existing features,
production-quality Settings, shared category components and maintainable modules.
Each verified coherent batch must be committed and pushed immediately.

## Scope and status

- [ ] Expand shared surfaces without losing native element semantics, events,
  keyed state, ARIA or layout attributes. Migrate remaining base card markup.
- [ ] Extract focused rendering, workflow and persistence modules from the large
  files identified in UI-MAINTENANCE.md, preserving public call sites.
- [ ] Add repeatable checks using rendered production components and CSS across
  compact/wide layouts, themes, RTL/LTR, large text and keyboard interactions.
- [x] Integrate merged preference-recovery PR #481 and Android Back PR #482.
- [ ] Complete strict Clippy, workspace tests, content/release-note gates and CI.

## Review boundaries

- Start from current main. Earlier preference-recovery and Android Back PRs stay
  independent. Retain their changes when those branches are merged.
- Preserve features, data formats, native startup guards and content approvals.
- No new paid services or shared runtime dependency between the applications.
- Real Mac/Android device access is unavailable here. Automated layout and
  keyboard evidence must be identified as such, with physical-device QA recorded
  separately rather than reported as completed.

## Implementation checkpoint (PR #483)

- Integrated latest main, retaining the merged Android navigation and
  preference-failure recovery improvements.
- Added `ui::DivCard` alongside `ui::Card` so native div and section
  semantics remain unchanged. Migrated 26 static cards across real screens;
  click handlers, dynamic wrappers and other attributes remain native.
- Extracted focused modules: `journey/today.rs` (daily-plan routing) and `lesson/decision.rs` (answer marks and voice-note decisions).
- Added a Dioxus VirtualDom smoke test mounting the production Card, DivCard
  and ActionRow components with a real button and labelled input. Batna also
  tests the extracted answer-state decisions.
- Automated CI, Android arm64 compilation, content gates and WebView renderer
  regression tests must pass on the latest commit before review. These tests
  do **not** validate visual appearance on physical devices.

## Remaining before calling this scope complete

- Migrate remaining safe surfaces, and document any deliberately retained
  interactive/dynamic card wrappers; they must not lose native semantics.
- Continue focused module extraction without changing public interfaces or
  stored formats. Avoid broad refactors in sensitive update/install flows.
- Extend real-component tests to cover keyboard, responsive layout, all
  themes and RTL/LTR; VirtualDom smoke tests alone do not supply this evidence.
- Run the full CI gate and perform native-device UI QA independently.

No merge, deployment, medical-content change or new paid service is requested.
