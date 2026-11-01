# Content ids and learner records

Kabar stores the learner's history against ids written in the content:

| Record | Points at |
|---|---|
| Lesson events (finished, answers, Lanjut position) | lesson id, exercise id inside the lesson |
| Review cards | lesson id and word (`review::from_lesson::lesson_card_id`) |
| Coach plan items, souvenirs | unit id, souvenir id |
| Level and placement test results | test id, test item id |
| Stories, Radio, personas, grammar | their file ids |

An id that disappears, or that starts naming a different kind of exercise,
silently cuts that history loose. Content changes are therefore checked
against the previous version of the content.

## The check

```sh
cargo run -p kabar-tools -- ids check --base origin/main      # against a commit
cargo run -p kabar-tools -- ids check --base-bundle FILE      # against a published bundle
cargo run -p kabar-tools -- ids list                          # every id and its parent
```

CI runs `ids check --base HEAD^1` on every pull request (its base) and on
every push to main (the previous main). The rules
(`crates/core/src/content_ids.rs`):

| Change | Result |
|---|---|
| New lesson, exercise, unit, test item… | allowed (`added`) |
| Text, answers, options, explanations corrected | allowed (no id changes) |
| Lesson moved to another unit, test item to another section | allowed, reported (`moved`) |
| An id removed or renamed | **fails** (`removed`) |
| An exercise id kept but its `type` changed | **fails** (`repurposed`) |
| Duplicate ids | fail earlier, in `kabar_core::bundle_check` |

The baseline is the content itself at the base commit (or in a published
bundle), so there is no separate snapshot file to maintain.

## When an id really has to go

Never renumber or reuse ids, and never migrate learner records silently.
If an id must be retired, decide first (in its own PR, with the learner's
approval) what happens to the records that point at it, then list the id in
`content/ids-retired.txt` with the reason. A retired id is never reused.
