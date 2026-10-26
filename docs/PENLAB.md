# Pen Lab findings

Reports live in `fixtures/penlab/` (one JSON per device and date) and are
parsed by a test in `crates/protocol/src/pen.rs`.

## Galaxy S24 Ultra (SM-S928B), Android 17, WebView Chrome 155 — 2026-10-26

| Signal | Result | What it means for ink |
|---|---|---|
| Pressure | 426 distinct levels, 0.0002–0.62 | Full pressure curve available; the learner writes lightly, so map 0–0.6 to the full width range. |
| Tilt / hover | yes / yes | Hover preview over words (Reader) and tilt-shaped brush are possible. |
| Twist, eraser | no / no | Expected: the S24 S Pen has neither. |
| Barrel button | not seen, twice (second run held it) | Samsung reserves it for Air Actions; don't rely on it. Every pen-button action gets an on-screen control. |
| Palm touches with pen in range | 13 in the palm run, all flagged `touch:palm`, none drew | Our palm filter works; keep ignoring touch while the pen is in range. |
| Pen move rate | ~60 Hz | WebView delivers pen moves once per frame; the digitizer is far faster. |
| Coalesced points per event | 1.0 | Chrome's coalesced API returns no extra samples here. |
| Predicted points per event | ~0.96 | Prediction works; use it to draw ahead of the tip. |
| Input delay | p50 8.6 ms, p95 21 ms | Good. |
| Frame delay | p50 1 ms, p95 19 ms | Desynchronized canvas is effective. |

**Decision for the web ink layer:** desynchronized 2D canvas, draw predicted
points as a provisional tail that's replaced on the next event, and smooth the
60 Hz samples (centripetal Catmull-Rom) so curves stay round.

**Learner's verdict:** the ink feels attached to the pen tip. The web ink
layer stays; the native Kotlin overlay (MotionEvent historical samples, Jetpack
Ink front-buffered rendering) is kept only as a phased option if a later device
or heavier canvas makes it trail.

Second run (`s24-ultra-2026-10-26-palm.json`): same rates (60.9 Hz, input
delay p50 7.8 ms), pressure max 0.67.

## Galaxy Tab S11

Not tested yet.
