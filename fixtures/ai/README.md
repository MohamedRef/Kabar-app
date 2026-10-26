# AI fixtures

Recorded `EngineReply` JSON (crates/protocol `ai.rs`) for router and parsing
tests. Tests never call live APIs. Name: `<task>.<case>[.<variant>].<engine>.json`.

The replies here are hand-written in the shape real engines return (fences,
preamble, missing fields) until the bake-off on the Mac records real ones;
replace them with recorded replies then, keeping the invalid variants.
