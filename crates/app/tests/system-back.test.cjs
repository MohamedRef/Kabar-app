// No browser or npm dependency: exercise the shipped WebView controller.
const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const path = require('node:path');

const source = fs.readFileSync(path.join(__dirname, '../assets/system-back.js'), 'utf8');
const prefix = /window\.__([a-z]+)BackSend/.exec(source)?.[1];
if (!prefix) throw new Error('Expected a namespaced Back bridge');
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

function start() {
  const nodes = [];
  const received = [];
  const listeners = new Map();
  const callbacks = [];
  const window = { addEventListener: (event, fn) => listeners.set(event, fn) };
  const states = [null];
  // Chromium's history intervention: an entry the page pushed past with no
  // user activation since its last push is skipped by WebView.goBack(), and
  // canGoBack() ignores it, so wry then lets Android close the app.
  const skip = [false];
  let current = 0;
  let activated = false;
  const go = (to) => {
    current = to;
    queueMicrotask(() => listeners.get('popstate')?.());
  };
  const history = {
    get state() { return states[current]; },
    get current() { return current; },
    get length() { return states.length; },
    pushState(state) {
      if (!activated) skip[current] = true;
      activated = false;
      states.splice(current + 1, Infinity, state);
      skip.splice(current + 1, Infinity, false);
      current += 1;
    },
    back() { if (current > 0) go(current - 1); },
    forward() { if (current + 1 < states.length) go(current + 1); },
    // What the Android Back gesture does (WryActivity.onKeyDown).
    system() {
      for (let to = current - 1; to >= 0; to--) {
        if (!skip[to]) { go(to); return 'back'; }
      }
      return 'exit';
    },
    tap() { activated = true; },
  };
  const document = {
    documentElement: {},
    querySelectorAll: (selector) => {
      if (selector === '[data-back="true"]') return nodes.filter((node) => node.back);
      if (selector === '[role="dialog"], dialog[open]') return nodes.filter((node) => node.dialog);
      throw new Error('Unexpected selector ' + selector);
    },
  };
  const MutationObserver = class {
    constructor(callback) { callbacks.push(callback); }
    observe() {}
  };
  const getComputedStyle = (node) => ({
    zIndex: node.z ?? 'auto',
    visibility: node.hidden ? 'hidden' : 'visible',
  });
  const run = new AsyncFunction(
    'window', 'document', 'history', 'MutationObserver', 'getComputedStyle',
    'dioxus', 'queueMicrotask', source,
  );
  run(window, document, history, MutationObserver, getComputedStyle,
    { send: (message) => received.push(message) }, queueMicrotask);
  const mutation = () => callbacks.forEach((callback) => callback());
  function add({ back = false, dialog = false, z = 'auto', parentElement = null,
                 click = null, hidden = false, disabled = false } = {}) {
    const element = {
      back, dialog, z, parentElement, hidden, disabled, active: true,
      getClientRects() { return this.active && !this.hidden ? [{}] : []; },
      closest() {
        for (let node = this; node; node = node.parentElement) {
          if (node.hidden || node.inert || node.ariaHidden) return node;
        }
        return null;
      },
      contains(other) {
        for (let node = other; node; node = node.parentElement) {
          if (node === this) return true;
        }
        return false;
      },
      click() { click?.(); },
    };
    nodes.push(element);
    mutation();
    return element;
  }
  const flush = async () => { for (let n = 0; n < 8; n++) await Promise.resolve(); };
  const setScreenBack = (value) => window[`__${prefix}SetScreenBack`](value);
  const nativeBack = () => window[`__${prefix}NativeBack`]();
  return { add, flush, mutation, history, received, setScreenBack, nativeBack };
}

test('Back closes highest visual dialog before the later DOM sibling', async () => {
  const app = start();
  const closed = [];
  const update = app.add({ dialog: true, z: 61 });
  const updateClose = app.add({ back: true, parentElement: update, click: () => {
    closed.push('update'); update.active = false; updateClose.active = false; app.mutation();
  } });
  const settings = app.add({ dialog: true, z: 41 });
  const settingsClose = app.add({ back: true, parentElement: settings, click: () => {
    closed.push('settings'); settings.active = false; settingsClose.active = false; app.mutation();
  } });
  app.setScreenBack(true);
  await app.flush();
  assert.equal(app.history.current, 1);
  app.history.back();
  await app.flush();
  assert.deepEqual(closed, ['update']);
  assert.equal(app.history.current, 1);
  app.history.back();
  await app.flush();
  assert.deepEqual(closed, ['update', 'settings']);
  assert.equal(app.history.current, 1);
  app.history.back();
  await app.flush();
  assert.deepEqual(app.received, ['ready', 'back']);
  app.setScreenBack(false);
  await app.flush();
  assert.equal(app.history.current, 0);
});

