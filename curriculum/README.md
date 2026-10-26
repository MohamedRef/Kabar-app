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

| Level | Journey | UKBI band | CEFR (approx.) | Evidence | Units / lessons | Lesson time |
|---|---|---|---|---|---|---|
| BIPA 1 | Jakarta | Terbatas | A1 | high: published analysis | 10 / 50 | ~9 h |
| BIPA 2 | Bali | Marginal | A2 | medium | 10 / 50 | ~9 h |
| BIPA 3 | Lombok | Semenjana | B1 | high: published analysis | 10 / 40 | ~7.5 h |
| BIPA 4 | Sulawesi | Madya | B2 | medium: inferred | 10 / 40 | ~8.5 h |
| BIPA 5 | Kalimantan | Unggul | C1 | low: inferred | 10 / 40 | ~9 h |
| BIPA 6 | Sumatra | Sangat Unggul | C2 | low: inferred | 10 / 40 | ~9 h |
| BIPA 7 | Raja Ampat, Papua | Istimewa | C2+ | low: inferred | 10 / 40 | ~9.5 h |

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
themes fixed in SPEC §21: J1 Surabaya (survival ngoko, chat, a taste of Arek) → J2 Madiun
(Mataraman ngoko) → J3 Solo (krama basics) → J4 Yogyakarta (krama inggil / andhap,
etiquette) → J5 Semarang (real texts) → J6 Kotagede (aksara Jawa, plus Pegon — Javanese in
Arabic script, an easy win for an Arabic reader). 37 units, 148 lessons, ~27 h of lessons.
J1 unlocks after BIPA 1. **Everything Javanese stays unverified until you approve it.**

## Reviewing

Grammar entries carry `review: { status: draft, verified: false }`. When you approve one,
set `status: approved` and `verified: true`; ask for changes with `status: needs-changes`
and a `notes` line.
