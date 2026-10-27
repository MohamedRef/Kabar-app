// Journey Map pan and zoom. Pointer Events only, so mouse, pen and touch share
// one path: drag with one pointer, pinch with two, wheel/trackpad to zoom.
// Moves only touch one transform attribute on `.jm-viewport` (batched per
// frame), so Dioxus never re-renders while the map moves.
// Called as `(this snippet)(stageId, focusX, focusY)`: the focus point (map
// units) starts centred. Idempotent per element.
(function (stageId, focusX, focusY) {
  const stage = document.getElementById(stageId);
  if (!stage || stage.__kz) return;
  const svg = stage.querySelector("svg");
  const g = svg && svg.querySelector(".jm-viewport");
  if (!g) return;

  const MAX = 12, DRAG_SLOP = 6;
  const vb = svg.viewBox.baseVal;
  // Pins, names and stickers are drawn in design units of viewBox width / 200
  // (`pin_scale` in journey.rs). A tall stage (the Mac and tablet landscape
  // fill the window's height) slices the map, so it shows bigger: `pinFit`
  // (≤ 1) caps a design unit at PIN_PX screen pixels so pins keep the size
  // they have on a short stage, and `meet` is how far the learner may zoom
  // out to see the whole map again.
  const PIN_PX = 5.2;
  function pinFit() {
    const m = svg.getScreenCTM();
    if (!m || !m.a) return 1;
    return Math.min(1, PIN_PX / (m.a * vb.width / 200));
  }
  function meet() {
    const m = svg.getScreenCTM(), r = stage.getBoundingClientRect();
    if (!m || !m.a || !r.width || !r.height) return 1;
    return Math.min(1, Math.min(r.width / vb.width, r.height / vb.height) / m.a);
  }
  const st = { s: 1, x: 0, y: 0 };
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  let raf = 0, anim = 0;

  // The stage's visible rectangle in SVG user units (the SVG uses `slice`,
  // so on narrow stages less than the whole viewBox shows).
  function visible() {
    const r = stage.getBoundingClientRect();
    const a = toSvg(r.left, r.top), b = toSvg(r.right, r.bottom);
    return { x0: Math.min(a.x, b.x), x1: Math.max(a.x, b.x), y0: Math.min(a.y, b.y), y1: Math.max(a.y, b.y) };
  }
  // Keep the map under the stage: no panning into the void. Where the map is
  // smaller than the stage, centre it.
  function clampAxis(t, v0, v1, b0, b1) {
    const hi = v0 - st.s * b0, lo = v1 - st.s * b1;
    return lo > hi ? (lo + hi) / 2 : Math.min(hi, Math.max(lo, t));
  }
  function clamp() {
    st.s = Math.min(MAX, Math.max(meet(), st.s));
    const v = visible();
    st.x = clampAxis(st.x, v.x0, v.x1, vb.x, vb.x + vb.width);
    st.y = clampAxis(st.y, v.y0, v.y1, vb.y, vb.y + vb.height);
  }
  function apply() {
    raf = 0;
    g.setAttribute("transform", `translate(${st.x.toFixed(3)} ${st.y.toFixed(3)}) scale(${st.s.toFixed(4)})`);
    // Pins and labels keep their size; the land and the stops around each
    // place spread out underneath them. Zoomed out past the map's designed
    // scale (whole map in a tall stage), they shrink with it so names stay clear.
    svg.style.setProperty("--inv", Math.min(1, pinFit() / st.s).toFixed(4));
    stage.dataset.zoomed = st.s > 1.01 ? "true" : "false";
  }
  function schedule() {
    clamp();
    if (!raf) raf = requestAnimationFrame(apply);
  }
  function toSvg(cx, cy) {
    const m = svg.getScreenCTM();
    if (!m) return { x: 0, y: 0 };
    const p = new DOMPoint(cx, cy).matrixTransform(m.inverse());
    return { x: p.x, y: p.y };
  }
  function zoomAt(p, factor) {
    const ns = Math.min(MAX, Math.max(meet(), st.s * factor));
    const f = ns / st.s;
    st.x = p.x - (p.x - st.x) * f;
    st.y = p.y - (p.y - st.y) * f;
    st.s = ns;
    schedule();
  }
  // Calm 200 ms glide for buttons and keys (instant with reduced motion).
  function glide(to) {
    cancelAnimationFrame(anim);
    if (reduced()) { Object.assign(st, to); schedule(); return; }
    const from = { ...st }, t0 = performance.now();
    const step = (now) => {
      const k = Math.min(1, (now - t0) / 200), e = 1 - Math.pow(1 - k, 3);
      st.s = from.s + (to.s - from.s) * e;
      st.x = from.x + (to.x - from.x) * e;
      st.y = from.y + (to.y - from.y) * e;
      schedule();
      if (k < 1) anim = requestAnimationFrame(step);
    };
    anim = requestAnimationFrame(step);
  }
  function center() { return { x: vb.x + vb.width / 2, y: vb.y + vb.height / 2 }; }
  function target(factor, p) {
    const ns = Math.min(MAX, Math.max(meet(), st.s * factor)), f = ns / st.s;
    return { s: ns, x: p.x - (p.x - st.x) * f, y: p.y - (p.y - st.y) * f };
  }

  const pointers = new Map();
  let moved = false, last = null;

  function snapshot() {
    const pts = [...pointers.values()];
    if (pts.length === 1) return { mid: pts[0], dist: 0 };
    const [a, b] = pts;
    return { mid: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }, dist: Math.hypot(a.x - b.x, a.y - b.y) };
  }

  stage.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 && e.pointerType === "mouse") return;
    if (e.target.closest(".jm-controls")) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY, x0: e.clientX, y0: e.clientY });
    if (pointers.size === 1) moved = false;
    last = snapshot();
  });
  stage.addEventListener("pointermove", (e) => {
    const p = pointers.get(e.pointerId);
    if (!p) return;
    p.x = e.clientX; p.y = e.clientY;
    if (!moved && (pointers.size > 1 || Math.hypot(p.x - p.x0, p.y - p.y0) > DRAG_SLOP)) {
      moved = true;
      stage.dataset.dragging = "true";
      for (const id of pointers.keys()) { try { stage.setPointerCapture(id); } catch (_) {} }
    }
    if (!moved) return;
    const now = snapshot();
    const a = toSvg(last.mid.x, last.mid.y), b = toSvg(now.mid.x, now.mid.y);
    st.x += b.x - a.x;
    st.y += b.y - a.y;
    if (pointers.size > 1 && last.dist > 0 && now.dist > 0) zoomAt(b, now.dist / last.dist);
    else schedule();
    last = now;
  });
  const end = (e) => {
    if (!pointers.delete(e.pointerId)) return;
    if (pointers.size === 0) delete stage.dataset.dragging;
    else last = snapshot();
  };
  stage.addEventListener("pointerup", end);
  stage.addEventListener("pointercancel", end);
  // A drag is not a tap: stop the click before Dioxus sees it.
  stage.addEventListener("click", (e) => {
    if (moved) { e.stopPropagation(); e.preventDefault(); moved = false; }
  }, true);

  stage.addEventListener("wheel", (e) => {
    e.preventDefault();
    const p = toSvg(e.clientX, e.clientY);
    const px = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
    const dx = e.deltaX * px, dy = e.deltaY * px;
    // Trackpad pinch arrives as ctrl+wheel; a mouse wheel moves in coarse
    // vertical steps. Both zoom. Two-finger trackpad scrolling pans.
    const mouseWheel = e.deltaMode !== 0 || (dx === 0 && Math.abs(dy) >= 40 && Number.isInteger(dy));
    if (e.ctrlKey || mouseWheel) {
      zoomAt(p, Math.exp(-dy * (e.ctrlKey ? 0.01 : 0.002)));
    } else {
      const m = svg.getScreenCTM();
      const k = m ? 1 / m.a : 1;
      st.x -= dx * k; st.y -= dy * k;
      schedule();
    }
  }, { passive: false });

  stage.addEventListener("keydown", (e) => {
    if (e.target.closest(".jm-controls")) return;
    const step = vb.width / 12;
    const keys = {
      "+": () => glide(target(1.5, center())), "=": () => glide(target(1.5, center())),
      "-": () => glide(target(1 / 1.5, center())),
      "0": () => glide({ s: 1, x: 0, y: 0 }),
      ArrowLeft: () => glide({ ...st, x: st.x + step }), ArrowRight: () => glide({ ...st, x: st.x - step }),
      ArrowUp: () => glide({ ...st, y: st.y + step }), ArrowDown: () => glide({ ...st, y: st.y - step }),
    };
    const f = keys[e.key];
    if (f && !e.metaKey && !e.ctrlKey && !e.altKey) { e.preventDefault(); f(); }
  });

  stage.__kz = {
    zoom: (factor) => glide(target(factor, center())),
    reset: () => glide({ s: 1, x: 0, y: 0 }),
  };
  if (typeof focusX === "number" && typeof focusY === "number") {
    const v = visible();
    st.x = (v.x0 + v.x1) / 2 - focusX;
    st.y = (v.y0 + v.y1) / 2 - focusY;
  }
  clamp();
  apply();
  // Window, split-screen or rotation changes: keep the map under the stage.
  new ResizeObserver(() => schedule()).observe(stage);
})
