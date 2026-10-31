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

## Redesign 2026-10-29 (Calm / Glossy / Liquid Glass)

The design handoff lives in the project files
(`design/redesign-2026-10-29/`). Kabar's new Light and Dark palettes replace
the old Paper and Merapi values under the same theme ids, so System follows
them; True black, Malam, Batik and Sawah keep their colours and gain the new
roles below.

**Appearance** is a second dimension next to the theme: Calm (default, solid
surfaces) or Glossy · Material You (tonal surfaces, a top-edge highlight,
22 px cards, pill main actions, blur only on the phone bar and the More
sheet). It is a device preference (`pref.appearance` in the local `meta`
table, never synced), shown in Settings › Look as "نمط الواجهة", and reaches
CSS as `.app[data-appearance="calm|glossy|liquid-glass"]`. A missing or unknown value is
Calm. Switching only flips the attribute, so no screen loses its state.
Glossy goes solid under `prefers-reduced-transparency`, adds no animation,
and leaves True black's pixels off (no gradients on `data-theme="black"`).
It does not read wallpaper colours.

**Liquid Glass** is an optional iOS-inspired *CSS* appearance, not Apple's
native refraction effect. Theme-aware gradient backdrops, visibly layered
translucent cards and collections, a stronger selected tab and frosted chrome
make the choice distinct from Calm and Glossy. Settings is also glass-themed
because it occupies the whole phone screen while choosing an appearance.
Study answers keep their existing semantic colours. The Reader word sheet
stays opaque; blur is limited to small chrome and Settings cards.
True black retains a pure black page with subtle accent rims for controls.
Browser support, reduced transparency, high contrast and forced colours fall
back to solid surfaces. See `assets/liquid-glass.css`.

### Colour roles (`kabar_core::theme`, contrast-tested)

