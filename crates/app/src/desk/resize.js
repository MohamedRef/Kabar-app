// Drag-to-resize panes on the Mac and other mouse-driven windows.
// Adapted from batna-app main: crates/app/src/desk/resize.js.
// The Rust side keeps the saved width (tenths of a rem, device-only); this
// script only gives live feedback and sends one value when a gesture ends:
// "nav:185", or "nav:0" after a double-click puts the default back.
if (window.__kabarResize) {
  for (const [name, fn] of Object.entries(window.__kabarResize)) {
    document.removeEventListener(name, fn, true);
  }
}
const root = document.documentElement;
const fine = () => matchMedia("(hover: hover) and (pointer: fine)").matches;
const remPx = () => parseFloat(getComputedStyle(root).fontSize) || 16;
const info = h => ({
  pane: h.dataset.pane,
  side: h.dataset.side,
  min: Number(h.dataset.min),
  max: Number(h.dataset.max),
  def: Number(h.dataset.default),
  anchor: h.closest(h.dataset.anchor) || document.querySelector(h.dataset.anchor),
});
const bound = (h, tenths) => {
  const i = info(h);
  return Math.min(i.max, Math.max(i.min, Math.round(tenths)));
};
const show = (h, tenths) => {
  root.style.setProperty("--kb-" + h.dataset.pane + "-w", tenths / 10 + "rem");
  h.setAttribute("aria-valuenow", String(tenths));
  h.setAttribute("aria-valuetext", (tenths / 10).toFixed(1) + " rem");
};
const save = (h, tenths) => dioxus.send(h.dataset.pane + ":" + tenths);
let drag = null;
// Width in tenths of a rem for a pointer at clientX.
const widthAt = (h, x) => {
  const { side, anchor } = info(h);
  if (!anchor) return null;
  const b = anchor.getBoundingClientRect();
  const rtl = getComputedStyle(h).direction === "rtl";
  // "start": the pane runs from the anchor's inline-start edge to the
  // pointer (the sidebar); "end": from the pointer to its inline-end edge
  // (the Reader's word panel).
  const fromLeft = (side === "start") !== rtl;
  const px = fromLeft ? x - b.left : b.right - x;
  return (10 * px) / remPx();
};
const pointerdown = e => {
  const h = e.target.closest?.(".kb-resize");
  if (!h || !fine() || e.button !== 0) return;
  drag = { h, id: e.pointerId, moved: false, value: Number(h.getAttribute("aria-valuenow")) };
  h.setPointerCapture?.(e.pointerId);
  root.classList.add("kb-resizing");
  e.preventDefault();
};
const pointermove = e => {
  if (!drag || e.pointerId !== drag.id) return;
  const w = widthAt(drag.h, e.clientX);
  if (w == null) return;
  drag.moved = true;
  drag.value = bound(drag.h, w);
  show(drag.h, drag.value);
};
const end = (e, keep) => {
  if (!drag || e.pointerId !== drag.id) return;
  const { h, moved, value } = drag;
  drag = null;
  root.classList.remove("kb-resizing");
  try { if (h.hasPointerCapture?.(e.pointerId)) h.releasePointerCapture(e.pointerId); } catch (_) {}
  // A plain click doesn't change anything; an interrupted drag isn't saved.
  if (keep && moved && Number.isFinite(value)) save(h, value);
};
const pointerup = e => end(e, true);
const pointercancel = e => end(e, false);
const keydown = e => {
  const h = e.target.closest?.(".kb-resize");
  if (!h || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
  const { side, min, max } = info(h);
  const rtl = getComputedStyle(h).direction === "rtl";
  // Arrows move the divider the way they point: wider or narrower
  // depending on which side of it the pane is.
  const grows = (side === "start") !== rtl ? "ArrowRight" : "ArrowLeft";
  const shrinks = grows === "ArrowRight" ? "ArrowLeft" : "ArrowRight";
  const now = Number(h.getAttribute("aria-valuenow"));
  let next = null;
  if (e.key === grows) next = now + 10;
  else if (e.key === shrinks) next = now - 10;
  else if (e.key === "Home") next = min;
  else if (e.key === "End") next = max;
  if (next == null) return;
  e.preventDefault();
  e.stopPropagation();
  next = bound(h, next);
  show(h, next);
  save(h, next);
};
const dblclick = e => {
  const h = e.target.closest?.(".kb-resize");
  if (!h) return;
  e.preventDefault();
  root.style.removeProperty("--kb-" + h.dataset.pane + "-w");
  const def = info(h).def;
  h.setAttribute("aria-valuenow", String(def));
  h.setAttribute("aria-valuetext", (def / 10).toFixed(1) + " rem");
  dioxus.send(h.dataset.pane + ":0");
};
const listeners = { pointerdown, pointermove, pointerup, pointercancel, keydown, dblclick };
window.__kabarResize = listeners;
for (const [name, fn] of Object.entries(listeners)) document.addEventListener(name, fn, true);
await new Promise(() => {});