test('hidden, disabled and inert Back controls are not selected', async () => {
  const app = start();
  const pressed = [];
  app.add({ back: true, click: () => pressed.push('real') });
  app.add({ back: true, disabled: true, click: () => pressed.push('disabled') });
  app.add({ back: true, hidden: true, click: () => pressed.push('hidden') });
  const inert = app.add();
  inert.inert = true;
  app.add({ back: true, parentElement: inert, click: () => pressed.push('inert') });
  await app.flush();
  app.history.back();
  await app.flush();
  assert.deepEqual(pressed, ['real']);
});

test('closing the last sheet with X releases history on Today', async () => {
  const app = start();
  const sheet = app.add({ dialog: true, z: 50 });
  const close = app.add({ back: true, parentElement: sheet, click: () => {
    sheet.active = false; close.active = false; app.mutation();
  } });
  await app.flush();
  assert.equal(app.history.current, 1);
  close.click();
  await app.flush();
  assert.equal(app.history.current, 0);
  assert.deepEqual(app.received, ['ready']);
});

test('a sheet closes before going back to an earlier screen', async () => {
  const app = start();
  const pressed = [];
  const sheet = app.add({ dialog: true, z: 25 });
  app.add({ back: true, parentElement: sheet, click: () => pressed.push('sheet') });
  app.setScreenBack(true);
  await app.flush();
  app.history.back();
  await app.flush();
  assert.deepEqual(pressed, ['sheet']);
  assert.deepEqual(app.received, ['ready']);
});

test('Changes > Updates > Settings > Home survives hidden modal controls', async () => {
  const app = start();
  const steps = [];
  const settings = app.add({ dialog: true, z: 41 });
  const closeSettings = app.add({ back: true, parentElement: settings, click: () => {
    steps.push('home'); settings.active = false; closeSettings.active = false; app.mutation();
  } });
  const backSection = app.add({ back: true, parentElement: settings, click: () => {
    steps.push('settings'); backSection.active = false; app.mutation();
  } });
  const changes = app.add({ dialog: true, z: 61 });
  const closeChanges = app.add({ back: true, parentElement: changes, click: () => {
    steps.push('updates'); closeChanges.hidden = true; app.mutation();
  } });
  closeSettings.hidden = true;
  backSection.hidden = true;
  app.mutation();
  await app.flush();
  app.history.back();
  await app.flush();
  assert.deepEqual(steps, ['updates']);
  assert.equal(app.history.current, 1, 'do not exit while dialogs are still mounted');
  changes.active = false;
  closeChanges.active = false;
  closeSettings.hidden = false;
  backSection.hidden = false;
  app.mutation();
  await app.flush();
  app.history.back();
  await app.flush();
  assert.deepEqual(steps, ['updates', 'settings']);
  assert.equal(app.history.current, 1);
  app.history.back();
  await app.flush();
  assert.deepEqual(steps, ['updates', 'settings', 'home']);
  assert.equal(app.history.current, 0);
});

test('modal without a visible Back control cannot pass Back to underlying screen', async () => {
  const app = start();
  const actions = [];
  app.add({ back: true, click: () => actions.push('underlying screen') });
  const modal = app.add({ dialog: true, z: 60 });
  app.add({ back: true, parentElement: modal, hidden: true });
  app.setScreenBack(true);
  await app.flush();
  app.history.back();
  await app.flush();
  assert.deepEqual(actions, []);
  assert.deepEqual(app.received, ['ready']);
  assert.equal(app.history.current, 1);
});