| Token | Meaning |
|---|---|
| `--bg`, `--surface`, `--surface-2` | page, card, quiet fill (chips, stat tiles) |
| `--text`, `--muted`, `--line`, `--line-soft` | text and hairlines |
| `--primary` (= `--sawah`), `--on-primary` | main action fill and its label |
| `--accent` (alias `--accent-soft`), `--accent-ink` | accent container (the hero's button, active nav pill in Glossy). Kabar keeps the design file's name; Batna's `--accent` was already its brand colour, so Batna calls the container `--accent-soft`, and Kabar exports that alias too so the two apps match |
| `--soft` | soft tonal container (tonal button, picked answer, row icon) |
| `--good`, `--good-bg`, `--bad`, `--bad-bg` | right and wrong, each with its container |
| `--gold`, `--gold-bg` (= `--gold-soft`) | Bridge gold: Arabic-origin words only |
| `--indigo` | selected state and headings (the primary green in Light/Dark) |
| `--dawn`, `--dawn-soft` | warnings and the default pill |
| `--sheet` | the word sheet's own layer |

Feature CSS takes colours from these tokens only
(`stylesheets_take_colours_from_tokens`).

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
| Control heights | `--tap` (buttons, 44px; 40px compact), `--tap-lg` (main actions), `--row` (big answer rows); in rem so Text size scales them |
| Compact notch | Under 600dp the type scale, `--lh-read`/`--lh-arabic`, density and control heights each step down one notch (`.app[data-width="compact"]`) |
| Radius | `--radius-sm` 8px · `--radius-control` 13px (main buttons, fields, answers) · `--radius` 14px (older rules) · `--radius-card` 18px (22px in Glossy) · `--radius-lg` 20px · `--radius-sheet` 24px · `--radius-pill` |
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

## Components

Use these before inventing a new look. The shared redesign set is in
`crates/app/assets/components.css`; anything that looks tappable opens what
it shows (`dead_ends.rs` lists the classes).

| Component | Classes | Notes |
|---|---|---|
| Primary button | `button.primary` | One per view: the step that moves the learner on. 13 px corners (pill in Glossy). Disabled turns neutral grey, never pale green |
| Tonal / outline / text button | `button.tonal`, `button.outline`, `button.text-btn` | Second action, plain secondary action, inline link-like action. Add `.wide` to fill the column |
| Icon button | `button.icon-button` | Round, faint fill, needs `aria-label` |
| Page heading | `.page-head` > `div` > `h1` + `p.sub`, then actions | One per screen |
| Section heading | `.section-head` > `h2` + `.aside` | `.aside` is a short count or hint |
| Status chip | `.status-chip` + `good` / `bad` / `gold` / `accent` / `neutral` | A label, never a button; colour plus a word or glyph |
| Flat / hero card | `.card.flat`, `.card.hero` | Hero = the one big next step (primary fill; its `button.primary` takes the accent) |
| Grouped list | `.card.grouped` > `button.activity-row` | Hairlines between rows |
| Activity row | `button.activity-row` > `.row-icon` (+`.gold`) · `.row-copy` > `strong` + `small` · `.row-end` (+`.chev`) | Always opens what it names |
| Empty state | `.empty-state` > `.row-icon` + `h2` + `p` + action | Say what fills it |
| Answer choice | `.answers` > `button.answer` (+ `.answer-mark`) | `aria-pressed="true"` picked, `.correct`, `.incorrect`; disabled after Check keeps its colours |
| Counter | `.ltr-count` | "2 / 5" stays in order inside Arabic |
| Progress | `.progress-line` > `.fill` | Width from the screen |
| Rating row | `.rating-row` > `button.rating.again/.hard/.good/.easy` + `small` interval | Real scheduler intervals; stays put |
| Search | `label.search-field` > icon + `input` | |
| Field | `.field` + `.field-hint`; invalid: `.field.invalid` + `.field-error` | Keep what was typed |
| Segmented | `.segmented[role=radiogroup]` > `button[role=radio][aria-checked]` | Two to four short options |
| Switch | `input.switch[type=checkbox][role=switch]` | |
| Button | `button` | Pill, 44 px tall, quiet hover on pointer devices |
| Quiet button | `button.quiet`, `.ghost` | Secondary actions, icons |
| Card | `.card` | Surface, `--line-soft` border, `--card-pad`; one topic per card |
| Pill | `.pill` + `neutral` / `info` / `gold` / `good` | Short labels and states. Default (dawn) for levels and "new" |
| Note / callout | `.note` | Tinted surface with an indigo start border |
| Side pane | `.panel`, `.lp-notes` | Supporting content beside the main column on expanded+; a bottom sheet on phones |
| Sidebar group | `.nav-group` + `.nav-heading` | Learn · Practise · Me. Named on the sidebar, thin dividers on the rail, invisible in the phone's bottom bar |
| Answer row | `.rate-row` | Stays in one place: bottom of the screen on phone and tablet portrait, under a steady-height card on wide windows |
| Feedback | glyph + words + colour | Never colour alone (✓ / ↺ / –, a verdict line) |
| View switcher | `views::ViewBar { of: Collection }` → `.view-bar` > `.view-sort`? + `.view-seg[role=radiogroup]` | Cards / list / compact plus the list's orders. Saved per list on this device (local `view.<key>`). `KABAR_VIEW=cards\|list\|compact` forces a layout for screenshots |
| Collection | `ul.coll[data-layout]` > `li` > `button.coll-btn` (or a details `summary.coll-btn`) > `.coll-lead` + `.coll-main` (`.coll-title`, `.coll-sub`, `.coll-extra`) + `.coll-end` + `.chev`; optional `.coll-aside` button | One markup, three looks. Compact hides `.coll-sub`/`.coll-extra`/`.coll-when-roomy`, shows `.coll-when-compact`. Test: `every_view_of_a_list_opens_its_items` |

List screens (2026-10-29): every list a learner browses has the view
switcher, like a Files app: games, Reader texts, the Arabic Bridge deck,
chat partners and past chats, Sinetron and Radio, and each Journey stop's
lessons (layout only: the path is the order). A new list declares a
`views::Collection`, puts a `ViewBar` above its `ul.coll` and builds its
items from the collection markup.

Key hints (Space, 1–4, ?) show only on keyboard and mouse devices, never on
touch screens. A list-detail selection (Journey's stop) lives above the
layout switch, so it survives a resize or a rotation.

Disabled controls say why when it isn't obvious from the screen (a line of
muted text next to them), instead of only greying out.

## Lesson and review states (K2, 2026-10-29)

- **Lesson question:** Unanswered (Check disabled, a hint under it says what it waits for) → Draft (the pick shows at once) → Check (grades once: `Player::check` returns the pending grade on a second call) → Feedback → Continue. The counter is `n / total` from `Player::counter`, isolated left to right.
- **Resume:** the Lanjut position (`LessonPosition.response`, protocol `SavedResponse`) keeps the answer on screen by content (option text, tile texts, matched pair words, typed text) and, once checked, its grade. Reopening restores the draft before Check/Continue is decided; a checked answer reopens on its feedback. Positions from before this kept only results: an exercise with a recorded result reopens answered (`Grade` from its outcome), never as a blank task. No explanatory notes on the screen (Mohamed, 2026-10-29): the Check hint is for screen readers only. A ↻ icon beside Continue tries an answered step again (`Player::try_again`): practice, graded on screen, never recorded over the first grade.
- **Finished vs content checked:** passing (`lesson::passes`, ≥80 first try) is progress only. The content check is the learner's explicit "The content is right" on the summary: a `verified` lesson event, undone by an `unverified` correction event (lesson events are append-only; the newer by HLC wins). `Store::verified_lessons` also keeps old-rule passes (`LessonCompleted.pass_verifies`, absent = true in events written before) as `Verification::PassedEarlier`, shown as such. Mohamed chose this on 2026-10-29.
- **Review:** reveal on request; the footer holds one primary action before the answer and the four ratings with real `Scheduler::preview` intervals after, in the same place. A Javanese card's content check is its own row, apart from the rating. Undo tombstones the last event first, then hands the card back and forgets the event in the deck (`review::take_back`), so the schedule is exactly as before.

## Maintenance and shared primitives

See [UI-MAINTENANCE.md](UI-MAINTENANCE.md) for the code review, shared Card
and ActionRow components, Settings search and the remaining improvements.
