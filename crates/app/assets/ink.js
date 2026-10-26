// Kabar ink canvas (Tablet Studio, PLAN §17). Runs inside the WebView via
// `document::eval`; one eval per canvas. Built on the Pen Lab findings:
// desynchronized canvas, coalesced + predicted events, pressure and hover
// from the S Pen, no barrel/eraser buttons (the eraser is a tool), and palm
// touches that arrive while the pen is in range.
//
// Rust -> JS: first {id, size: [w, h] | null, focus, tool, strokes} through
// dioxus.recv; later commands as "kabar-ink" CustomEvents on the wrapper:
//   {cmd: "strokes", strokes}  replace every stroke (remote sync)
//   {cmd: "tool", tool, color, width}  tool: pen | highlighter | brush | eraser
//   {cmd: "focus", rect: [x, y, w, h]}  scroll/zoom to a page rect
//   {cmd: "clear"} | {cmd: "redraw"} | {cmd: "snapshot", ids: [..] | null}
// JS -> Rust (dioxus.send):
//   {type: "stroke", stroke}  a finished stroke (kabar_protocol::ink::InkStroke, doc = "")
//   {type: "erase", ids}      strokes the eraser removed
//   {type: "view", rect}      the visible page rect, after pans and zooms
//   {type: "png", data}       answer to "snapshot": the ink as a PNG data URL, or null

const init = await dioxus.recv();
const wrap = document.getElementById(init.id);
if (!wrap) return;
const FIXED = init.size; // [w, h] page px, or null for an infinite page
const MAX_POINTS = 2000;
const SCALE = 10;

const base = document.createElement("canvas");
const live = document.createElement("canvas");
live.style.pointerEvents = "none";
const items = document.getElementById(init.id + "-items");
// Rust never renders into the surface, so the canvases are ours.
document.getElementById(init.id + "-surface").replaceChildren(base, live);
const ctx = base.getContext("2d", { desynchronized: true });
const liveCtx = live.getContext("2d", { desynchronized: true });

let strokes = []; // InkStroke, page coordinates x SCALE
let view = { ox: 0, oy: 0, z: 1 };
let tool = { tool: "pen", color: "ink", width: 2.2 };
let dpr = window.devicePixelRatio || 1;
let colors = {};
let penSeen = false;
let penInRangeUntil = 0;
let current = null; // stroke being drawn
let erased = [];
const touches = new Map(); // pointerId -> {x, y} screen
let gesture = null;

function readColors() {
  const css = getComputedStyle(wrap);
  for (const k of ["ink", "paper", "dawn", "sawah", "indigo", "gold", "muted"]) {
    colors[k] = css.getPropertyValue("--" + k).trim();
  }
}
const colorOf = (c) => (c && c.startsWith("#") ? c : colors[c] || colors.ink || "#2b3a70");

