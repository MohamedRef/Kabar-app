// Pen Lab ink + probe. Runs inside the WebView via `document::eval`.
// Draws ink with the lowest-latency path the WebView offers and streams
// every pointer event to Rust as a sample (see kabar_protocol::pen).
//
// Messages to Rust: {type: "info", info} once, then {type: "samples", samples} batches.
// Rust clears the page by dispatching a "kabar-clear" event on #penlab-ink.

const wrap = document.getElementById("penlab-ink");
if (!wrap) return;

const ink = document.createElement("canvas");
const live = document.createElement("canvas"); // predicted segments, redrawn every event
live.style.pointerEvents = "none";
wrap.replaceChildren(ink, live);

const ctx = ink.getContext("2d", { desynchronized: true, alpha: false });
const liveCtx = live.getContext("2d", { desynchronized: true });
const attrs = ctx.getContextAttributes ? ctx.getContextAttributes() : {};
const probe = typeof PointerEvent !== "undefined" ? PointerEvent.prototype : {};

const themeVars = getComputedStyle(document.documentElement);
const INK = themeVars.getPropertyValue("--ink").trim() || "#2b3a70";
const PAPER = themeVars.getPropertyValue("--paper").trim() || "#fffdf8";

function resize() {
  const dpr = window.devicePixelRatio || 1;
  const r = wrap.getBoundingClientRect();
  for (const c of [ink, live]) {
    c.width = Math.round(r.width * dpr);
    c.height = Math.round(r.height * dpr);
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  liveCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
  clear();
}

function clear() {
  ctx.fillStyle = PAPER;
  ctx.fillRect(0, 0, ink.width, ink.height);
  liveCtx.clearRect(0, 0, live.width, live.height);
}

dioxus.send({
  type: "info",
  info: {
    user_agent: navigator.userAgent,
    device_pixel_ratio: window.devicePixelRatio || 1,
    viewport_w: window.innerWidth,
    viewport_h: window.innerHeight,
    desynchronized_canvas: !!attrs.desynchronized,
    coalesced_api: "getCoalescedEvents" in probe,
    predicted_api: "getPredictedEvents" in probe,
  },
});

const strokes = new Map(); // pointerId -> last point
let penInRangeUntil = 0;
let pending = [];
let awaitingFrame = [];
let frameRequested = false;

function point(e) {
  const r = ink.getBoundingClientRect();
  return { x: e.clientX - r.left, y: e.clientY - r.top, p: e.pressure };
}

function width(e) {
  const p = e.pointerType === "pen" ? (e.pressure || 0.5) : 0.5;
  return 0.6 + p * 3.4;
}

function segment(c, a, b, w, color) {
  c.strokeStyle = color;
  c.lineWidth = w;
  c.lineCap = "round";
  c.lineJoin = "round";
  c.beginPath();
  c.moveTo(a.x, a.y);
  c.lineTo(b.x, b.y);
  c.stroke();
}

function record(kind, e, coalesced, predicted) {
  const s = {
    kind,
    pointer: ["pen", "touch", "mouse"].includes(e.pointerType) ? e.pointerType : "other",
    t_event: e.timeStamp,
    t_handled: performance.now(),
    t_frame: null,
    pressure: e.pressure || 0,
    tilt_x: e.tiltX || 0,
    tilt_y: e.tiltY || 0,
    twist: e.twist || 0,
    buttons: e.buttons || 0,
    coalesced,
    predicted,
  };
  pending.push(s);
  awaitingFrame.push(s);
  if (!frameRequested) {
    frameRequested = true;
    requestAnimationFrame(() => {
      const t = performance.now();
      for (const x of awaitingFrame) x.t_frame = t;
      awaitingFrame = [];
      frameRequested = false;
    });
  }
}

function isPalm(e) {
  return e.pointerType === "touch" && performance.now() < penInRangeUntil;
}

function onDown(e) {
  e.preventDefault();
  if (e.pointerType === "pen") penInRangeUntil = performance.now() + 500;
  if (isPalm(e)) return record("palm", e, null, null);
  ink.setPointerCapture?.(e.pointerId);
  strokes.set(e.pointerId, point(e));
  const erasing = (e.buttons & 32) !== 0;
  const p = point(e);
  segment(ctx, p, p, width(e), erasing ? PAPER : INK);
  record("down", e, null, null);
}

function onMove(e) {
  const coalesced = e.getCoalescedEvents ? e.getCoalescedEvents() : null;
  const predicted = e.getPredictedEvents ? e.getPredictedEvents() : null;
  const nc = coalesced ? coalesced.length : null;
  const np = predicted ? predicted.length : null;

  if (e.pointerType === "pen") penInRangeUntil = performance.now() + 500;
  if (isPalm(e)) return record("palm", e, nc, np);

  const last = strokes.get(e.pointerId);
  if (!last) {
    // In range but not touching: pen hover (mouse moves are ignored).
    if (e.pointerType === "pen") record("hover", e, nc, np);
    return;
  }

  const erasing = (e.buttons & 32) !== 0;
  const color = erasing ? PAPER : INK;
  let prev = last;
  for (const ce of coalesced && coalesced.length ? coalesced : [e]) {
    const p = point(ce);
    segment(ctx, prev, p, erasing ? 18 : width(ce), color);
    prev = p;
  }
  strokes.set(e.pointerId, prev);

  liveCtx.clearRect(0, 0, live.width, live.height);
  if (predicted && predicted.length && !erasing) {
    let a = prev;
    for (const pe of predicted) {
      const b = point(pe);
      segment(liveCtx, a, b, width(pe), INK);
      a = b;
    }
  }
  record("move", e, nc, np);
}

function onUp(kind) {
  return (e) => {
    if (isPalm(e)) return record("palm", e, null, null);
    if (strokes.delete(e.pointerId)) record(kind, e, null, null);
    liveCtx.clearRect(0, 0, live.width, live.height);
  };
}

ink.addEventListener("pointerdown", onDown);
ink.addEventListener("pointermove", onMove);
ink.addEventListener("pointerup", onUp("up"));
ink.addEventListener("pointercancel", onUp("cancel"));
ink.addEventListener("pointerenter", (e) => { if (e.pointerType === "pen") record("enter", e, null, null); });
ink.addEventListener("pointerleave", (e) => { if (e.pointerType === "pen") record("leave", e, null, null); });
ink.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("resize", resize);
resize();

setInterval(() => {
  // Only send samples whose frame time is known (or that are old enough).
  const now = performance.now();
  const ready = pending.filter((s) => s.t_frame !== null || now - s.t_handled > 500);
  if (!ready.length) return;
  pending = pending.filter((s) => !ready.includes(s));
  dioxus.send({ type: "samples", samples: ready });
}, 250);

wrap.addEventListener("kabar-clear", clear);

// Keep this eval alive for as long as the page is open.
await new Promise(() => {});
