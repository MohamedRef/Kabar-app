// Android Back for the whole app. Every visible data-back="true" control
// participates automatically; Rust only supplies the top-level screen trail.
window.__kabarBackSend = (message) => dioxus.send(message);
if (!window.__kabarBackInit) {
  window.__kabarBackInit = true;
  let screenCanBack = false;
  let waitingForScreen = false;
  let releasing = false;
  let scheduled = false;

  const trapped = () => history.state?.kabarBackTrap === true;
  const visible = (element) =>
    !element.disabled &&
    element.getClientRects().length > 0 &&
    !element.closest('[hidden], [inert], [aria-hidden="true"]') &&
    getComputedStyle(element).visibility !== 'hidden';
  // The last DOM node need not be the frontmost sheet. Prefer the dialog
  // painted on top (its own z-index, or the nearest positioned ancestor),
  // then the last eligible Back control *inside* it.
  const layer = (element) => {
    for (let node = element; node; node = node.parentElement) {
      const z = getComputedStyle(node).zIndex;
      if (z !== 'auto' && z !== '') {
        const index = Number(z);
        if (Number.isFinite(index)) return index;
      }
    }
    return 0;
  };
  const topDialog = () =>
    [...document.querySelectorAll('[role="dialog"], dialog[open]')].filter(visible)
      .reduce((top, dialog) => !top || layer(dialog) >= layer(top) ? dialog : top, null);
  const topBackControl = (dialog = topDialog()) => {
    const controls = [...document.querySelectorAll('[data-back="true"]')].filter(visible);
    // Never select a Back action from underneath a transitioning modal.
    return dialog ? controls.filter((control) => dialog.contains(control)).at(-1) : controls.at(-1);
  };

  function sync() {
    if (releasing || waitingForScreen) return;
    // A dialog keeps Android history armed even if its button briefly hides.
    const dialog = topDialog();
    const wantsBack = screenCanBack || !!dialog || !!topBackControl(dialog);
    if (wantsBack && !trapped()) {
      history.pushState({ ...(history.state || {}), kabarBackTrap: true }, '');
    } else if (!wantsBack && trapped()) {
      // Closing the final overlay with X must not leave a phantom Back.
      releasing = true;
      history.back();
    }
  }
  function scheduleSync() {
    if (scheduled) return;
    scheduled = true;
    queueMicrotask(() => {
      scheduled = false;
      sync();
    });
  }

  window.__kabarSetScreenBack = (canBack) => {
    screenCanBack = !!canBack;
    waitingForScreen = false;
    scheduleSync();
  };

  // Covers any new dialog, popover, sheet or in-screen Back control,
  // including those which never change the top-level Screen signal.
  new MutationObserver(scheduleSync).observe(document.documentElement, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ['data-back', 'class', 'style', 'hidden', 'inert', 'disabled', 'aria-hidden', 'open'],
  });

  window.addEventListener('popstate', () => {
    if (releasing) {
      releasing = false;
      scheduleSync();
      return;
    }
    const dialog = topDialog();
    const top = topBackControl(dialog);
    if (top) {
      top.click();
      scheduleSync();
    } else if (dialog) {
      // Do not navigate the screen underneath a dialog in transition.
      scheduleSync();
    } else if (screenCanBack) {
      // A screen change is asynchronous. Wait for Rust to confirm the
      // next destination before installing the next history trap.
      waitingForScreen = true;
      window.__kabarBackSend('back');
    }
  });
  scheduleSync();
}
window.__kabarBackSend('ready');
await new Promise(() => {});