test('Back after closing Changes in Settings never leaves the app (WebView skips untapped entries)', async () => {
  const app = start();
  const steps = [];
  // Tap the gear: Settings opens straight into its Updates section.
  app.history.tap();
  const settings = app.add({ dialog: true, z: 41 });
  const closeSettings = app.add({ back: true, parentElement: settings, click: () => {
    steps.push('closed settings'); settings.active = false; closeSettings.active = false; app.mutation();
  } });
  const section = app.add({ back: true, parentElement: settings, click: () => {
    steps.push('section list'); section.active = false; app.mutation();
  } });
  await app.flush();
  // Tap "What's new": the Changes sheet opens over Settings.
  app.history.tap();
  const changes = app.add({ dialog: true, z: 61 });
  const closeChanges = app.add({ back: true, parentElement: changes, click: () => {
    steps.push('closed changes'); changes.active = false; closeChanges.active = false; app.mutation();
  } });
  await app.flush();
  // From here on only Back gestures, no taps.
  for (const expected of ['closed changes', 'section list', 'closed settings']) {
    assert.equal(app.history.system(), 'back', `Back must not leave the app before "${expected}"`);
    await app.flush();
    assert.equal(steps.at(-1), expected);
  }
  assert.deepEqual(app.received, ['ready']);
  assert.equal(app.history.system(), 'exit', 'Back on Today leaves the app');
});

test('screen Back twice in a row keeps a way back each time', async () => {
  const app = start();
  app.history.tap();
  app.setScreenBack(true);
  await app.flush();
  for (let n = 0; n < 3; n++) {
    assert.equal(app.history.system(), 'back');
    await app.flush();
    assert.equal(app.received.at(-1), 'back');
    app.setScreenBack(n < 2);
    await app.flush();
  }
  assert.equal(app.history.system(), 'exit');
});

test('a sheet opened without a tap after one closed by X still takes Back', async () => {
  const app = start();
  app.history.tap();
  const first = app.add({ dialog: true, z: 50 });
  const closeFirst = app.add({ back: true, parentElement: first, click: () => {
    first.active = false; closeFirst.active = false; app.mutation();
  } });
  await app.flush();
  app.history.tap();
  closeFirst.click();
  await app.flush();
  assert.equal(app.history.current, 0, 'nothing left open: history is released');
  // A sheet that opens by itself (no tap), e.g. What's new after an update.
  const steps = [];
  const second = app.add({ dialog: true, z: 50 });
  const closeSecond = app.add({ back: true, parentElement: second, click: () => {
    steps.push('closed'); second.active = false; closeSecond.active = false; app.mutation();
  } });
  await app.flush();
  assert.equal(app.history.current, 1, 're-armed on the entry already ahead');
  assert.equal(app.history.length, 2, 'no new history entry');
  assert.equal(app.history.system(), 'back');
  await app.flush();
  assert.deepEqual(steps, ['closed']);
  assert.equal(app.history.system(), 'exit');
});

test('Android Back key: one step per press, and exit only on Today with nothing open', async () => {
  const app = start();
  const steps = [];
  app.setScreenBack(true); // on Stats, came from Today
  const settings = app.add({ dialog: true, z: 41 });
  const closeSettings = app.add({ back: true, parentElement: settings, click: () => {
    steps.push('closed settings'); settings.active = false; closeSettings.active = false; app.mutation();
  } });
  const section = app.add({ back: true, parentElement: settings, click: () => {
    steps.push('section list'); section.active = false; app.mutation();
  } });
  const changes = app.add({ dialog: true, z: 61 });
  const closeChanges = app.add({ back: true, parentElement: changes, click: () => {
    steps.push('closed changes'); changes.active = false; closeChanges.active = false; app.mutation();
  } });
  await app.flush();
  for (const expected of ['closed changes', 'section list', 'closed settings']) {
    assert.equal(app.nativeBack(), 'step');
    await app.flush();
    assert.equal(steps.at(-1), expected);
  }
  assert.equal(app.nativeBack(), 'step', 'Stats steps back to Today');
  assert.deepEqual(app.received, ['ready', 'back']);
  assert.equal(app.nativeBack(), 'step', 'wait for Rust to confirm the screen');
  app.setScreenBack(false); // Today
  await app.flush();
  assert.equal(app.nativeBack(), 'exit', 'Today with nothing open leaves the app');
  assert.deepEqual(app.received, ['ready', 'back']);
});

test('Android Back key never leaves from under a dialog in transition', async () => {
  const app = start();
  const sheet = app.add({ dialog: true, z: 50 });
  app.add({ back: true, parentElement: sheet, hidden: true });
  await app.flush();
  assert.equal(app.nativeBack(), 'step');
});
