# Kak Nur's voice (`content/coach/`)

Every line the coach says, written at build time and reviewed by the learner
(spec §6, §18). The runtime fills in only the personal details. Rendered by
`crates/core/src/coach/voice.rs` (`voice::say`), whose tests are the content
gate: `cargo test -p kabar-core coach::voice`.

**Review status:** `reviewed = false`. The whole file is waiting for the
learner's review. Set it to `true` only after the learner has read and approved
every line in all three languages.

## Format

`kak-nur.toml`:

```toml
reviewed = false
version = 1

[lines.plan_morning]            # snake_case key = voice::Line variant (Line::key())
moment = "Today's plan card…"   # for the reviewer: when this is said
placeholders = ["name", "minutes", "count", "topic"]   # what the line may use
ar-eg = ["…", "…"]              # Egyptian colloquial Arabic
ar = ["…", "…"]                 # Modern Standard Arabic
en = ["…", "…"]                 # English
```

- Each language has 1–4 variants. Frequent moments (plan, reminders, recap,
  report notes, Pelita) have at least 2. `say(lang, line, seed, args)` uses
  variant `seed % n`. The seed is the day number, so every device shows the
  same wording on the same day.
- Every `[lines.*]` key matches a `voice::Line` variant. The tests fail on a
  missing key or an extra one.
- The learner is addressed in the masculine (إنت / أنت), as in the app's UI strings.

## Placeholders

| Placeholder | Holds | Kind |
|---|---|---|
| `{name}` | the name Kak Nur uses | text |
| `{topic}` | a lesson point or phrase, e.g. "-kan verbs", "Apa kabar?" | text |
| `{skill}` | a skill's name in the UI language | text |
| `{item}` | a Warung item's name, e.g. "Kawung Senja" | text |
| `{level}` | a level, e.g. "BIPA 2" | text |
| `{date}` | a date, already formatted for the UI language | text |
| `{proverb}` | an Indonesian proverb (the achievement card shows its meaning) | text |
| `{minutes}` `{count}` `{words}` `{streak}` `{days}` `{koin}` | a whole number | numeric |

- Each line lists the placeholders it may use in `placeholders`. Using an
  undeclared or unknown placeholder fails the tests.
- **Plural groups** (numeric only): `{days|يوم|يومين|أيام}`.
  - Arabic takes `one|two|few` with an optional 4th `many` form.
    - 1 and 2 render as the noun alone: "يوم", "يومين".
    - 3–10 render as the number plus `few`: "5 أيام".
    - 11–99 render as the number plus `many`, which defaults to `one`:
      "15 يوم", or "15 يومًا" with `many = يومًا` in MSA.
    - 0 and 100+ render as the number plus `one`.
  - English takes `one|other`: "1 day", "7 days".
  - Digits stay as the caller passes them (ASCII or ٠–٩).
- **Missing arguments.** `say` first looks for a variant whose placeholders are
  all supplied, starting at the seed's variant. So callers omit an argument
  that doesn't apply, such as `count` when no reviews are due, and a fitting
  variant is used.
  - If no variant fits, a missing text placeholder renders empty and the
    sentence is tidied ("Today's plan is done, {name}." → "Today's plan is done.").
  - A missing count renders as its plural noun alone.
  - Raw `{…}` never reaches the screen.
- Literal `{`, `}` and `|` are not allowed in lines. Values are never
  re-parsed.

## Right-to-left: isolating Latin text

In `ar-eg` and `ar` lines, every Latin or Indonesian snippet and every **text**
placeholder sits between `⁨` (FSI) and `⁩` (PDI). This keeps
right-to-left rendering correct, matching `<bdi>` in the UI:

```toml
"⁨Selamat pagi⁩ يا ⁨{name}⁩، يعني صباح الخير."
```

The tests reject Latin letters outside an isolate, unbalanced isolates, other
bidi controls, and isolates in English lines. Numeric placeholders don't need
isolating.

## Tone rules (spec §2, §6) — hard requirements

- Warm, honest, specific, like an older sibling (*Kak*). Celebrate effort and
  comebacks, not only perfection.
- Mistakes are tomorrow's plan, never failures.
- **Never** guilt, fear, fake urgency, shame, streak-loss threats or dark
  patterns. No "don't lose your streak", "you missed", "hurry", "last
  chance", "failed". No exclamation spam: at most one `!` per line, never `!!`.
  The tests carry a forbidden-phrase list for each language, matched without
  Arabic diacritics.
- **Welcome back** (`plan_comeback`, `remind_welcome_back`, `pelita_relit`,
  `report_comeback`) is light. It has no streak talk and never counts the days
  away.
- Indonesian is sprinkled in, and its meaning is always clear from context:
  "⁨Semangat⁩، يعني شد حيلك", "Sampai besok: see you tomorrow".
  Words used so far:
  - Halo, Apa kabar, Selamat pagi, Selamat datang (kembali)
  - Ayo, Semangat, Hebat, Mantap, Luar biasa
  - Pelan-pelan saja, Sedikit demi sedikit, Sampai besok
  - Rencana Hari Ini, Laporan, Selamat, Bahasa Jawa
  - Kak, Nur, Pelita, Koin, Warung
- Kak Nur's content is original writing. It needs no `ATTRIBUTION.md` entry.
