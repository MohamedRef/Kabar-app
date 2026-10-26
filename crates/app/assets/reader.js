// Kabar Reader input layer: one input model on every device (SPEC §3).
//   peek  : mouse hover · pen hover · finger tap
//   open  : mouse click · pen tap · finger long-press
//   phrase: drag-select (mouse, finger) · pen loop around words (lasso)
//   save  : right-click → Save · pen highlight stroke · long-press → Save
// Words are spans with data-i inside #reader-doc; the doc re-renders, so all
// listeners are delegated from document.
// Each mount of the Reader runs this again: the listeners are installed once,
// and messages go to the newest mount's channel.
window.__kabarReaderSend = (m) => dioxus.send(m);
const send = (m) => window.__kabarReaderSend(m);

if (!window.__kabarReaderInit) {
  window.__kabarReaderInit = true;
  const ROOT = "reader-doc";
  const root = () => document.getElementById(ROOT);
  const inDoc = (e) => e.target instanceof Element && e.target.closest("#" + ROOT);
  const wordAt = (x, y) => document.elementFromPoint(x, y)?.closest?.("[data-i]");
  const idx = (el) => Number(el.dataset.i);

  let penInRange = false;
  let peekTimer = 0;
  let lastPeek = -1;
  let press = null; // { id, type, x, y, t, moved, pts, el }
  let longTimer = 0;
  let suppressClick = false;

  // Where to float the peek card: below the word, or above it near the bottom
  // of the window. `start` is measured from the inline-start edge (RTL-aware).
  function rectOf(el) {
    const r = el.getBoundingClientRect();
    const rtl = getComputedStyle(document.documentElement.querySelector(".app") || el).direction === "rtl";
    const raw = rtl ? innerWidth - r.right : r.left;
    const start = Math.max(8, Math.min(raw, innerWidth - 340));
    return r.bottom > innerHeight * 0.6
      ? { start, below: false, y: innerHeight - r.top + 6 }
      : { start, below: true, y: r.bottom + 6 };
  }

  function peek(el) {
    const i = idx(el);
    if (i === lastPeek) return;
    lastPeek = i;
    send({ type: "peek", i, ...rectOf(el) });
  }

  function unpeek() {
    if (lastPeek === -1) return;
    lastPeek = -1;
    send({ type: "unpeek" });
  }

  // Highlight a word on both sides of split mode.
  function link(i) {
    document.querySelectorAll("#" + ROOT + " .linked").forEach((n) => n.classList.remove("linked"));
    if (i !== null) document.querySelectorAll(`#${ROOT} [data-i="${i}"]`).forEach((n) => n.classList.add("linked"));
  }

  // --- ink overlay for lasso / highlight strokes ------------------------------
  function overlay() {
    const r = root();
    if (!r) return null;
    let c = r.querySelector("canvas.reader-ink");
    if (!c) {
      c = document.createElement("canvas");
      c.className = "reader-ink";
      c.setAttribute("aria-hidden", "true");
      r.appendChild(c);
    }
    const b = r.getBoundingClientRect();
    const dpr = devicePixelRatio || 1;
    if (c.width !== Math.round(b.width * dpr) || c.height !== Math.round(b.height * dpr)) {
      c.width = Math.round(b.width * dpr);
      c.height = Math.round(b.height * dpr);
    }
    const ctx = c.getContext("2d", { desynchronized: true });
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    return { c, ctx, b };
  }

  function drawTrail(pts) {
    const o = overlay();
    if (!o || pts.length < 2) return;
    const { ctx, b, c } = o;
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.lineWidth = 3;
    ctx.lineCap = ctx.lineJoin = "round";
    ctx.strokeStyle = getComputedStyle(root()).getPropertyValue("--indigo") || "#2b3a70";
    ctx.globalAlpha = 0.6;
    ctx.beginPath();
    ctx.moveTo(pts[0].x - b.left, pts[0].y - b.top);
    for (const p of pts.slice(1)) ctx.lineTo(p.x - b.left, p.y - b.top);
    ctx.stroke();
  }

  function clearTrail() {
    const c = root()?.querySelector("canvas.reader-ink");
    c?.getContext("2d").clearRect(0, 0, c.width, c.height);
  }

  function inside(pt, poly) {
    let hit = false;
    for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
      const a = poly[i], b = poly[j];
      if (a.y > pt.y !== b.y > pt.y && pt.x < ((b.x - a.x) * (pt.y - a.y)) / (b.y - a.y) + a.x) hit = !hit;
    }
    return hit;
  }

  // A stroke that ends near where it started (or encloses area) is a lasso;
  // an open stroke along a line of text is a highlight.
  function finishStroke(pts) {
    clearTrail();
    if (pts.length < 4) return;
    const first = pts[0], last = pts[pts.length - 1];
    const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
    const w = Math.max(...xs) - Math.min(...xs), h = Math.max(...ys) - Math.min(...ys);
    const closed = Math.hypot(last.x - first.x, last.y - first.y) < Math.max(28, 0.35 * Math.max(w, h)) && h > 14;
    const words = [...root().querySelectorAll("[data-i]")].filter((el) => !el.closest(".explain"));
    let hit;
    if (closed) {
      hit = words.filter((el) => {
        const r = el.getBoundingClientRect();
        return inside({ x: r.left + r.width / 2, y: r.top + r.height / 2 }, pts);
      });
    } else {
      hit = words.filter((el) => {
        const r = el.getBoundingClientRect();
        return pts.some((p) => p.x >= r.left - 2 && p.x <= r.right + 2 && p.y >= r.top - 4 && p.y <= r.bottom + 4);
      });
    }
    const tokens = [...new Set(hit.map(idx))].sort((a, b) => a - b);
    if (tokens.length) send({ type: closed ? "phrase" : "highlight", tokens, how: closed ? "lasso" : "stroke" });
  }

  // --- pointer handling --------------------------------------------------------
  document.addEventListener("pointerover", (e) => {
    if (!inDoc(e)) return;
    const el = e.target.closest("[data-i]");
    link(el ? idx(el) : null);
    if (e.pointerType === "pen") {
      penInRange = true;
      const r = root();
      if (r) r.style.touchAction = "none"; // the pen draws instead of scrolling
    }
  });

  document.addEventListener("pointermove", (e) => {
    if (press && e.pointerId === press.id) {
      const moved = Math.hypot(e.clientX - press.x, e.clientY - press.y);
      if (moved > 10) {
        press.moved = true;
        clearTimeout(longTimer);
      }
      if (press.type === "pen" && press.moved) {
        for (const ev of e.getCoalescedEvents?.() ?? [e]) press.pts.push({ x: ev.clientX, y: ev.clientY });
        drawTrail(press.pts);
      }
      return;
    }
    if (!inDoc(e)) return;
    // Hover: mouse, or a pen that is in range but not touching.
    if (e.pointerType === "mouse" || (e.pointerType === "pen" && e.buttons === 0)) {
      const el = wordAt(e.clientX, e.clientY);
      clearTimeout(peekTimer);
      if (el) peekTimer = setTimeout(() => peek(el), e.pointerType === "pen" ? 60 : 140);
    }
  });

  document.addEventListener("pointerleave", (e) => {
    if (e.pointerType === "pen") {
      penInRange = false;
      const r = root();
      if (r) r.style.touchAction = "";
    }
  }, true);

  document.addEventListener("pointerout", (e) => {
    if (!inDoc(e) || e.pointerType === "touch") return;
    const to = e.relatedTarget instanceof Element ? e.relatedTarget : null;
    if (!to?.closest?.("#" + ROOT)) {
      clearTimeout(peekTimer);
      link(null);
    }
  });

  document.addEventListener("pointerdown", (e) => {
    if (!inDoc(e) || e.button > 0) return;
    // Palm rejection: while the pen is in range, ignore touch here.
    if (e.pointerType === "touch" && penInRange) {
      e.preventDefault();
      return;
    }
    press = { id: e.pointerId, type: e.pointerType, x: e.clientX, y: e.clientY, t: performance.now(), moved: false,
              pts: [{ x: e.clientX, y: e.clientY }], el: e.target.closest("[data-i]") };
    if (e.pointerType === "touch" && press.el) {
      const el = press.el;
      longTimer = setTimeout(() => {
        suppressClick = true;
        navigator.vibrate?.(12);
        send({ type: "open", i: idx(el) });
      }, 480);
    }
  });

  document.addEventListener("pointerup", (e) => {
    if (!press || e.pointerId !== press.id) return;
    clearTimeout(longTimer);
    const p = press;
    press = null;
    if (p.type === "pen" && p.moved) {
      suppressClick = true;
      finishStroke(p.pts);
      return;
    }
    if (p.moved || suppressClick) return;
    const el = p.el;
    if (!el) {
      unpeek();
      return;
    }
    suppressClick = true;
    if (p.type === "touch") {
      lastPeek = -1;
      peek(el);
    } else {
      send({ type: "open", i: idx(el) });
    }
  });

  document.addEventListener("pointercancel", () => {
    clearTimeout(longTimer);
    press = null;
    clearTrail();
  });

  document.addEventListener("click", (e) => {
    if (suppressClick) {
      suppressClick = false;
      if (inDoc(e)) e.preventDefault();
    }
  }, true);

  // Right-click → Save (Mac, DeX mouse).
  document.addEventListener("contextmenu", (e) => {
    if (!inDoc(e)) return;
    e.preventDefault();
    const sel = selectedTokens();
    const el = e.target.closest("[data-i]");
    const tokens = sel.length ? sel : el ? [idx(el)] : [];
    if (tokens.length) send({ type: "menu", tokens, start: e.clientX, top: e.clientY });
  });

  // Drag-select (mouse, finger) looks up a phrase.
  function selectedTokens() {
    const sel = getSelection();
    if (!sel || sel.isCollapsed || !root()) return [];
    return [...root().querySelectorAll("[data-i]")]
      .filter((el) => !el.closest(".explain") && sel.containsNode(el, true))
      .map(idx);
  }

  let selTimer = 0;
  document.addEventListener("selectionchange", () => {
    clearTimeout(selTimer);
    selTimer = setTimeout(() => {
      const tokens = [...new Set(selectedTokens())];
      if (tokens.length > 1) send({ type: "phrase", tokens, how: "select" });
    }, 450);
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") send({ type: "escape" });
  });
}

await new Promise(() => {});
