# Curriculum

`indonesian.yaml` (BIPA 1–7) and `javanese.yaml` (J1–J6): level → units → lessons, each
with can-do objectives, grammar points, vocabulary themes, culture notes (souvenir cards)
and estimated minutes. Shape: `schemas/curriculum.schema.json`. The grammar library the
lessons point to lives in `content/grammar/` (`schemas/grammar-point.schema.json`).

Check everything with:

```sh
cargo test -p kabar-content
```

The tests validate every file against its JSON Schema, then check ids (unique, nested,
sequential), that every lesson's grammar is listed on its unit and every unit's grammar is
taught in a lesson, that each unit ends with a review, that every grammar point taught in
BIPA 1–2 exists in the library, and that SPEC §24 test sentences appear as examples.

## Indonesian (BIPA 1–7)

| Level | Journey | UKBI band | CEFR (approx.) | Evidence |
|---|---|---|---|---|
| BIPA 1 | Jakarta | Terbatas | A1 | high: published analysis |
| BIPA 2 | Bali | Marginal | A2 | medium |
| BIPA 3 | Lombok | Semenjana | B1 | high: published analysis |
| BIPA 4 | Sulawesi | Madya | B2 | medium: inferred |
| BIPA 5 | Kalimantan | Unggul | C1 | low: inferred |
| BIPA 6 | Sumatra | Sangat Unggul | C2 | low: inferred |
| BIPA 7 | Raja Ampat, Papua | Istimewa | C2+ | low: inferred |

Units, lessons, exercises, new words and lesson time per level are counted from the files
by `just readme` and shown in the [README's content table](../README.md); they are not
repeated here, so they can't go stale. Units hold about 8–15 lessons each (word lessons
first, then practice, then a review).

- **Official basis:** Permendikbud No. 27/2017 (SKL Kursus dan Pelatihan BIPA). Each level's
  `capability.official` is the regulation's wording; per-skill units are summarised in `skills`.
  The regulation spells out per-skill units for BIPA 1–3, partly for BIPA 4, and only a
  capability statement for BIPA 5–7. It says the levels are adapted from CEFR and follow
  the UKBI bands, but publishes no level-by-level CEFR table, so the CEFR column is graded
  by evidence and used only for orientation.
- **Topic sequence:** *Sahabatku Indonesia* (2019). Unit order and grammar follow the Umum
  (general) edition for BIPA 1–4 and the Pelajar edition for BIPA 7. All Kabar lesson
  content is written fresh; only unit titles are cited, so you can cross-check with the book.
- **Gaps (re-align later):** the BIPA 5 and 6 books' unit lists could not be retrieved
  online, so those units are derived from the SKL capability statements and C1/C2 themes
  (marked in `gaps`). The BIPA 7 Umum edition was not retrievable; the Pelajar list is used.
- **Lesson time** counts lessons only. Reviews (FSRS), the Reader, games and stories add
  daily time on top; the coach re-estimates from real attempt times.

## Javanese (J1–J6)

No official foreign-learner standard exists, so the sequence is Kabar's own design on the
themes fixed in SPEC §21, run as a train line across Java. Javanese here is **Mataraman
only: Yogyakarta and Madiun**, in street speech (Mohamed, 2026-10-27;
[JAVANESE-FOCUS.md](../docs/curriculum/JAVANESE-FOCUS.md)).

| Level | Place | Focus |
|---|---|---|
| J1 | Malioboro, Yogyakarta | street ngoko and chat Javanese (ra, wis, sik, neng, to, wae beside the standard forms) |
| J2 | Madiun | everyday Mataraman ngoko |
| J3 | Pakualaman, Yogyakarta | krama basics: when and with whom |
| J4 | Kraton, Yogyakarta | krama inggil, krama andhap and etiquette |
| J5 | Kutha Pendekar (Madiun) | reading real Javanese texts, with day trips to Kudus and Demak |
| J6 | Kotagede, old Mataram capital | aksara Jawa, plus Pegon (Javanese in Arabic script) |

- **No Arek (Surabaya) or Semarangan forms.** `cargo run -q -p kabar-tools -- dialect`
  checks every Javanese lesson, Sinetron and Radio file against
  `content/javanese/dialect-guard.yaml` and fails CI on a hit. The only taught Arek is
  hear-only: grammar `jv.dialect.arek` in `j1.u05.l06` (hear gak / onok / koen, answer in
  Jogja ngoko). Every deliberate exception is in the guard's `allow:` list with its reason.
- J1 unlocks after BIPA 1; each later J level needs the one before. Counts per level are in
  the [README's content table](../README.md).
- **Everything Javanese stays unverified until you approve it.**

## Reviewing

Grammar entries carry `review: { status: draft, verified: false }`. When you approve one,
set `status: approved` and `verified: true`; ask for changes with `status: needs-changes`
and a `notes` line.
