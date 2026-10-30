import init, { LocalGameplay, game_backing_size, CosmeticImages } from './pkg/slither_wgpu_slice.js';
import { localHost } from './local-host.mjs';
import { installLocalInput } from './local-input.mjs';
import { loadCosmetics } from './cosmetics.mjs';
const canvas = document.getElementById('game');
const status = document.getElementById('status');
const play = document.getElementById('play');
const stop = document.getElementById('stop');
const endpoint = document.getElementById('endpoint');
const nickname = document.getElementById('nickname');
const skin = document.getElementById('skin');
let host, owner, raf, disposed = false;
const abort = new AbortController();
const listen = (node, type, fn) => node.addEventListener(type, fn, { signal: abort.signal });
listen(document.querySelector('form'), 'submit', e => e.preventDefault());
function extent() {
  const [w, h] = game_backing_size(canvas.clientWidth, canvas.clientHeight);
  if (canvas.width !== w) canvas.width = w;
  if (canvas.height !== h) canvas.height = h;
  return [w, h, 1];
}
function dispose() {
  if (disposed) return;
  disposed = true; cancelAnimationFrame(raf); abort.abort();
  if (host) host.dispose(); else owner?.free();
  host = undefined; owner = undefined;
}
addEventListener('pagehide', dispose, { once: true });
try {
  if (!navigator.gpu) throw new Error('WebGPU unavailable; no fallback');
  await init();
  if (!disposed) {
    owner = await LocalGameplay.create(canvas, performance.now());
    if (disposed) { owner.free(); owner = undefined; throw new Error('Disposed during startup'); }
    try {
      const configResponse = await fetch('./config.json', { credentials: 'omit', signal: abort.signal });
      if (configResponse.ok) {
        const config = await configResponse.json();
        if (config.simulatorUrl) endpoint.value = config.simulatorUrl;
        if (config.cosmeticsBaseUrl) {
          const base = new URL(config.cosmeticsBaseUrl, location.href);
          if (base.origin !== location.origin || !['127.0.0.1', 'localhost', '[::1]'].includes(base.hostname)) throw new Error('Cosmetics require a same-origin local development URL');
          await loadCosmetics({ owner, CosmeticImages, baseUrl: base, accessory: config.accessory ?? -1, signal: abort.signal });
        }
      }
    } catch (error) {
      if (!disposed) console.warn(`Cosmetics unavailable: ${String(error)}`);
    }
    if (disposed) { owner?.free(); owner = undefined; }
    else {
      const adapter = owner.adapter();
      host = localHost({ owner, socket: url => new WebSocket(url), now: () => performance.now(), extent,
        readout(text, canPlay, failed, error) {
          status.textContent = `${text}\n${adapter}${failed ? `\nHOST ERROR ${String(error)}` : ''}`;
          play.disabled = !canPlay;
        } });
      const start = () => host.call(() => {
        // DOM conversion only; Rust validates ID/endpoint/nickname before any socket exists.
        const id = Number(skin.value);
        if (!Number.isInteger(id) || id < 0 || id > 65) throw new Error('Skin must be 0..65');
        owner.play(endpoint.value, nickname.value, id);
      });
      listen(play, 'click', start);
      listen(stop, 'click', () => host.call(() => owner.stop()));
      installLocalInput({ realm: window, canvas, owner, host, start, controls: [play, stop], signal: abort.signal });
      function draw(time) { if (!disposed) { host.frame(time); raf = requestAnimationFrame(draw); } }
      raf = requestAnimationFrame(draw);
    }
  }
} catch (error) { status.textContent = `Local GPU unavailable: ${String(error)}`; dispose(); }
