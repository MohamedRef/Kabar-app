// The Settings modal owns these listeners until it is removed. Reopening
// installs a fresh boundary rather than keeping a stale evaluation channel.
(() => {
  const dialog = document.querySelector('.settings-window');
  if (!dialog || dialog.dataset.focusReady) return;
  dialog.dataset.focusReady = 'true';
  const previous = document.activeElement;
  const locked = [];
  for (let node = dialog; node.parentElement; node = node.parentElement) {
    for (const sibling of node.parentElement.children) {
      if (sibling === node || sibling.matches('style, script, .settings-backdrop')) continue;
      locked.push([sibling, sibling.inert]);
      sibling.inert = true;
    }
  }
  const visible = (el) => el.getClientRects().length > 0 && !el.closest('[hidden], [inert]');
  const keydown = (event) => {
    if (event.key !== 'Tab' && event.key !== 'Escape') return;
    const top = [...document.querySelectorAll('[role="dialog"]')].filter(visible).at(-1) || dialog;
    // Independent sheets (e.g. an update confirmation) manage themselves.
    if (top !== dialog && !dialog.contains(top)) return;
    if (event.key === 'Escape') {
      const back = [...top.querySelectorAll('[data-back="true"]')].filter(visible).at(-1);
      if (back) {
        event.preventDefault();
        event.stopImmediatePropagation();
        back.click();
      }
      return;
    }
    const targets = [...top.querySelectorAll('button, a[href], input, select, textarea, [tabindex]')]
      .filter((el) => !el.disabled && el.tabIndex >= 0 && visible(el));
    const first = targets[0];
    const last = targets.at(-1);
    const active = document.activeElement;
    if (!first) {
      event.preventDefault();
      dialog.focus();
    } else if (event.shiftKey && (active === first || !targets.includes(active))) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (active === last || !targets.includes(active))) {
      event.preventDefault();
      first.focus();
    }
  };
  document.addEventListener('keydown', keydown, true);
  dialog.focus({ preventScroll: true });
  const observer = new MutationObserver(() => {
    if (dialog.isConnected) return;
    observer.disconnect();
    document.removeEventListener('keydown', keydown, true);
    for (const [element, wasInert] of locked) element.inert = wasInert;
    if (previous?.isConnected && !previous.closest('[inert]')) previous.focus({ preventScroll: true });
  });
  observer.observe(document.body, { childList: true, subtree: true });
})();