function uuid7() {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  const ts = Date.now();
  for (let i = 0; i < 6; i++) b[i] = Math.floor(ts / Math.pow(256, 5 - i)) % 256;
  b[6] = (b[6] & 0x0f) | 0x70;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

// ---------- view ----------

function rect() {
  return wrap.getBoundingClientRect();
}

function toPage(e) {
  const r = rect();
  return { x: (e.clientX - r.left) / view.z + view.ox, y: (e.clientY - r.top) / view.z + view.oy };
}

function applyView() {
  for (const [c, cx] of [[base, ctx], [live, liveCtx]]) {
    cx.setTransform(dpr * view.z, 0, 0, dpr * view.z, -view.ox * view.z * dpr, -view.oy * view.z * dpr);
  }
  if (items) items.style.transform = `scale(${view.z}) translate(${-view.ox}px, ${-view.oy}px)`;
}

let viewTimer = 0;
function sendView() {
  clearTimeout(viewTimer);
  viewTimer = setTimeout(() => {
    const r = rect();
    dioxus.send({ type: "view", rect: [view.ox, view.oy, r.width / view.z, r.height / view.z] });
  }, 200);
}

function resize() {
  dpr = window.devicePixelRatio || 1;
  const r = rect();
  for (const c of [base, live]) {
    c.width = Math.round(r.width * dpr);
    c.height = Math.round(r.height * dpr);
  }
  if (FIXED) view = { ox: 0, oy: 0, z: r.width / FIXED[0] };
  redraw();
  sendView();
}

// ---------- drawing ----------

function widthAt(s, i) {
  const p = s.ps && s.ps.length ? s.ps[i] / 255 : 0.5;
  if (s.tool === "highlighter") return s.width * 6;
  if (s.tool === "brush") return s.width * (0.2 + p * 2.6);
  return s.width * (0.45 + p * 1.1);
}

function styleFor(c, s) {
  c.strokeStyle = colorOf(s.color);
  c.fillStyle = c.strokeStyle;
  c.globalAlpha = s.tool === "highlighter" ? 0.3 : 1;
  c.lineCap = "round";
  c.lineJoin = "round";
}

function drawSegment(c, s, i) {
  const a = Math.max(0, i - 1);
  c.lineWidth = widthAt(s, i);
  c.beginPath();
  c.moveTo(s.xs[a] / SCALE, s.ys[a] / SCALE);
  c.lineTo(s.xs[i] / SCALE, s.ys[i] / SCALE);
  c.stroke();
}

function drawStroke(c, s) {
  styleFor(c, s);
  if (s.tool === "highlighter") {
    // One path, so overlaps don't darken.
    c.lineWidth = widthAt(s, 0);
    c.beginPath();
    c.moveTo(s.xs[0] / SCALE, s.ys[0] / SCALE);
    for (let i = 1; i < s.xs.length; i++) c.lineTo(s.xs[i] / SCALE, s.ys[i] / SCALE);
    if (s.xs.length === 1) c.lineTo(s.xs[0] / SCALE + 0.01, s.ys[0] / SCALE);
    c.stroke();
  } else {
    for (let i = 0; i < s.xs.length; i++) drawSegment(c, s, i);
  }
  c.globalAlpha = 1;
}

function bboxOf(s) {
  if (!s._b) {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (let i = 0; i < s.xs.length; i++) {
      x0 = Math.min(x0, s.xs[i]); x1 = Math.max(x1, s.xs[i]);
      y0 = Math.min(y0, s.ys[i]); y1 = Math.max(y1, s.ys[i]);
    }
    const pad = s.width * 6;
    s._b = [x0 / SCALE - pad, y0 / SCALE - pad, x1 / SCALE + pad, y1 / SCALE + pad];
  }
  return s._b;
}

function redraw() {
  readColors();
  applyView();
  for (const c of [ctx, liveCtx]) {
    c.save();
    c.setTransform(1, 0, 0, 1, 0, 0);
    c.clearRect(0, 0, base.width, base.height);
    c.restore();
  }
  const r = rect();
  const vx1 = view.ox + r.width / view.z, vy1 = view.oy + r.height / view.z;
  const visible = (s) => { const b = bboxOf(s); return b[2] >= view.ox && b[0] <= vx1 && b[3] >= view.oy && b[1] <= vy1; };
  // Highlighter under the ink.
  for (const s of strokes) if (s.tool === "highlighter" && visible(s)) drawStroke(ctx, s);
  for (const s of strokes) if (s.tool !== "highlighter" && visible(s)) drawStroke(ctx, s);
}

function clearLive() {
  liveCtx.save();
  liveCtx.setTransform(1, 0, 0, 1, 0, 0);
  liveCtx.clearRect(0, 0, live.width, live.height);
  liveCtx.restore();
}

// ---------- strokes ----------

function begin(e) {
  const p = toPage(e);
  current = {
    id: uuid7(),
    doc: "",
    tool: tool.tool,
    color: tool.color,
    width: tool.width,
    pointer: ["pen", "touch", "mouse"].includes(e.pointerType) ? e.pointerType : "other",
    xs: [], ys: [], ps: [], ts: [],
    started_ms: Date.now(),
    _t0: e.timeStamp,
    _pid: e.pointerId,
  };
  addPoint(e, p);
}

function addPoint(e, p) {
  const s = current;
  s.xs.push(Math.round(p.x * SCALE));
  s.ys.push(Math.round(p.y * SCALE));
  if (s.pointer === "pen" || s.pointer === "touch") s.ps.push(Math.round(Math.min(1, e.pressure || 0.5) * 255));
  s.ts.push(Math.max(0, Math.round(e.timeStamp - s._t0)));
  if (s.tool === "highlighter") {
    clearLive();
    drawStroke(liveCtx, s);
  } else {
    styleFor(ctx, s);
    drawSegment(ctx, s, s.xs.length - 1);
  }
  if (s.xs.length >= MAX_POINTS) {
    // Too long for one op: finish it and carry on from the same point.
    const pid = s._pid, t0 = s._t0;
    finish();
    begin(e);
    current._pid = pid;
    current._t0 = t0;
  }
}

function finish() {
  const s = current;
  current = null;
  clearLive();
  if (!s || !s.xs.length) return;
  if (s.pointer === "mouse" || s.pointer === "other") s.ps = [];
  const out = { ...s };
  delete out._t0; delete out._pid; delete out._b;
  strokes.push(s);
  if (s.tool === "highlighter") redraw();
  dioxus.send({ type: "stroke", stroke: out });
}

function drawPredicted(e) {
  const s = current;
  if (!s || s.tool === "highlighter" || !e.getPredictedEvents) return;
  const predicted = e.getPredictedEvents();
  clearLive();
  if (!predicted.length) return;
  styleFor(liveCtx, s);
  liveCtx.lineWidth = widthAt(s, s.xs.length - 1);
  liveCtx.beginPath();
  liveCtx.moveTo(s.xs[s.xs.length - 1] / SCALE, s.ys[s.ys.length - 1] / SCALE);
  for (const pe of predicted) { const p = toPage(pe); liveCtx.lineTo(p.x, p.y); }
  liveCtx.stroke();
}

// ---------- eraser ----------

function eraseAt(p) {
  const radius = 10 / view.z;
  const keep = [];
  let changed = false;
  for (const s of strokes) {
    if (hits(s, p, radius)) { erased.push(s.id); changed = true; } else keep.push(s);
  }
  if (changed) { strokes = keep; redraw(); }
}

function hits(s, p, radius) {
  const b = bboxOf(s);
  if (p.x < b[0] - radius || p.x > b[2] + radius || p.y < b[1] - radius || p.y > b[3] + radius) return false;
  const reach = radius + s.width;
  for (let i = 0; i < s.xs.length; i++) {
    const a = Math.max(0, i - 1);
    const ax = s.xs[a] / SCALE, ay = s.ys[a] / SCALE, bx = s.xs[i] / SCALE, by = s.ys[i] / SCALE;
    const dx = bx - ax, dy = by - ay, len2 = dx * dx + dy * dy;
    const t = len2 ? Math.max(0, Math.min(1, ((p.x - ax) * dx + (p.y - ay) * dy) / len2)) : 0;
    if (Math.hypot(p.x - (ax + t * dx), p.y - (ay + t * dy)) <= reach) return true;
  }
  return false;
}

function flushErased() {
  if (erased.length) dioxus.send({ type: "erase", ids: erased });
  erased = [];
}

function hover(p) {
  clearLive();
  liveCtx.strokeStyle = colors.muted || "#888";
  liveCtx.lineWidth = 1 / view.z;
  liveCtx.beginPath();
  const r = tool.tool === "eraser" ? 10 / view.z : Math.max(1.5, tool.width) / view.z;
  liveCtx.arc(p.x, p.y, r, 0, Math.PI * 2);
  liveCtx.stroke();
}

// ---------- pan and zoom (infinite pages only) ----------

function zoomAt(sx, sy, factor) {
  const r = rect();
  const px = (sx - r.left) / view.z + view.ox, py = (sy - r.top) / view.z + view.oy;
  view.z = Math.max(0.25, Math.min(4, view.z * factor));
  view.ox = px - (sx - r.left) / view.z;
  view.oy = py - (sy - r.top) / view.z;
  redraw();
  sendView();
}

function panBy(dx, dy) {
  view.ox -= dx / view.z;
  view.oy -= dy / view.z;
  redraw();
  sendView();
}

function gestureStart() {
  const pts = [...touches.values()];
  gesture = {
    cx: pts.reduce((a, p) => a + p.x, 0) / pts.length,
    cy: pts.reduce((a, p) => a + p.y, 0) / pts.length,
    d: pts.length > 1 ? Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y) : 0,
  };
}

