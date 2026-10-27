# Kabar design system

How screens should look and adapt. Research behind it (what we took from
Material 3, Apple HIG, Duolingo, Busuu, LingQ, Readlang, Kindle, Anki…) is in
the project files: `design/2026-10-27-learn-from-other-apps.md`. Tracking
issue: #174.

## Principles

- **Content first.** The thing being learned is the biggest thing on screen;
  instructions and chrome are quiet.
- **Use the room.** Every screen has a deliberate layout per width class, not
  one stretched column.
- **Calm.** Space and surface tone separate things before lines and shadows
  do. Colour carries meaning (word classes, statuses, Bridge gold, right and
  wrong), never decoration.
- **No dark patterns.** No streak fear, lives, leagues or upsells (CLAUDE.md).

## Layout by width

Width classes come from `kabar_core::layout::WidthClass` and reach CSS as
`.app[data-width=…]`. Never branch on device names.

| Class | Width (dp) | Navigation | Panes |
|---|---|---|---|
| compact | < 600 | bottom bar + More | one column |
| medium | 600–839 | rail | one roomy column; two only for light content (Settings, lists) |
| expanded | 840–1199 | sidebar (or slim rail) | list-detail or supporting pane |
| large | 1200–1599 | sidebar | as expanded, wider panes |
| extra-large | ≥ 1600 | sidebar | may add a third pane or side sheet |

Canonical layouts to reach for (Material 3): **list-detail** (Journey stops,
Chat personas, Reader library), **supporting pane** (Lesson notes, Reader word
card, Review session side), **feed** (Play, Watch & listen).

## Tokens (`crates/app/assets/main.css`)

Colours come from `kabar_core::theme` (contrast-tested); everything else is a
CSS custom property. Sizes are rem so Settings › Text size scales them.

| Group | Tokens |
|---|---|
| Type | `--fs-xs` 0.81 · `--fs-sm` 0.875 · `--fs-md` 1 · `--fs-lg` 1.2 · `--fs-xl` 1.44 · `--fs-2xl` 1.73 · `--fs-3xl` 2.07 rem |
| Line height | `--lh-tight` 1.25 (headings) · `--lh-ui` 1.5 · `--lh-read` 1.65 · `--lh-arabic` 1.75 |
| Measure | `--measure` 68ch (Latin running text) · `--measure-ar` 60ch |
| Space | `--sp-1` … `--sp-8`: 0.25 · 0.5 · 0.75 · 1 · 1.25 · 1.5 · 2 · 3 rem |
| Density (on `.app`, per width) | `--gutter` (screen padding), `--stack` (gap between blocks), `--card-pad` |
| Radius | `--radius-sm` 8px · `--radius` 14px · `--radius-lg` 20px · `--radius-pill` |
| Surface | `--line-soft` (default card border), `--shadow`, `--shadow-raised` (sheets, popovers) |
| Motion | `--ease`, `--dur-fast` 120ms · `--dur` 200ms · `--dur-slow` 320ms |

Rules:

- New CSS uses tokens, not raw numbers or hex colours.
- Base rules use `:where()` so a screen's own rule always wins.
- Animations go inside `@media (prefers-reduced-motion: no-preference)`; the
  reduced-motion switch in `main.css` also stops anything that slips through.
- Logical properties only (`inline-size`, `margin-inline-start`…), so the
  Arabic UI mirrors. Latin snippets in Arabic text go in `<bdi>`.
- Touch targets are at least 44 px.
