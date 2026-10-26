// Card Wall (SPEC §4): drag or flick cards onto the tahu / belum piles with
// the pen, a mouse or a finger. Runs inside the WebView via `document::eval`.
// Movement is transform-only (the `translate` property) for smoothness.
// Palm rejection: touches are ignored while a pen is in range or down.
//
// Messages to Rust: {t: "flip", id} on a tap, {t: "drop", id, pile} on a sort.

if (window.__kabarWall) window.__kabarWall.dispose();

const FLICK_SPEED = 0.6; // px/ms: faster than this counts as a flick
const TAP_SLOP = 8; // px: less movement than this is a tap
const PEN_GRACE = 600; // ms a pen counts as "in range" after its last event
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;

let penUntil = 0;
let drag = null;

const table = () => document.getElementById("card-wall");
const piles = () => [...document.querySelectorAll("#card-wall ~ .piles [data-pile]")];
const center = (r) => [r.left + r.width / 2, r.top + r.height / 2];

function pileAt(x, y) {
  return piles().find((p) => {
    const r = p.getBoundingClientRect();
    return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
  });
}

// The pile the card was thrown toward, if it was thrown fast enough.
function pileByFlick(vx, vy, card) {
  const speed = Math.hypot(vx, vy);
  if (speed < FLICK_SPEED) return null;
  const [cx, cy] = center(card.getBoundingClientRect());
  let best = null;
  let bestCos = 0.5;
  for (const p of piles()) {
    const [px, py] = center(p.getBoundingClientRect());
    const dx = px - cx;
    const dy = py - cy;
    const cos = (dx * vx + dy * vy) / (Math.hypot(dx, dy) * speed || 1);
    if (cos > bestCos) {
      best = p;
      bestCos = cos;
    }
  }
  return best;
}

function highlight(pile) {
  for (const p of piles()) p.toggleAttribute("data-over", p === pile);
}

function place(card, dx, dy) {
  card.style.translate = `${dx}px ${dy}px`;
}

function springBack(card) {
  card.style.translate = "";
}

function notePen(e) {
  if (e.pointerType === "pen") penUntil = e.timeStamp + PEN_GRACE;
}

function onDown(e) {
  notePen(e);
  if (!table()) return dispose();
  const card = e.target.closest("#card-wall .wall-card");
  if (!card || e.target.closest("button") || drag) return;
  if (e.pointerType === "touch" && e.timeStamp < penUntil) return; // palm
  e.preventDefault();
  card.setPointerCapture?.(e.pointerId);
  card.setAttribute("data-dragging", "");
  drag = {
    card,
    pointer: e.pointerId,
    x0: e.clientX,
    y0: e.clientY,
    dx: 0,
    dy: 0,
    moved: false,
    samples: [{ x: e.clientX, y: e.clientY, t: e.timeStamp }],
  };
}

function onMove(e) {
  notePen(e);
  if (!drag || e.pointerId !== drag.pointer) return;
  drag.dx = e.clientX - drag.x0;
  drag.dy = e.clientY - drag.y0;
  if (!drag.moved && Math.hypot(drag.dx, drag.dy) > TAP_SLOP) drag.moved = true;
  if (!drag.moved) return;
  place(drag.card, drag.dx, drag.dy);
  drag.samples.push({ x: e.clientX, y: e.clientY, t: e.timeStamp });
  while (drag.samples.length > 2 && e.timeStamp - drag.samples[0].t > 100) drag.samples.shift();
  highlight(pileAt(e.clientX, e.clientY));
}

function onUp(e) {
  notePen(e);
  if (!drag || e.pointerId !== drag.pointer) return;
  const { card, moved, samples, dx, dy } = drag;
  drag = null;
  card.removeAttribute("data-dragging");
  highlight(null);
  if (e.type === "pointercancel") return springBack(card);
  if (!moved) {
    springBack(card);
    dioxus.send({ t: "flip", id: card.dataset.id });
    return;
  }
  const first = samples[0];
  const dt = Math.max(1, e.timeStamp - first.t);
  const pile =
    pileAt(e.clientX, e.clientY) || pileByFlick((e.clientX - first.x) / dt, (e.clientY - first.y) / dt, card);
  if (!pile) return springBack(card);

  const msg = { t: "drop", id: card.dataset.id, pile: pile.dataset.pile };
  if (reduceMotion) return dioxus.send(msg);
  // Glide onto the pile, then let Rust take the card off the table.
  const [cx, cy] = center(card.getBoundingClientRect());
  const [px, py] = center(pile.getBoundingClientRect());
  card.setAttribute("data-leaving", "");
  place(card, dx + px - cx, dy + py - cy);
  setTimeout(() => dioxus.send(msg), 160);
}

const opts = { passive: false };
document.addEventListener("pointerdown", onDown, opts);
document.addEventListener("pointermove", onMove, opts);
document.addEventListener("pointerup", onUp, opts);
document.addEventListener("pointercancel", onUp, opts);

function dispose() {
  document.removeEventListener("pointerdown", onDown, opts);
  document.removeEventListener("pointermove", onMove, opts);
  document.removeEventListener("pointerup", onUp, opts);
  document.removeEventListener("pointercancel", onUp, opts);
  if (window.__kabarWall?.dispose === dispose) window.__kabarWall = null;
}
window.__kabarWall = { dispose };

// Keep this eval alive while the wall is open.
await new Promise(() => {});