function gestureMove() {
  const pts = [...touches.values()];
  const cx = pts.reduce((a, p) => a + p.x, 0) / pts.length;
  const cy = pts.reduce((a, p) => a + p.y, 0) / pts.length;
  panBy(cx - gesture.cx, cy - gesture.cy);
  if (pts.length > 1 && gesture.d > 0) {
    const d = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
    zoomAt(cx, cy, d / gesture.d);
    gesture.d = d;
  }
  gesture.cx = cx;
  gesture.cy = cy;
}

// ---------- pointer events ----------

const isPalm = (e) =>
  e.pointerType === "touch" && (performance.now() < penInRangeUntil || (penSeen && (e.width > 48 || e.height > 48)));

function onDown(e) {
  e.preventDefault();
  if (e.pointerType === "pen") { penSeen = true; penInRangeUntil = performance.now() + 600; }
  if (isPalm(e)) return;
  base.setPointerCapture?.(e.pointerId);
  if (e.pointerType === "touch") {
    touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
    // With a pen around, fingers move the page; otherwise one finger writes
    // and a second one turns the stroke into a pan or pinch.
    if (!FIXED && (penSeen || touches.size > 1)) {
      if (current && current.pointer === "touch") { current = null; redraw(); clearLive(); }
      gestureStart();
      return;
    }
    if (FIXED && penSeen) return;
  }
  if (e.button === 1 && !FIXED) { gesture = { mouse: true, cx: e.clientX, cy: e.clientY }; return; }
  if (tool.tool === "eraser" || (e.buttons & 32) !== 0) { erased = []; eraseAt(toPage(e)); current = { erasing: e.pointerId }; return; }
  begin(e);
}

