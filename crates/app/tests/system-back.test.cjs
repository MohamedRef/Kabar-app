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
  let current = 0;
  const history = {
    get state() { return states[current]; },
    get current() { return current; },
    pushState(state) { states.splice(++current, Infinity, state); },
    back() {
      if (current === 0) return;
      current -= 1;
      queueMicrotask(() => listeners.get('popstate')?.());
    },
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
  return { add, flush, mutation, history, received, setScreenBack };
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
