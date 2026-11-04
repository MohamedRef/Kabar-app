# Testing and coverage

Kabar aims for **at least 90% line coverage** of the Rust workspace (Mohamed, 2026-11-04). CI's `coverage` job
(`.github/workflows/ci.yml`) fails a PR whose line coverage is below `scripts/coverage-floor`. The floor only
goes up: it started at the coverage of main on 2026-11-04 (66%) and is raised as tests land, until it reaches 90.

## Running it

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
just coverage            # per-file summary + total, checks the floor
just coverage --html     # also target/llvm-cov/html/index.html
```

`scripts/coverage.sh` runs `cargo test --workspace` and the browser-fixture exporters (`export_*`, ignored in a plain
`cargo test`; the browser jobs run them too) under cargo-llvm-cov. Code that only builds for wasm32 or Android is
not in the native build, so it is not counted; the Worker's wasm code is tested by `crates/server/tests/e2e.mjs`.

## What a good test here looks like

- It checks behaviour a learner or the data would notice: rendered text, a state change, a stored row, an error.
  A test that only runs lines without asserting anything does not count.
- UI components are rendered with Dioxus SSR in every meaningful state (empty, filled, error, Arabic/RTL) and the
  HTML is asserted on (see `crates/app/src/ui/tests.rs`).
- Event handlers do not run under SSR. When a handler holds real logic, move the logic into a plain function and
  unit-test that.
- No coverage exclusions (`#[coverage(off)]`, ignore regexes, cfg tricks) to lift the number.
- Tests never call live APIs; AI parsing is tested against recorded fixtures in `fixtures/`.

## Raising the floor

When main's coverage is a whole point or more above the floor, raise `scripts/coverage-floor` to the new whole number
in a housekeeping PR. Never lower it; if a change truly cannot be tested natively, say so in its PR.