function onMove(e) {
  if (e.pointerType === "pen") penInRangeUntil = performance.now() + 600;
  if (isPalm(e)) return;
  if (e.pointerType === "touch" && touches.has(e.pointerId)) {
    touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (gesture) return gestureMove();
  }
  if (gesture && gesture.mouse) { panBy(e.clientX - gesture.cx, e.clientY - gesture.cy); gesture.cx = e.clientX; gesture.cy = e.clientY; return; }
  const events = e.getCoalescedEvents && e.getCoalescedEvents().length ? e.getCoalescedEvents() : [e];
  if (current && current.erasing === e.pointerId) { for (const ce of events) eraseAt(toPage(ce)); return; }
  if (!current || current._pid !== e.pointerId) {
    if (e.pointerType === "pen" && !current) hover(toPage(e));
    return;
  }
  for (const ce of events) if (current) addPoint(ce, toPage(ce));
  drawPredicted(e);
}

function onUp(e) {
  if (e.pointerType === "touch") {
    touches.delete(e.pointerId);
    if (gesture) { if (touches.size) gestureStart(); else gesture = null; return; }
  }
  if (gesture && gesture.mouse) { gesture = null; return; }
  if (current && current.erasing === e.pointerId) { current = null; flushErased(); return; }
  if (current && current._pid === e.pointerId) finish();
}

base.addEventListener("pointerdown", onDown);
base.addEventListener("pointermove", onMove);
base.addEventListener("pointerup", onUp);
base.addEventListener("pointercancel", (e) => { if (current && current._pid === e.pointerId) { current = null; redraw(); } onUp(e); });
base.addEventListener("pointerleave", (e) => { if (e.pointerType === "pen" && !current) clearLive(); });
base.addEventListener("contextmenu", (e) => e.preventDefault());
base.addEventListener("wheel", (e) => {
  if (FIXED) return;
  e.preventDefault();
  if (e.ctrlKey || e.metaKey) zoomAt(e.clientX, e.clientY, Math.exp(-e.deltaY / 300));
  else panBy(-e.deltaX, -e.deltaY);
}, { passive: false });
new ResizeObserver(resize).observe(wrap);

// ---------- snapshot for the vision tasks ----------

function snapshot(ids) {
  const pick = ids ? strokes.filter((s) => ids.includes(s.id)) : strokes;
  if (!pick.length) return null;
  let b = [Infinity, Infinity, -Infinity, -Infinity];
  for (const s of pick) {
    const sb = bboxOf(s);
    b = [Math.min(b[0], sb[0]), Math.min(b[1], sb[1]), Math.max(b[2], sb[2]), Math.max(b[3], sb[3])];
  }
  const pad = 16, w = b[2] - b[0] + 2 * pad, h = b[3] - b[1] + 2 * pad;
  const k = Math.min(1024 / Math.max(w, h), 2);
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(w * k));
  c.height = Math.max(1, Math.round(h * k));
  const x = c.getContext("2d");
  x.fillStyle = "#ffffff";
  x.fillRect(0, 0, c.width, c.height);
  x.setTransform(k, 0, 0, k, (pad - b[0]) * k, (pad - b[1]) * k);
  // Dark ink on white reads best for the models, whatever the theme.
  const saved = colors;
  colors = { ink: "#1b1b1b", dawn: "#b04a2f", sawah: "#2f6b3a", indigo: "#2b3a70", gold: "#7a5600" };
  for (const s of pick) drawStroke(x, s);
  colors = saved;
  return c.toDataURL("image/png");
}

// ---------- commands from Rust ----------

function command(m) {
  if (!m) return;
  if (m.cmd === "strokes") { strokes = m.strokes || []; redraw(); }
  else if (m.cmd === "tool") { tool = { tool: m.tool, color: m.color, width: m.width }; }
  else if (m.cmd === "clear") { strokes = []; redraw(); }
  else if (m.cmd === "redraw") redraw();
  else if (m.cmd === "snapshot") dioxus.send({ type: "png", data: snapshot(m.ids) });
  else if (m.cmd === "focus" && !FIXED) {
    const [x, y, w, h] = m.rect, r = rect();
    view.z = Math.max(0.25, Math.min(2, Math.min(r.width / (w + 80), r.height / (h + 80))));
    view.ox = x + w / 2 - r.width / view.z / 2;
    view.oy = y + h / 2 - r.height / view.z / 2;
    redraw();
    sendView();
  }
}

if (init.tool) command({ cmd: "tool", ...init.tool });
strokes = init.strokes || [];
resize();
if (init.focus) command({ cmd: "focus", rect: init.focus });
wrap.addEventListener("kabar-ink", (e) => command(e.detail));

// Keep this eval alive for as long as the canvas is on screen.
await new Promise(() => {});
