// Journey Map pan and zoom. Pointer Events only, so mouse, pen and touch share
// one path: drag with one pointer, pinch with two, wheel/trackpad to zoom.
// Moves only touch one transform attribute on `.jm-viewport` (batched per
// frame), so Dioxus never re-renders while the map moves.
// Called as `kabarJourneyPanZoom(stageId)`; idempotent per element.
(function (stageId) {
  const stage = document.getElementById(stageId);
  if (!stage || stage.__kz) return;
  const svg = stage.querySelector("svg");
  const g = svg && svg.querySelector(".jm-viewport");
  if (!g) return;

  const MIN = 1, MAX = 12, DRAG_SLOP = 6;
  const vb = svg.viewBox.baseVal;
  const st = { s: 1, x: 0, y: 0 };
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  let raf = 0, anim = 0;

  function clamp() {
    st.s = Math.min(MAX, Math.max(MIN, st.s));
    // Keep the original view covered by the map: no panning into the void.
    const loX = (vb.x + vb.width) * (1 - st.s), hiX = vb.x * (1 - st.s);
    const loY = (vb.y + vb.height) * (1 - st.s), hiY = vb.y * (1 - st.s);
    st.x = Math.min(hiX, Math.max(loX, st.x));
    st.y = Math.min(hiY, Math.max(loY, st.y));
  }
  function apply() {
    raf = 0;
    g.setAttribute("transform", `translate(${st.x.toFixed(3)} ${st.y.toFixed(3)}) scale(${st.s.toFixed(4)})`);
    // Pins and labels grow gently, not with the full zoom.
    svg.style.setProperty("--inv", (1 / Math.sqrt(st.s)).toFixed(4));
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
    const ns = Math.min(MAX, Math.max(MIN, st.s * factor));
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
    const ns = Math.min(MAX, Math.max(MIN, st.s * factor)), f = ns / st.s;
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
  apply();
})
