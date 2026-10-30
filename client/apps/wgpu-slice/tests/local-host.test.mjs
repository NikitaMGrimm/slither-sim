// Labelled inert Wasm/socket doubles exercise actual production adapter ownership only.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { localHost } from '../web/local-host.mjs';
import { installLocalInput } from '../web/local-input.mjs';
import { createClientInput as createInputOracle } from '../../../tools/fixtures/client-input-oracle.mjs';

// Authored event-path/DOM doubles, not physical input or browser default actions.
function inputFixture({ pointerEvents = true, observeInput } = {}) {
  const listeners = new Map(), calls = [];
  function node(parent) {
    const value = { parent, addEventListener(type, handler, options) {
      assert.equal(options.capture, true);
      const entries = listeners.get(value) ?? [];
      entries.push({ type, handler, signal: options.signal }); listeners.set(value, entries);
    } };
    return value;
  }
  const realm = node(), document = node(realm), canvas = node(document), overlay = node(document);
  const button = node(document), input = node(document), textarea = node(document), editable = node(document);
  class Input { static [Symbol.hasInstance](target) { return target === input; } }
  class TextArea { static [Symbol.hasInstance](target) { return target === textarea; } }
  Object.assign(realm, { document, HTMLInputElement: Input, HTMLTextAreaElement: TextArea,
    ...(pointerEvents ? { PointerEvent: class {} } : {}) });
  editable.isContentEditable = true;
  canvas.getBoundingClientRect = () => ({ left: 20, top: 30, width: 800, height: 600 });
  const policy = createInputOracle({ protocolVersion: 19 });
  policy.setOwn({ token: 1n, wantedHeading: 0, gazeHeading: 0, dead: false,
    turnRate: 0.1, sizeTurnFactor: 1, speedTurnFactor: 1 });
  let starts = 0, stops = 0;
  const owner = {
    pointer(x, y, width, height, down) {
      calls.push(['pointer', x, y, width, height, down]);
      const input = { clientX: x, clientY: y, viewportWidth: width, viewportHeight: height, follow: true };
      if (down) policy.mouseDown(input); else policy.pointer(input);
    },
    mouse_up() { calls.push(['release']); policy.mouseUp(); },
    key(code, down) {
      calls.push(['key', code, down]);
      return down ? policy.keyDown(code) : policy.keyUp(code);
    },
    blur() { calls.push(['blur']); policy.blur(); },
    can_play: () => true, stop() { stops++; },
  };
  const abort = new AbortController();
  installLocalInput({ realm, canvas, owner, host: { call: fn => fn() }, start: () => starts++, controls: [button], signal: abort.signal, observeInput });
  function fire(target, type, properties = {}) {
    const event = { target, type, button: 0, clientX: 620, clientY: 180, defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; }, ...properties };
    const path = []; for (let current = target; current; current = current.parent) path.unshift(current);
    for (const current of path) for (const entry of listeners.get(current) ?? []) {
      if (entry.type === type && !entry.signal.aborted) entry.handler(event);
    }
    return event;
  }
  return { realm, document, canvas, overlay, button, input, textarea, editable, owner, abort, fire, calls, policy,
    get boosting() { return policy.snapshot().own.requestedBoost; }, get starts() { return starts; }, get stops() { return stops; } };
}

test('read-only receipts retain filtered events and compatibility releases', () => {
  const receipts = [], f = inputFixture({ observeInput: (event, stage) => receipts.push([event.type, stage]) });
  f.fire(f.textarea, 'mousedown');
  f.fire(f.canvas, 'pointerup'); f.fire(f.input, 'mouseup');
  assert.deepEqual(receipts, [['mousedown', 'received'], ['mousedown', 'filtered-target'],
    ['pointerup', 'received'], ['pointerup', 'gameplay'], ['mouseup', 'received'], ['mouseup', 'filtered-compatibility']]);
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 1);
});

