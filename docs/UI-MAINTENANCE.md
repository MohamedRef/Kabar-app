# UI maintenance and review

Reviewed against `fa0bb42` (2026-10-30). See [DESIGN.md](DESIGN.md) for the
visual vocabulary. This pass simplifies presentation without removing tools.

## Architecture

The app uses Rust structs/enums with `impl` methods and Dioxus component
composition. It does not need a class hierarchy to make one card change
reach the other cards. `crates/core` holds logic, `protocol` holds boundary
types, and `app` holds rendering, persistence and platform adapters.

`Ui` in `crates/app/src/main.rs` shares settings through Dioxus signals;
`Ui::update_settings` is the existing write path. Reuse it rather than
copying settings into a screen-local state. Device-only preferences belong
in `settings/device.rs`. A learner's stored content and an app surface are
different things: changing a Card component changes presentation, while
editing content must use its store operation and refresh subscribers.

## Where a shared change belongs

| Change | Owner | Effect |
|---|---|---|
| Card markup | `crates/app/src/ui.rs::Card` | Settings content surfaces using the component |
| Navigation row markup and semantics | `ui.rs::ActionRow` | Settings section links and profile shortcut |
| Card radius, padding, border, shadow | `assets/main.css`: `--radius-card`, `--card-pad`, `--card-border`, `--card-shadow` | Normal cards and collection surfaces; Glossy remains an explicit variant |
| Row layout, icon, title and detail | `assets/components.css` | Every `.activity-row` |
| Default list presentation | `views.rs::Layout::DEFAULT` | New collection choices; saved per-screen choices keep precedence |
| Settings sections and search terms | `settings/mod.rs`: `Section`, `GROUPS`, `SectionView` | The index, search and detail pane |
| Settings layout and controls | `assets/settings.css` | Phone and wide Settings layouts; also available to look updates |
| Settings keyboard boundary | `assets/settings-dialog.js` | Focus containment, Escape/Back selection and focus restoration |

Build feature layout around these parts. Add a modifier for a real variant
instead of copying the base card or assigning feature-specific colours.
Interactive cards still need a real button; `Card` itself is a surface.

## Settings behavior

Search finds sections by their titles, summaries and field-related keywords
in Arabic and English, regardless of the UI language. It tolerates Arabic
marks, alef variants and repeated spaces. Search filters the index, not the
controls inside a section. Empty results offer Clear search. Opening Settings
again clears the query; navigating back to its list keeps it.

Phones show the index or the selected section. Wide windows show both with
independent scrolling and a section description. Background controls become
inert while Settings is open. Tab stays inside the active dialog, Escape
uses the visible Back/Close control, and closing restores the previous focus.
The study or reading screen remains mounted beneath the sheet.

## Remaining work, in priority order

1. **Visible persistence failures.** `settings/device.rs::save` and collection
   writes report failures through diagnostics while keeping in-memory state.
   Introduce a shared save-result state with retry and an honest error near
   the control. Do not show a success banner before persistence succeeds.
2. **Smaller feature modules.** `lesson.rs`, `review.rs`, `journey.rs`,
   `store.rs` and `update/mod.rs` combine large rendering and workflow units.
   Extract pure decisions first, then move player stages and persistence
   operations behind narrow APIs, with existing behavior tests intact.
3. **Gradual primitive adoption.** This pass migrates Settings content and
   section links. Other features still have hand-written `.card` markup;
   their shared styles update now, but markup changes require migration.
4. **Native visual coverage.** Verify compact/medium/expanded widths, maximum
   text size, Paper/Night/True black, RTL/LTR, keyboard and Android Back on the
   actual Mac and Android WebViews. Browser fixtures alone cannot prove this.
5. **Cross-repository drift.** Batna owns adapted copies, not a dependency on
   Kabar. Port a proven improvement deliberately with source credit and tests;
   do not introduce a shared runtime package just to share a few primitives.

No new runtime service, dependency, data schema or content approval change is
needed for this UI pass.
