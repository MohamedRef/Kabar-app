# Batna in Anki

Batna's content is exported as **one Anki file, `batna.apkg`**. It holds a `Batna` deck with one subdeck per lecture (`Batna::L01 Liver Cirrhosis`, …), so you study it while the app is still being built, and you import only one file however many lectures exist.

## Import it
- **Phone or tablet:** open `batna.apkg` from the project files (or Files) and choose AnkiDroid. Then study the `Batna` deck, or one lecture's subdeck.
- **Mac:** double-click `batna.apkg`, or use File › Import in Anki.

## When new content lands
Import the newer `batna.apkg` the same way. Every note has a fixed id that comes from the item id (`hb.L01.003`), so Anki **updates the existing cards in place**, adds the new ones, and keeps your reviews and due dates. Nothing is duplicated.

## What the cards look like
| Item kind | In Anki |
|---|---|
| card | Basic note: front, back |
| cloze | Cloze note, one card per `{{cN::…}}` |
| mcq | Basic: the stem and options A–E on the front; the answer letter and one line on why each option is right or wrong on the back |
| enumerate | Basic: the prompt with "N points; pass at P", then the points |
| essay | Basic: "Essay outline: …", then the outline headings and bullets |
| case | One Basic note per step; each step shows the data revealed so far |
| table | One Basic note per blank cell: "Table: row → column?" |
| order | Basic: "Put in order: …", then the steps in order |
| calc | Basic: the vignette, then the expected score and what it means |

Every back shows the lecture and **book page**, and an amber **Update** box where a current guideline differs from the book (the book stays the answer for the exam).

Tags: `hb::L01` (lecture), `topic::<slug>`, `unverified` or `approved`, `draft` (not merged yet), and the item's own tags (`high-yield`, `numbers`, `book-question`). To study only what you have checked, search `tag:approved`. To drill numbers, search `tag:numbers`.

## Build it (developers)
```
just anki                                              # dist/anki/batna.apkg, every written lecture
cargo run -q -p batna-tools -- anki --lecture hb.L01   # dist/anki/batna-hb-L01.apkg, one lecture
cargo run -q -p batna-tools -- anki --verdicts export.yaml
```
- `--verdicts FILE` reads the app's ✓/✎/✗ export (`batna.verdicts/v1`). It leaves out items marked ✗, uses the edited text for ✎ items, and tags ✓ items `approved`.
- A lecture that isn't merged yet comes from the project's shared folder (`content-wip/hb/LNN.yaml`, then `drafts/LNN.yaml` or `LNN.draft.yaml` under `/mnt/project-files/medstudy`, or `--drafts DIR`), so the file never shrinks; those notes are tagged `draft`. `--merged-only` uses only `content/`.
- `content/approved.txt` also marks items `approved`. Items with `status: rejected` are always left out.
- The deck, note type and note ids match the old `make_apkg.py` (genanki), so decks imported from either one update each other. The Rust exporter replaces that script; `crates/tools/src/anki.rs` has the test that pins the ids.

## Fix list from the app
```
cargo run -q -p batna-tools -- verdicts export.yaml            # ✎/✗ items grouped by lecture, with notes and edits
cargo run -q -p batna-tools -- verdicts export.yaml --approve  # also adds the ✓ ids to content/approved.txt
```
Content threads fix the YAML from this list; only Mohamed's own ✓ goes into `content/approved.txt`.
