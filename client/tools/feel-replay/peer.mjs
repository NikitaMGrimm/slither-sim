import { parse, stringify } from './codec.mjs';
import { virtualClock } from './clock.mjs';
import { normalizeInput, packetTypes, targetFor } from './schedule.mjs';
import { safeCommand } from './schema.mjs';

const peer = location.pathname.includes('canvas-wasm') ? 'canvas-wasm' : 'webgpu-wasm';
const clock = virtualClock(window), endpoint = `ws://${location.host}/slither`;
const settle = () => new Promise(resolve => {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => { channel.port1.close(); channel.port2.close(); resolve(); };
  channel.port2.postMessage(null);
});
let activeSocket, runtime, owner, host, scene, quality, raf, seq = -1, failed = false, sendFailed = false;
let currentCommands = [], receipts = [], random = [], rng = 17, blocked = 0;
let currentEffects = [], socketGeneration = 0;
// Observe the existing page RNG; the WebGPU factory keeps its own production RNG.
Math.random = () => {
  rng = (Math.imul(rng, 1664525) + 1013904223) >>> 0;
  const value = rng / 2 ** 32;
  random.push([seq < 0 ? 'startup-page-unit' : 'factory-unit', value]); return value;
};
const deny = () => { blocked++; throw new Error('Offline replay network denied'); };
const nativeFetch = fetch.bind(window);
window.fetch = (resource, options) => {
  const url = new URL(typeof resource === 'string' || resource instanceof URL ? resource : resource.url, location.href);
  if (url.origin !== location.origin || options?.method && options.method !== 'GET') return deny();
  return nativeFetch(resource, { ...options, credentials: 'omit', redirect: 'error' });
};
for (const key of ['Worker', 'SharedWorker', 'EventSource', 'WebTransport', 'RTCPeerConnection', 'XMLHttpRequest']) window[key] = function() { deny(); };
Object.defineProperty(navigator, 'sendBeacon', { value: deny });
if (navigator.serviceWorker) Object.defineProperty(navigator.serviceWorker, 'register', { value: deny });
window.open = deny;
addEventListener('error', () => { failed = true; });
addEventListener('unhandledrejection', () => { failed = true; });
class ReplaySocket extends EventTarget {
  constructor(url) {
    super();
    if (String(url) !== endpoint || activeSocket && activeSocket.readyState !== 3) deny();
    this.readyState = 0; this.bufferedAmount = 0; activeSocket = this;
    this.generation = ++socketGeneration; this.sends = 0;
    this.effect('connect');
    queueMicrotask(() => { this.readyState = 1; this.effect('opened'); this.emit('open', new Event('open')); });
  }
  effect(kind) { currentEffects.push(`socket-${this.generation}-${kind}`); }
  emit(type, event) { this.dispatchEvent(event); this[`on${type}`]?.(event); }
  send(value) {
    if (this.readyState !== 1) { sendFailed = true; throw new Error('Replay socket closed'); }
    const bytes = Array.from(new Uint8Array(value.buffer ?? value, value.byteOffset ?? 0, value.byteLength));
    if (this.controls && safeCommand(bytes)) currentCommands.push({ bytes, decisionMs: clock.now(), sendMs: clock.now(), ackMs: clock.now(), succeeded: true });
    else this.effect(`negotiation-${this.sends++}`);
    // Challenge/login bytes are neither forwarded nor retained.
  }
  close() { if (this.readyState !== 3) this.effect('closed'); this.readyState = 3; }
}
window.WebSocket = ReplaySocket;

