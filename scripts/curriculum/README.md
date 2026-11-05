> Copied into the repo on 2026-11-05 from the Claude project's shared folder (`/mnt/project-files/curriculum-tools/`) so it survives without it. Run from the repo root, e.g. `python3 scripts/curriculum/apply.py my_spec.py`. Issue numbers (#137, #224…) are Kabar-app issues. Snapshot: not run by CI.

# Word-lesson toolkit (#137 step 2)

Goal: each level reaches the `vocab_target` in its curriculum yaml. The README table's "Known / target" column is cumulative, so each level must add its own **delta** (the number of new words listed for your chunk), assuming the levels below it reach their targets in parallel.

## #224 rules (read before writing: these override anything above)

1. **Placement.** Word lessons go **before** the unit's practice lesson. The unit stays learn… → practice… → review (PEDAGOGY §3.1).
   - `apply.py` now does this for you. It inserts the lesson and then runs `reorder.py`, which renumbers the practice and review lessons and rewrites every reference to their ids across the repo.
   - Check the renumbered ids in `git diff`.
2. **The unit review covers the new words.** After you add word lessons to a unit, edit that unit's review lesson (the last one in the unit) so it retrieves at least one new word from **every** learn lesson in the unit.
   - Aim for 60% or more of the unit's new words across practice + review.
   - Do it by adding or replacing review exercises, mostly match and fill, and stay within the caps of 10 exercises and 7 minutes.
   - The validator enforces this now (`unit_shape.rs`, on main since PR #234).
3. **No tight semantic sets** (PEDAGOGY §4.4).
   - A lesson is a **situation** that mixes word kinds and fields, never one category.
   - At most 3 words from any one tight set per lesson. Tight sets include colours, kinship terms, body or face parts, animals, fruits, vegetables, tableware, clothes, emotions, vehicles, days, months and numbers.
   - The validator now fails any learn lesson with **more than 4** new words from one set in `content/semantic-sets.yaml` (colours, fruit, vegetables, body, kinship, jobs, clothes, animals, measures, vehicles, tableware, emotions). 4 is the hard limit; 3 is the target. If your lesson introduces a set member the file doesn't list yet, add it to the list.
   - Example: spread kinship terms over "Foto keluarga" (nenek, kakek, cucu + foto, tua, mirip…) and "Lebaran di rumah Om" (om, tante, sepupu + mudik, ketupat, maaf…).
4. **Arabic Bridge links (#143, PR #253).** Every new word with `arabic:` must lead to a card in `content/bridge/id.jsonl`, and the card must name the same source. Vowels and the article «ال» are ignored.
   - The lookup tries the word, then its gloss lemma or root, then each word of a phrase, then (for jv) its Indonesian equivalent.
   - If none of those has a card, add `bridge: <card word>` under `arabic:`. Examples: `bridge: salam` for bersalaman, `bridge: senin` for Senen.
   - A derived word may give the card's root as its source (peradilan ← عدل). The card's own word must match the card exactly (adil ← عادل).
   - Names (Mesir) are exempt. For a genuinely new loan, write a card following `prompts/bridge@v1.md`.
5. **Core words.** Moving a lesson earlier can make it use a core word that is only taught in the practice lesson. Rewrite that sentence rather than moving words around.

6. **Answer explanations (#140).** Every closed exercise item (choice, fill, order, match…) in BIPA 3+ and J4+ needs a hand-written `explain:` with `ar_eg`, `ar` and `en`: one or two sentences on *why* the answer is right, pointing at the text or the grammar. Run `cargo run -q -p kabar-tools -- explains --list <level>` to see what is missing. CI reports the gaps as warnings for now.
7. **Javanese = Jogja + Madiun, street speech (JAVANESE-FOCUS.md).** Javanese lessons, Sinetron and Radio may not use Arek or Semarangan forms (rek, koen, gak, onok, ambek, nemen, ik, ndes…). `cargo run -q -p kabar-tools -- dialect` lists every hit with its Mataraman replacement (list: `content/javanese/dialect-guard.yaml`). It is advisory in CI now and becomes an error once the J1/J3/J5 relocations land.

8. **Indonesian street speech (INDONESIAN-STREET.md).** Dialogues sound like real casual Indonesian (nggak, udah, aja, gimana, kok, sih, dong, -in), formal Indonesian stays in reading, writing and official scenes, slides show both forms, `new_words` keep the dictionary form, and closed exercises accept both via `alternatives:`.

## Tools (run from the repo root; Python needs `pyyaml`)

| Command | What it does |
|---|---|
| `python3 scripts/curriculum/apply.py my_spec.py` | Renders each lesson in the spec. A new title is inserted **before the unit's practice lesson**; the lessons after it are renumbered and references to them rewritten. An existing title is regenerated in place, so you can edit the spec and re-run it. |
| `python3 .../reorder.py bipa3` | Moves any learn lesson that sits after its unit's practice lesson to before it, and renumbers the ids. |
| `python3 .../chk.py id "w1, w2, ..."` (or `jv`) | Shows where each candidate word is already taught, or its frequency rank. **Run this before writing a lesson. Never re-teach a word.** |
| `python3 .../gap.py id 3000` | Lists frequency words up to rank N that no lesson teaches. This is the best place to find words. |
| `python3 .../unk.py <lesson ids>` | Lists words in a lesson's target text that aren't taught at or before that lesson. Phrase halves show up as noise. |
| `bash .../fixmin.sh .` | Copies the validator's minute estimates into the curriculum. Run it after every apply. |
| `bash .../vcheck.sh .` | Runs only the content-validity test. |

The spec format is in `examples/u0507.py` and `examples/u0810.py`. Each of those holds real merged BIPA 2 lessons (8 words each). Copy that shape exactly.

## Lesson rules the validator enforces

- **Size:** 1 slide with 3 examples, 10 exercises using at least 4 types, a 4–6 line dialogue and 8 new words, for about 6–7 minutes in total.
- **Exercise types and their tuples:** see the top of `ws.py`. Use `match`×2, `listen`, `fill`×2, `mc`×2, `build`, `tr_from` and `speak`.
- **Retrieval:** each new word appears in at least 3 exercises of at least 2 types. A word counts when it's listed in an exercise's `words` tag list.
- **Build answers:** at least 3 words long.
- **Arabic in explanations:** every `explain` contains Arabic script.
- **Top-300 frequency words:** they can't be used before the lesson that teaches them. The validator names each one. Rewrite the sentence; don't teach the word early.
- **Practising words before they're taught:** an earlier lesson can't practise (tag) a word that you now teach later. Drop that tag from the earlier lesson.
- **Dialogue speakers:**
  - Use cast members only in their registered role. The registry is `content/cast.yaml`; read each entry's `about:`.
  - Otherwise use a role label such as `Penjual`, `Dokter`, `Perawat` or `Pelatih`.
  - A speaker must not share a base name with a cast entry unless it really is that person.
- **Three glosses:** every text gets `ar_eg` (Egyptian Arabic), `ar` (MSA) and `en`, in Arabic script where Arabic is needed.

## After you apply

1. Run `fixmin.sh`, `vcheck.sh` and `unk.py` on the new ids.
2. Check `crates/content/tests/content.rs` for any hardcoded per-level lesson counts that need updating.
3. Run `cargo run -q -p kabar-tools -- packs build`, then `cargo run -q -p kabar-tools -- readme`.
4. Run `cargo fmt --all --check`, then `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo test --workspace`, then `bash scripts/check-tracked-files.sh`.

## Shared files

- **Edit only your own level's section** of `curriculum/*.yaml`.
- **`content/packs/` and `README.md` are generated.** On a merge conflict, take main's version and rerun both generators.
- **Disk:** the shared cargo target fills the disk fast. Delete `$CARGO_TARGET_DIR/debug/incremental` when space runs low.