test('window capture tracks canvas and overlay coordinates in the game viewport', () => {
  const f = inputFixture();
  f.fire(f.canvas, 'mousemove'); f.fire(f.overlay, 'mousemove');
  assert.deepEqual(f.calls, [['pointer', 600, 150, 800, 600, false], ['pointer', 600, 150, 800, 600, false]]);
  f.fire(f.overlay, 'mousedown'); assert.equal(f.boosting, true);
  f.abort.abort(); f.fire(f.canvas, 'mousemove'); assert.equal(f.calls.length, 3);
});

test('input, textarea and inherited contenteditable suppress presses and keep editing defaults', () => {
  const f = inputFixture();
  for (const target of [f.input, f.textarea, f.editable]) {
    for (const code of ['Space', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Enter', 'Escape']) {
      assert.equal(f.fire(target, 'keydown', { code }).defaultPrevented, false);
    }
    for (const type of ['pointermove', 'pointerdown', 'mousemove', 'mousedown']) assert.equal(f.fire(target, type).defaultPrevented, false);
  }
  assert.deepEqual(f.calls, []); assert.equal(f.starts, 0); assert.equal(f.stops, 0);
});

test('button Space and Enter retain activation defaults without gameplay press or shortcut', () => {
  const f = inputFixture();
  for (const code of ['Space', 'Enter']) {
    assert.equal(f.fire(f.button, 'keydown', { code }).defaultPrevented, false);
    assert.equal(f.fire(f.button, 'keyup', { code }).defaultPrevented, false);
  }
  assert.deepEqual(f.calls, [['key', 'Space', false], ['key', 'Enter', false]]);
  for (const type of ['pointermove', 'pointerdown', 'mousemove', 'mousedown']) f.fire(f.button, type);
  assert.equal(f.boosting, false);
  assert.deepEqual(f.calls, [['key', 'Space', false], ['key', 'Enter', false]]);
  assert.equal(f.starts, 0);
  f.fire(f.canvas, 'keydown', { code: 'Enter' }); assert.equal(f.starts, 1);
  f.fire(f.canvas, 'keydown', { code: 'Escape' }); assert.equal(f.stops, 1);
  assert.equal(f.fire(f.canvas, 'keydown', { code: 'Space' }).defaultPrevented, true);
});

test('canvas presses release over editors and controls for both pointer and mouse release', () => {
  const f = inputFixture();
  for (const target of [f.input, f.textarea, f.editable, f.button, f.overlay]) {
    for (const type of ['pointerup', 'mouseup', 'pointercancel']) {
      f.fire(f.canvas, 'mousedown'); assert.equal(f.boosting, true);
      assert.equal(f.fire(target, type).defaultPrevented, false); assert.equal(f.boosting, false);
    }
    for (const [code, field] of [['Space', 'boostKeyHeld'], ['ArrowLeft', 'leftHeld'], ['ArrowRight', 'rightHeld'], ['ArrowUp', 'boostKeyHeld']]) {
      f.fire(f.canvas, 'keydown', { code }); assert.equal(f.policy.snapshot()[field], true);
      f.fire(target, 'keyup', { code }); assert.equal(f.policy.snapshot()[field], false);
    }
  }
});

test('blur and visibility loss preserve input policy until genuine releases arrive', () => {
  const f = inputFixture();
  for (const type of ['blur', 'visibilitychange']) {
    f.fire(f.canvas, 'mousedown'); f.fire(f.canvas, 'keydown', { code: 'Space' });
    f.fire(f.canvas, 'keydown', { code: 'ArrowLeft' });
    f.document.hidden = false; f.fire(f.document, 'visibilitychange'); assert.equal(f.boosting, true);
    f.document.hidden = true; f.fire(type === 'blur' ? f.realm : f.document, type);
    assert.equal(f.boosting, true); assert.equal(f.policy.snapshot().leftHeld, true);
    assert.equal(f.policy.snapshot().boostKeyHeld, true);
    f.fire(f.input, 'pointerup'); assert.equal(f.boosting, false);
    f.fire(f.input, 'mouseup');
    f.fire(f.input, 'keyup', { code: 'Space' }); f.fire(f.input, 'keyup', { code: 'ArrowLeft' });
    assert.equal(f.policy.snapshot().boostKeyHeld, false); assert.equal(f.policy.snapshot().leftHeld, false);
  }
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 2);
});

