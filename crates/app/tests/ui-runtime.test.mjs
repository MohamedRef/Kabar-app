// Headless regression tests for the *production* Android Back and Settings
// keyboard boundary scripts. No browser, emulator or third-party npm deps.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const asset = (name) => readFileSync(new URL('../assets/' + name, import.meta.url), 'utf8');

function backHarness() {
  const source = asset('system-back.js');
  const prefix = source.match(/window\.__([a-z]+)BackSend/)?.[1];
  assert.ok(prefix, 'expected the app-specific production Back script');
  const handlers = {};
  const messages = [];
  const clicks = [];
  const entries = [null];
  let position = 0;
  let controls = [];
  let changed = () => {};
  const history = {
    get state() { return entries[position]; },
    pushState(state) {
      entries.splice(position + 1);
      entries.push(state);
      position = entries.length - 1;
    },
    back() {
      if (position > 0) {
        position--;
        handlers.popstate?.();
      }
    },
    forward() {
      if (position + 1 < entries.length) {
        position++;
        handlers.popstate?.();
      }
    },
  };
  class Observer {
    constructor(onChange) { changed = onChange; }
    observe() {}
  }
  const window = { addEventListener: (event, handler) => { handlers[event] = handler; } };
  vm.runInNewContext('(async () => {\n' + source + '\n})()', {
    window,
    history,
    dioxus: { send: (message) => messages.push(message) },
    document: {
      documentElement: {},
      querySelectorAll: (selector) => selector === '[data-back="true"]' ? controls : [],
    },
    MutationObserver: Observer,
    getComputedStyle: () => ({ visibility: 'visible' }),
    queueMicrotask,
    setTimeout,
    clearTimeout,
  });
  return {
    history,
    messages,
    clicks,
    prefix,
    control(name, shown = true) {
      return {
        disabled: false,
        shown,
        getClientRects() { return this.shown ? [{}] : []; },
        closest() { return null; },
        click() {
          clicks.push(name);
          this.shown = false;
          changed();
        },
      };
    },
    controls(next) { controls = next; changed(); },
    changed() { changed(); },
    setScreenBack(value) { window['__' + prefix + 'SetScreenBack'](value); },
    async flush() { await Promise.resolve(); await Promise.resolve(); },
  };
}

test('Android Back unwinds nested sheets in reverse opening order', async () => {
  const h = backHarness();
  const settings = h.control('settings');
  const updates = h.control('updates');
  h.controls([settings, updates]);
  await h.flush();
  assert.equal(h.history.state?.[h.prefix + 'BackTrap'], true);
  h.history.back();
  await h.flush();
  assert.deepEqual(h.clicks, ['updates']);
  assert.equal(h.history.state?.[h.prefix + 'BackTrap'], true);
  h.history.back();
  await h.flush();
  assert.deepEqual(h.clicks, ['updates', 'settings']);
  assert.equal(h.history.state, null, 'no phantom Android history entry');
});

test('closing a sheet by tapping X releases the synthetic Back entry', async () => {
  const h = backHarness();
  const sheet = h.control('sheet');
  h.controls([sheet]);
  await h.flush();
  assert.equal(h.history.state?.[h.prefix + 'BackTrap'], true);
  sheet.shown = false;
  h.changed();
  await h.flush();
  assert.equal(h.history.state, null);
  assert.deepEqual(h.clicks, [], 'closing by X did not issue another Back click');
});

test('Back ignores hidden controls and defers screen steps to the native trail', async () => {
  const h = backHarness();
  const visible = h.control('visible');
  const hidden = h.control('hidden', false);
  h.controls([visible, hidden]);
  await h.flush();
  h.history.back();
  await h.flush();
  assert.deepEqual(h.clicks, ['visible']);
  h.setScreenBack(true);
  await h.flush();
  h.history.back();
  await h.flush();
  assert.equal(h.messages.at(-1), 'back');
  h.setScreenBack(false);
  await h.flush();
  assert.equal(h.history.state, null);
});

function settingsHarness() {
  const handlers = {};
  let mutation = () => {};
  const clicks = [];
  const make = (name) => ({
    name,
    tabIndex: 0,
    disabled: false,
    inert: false,
    isConnected: true,
    getClientRects() { return [{}]; },
    closest() { return null; },
    matches() { return false; },
    focus() { document.activeElement = this; },
  });
  const previous = make('previous');
  const back = make('back');
  back.click = () => clicks.push('back');
  const first = make('first');
  const last = make('last');
  const outside = make('outside');
  const dialog = make('dialog');
  dialog.dataset = {};
  dialog.contains = (node) => [dialog, first, last, back].includes(node);
  dialog.querySelectorAll = (selector) => selector === '[data-back="true"]'
    ? [back]
    : [first, last];
  const body = { children: [dialog, outside], parentElement: null };
  dialog.parentElement = body;
  const document = {
    activeElement: previous,
    body,
    querySelector: (selector) => selector === '.settings-window' ? dialog : null,
    querySelectorAll: () => [dialog],
    addEventListener: (type, handler) => { handlers[type] = handler; },
    removeEventListener: (type) => { delete handlers[type]; },
  };
  class Observer {
    constructor(callback) { mutation = callback; }
    observe() {}
    disconnect() {}
  }
  vm.runInNewContext(asset('settings-dialog.js'), { document, MutationObserver: Observer });
  return {
    document,
    dialog,
    first,
    last,
    back,
    outside,
    previous,
    clicks,
    mutation: () => mutation(),
    press(key, shiftKey = false) {
      const event = {
        key,
        shiftKey,
        prevented: false,
        stopped: false,
        preventDefault() { this.prevented = true; },
        stopImmediatePropagation() { this.stopped = true; },
      };
      handlers.keydown?.(event);
      return event;
    },
  };
}

test('Settings traps Tab, handles Escape and restores inert background', () => {
  const h = settingsHarness();
  assert.equal(h.outside.inert, true, 'underlying screen is inert while Settings is open');
  h.document.activeElement = h.last;
  assert.equal(h.press('Tab').prevented, true);
  assert.equal(h.document.activeElement, h.first);
  assert.equal(h.press('Tab', true).prevented, true);
  assert.equal(h.document.activeElement, h.last);
  const event = h.press('Escape');
  assert.equal(event.prevented, true);
  assert.equal(event.stopped, true);
  assert.deepEqual(h.clicks, ['back']);
  h.dialog.isConnected = false;
  h.mutation();
  assert.equal(h.outside.inert, false);
  assert.equal(h.document.activeElement, h.previous);
});
