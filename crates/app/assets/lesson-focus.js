// The keyed step remounts only when the learner moves to another activity.
requestAnimationFrame(() => {
  const stage = document.querySelector('.lp-body > .lp-col[data-lesson-stage]');
  const body = stage?.parentElement;
  if (!body || document.querySelector('dialog[open], [aria-modal="true"]')) return;
  body.scrollTop = 0;
  stage.focus({ preventScroll: true });
});
