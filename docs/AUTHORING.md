# Authoring content

How to add or correct lessons and other content, check it, and see what
devices will get. Everything here is a `kabar-tools` command (also as a
`just` recipe); nothing needs Rust knowledge beyond running them. Content
formats are described in `docs/CONTENT.md`, `docs/PEDAGOGY.md` and the
JSON Schemas in `schemas/`.

## The workflow

| Step | Command | What it does |
|---|---|---|
| 1. Start a lesson | `just content-new bipa1.u01.l09` | Writes `content/lessons/id/bipa1.u01.l09.yaml` from `content/templates/lesson.yaml` (6 exercises, every field to fill marked `TODO(author)`). Refuses an existing, malformed or retired id. |
| 2. Place it | edit `curriculum/indonesian.yaml` (or `javanese.yaml`) | Add `- { id: bipa1.u01.l09, title: …, minutes: … }` under its unit. |
| 3. Write it | edit the YAML | Replace every `TODO(author)`. The bundle check refuses a lesson that still has one, so an unfinished lesson can't ship. |
| 4. Offline packs | `cargo run -p kabar-tools -- packs build` | Refreshes `content/packs/` for the new lines (validate fails until you do). |
| 5. Check everything | `just content-preflight` | Runs every check below and prints one table. |
| 6. See the effect | `just content-diff` | Lists the content files added, changed and removed, as devices will see them. |
| 7. Pull request | — | Add `changes/<topic>.yaml`. CI runs the same checks; a merge to main publishes the content to the preview feed (`docs/RELEASE.md` §5). |

Correcting existing content is steps 3, 5 and 7: fix the text, never the id.

## What preflight checks

| Check | Catches |
|---|---|
| content checks (`kabar-tools validate`) | schemas up to date, glosses, the Bridge, lesson ↔ Bridge, YAML cut by ` #`, speech levels, golden sets, offline pack manifests |
| schemas and relationships (`kabar-content`) | every curriculum, grammar, lesson and test file against its JSON Schema; ids nested under their parents; grammar points that exist; lessons the curriculum lists |
| bundle devices take (`kabar_core::bundle_check`) | what a device checks before it takes a content update: every file unpacks within the size limits, both curricula with unique ids, every lesson and test parses and passes the player's shape rules, the cast, no `TODO(author)` left |
| content ids (`kabar-tools ids check`) | an id that learner records point at removed or repurposed (`docs/CONTENT-IDS.md`) |

Each failing line names the file (or bundle key), the id and the reason.
`--base` (default `origin/main`) is what ids and files are compared with;
`kabar-tools content diff --base-bundle FILE` and `ids check --base-bundle
FILE` compare with a published bundle instead.

## Other useful commands

| Command | Use |
|---|---|
| `cargo run -p kabar-tools -- ids list` | every content id and what it hangs under |
| `cargo run -p kabar-tools -- update bundle` | build the content bundle locally (`dist/bundle/kabar-content.bundle`) |
| `cargo run -p kabar-tools -- update changes --since <commit>` | what the next build's "What's new" will say |
| `cargo test -p kabar-content` | the schema and relationship checks alone |
