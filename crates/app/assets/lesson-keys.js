// The Lesson Player's keys. They are caught on the document, not on the
// player: a tapped tile or option can be disabled under the focus, and the
// next Enter must still check. Enter = check/continue (Shift+Enter is a new
// line in text answers); 1–9 pick an option or a fill-in chip.
//
// Any other button or link keeps its own Enter: the feedback sheet, the
// close button, Back, ↻, a play button, «احفظ تاني» and the summary's
// buttons (#711). Only the answer's own controls and the primary button
// send Enter to the player.
if (window.__kabarLessonKeys) document.removeEventListener("keydown", window.__kabarLessonKeys, true);
window.__kabarLessonKeys = (e) => {
  if (!document.querySelector(".lp") || e.isComposing) return;
  const t = e.target;
  const near = (selector) => (t && t.closest ? t.closest(selector) : null);
  if (near(".lp-feedback, .lp-top")) return;
  const typing = t.tagName === "TEXTAREA" || t.tagName === "INPUT";
  if (e.key === "Enter" && !e.shiftKey) {
    const own = near("button, a[href], [role=button], select, summary");
    if (own && !near(".lp-primary, .lp-choices, .lp-pairs, .lp-bank, .lp-answer-line")) return;
    e.preventDefault();
    dioxus.send("enter");
  } else if (!typing && !e.ctrlKey && !e.metaKey && !e.altKey && /^[1-9]$/.test(e.key)) {
    dioxus.send(e.key);
  }
};
document.addEventListener("keydown", window.__kabarLessonKeys, true);
await new Promise(() => {});