// Read-only receipts at each production adapter's real filtering boundary.
const observeInput = (event, stage) => receipts.push({ seq, stage, ...received(event), prevented: event.defaultPrevented });
function received(event) {
  return { type: event.type, target: event.target === window ? 'window' : event.target.id || event.target.localName,
    coordinates: /^(mouse|pointer)/.test(event.type) ? { x: event.clientX, y: event.clientY } : null,
    code: event.code ?? null, trusted: event.isTrusted, repeat: event.repeat ?? false, buttons: event.buttons ?? 0,
    focused: document.hasFocus(), visibility: document.visibilityState };
}
function geometry() {
  const canvas = document.querySelector(targetFor(peer, 'surface'));
  if (!canvas) throw new Error('Production canvas missing');
  return { css: { width: innerWidth, height: innerHeight }, backing: { width: canvas.width, height: canvas.height },
    surface: { width: canvas.width, height: canvas.height }, dpr: devicePixelRatio };
}
function dispatch(input) {
  const n = normalizeInput(input, { width: innerWidth, height: innerHeight });
  const target = n.type === 'blur' || n.type === 'resize' ? window : document.querySelector(targetFor(peer, n.target));
  if (!target) throw new Error('Mapped DOM target missing');
  let event;
  const options = { bubbles: true, cancelable: true, clientX: n.x, clientY: n.y, button: n.button, buttons: n.buttons,
    code: n.code, key: n.code === 'Space' ? ' ' : n.code, repeat: n.repeat, pointerId: 1, pointerType: 'mouse', isPrimary: true };
  if (n.type.startsWith('pointer')) event = new PointerEvent(n.type, options);
  else if (n.type.startsWith('mouse')) event = new MouseEvent(n.type, options);
  else if (n.type.startsWith('key')) event = new KeyboardEvent(n.type, options);
  else event = new Event(n.type, options);
  target.dispatchEvent(event);
  const observed = received(event);
  receipts.push({ seq, stage: 'dispatched', ...observed, prevented: event.defaultPrevented });
  return observed;
}
function metadata() {
  if (peer === 'canvas-wasm') {
    const s = runtime.diagnostics().gameplay.session;
    return { lag: { active: s.lagging, pingPending: s.pingPending, pongAgeMs: Math.max(0, clock.now() - s.lastPingMs) },
      highQuality: quality.feel_high_quality(), pendingFrames: 0, pendingEffects: 0 };
  }
  const m = owner.feel_metadata(clock.now());
  return { lag: { active: !!m[0], pingPending: !!m[1], pongAgeMs: m[2] }, highQuality: !!m[3], pendingFrames: m[4], pendingEffects: m[5] };
}
window.replayReady = (async () => {
  if (peer === 'canvas-wasm') {
    const wasm = await import('/game/pkg/slither_client.js'); await wasm.default();
    const { createLocalGameHost } = await import('/game/local-game-host.mjs');
    const { readAuthoredSourceRecipes } = await import('/game/authored-source-recipes.mjs');
    const { startGamePage } = await import('/game/runtime.mjs');
    const localHost = await createLocalGameHost({ endpoint, sourceRecipes: readAuthoredSourceRecipes(wasm), observeInput });
    // Existing Wasm factory capability; retain the real instance and prototype.
    const api = { ...wasm, AnimatedScene: function(...args) { scene = new wasm.AnimatedScene(...args); scene.enable_feel_trace(); return scene; },
      DesktopQuality: function(...args) { quality = new wasm.DesktopQuality(...args); return quality; } };
    api.AnimatedScene.prototype = wasm.AnimatedScene.prototype;
    api.DesktopQuality.prototype = wasm.DesktopQuality.prototype;
    runtime = await startGamePage({ initialize: async () => {}, wasmApi: api, localHost, observeInput, hudReadout: true,
      ownedHudUi: true, ownedPlayUi: true, ownedMenuUi: true, ownedAppearancePreview: true, ownedLoadingUi: true,
      ownedLeaderboardUi: true, ownedMinimapUi: true, ownedAnnouncementUi: true, ownedEndScreenUi: true, diagnosticUi: false });
    runtime.play({ endpoint, nickname: 'Replay', skin: 0, customSkin: new Uint8Array(), profile: localHost.profile });
  } else {
    const wasm = await import('/gpu/slither_wgpu_slice.js'); await wasm.default();
    const { localHost } = await import('/apps/wgpu-slice/web/local-host.mjs');
    const { installLocalInput } = await import('/apps/wgpu-slice/web/local-input.mjs');
    const canvas = document.querySelector('#game');
    owner = wasm.LocalGameplay.replay(clock.now());
    const extent = () => { const [w, h] = wasm.game_backing_size(canvas.clientWidth, canvas.clientHeight); canvas.width = w; canvas.height = h; return [w, h, 1]; };
    host = localHost({ owner, socket: url => new WebSocket(url), now: clock.now, extent, readout(_text, _canPlay, error) { if (error) failed = true; } });
    const start = () => host.call(() => owner.play(endpoint, 'Replay', 0));
    installLocalInput({ realm: window, canvas, owner, host, start, observeInput, controls: [document.querySelector('#replay-button')], signal: new AbortController().signal });
    const tick = time => { host.frame(time); raf = requestAnimationFrame(tick); };
    raf = requestAnimationFrame(tick); start();
  }
  // Equal explicit startup pointer/key state, before any recorded input or RAF.
  // Use both families: their actual filtering is retained in receipt sidecars.
  for (const type of ['pointermove', 'mousemove']) dispatch({ type, x: 0.5, y: 0.5 });
  return true;
})();
window.replayStep = async (event, data, ordinal) => {
  await window.replayReady;
  seq = ordinal; currentCommands = []; currentEffects = [];
  clock.advance(event.atMs);
  let observed = { target: 'host', type: event.kind, coordinates: null, code: null,
    trusted: false, repeat: false, buttons: 0, focused: document.hasFocus(), visibility: document.visibilityState };
  if (event.kind === 'packet') {
    if (!activeSocket || activeSocket.readyState !== 1) throw new Error('Packet before socket ready');
    const bytes = Uint8Array.from(atob(data.body), c => c.charCodeAt(0));
    if (packetTypes(data.body).includes(97)) activeSocket.controls = true;
    activeSocket.emit('message', new MessageEvent('message', { data: bytes.buffer }));
  } else if (event.kind === 'input') observed = dispatch(data);
  else clock.frame();
  // A task boundary drains the entire microtask queue, including recursively
  // queued publication work, without advancing virtual time or adding a draw.
  await settle();
  const observations = (peer === 'canvas-wasm' ? scene.take_feel_trace() : owner.take_feel_trace()).map(([phase, token, cells]) => [phase, token, Array.from(cells)]);
  return stringify({ seq, sourceId: event.sourceId, atMs: clock.now(), observations, commands: currentCommands, effects: currentEffects,
    event: { seq, sourceId: event.sourceId, kind: event.kind, scheduledMs: event.atMs, actualMs: clock.now(),
      ...observed, target: event.kind === 'input' ? (data.target ?? 'surface') : observed.target },
    geometry: geometry(), ...metadata(), failed, sendFailed });
};
window.replayFinish = () => stringify({ atMs: clock.now(), completed: !failed, unexpectedNetwork: blocked !== 0,
  ...metadata(), receipts, random: peer === 'canvas-wasm' ? random : owner.take_feel_random(), queuedRaf: clock.pendingFrames() });
window.replayStepEncoded = encoded => window.replayStep(...parse(encoded));
