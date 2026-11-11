// Ctrl/⌘+K opens quick search anywhere (also while typing, like every Mac
// app); inside it ↑/↓ move between results, Enter in the field opens the
// first one, Tab stays inside and Esc closes it.
// Adapted from batna-app main: crates/app/src/shortcuts/mod.rs (KEYS_JS).
if (window.__kabarSearchKeys) document.removeEventListener("keydown", window.__kabarSearchKeys, true);
window.__kabarSearchKeys = e => {
  if (e.defaultPrevented || e.isComposing) return;
  const mod = e.metaKey || e.ctrlKey;
  if (mod && !e.altKey && !e.shiftKey && e.code === "KeyK") {
    e.preventDefault();
    if (!e.repeat) dioxus.send("open");
    return;
  }
  const box = document.querySelector('.quick-search[role="dialog"]');
  if (!box || mod || e.altKey) return;
  const t = e.target;
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopImmediatePropagation();
    dioxus.send("close");
    return;
  }
  if (e.key === "Tab") {
    const all = [...box.querySelectorAll("button:not(:disabled), input:not(:disabled)")];
    const first = all[0], last = all[all.length - 1];
    if (all.length && (e.shiftKey ? document.activeElement === first : document.activeElement === last)) {
      e.preventDefault();
      (e.shiftKey ? last : first).focus();
    }
    return;
  }
  // Keys typed in search belong to search: the lesson or review screen
  // underneath must not grade, reveal or answer with them.
  if (box.contains(t)) e.stopImmediatePropagation();
  if (e.shiftKey) return;
  const rows = [...box.querySelectorAll(".qs-result")];
  const at = rows.indexOf(document.activeElement);
  if ((e.key === "ArrowDown" || e.key === "ArrowUp") && rows.length) {
    e.preventDefault();
    e.stopImmediatePropagation();
    const step = e.key === "ArrowDown" ? 1 : -1;
    rows[at < 0 ? (step > 0 ? 0 : rows.length - 1) : (at + step + rows.length) % rows.length].focus();
    return;
  }
  if (e.key === "Enter" && t?.matches?.(".qs-field") && rows.length) {
    e.preventDefault();
    e.stopImmediatePropagation();
    rows[0].click();
  }
};
document.addEventListener("keydown", window.__kabarSearchKeys, true);
await new Promise(() => {});
