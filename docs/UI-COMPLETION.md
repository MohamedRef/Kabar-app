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
- [ ] Verify compatibility with open PRs #481 and #482.
- [ ] Complete strict Clippy, workspace tests, content/release-note gates and CI.

## Review boundaries

- Start from current main. Earlier preference-recovery and Android Back PRs stay
  independent. Retain their changes when those branches are merged.
- Preserve features, data formats, native startup guards and content approvals.
- No new paid services or shared runtime dependency between the applications.
- Real Mac/Android device access is unavailable here. Automated layout and
  keyboard evidence must be identified as such, with physical-device QA recorded
  separately rather than reported as completed.

## Latest checkpoint

Repository conventions, surface usage and concurrent PRs reviewed. No code has
changed in this initial checkpoint. Next: implement the shared surface contract
and validate its semantics before migrating call sites.
