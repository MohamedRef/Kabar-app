# Teach Kabar fixtures

Inputs for `kabar_core::learn` tests (docs/SPEC.md §26). No live calls.

- `wiktionary.<word>.json`: answers of Wiktionary's definition API
  (`https://en.wiktionary.org/api/rest_v1/page/definition/<word>`). Hand-written
  in the API's documented shape because the cloud sandbox can't reach
  Wikimedia; replace with a recorded answer from the Mac (see docs/MAC-TODO.md).

Model replies for the `learn_entry` task live with the other router fixtures in
`fixtures/ai/learn_entry.*.json`.
- `taught.<word>.json`: a stored `TaughtEntry` (the `taught_entries` sync
  payload), used by the app's store tests.