test('keyboard-only boost clears on mouseup just as in Canvas, without changing held-key state', () => {
  const f = inputFixture();
  f.fire(f.canvas, 'keydown', { code: 'Space' }); assert.equal(f.boosting, true);
  f.fire(f.textarea, 'mouseup'); assert.equal(f.boosting, false);
  assert.equal(f.policy.snapshot().boostKeyHeld, true);
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 1);
});

test('unmatched releases are delivered; only the matching compatibility mouseup is skipped', () => {
  const f = inputFixture();
  f.fire(f.overlay, 'pointerup'); f.fire(f.input, 'mouseup');
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 1);
  f.fire(f.overlay, 'mouseup');
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 2);
  f.fire(f.canvas, 'pointerup', { button: 1 }); f.fire(f.canvas, 'mouseup', { button: 0 });
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 4);
  f.fire(f.canvas, 'pointerup'); f.fire(f.canvas, 'keydown', { code: 'Space' });
  f.fire(f.overlay, 'mouseup'); assert.equal(f.boosting, false);
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 6);
});

test('blur retains compatibility pairing, while a new suppressed press starts a fresh release', () => {
  const f = inputFixture();
  f.fire(f.canvas, 'pointerup'); f.fire(f.realm, 'blur'); f.fire(f.overlay, 'mouseup');
  assert.equal(f.calls.filter(call => call[0] === 'release').length, 1);
  for (const type of ['pointerdown', 'mousedown']) {
    const g = inputFixture();
    g.fire(g.canvas, 'pointerup'); g.fire(g.textarea, type); g.fire(g.overlay, 'mouseup');
    assert.equal(g.calls.filter(call => call[0] === 'release').length, 2);
  }
});

test('movement pairs follow Canvas admissions; press and release submit once', () => {
  const f = inputFixture();
  for (const type of ['pointermove', 'mousemove', 'pointerdown', 'mousedown', 'pointerup', 'mouseup']) f.fire(f.canvas, type);
  assert.deepEqual(f.calls, [['pointer', 600, 150, 800, 600, false], ['pointer', 600, 150, 800, 600, false],
    ['pointer', 600, 150, 800, 600, true], ['release']]);
  const mouse = inputFixture({ pointerEvents: false });
  for (const type of ['mousemove', 'mousedown', 'mouseup']) mouse.fire(mouse.overlay, type);
  assert.deepEqual(mouse.calls, f.calls.slice(1));
});

test('compatibility movement overwrites pointer coordinates; press uses mouse; release only clears boost', () => {
  const f = inputFixture();
  // Authored DOM doubles supply the values exposed by Chromium's respective
  // getters. The browser fractional fixture exercises real DOM getters too.
  assert.equal(f.fire(f.overlay, 'pointermove', { clientX: 620.75, clientY: 180.5 }).defaultPrevented, false);
  assert.equal(f.fire(f.overlay, 'pointerdown', { clientX: 620.75, clientY: 180.5 }).defaultPrevented, false);
  assert.deepEqual(f.calls, [['pointer', 600.75, 150.5, 800, 600, false]]);
  f.fire(f.overlay, 'mousemove', { clientX: 620, clientY: 180 });
  assert.equal(f.fire(f.overlay, 'mousedown', { clientX: 621, clientY: 181 }).defaultPrevented, true);
  assert.equal(f.boosting, true);
  const point = { ...f.policy.snapshot().pointer };
  f.fire(f.textarea, 'pointerup', { clientX: 900.75, clientY: 400.5 });
  f.fire(f.textarea, 'mouseup', { clientX: 900, clientY: 400 });
  assert.equal(f.boosting, false);
  assert.deepEqual(f.policy.snapshot().pointer, point);
  assert.deepEqual(f.calls, [['pointer', 600.75, 150.5, 800, 600, false], ['pointer', 600, 150, 800, 600, false],
    ['pointer', 601, 151, 800, 600, true], ['release']]);
  f.fire(f.canvas, 'mousedown', { button: 1 });
  assert.equal(f.calls.length, 4);
});

