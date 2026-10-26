# Pen Lab findings

Reports live in `fixtures/penlab/` (one JSON per device and date) and are
parsed by a test in `crates/protocol/src/pen.rs`.

## Galaxy S24 Ultra (SM-S928B), Android 17, WebView Chrome 155 — 2026-10-26

| Signal | Result | What it means for ink |
|---|---|---|
| Pressure | 426 distinct levels, 0.0002–0.62 | Full pressure curve available; the learner writes lightly, so map 0–0.6 to the full width range. |
| Tilt / hover | yes / yes | Hover preview over words (Reader) and tilt-shaped brush are possible. |
| Twist, eraser | no / no | Expected: the S24 S Pen has neither. |
| Barrel button | not seen | Re-test while holding the button; Samsung may reserve it for Air Actions. |
| Palm touches with pen in range | 0 | OS palm rejection works, or the palm wasn't tested; re-test. |
| Pen move rate | ~60 Hz | WebView delivers pen moves once per frame; the digitizer is far faster. |
| Coalesced points per event | 1.0 | Chrome's coalesced API returns no extra samples here. |
| Predicted points per event | ~0.96 | Prediction works; use it to draw ahead of the tip. |
| Input delay | p50 8.6 ms, p95 21 ms | Good. |
| Frame delay | p50 1 ms, p95 19 ms | Desynchronized canvas is effective. |

**Decision for the web ink layer:** desynchronized 2D canvas, draw predicted
points as a provisional tail that's replaced on the next event, and smooth the
60 Hz samples (centripetal Catmull-Rom) so curves stay round.

**Fallback (phased, not dropped):** if the learner reports the ink trailing the
tip, add a native Kotlin ink overlay (MotionEvent historical samples at full
digitizer rate, Jetpack Ink front-buffered rendering) and hand finished strokes
to Rust. Decide after the learner's "does it feel attached" answer.

## Galaxy Tab S11

Not tested yet.
