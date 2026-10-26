# Build-time content

Everything here is written at build time, reviewed by the learner, and shipped
in content packs. `cargo test -p kabar-content` validates all of it (shape,
then content rules) on every CI run.

| Folder | Format | What |
|---|---|---|
| `grammar/<lang>/<id>.yaml` | `kabar.grammar/v1` (`schemas/grammar-point.schema.json`) | Grammar library: one point per file, explained in Egyptian Arabic, MSA and English. BIPA 1–7 and J1–J3 are written; J4–J6 come later. |
| `lessons/<lang>/<lesson-id>.yaml` | `kabar.lesson/v1` (`kabar_protocol::lesson::LessonContent`) | One file per curriculum lesson: slides → exercises → dialogue → new words → souvenir. BIPA 1–2 (100 lessons) are written. |
| `tests/<lang>/<id>.yaml` | `kabar.test/v1` (below) | Level tests (`bipa1.test`, `bipa2.test`, also used for "Take a flight") and the onboarding `placement` test. |

## Lesson rules (checked in `crates/content/src/lessons.rs`)

- `id` = file name = a curriculum lesson id; `title` = the curriculum title.
- learn lessons: 3–6 slides, and every grammar id of the lesson is taught by a slide (`grammar:`). practice/review: 0–2 recap slides.
- 8–15 exercises, ids `e01…`, at least 4 types; each has a reference answer
  (multiple choice/listen: an index into `options`; fill-blank: exactly one
  `___`; translate: accepted answers in the right direction; build-sentence:
  alternatives use the same tiles, distractors are not in the answer).
- every lesson has a dialogue (≥4 lines); learn lessons list ≥3 new words (`arabic:` marks a real Arabic loan).
- the unit's last lesson carries its souvenir card, id `<unit-id>.souvenir`; no other lesson does.
- unknown fields are errors, so a typo never silently drops content.

## `kabar.test/v1`

```yaml
schema: kabar.test/v1
id: bipa1.test              # = file name; level tests are <level>.test
language: id
kind: level                 # level | placement
title: {ar_eg, ar, en}
intro: {ar_eg, ar, en}      # optional
minutes: 30
pass_mark: 0.75             # level tests
scoring: "..."              # optional, how to score / route
sections:
  - id: listening
    skill: listening        # listening | reading | writing | speaking | vocabulary | grammar
    level: bipa1            # placement: the level this section probes
    title: {ar_eg, ar, en}
    items:                  # lesson exercise kinds (same `type` tag and fields) plus:
      - id: t01             # unique in the test
        points: 1           # optional
        skill: reading      # optional, per-item skill for the radar
        type: writing_task  # or speaking_task: open, graded by AI against the rubric
        prompt: {ar_eg, ar, en}
        rubric: ["...", "..."]
        sample: "model answer"
review: {status: draft, verified: false}
```

Placement: sections run easiest first; the app stops at the first section
scored below 0.7 and starts the learner at that level.

## Review

All content starts as `review: {status: draft, verified: false}`. Javanese
stays unverified until the learner approves it (the checks enforce this).