test('isolated pointer movement reaches gameplay without a compatibility mousemove', () => {
  for (const pointerType of ['pen', 'touch', 'mouse']) {
    const f = inputFixture();
    // Authored stream: a canceled pointer press has no compatibility mouse
    // events. Movement must still follow Canvas without admitting the press.
    f.fire(f.overlay, 'pointerdown', { pointerType, defaultPrevented: true });
    f.fire(f.overlay, 'pointermove', { pointerType, clientX: 620.75, clientY: 180.5 });
    assert.deepEqual(f.calls, [['pointer', 600.75, 150.5, 800, 600, false]], pointerType);
    assert.equal(f.boosting, false);
    for (const target of [f.input, f.textarea, f.editable, f.button]) {
      f.fire(target, 'pointermove', { pointerType });
    }
    assert.equal(f.calls.length, 1);
  }
});
function fixture(make) {
  const log = [], sockets = [], queue = [];
  let generation = 1, next = 9007199254740999n, time = 123;
  const add = (kind, data) => queue.push({ kind, data, id: next++, generation });
  const owner = {
    play() { add(1, 'ws://127.0.0.1:19491/slither'); },
    effect_kind: () => queue[0]?.kind ?? 0, effect_id: () => queue[0].id,
    effect_generation: () => queue[0].generation, effect_url: () => queue[0].data,
    effect_bytes: () => queue[0].data,
    construct(g, id, time) { assert.equal(g, queue[0].generation); assert.equal(id, queue[0].id); log.push(['begin-construct',g,id,time]); },
    acknowledge(g, id, ok) { assert.equal(g, queue[0].generation); assert.equal(id, queue[0].id); assert.equal(typeof id, 'bigint'); log.push(['ack', id, ok]); queue.shift(); if (!ok) this.fail(); },
    opened(g, time) { log.push(['opened', g, time]); add(2, Uint8Array.of(1)); add(2, Uint8Array.of(99, 0)); },
    receive(g, b, t) { log.push(['received', g, [...b], t]); },
    closed(g) { log.push(['closed', g]); },
    fail() { log.push(['fail']); queue.length = 0; if (sockets.length) add(3); },
    stop() { queue.length = 0; add(3); log.push(['stop']); },
    free() { log.push(['free']); }, hud: () => 'RUST HUD', can_play: () => true,
    frame(...args) { log.push(['frame', ...args]); }, flush(...args) { log.push(['flush', ...args]); },
  };
  function construct(url) {
    const socket = { readyState: 1, bufferedAmount: 0, send(b) { log.push(['send', [...b]]); }, close() { log.push(['close']); this.onclose?.(); } };
    sockets.push(socket); log.push(['construct', url]); make?.(socket); return socket;
  }
  const host = localHost({ owner, socket: construct, now: () => time, extent: () => [960, 540, 1], readout: (s) => log.push(['hud', s]) });
  return { host, owner, log, sockets, clock: value => { time=value; }, restart: () => generation++ };
}
test('no implicit socket; full-u64 effects acknowledged outside callbacks; copied binary receive', () => {
  const f = fixture(); f.host.frame(20); assert.equal(f.sockets.length, 0);
  f.host.call(() => f.owner.play()); const s = f.sockets[0]; assert.equal(s.binaryType, 'arraybuffer');
  s.onopen(); assert.deepEqual(f.log.filter(x => x[0] === 'send'), [['send', [1]], ['send', [99, 0]]]);
  s.onmessage({ data: Uint8Array.of(0, 2, 112).buffer });
  assert.deepEqual(f.log.find(x => x[0] === 'received'), ['received', 1, [0, 2, 112], 123]);
});
test('failed send preserves exact undefined, closes once and suppresses later effects', () => {
  const f = fixture(s => { s.send = () => { throw undefined; }; }); f.host.call(() => f.owner.play()); f.sockets[0].onopen();
  assert.deepEqual(f.host.failure(), { failed: true, failure: undefined });
  assert.equal(f.log.filter(x => x[0] === 'close').length, 1); assert.equal(f.log.filter(x => x[0] === 'send').length, 0);
});
test('remote-close and stale-generation handlers are fenced; explicit restart only', () => {
  const f = fixture(); f.host.call(() => f.owner.play()); const old = f.sockets[0]; const late = old.onmessage;
  old.onclose(); late({ data: new ArrayBuffer(0) }); assert.equal(f.log.filter(x => x[0] === 'received').length, 0);
  f.host.frame(100); assert.equal(f.sockets.length, 1); f.restart(); f.host.call(() => f.owner.play()); assert.equal(f.sockets.length, 2); late({ data: new ArrayBuffer(0) }); assert.equal(f.log.filter(x => x[0] === 'received').length, 0);
});
test('nonbinary and oversized frames fail before Wasm receive', () => {
  for (const data of ['text', new ArrayBuffer(1048577)]) {
    const f = fixture(); f.host.call(() => f.owner.play()); f.sockets[0].onmessage({ data }); assert.equal(f.host.failure().failed, true); assert.equal(f.log.filter(x => x[0] === 'received').length, 0); f.host.dispose();
  }
});
test('dispose detaches before synchronous close, frees once, no late owner calls', () => {
  const f = fixture(); f.host.call(() => f.owner.play()); const late = f.sockets[0].onopen; f.host.dispose(); f.host.dispose(); late(); f.host.frame(5);
  assert.equal(f.log.filter(x => x[0] === 'free').length, 1); assert.equal(f.log.filter(x => x[0] === 'opened').length, 0);
});
test('partial constructor setup failure closes the owned socket', () => {
  const f = fixture(s => Object.defineProperty(s, 'binaryType', { set() { throw new Error('setter failure'); } }));
  f.host.call(() => f.owner.play()); assert.match(f.host.failure().failure.message, /setter failure/); assert.equal(f.log.filter(x => x[0] === 'close').length, 1);
});
test('GPU failure stops further frame attempts and disables Play without reconnect', () => {
  const f = fixture(); let frames = 0;
  f.owner.frame = () => { frames++; throw new Error('device lost'); };
  f.host.frame(1); f.host.frame(2); assert.equal(frames, 1); assert.equal(f.sockets.length, 0);
  assert.match(f.host.failure().failure.message, /device lost/); f.host.dispose();
});
test('fresh host clocks fence actual construction, open and presentation flush', () => {
  const f=fixture();f.clock(5000);f.host.call(()=>f.owner.play());
  const begin=f.log.findIndex(x=>x[0]==='begin-construct');const native=f.log.findIndex(x=>x[0]==='construct');
  assert.ok(begin>=0 && begin<native);assert.equal(f.log[begin][3],5000);
  f.clock(5001);f.sockets[0].onopen();assert.deepEqual(f.log.find(x=>x[0]==='opened'),['opened',1,5001]);
  assert.deepEqual(f.log.filter(x=>x[0]==='flush').at(-1),['flush',5001,960,540,1]);
});
test('failed fresh-construction admission creates no socket', () => {
  const f=fixture();f.owner.construct=()=>{throw undefined;};f.host.call(()=>f.owner.play());
  assert.equal(f.sockets.length,0);assert.deepEqual(f.host.failure(),{failed:true,failure:undefined});
});
