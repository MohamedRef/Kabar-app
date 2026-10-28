// Kabar word sheet gestures (crates/app/src/sheet.rs).
//   handle : drag freely; let go near a stop (peek / half / near full) and
//            it settles there; fling up or down to the next stop; drag or
//            fling below the peek to close. Double-tap toggles half ↔ full.
//            Arrow keys step between stops, Home/End jump to the ends,
//            Enter toggles half ↔ full.
//   body   : scrolled to the top, pulling down drags the sheet too.
// The size is a fraction of the window height (--sheet-f), remembered for
// the next sheet. A sheet only resizes while docked at the bottom (fixed);
// the Reader's side column on wide windows is left alone.
// Listeners are delegated from document, installed once.
if (!window.__kabarSheetInit) {
  window.__kabarSheetInit = true;
  const STOPS = [0.32, 0.56, 0.9];
  const MIN = 0.12; // let go below this and the sheet closes
  const MAX = 0.94;
  const PULL = 0.05; // a stop this close catches the sheet
  const FLING = 0.5; // px per ms
  const KEY = "kabar.sheet.f";

  let last = 0.56;
  try {
    const v = Number(localStorage.getItem(KEY));
    if (v >= STOPS[0] && v <= MAX) last = v;
  } catch (_) {}
  const remember = (f) => {
    last = f;
    try { localStorage.setItem(KEY, String(f)); } catch (_) {}
  };

  const vh = () => window.innerHeight || document.documentElement.clientHeight;
  const docked = (el) => getComputedStyle(el).position === "fixed";
  const sheets = () => [...document.querySelectorAll(".bsheet")];
  const clamp = (f) => Math.min(MAX, Math.max(0.05, f));
  // The fraction the sheet really covers (short content stays short).
  const shown = (el) => el.offsetHeight / vh();
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Room under the page so its last lines can scroll above the sheet.
  const room = () => {
    const open = sheets().find((el) => docked(el) && !el.classList.contains("closing"));
    const root = document.documentElement;
    if (open) {
      root.dataset.sheet = "open";
      root.style.setProperty("--sheet-room", open.offsetHeight + "px");
    } else {
      delete root.dataset.sheet;
      root.style.removeProperty("--sheet-room");
      document.querySelectorAll("[data-sheet-room]").forEach((b) => delete b.dataset.sheetRoom);
    }
  };

  const setF = (el, f, settle) => {
    el.classList.toggle("settling", !!settle && !reduced());
    el.style.setProperty("--sheet-f", clamp(f).toFixed(4));
    if (settle) setTimeout(room, 260);
    else room();
  };

  const close = (el) => {
    if (el.classList.contains("closing")) return;
    const btn = el.querySelector("[data-sheet-close]");
    if (reduced()) {
      btn?.click();
      return;
    }
    el.classList.add("closing");
    room();
    setTimeout(() => btn?.click(), 170);
  };

  // Settle after a drag: fling to the next stop, else stay where let go
  // unless a stop is near. Below the floor, close.
  const settle = (el, f, v) => {
    if (v > FLING) {
      const lower = [...STOPS].reverse().find((s) => s < f - 0.02);
      if (lower === undefined) return close(el);
      return land(el, lower);
    }
    if (v < -FLING) {
      const higher = STOPS.find((s) => s > f + 0.02);
      return land(el, higher ?? MAX);
    }
    if (f < MIN) return close(el);
    const near = STOPS.find((s) => Math.abs(s - f) <= PULL);
    land(el, near ?? f);
  };
  const land = (el, f) => {
    setF(el, f, true);
    remember(clamp(f));
  };

  // ---- dragging: pointer on the handle, touch on the body ----
  let drag = null; // { el, y0, f0, t, y, v, moved }

  const start = (el, y) => {
    drag = { el, y0: y, f0: shown(el), y, t: performance.now(), v: 0, moved: false };
    el.classList.add("dragging");
    el.classList.remove("settling");
  };
  const move = (y) => {
    const now = performance.now();
    const dt = Math.max(1, now - drag.t);
    // Smoothed velocity, positive downwards.
    drag.v = 0.7 * ((y - drag.y) / dt) + 0.3 * drag.v;
    drag.y = y;
    drag.t = now;
    if (Math.abs(y - drag.y0) > 4) drag.moved = true;
    setF(drag.el, drag.f0 - (y - drag.y0) / vh(), false);
  };
  const end = () => {
    const d = drag;
    drag = null;
    d.el.classList.remove("dragging");
    if (d.moved) settle(d.el, d.f0 - (d.y - d.y0) / vh(), d.v);
    return d;
  };

  let lastTap = 0;
  document.addEventListener("pointerdown", (e) => {
    const grip = e.target instanceof Element && e.target.closest(".bsheet-grip");
    if (!grip || e.button > 0) return;
    const el = grip.closest(".bsheet");
    if (!el || !docked(el)) return;
    e.preventDefault();
    grip.setPointerCapture?.(e.pointerId);
    start(el, e.clientY);
  });
  document.addEventListener("pointermove", (e) => {
    if (drag && drag.kind !== "touch") move(e.clientY);
  });
  const up = () => {
    if (!drag || drag.kind === "touch") return;
    const d = end();
    if (d.moved) return;
    // A tap on the handle: a double tap toggles half and near full.
    const now = performance.now();
    if (now - lastTap < 350) {
      const f = shown(d.el);
      land(d.el, f < (STOPS[1] + STOPS[2]) / 2 ? STOPS[2] : STOPS[1]);
      lastTap = 0;
    } else {
      lastTap = now;
    }
  };
  document.addEventListener("pointerup", up);
  document.addEventListener("pointercancel", up);

  // Pull-down from the top of the body. Touch events, because the body
  // must keep scrolling natively until the pull starts.
  let pull = null; // { el, body, y0 }
  document.addEventListener(
    "touchstart",
    (e) => {
      const body = e.target instanceof Element && e.target.closest(".bsheet-body");
      const el = body?.closest(".bsheet");
      if (!el || !docked(el) || e.touches.length > 1) return;
      pull = { el, body, y0: e.touches[0].clientY };
    },
    { passive: true },
  );
  document.addEventListener(
    "touchmove",
    (e) => {
      if (!pull) return;
      const y = e.touches[0].clientY;
      if (!drag) {
        if (pull.body.scrollTop > 0 || y - pull.y0 < 8) {
          if (pull.y0 - y > 8) pull = null; // scrolling up: leave it be
          return;
        }
        start(pull.el, y);
        drag.kind = "touch";
      }
      e.preventDefault();
      move(y);
    },
    { passive: false },
  );
  const touchEnd = () => {
    if (drag && drag.kind === "touch") end();
    pull = null;
  };
  document.addEventListener("touchend", touchEnd);
  document.addEventListener("touchcancel", touchEnd);

  // ---- keyboard ----
  document.addEventListener("keydown", (e) => {
    const grip = e.target instanceof Element && e.target.closest(".bsheet-grip");
    if (grip) {
      const el = grip.closest(".bsheet");
      if (!el || !docked(el)) return;
      const f = shown(el);
      let to = null;
      if (e.key === "ArrowUp") to = STOPS.find((s) => s > f + 0.02) ?? MAX;
      else if (e.key === "ArrowDown") to = [...STOPS].reverse().find((s) => s < f - 0.02) ?? STOPS[0];
      else if (e.key === "Home") to = MAX;
      else if (e.key === "End") to = STOPS[0];
      else if (e.key === "Enter" || e.key === " ") to = f < (STOPS[1] + STOPS[2]) / 2 ? STOPS[2] : STOPS[1];
      if (to !== null) {
        e.preventDefault();
        land(el, to);
      }
    }
  });

  // ---- a sheet opens ----
  // One word sheet at a time; it opens at the size last left, and the
  // tapped word is scrolled above it when the sheet would cover it.
  // The word's scrolling box (a lesson's body, the page): it gets room at
  // its end (sheet.css) so even its last line can scroll above the sheet.
  const scroller = (el) => {
    for (let p = el.parentElement; p; p = p.parentElement) {
      const o = getComputedStyle(p).overflowY;
      if (o === "auto" || o === "scroll") return p;
    }
    return document.scrollingElement;
  };
  const reveal = (el) => {
    const word =
      el.closest(".annot")?.querySelector(".annot-w.on") ?? document.querySelector("#reader-doc .w.selected");
    if (!word) return;
    const box = scroller(word);
    box.dataset.sheetRoom = "true";
    const r = word.getBoundingClientRect();
    const top = vh() - el.offsetHeight;
    if (r.bottom <= top - 12 && r.top >= 0) return;
    // Put the word a little above the middle of what the sheet leaves.
    box.scrollBy({ top: r.top + r.height / 2 - top * 0.45, behavior: reduced() ? "auto" : "smooth" });
  };
  const opened = (el) => {
    for (const other of sheets()) {
      if (other !== el && docked(other)) other.querySelector("[data-sheet-close]")?.click();
    }
    if (!docked(el)) return;
    setF(el, last, false);
    requestAnimationFrame(() => requestAnimationFrame(() => {
      room();
      reveal(el);
    }));
  };
  new MutationObserver((records) => {
    let changed = false;
    for (const r of records) {
      for (const n of r.addedNodes) {
        if (!(n instanceof Element)) continue;
        const el = n.classList.contains("bsheet") ? n : n.querySelector(".bsheet");
        if (el) opened(el);
      }
      if (r.removedNodes.length) changed = true;
    }
    if (changed) room();
  }).observe(document.body, { childList: true, subtree: true });
  // Content inside changes height (another word, a breakdown opened).
  if (window.ResizeObserver) {
    const ro = new ResizeObserver(() => room());
    new MutationObserver(() => sheets().forEach((el) => ro.observe(el))).observe(document.body, {
      childList: true,
      subtree: true,
    });
  }
  window.addEventListener("resize", room);
}
