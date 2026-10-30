import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { stringify } from './json.mjs';

// Subscribe at spawn time, before Browser.close or a signal can end the process.
// See browser-peers.md (cleanup) and browser-synthetic.json (offline run).
export function processTermination(child) {
  const terminated = new Promise(resolve => { child.once('exit', resolve); child.once('error', resolve); });
  return async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await terminated;
  };
}

// A dedicated headless process, never the user's profile or desktop session.
export async function launchBrowser(executable) {
  const profile = await mkdtemp(path.join(tmpdir(), 'feel-replay-'));
  const child = spawn(executable, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`,
    '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-component-update',
    '--disable-sync', '--disable-extensions', '--disable-default-apps', '--disable-domain-reliability',
    '--disable-features=MediaRouter,OptimizationHints,AutofillServerCommunication', '--metrics-recording-only',
    '--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1', 'about:blank'],
  { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] });
  const terminate = processTermination(child);
  child.stderr.resume();
  // Chromium CDP pipe uses NUL-delimited UTF-8 JSON on fd 3/4. No debugging listener.
  let serial = 0, buffer = '', ended = false;
  const pending = new Map(), listeners = new Map();
  const rejectAll = () => { ended = true; for (const p of pending.values()) { clearTimeout(p.timeout); p.reject(new Error('Headless browser exited')); } pending.clear(); };
  child.on('error', rejectAll); child.on('exit', rejectAll);
  child.stdio[4].setEncoding('utf8');
  child.stdio[4].on('data', chunk => {
    buffer += chunk;
    let index;
    while ((index = buffer.indexOf('\0')) >= 0) {
      const packet = JSON.parse(buffer.slice(0, index)); buffer = buffer.slice(index + 1);
      if (packet.id) {
        const p = pending.get(packet.id); if (!p) continue;
        pending.delete(packet.id); clearTimeout(p.timeout);
        if (packet.error) p.reject(new Error(packet.error.message)); else p.resolve(packet.result);
      } else for (const callback of listeners.get(packet.method) ?? []) callback(packet);
    }
  });
  function call(method, params = {}, sessionId) {
    if (ended) return Promise.reject(new Error('Headless browser unavailable'));
    return new Promise((resolve, reject) => {
      const id = ++serial;
      const timeout = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 60000);
      pending.set(id, { resolve, reject, timeout });
      child.stdio[3].write(`${JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) })}\0`);
    });
  }
  return {
    call,
    async page(url, geometry, startup) {
      const { targetId } = await call('Target.createTarget', { url: 'about:blank', background: true });
      const { sessionId } = await call('Target.attachToTarget', { targetId, flatten: true });
      const send = (method, params) => call(method, params, sessionId);
      await send('Page.enable'); await send('Runtime.enable');
      await send('Page.addScriptToEvaluateOnNewDocument', { source: `globalThis.replayStartupEncoded=${JSON.stringify(stringify(startup))};` });
      await send('Emulation.setDeviceMetricsOverride', { width: geometry.width, height: geometry.height,
        deviceScaleFactor: geometry.dpr, mobile: false });
      await send('Emulation.setFocusEmulationEnabled', { enabled: true });
      await send('Fetch.enable', { patterns: [{ urlPattern: '*' }] });
      const origin = new URL(url).origin;
      let denied = 0;
      const onRequest = message => {
        if (message.sessionId !== sessionId) return;
        const { requestId, request } = message.params;
        const allowed = new URL(request.url).origin === origin;
        if (!allowed) denied++;
        void send(allowed ? 'Fetch.continueRequest' : 'Fetch.failRequest', allowed ? { requestId } : { requestId, errorReason: 'BlockedByClient' }).catch(() => {});
      };
      const handlers = listeners.get('Fetch.requestPaused') ?? [];
      handlers.push(onRequest); listeners.set('Fetch.requestPaused', handlers);
      const loaded = new Promise(resolve => {
        const fn = message => { if (message.sessionId === sessionId) resolve(); };
        const list = listeners.get('Page.loadEventFired') ?? []; list.push(fn); listeners.set('Page.loadEventFired', list);
      });
      await send('Page.navigate', { url }); await loaded;
      const evaluate = async expression => {
        const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
        if (result.exceptionDetails) throw new Error(`Peer evaluation failed: ${result.exceptionDetails.exception?.description ?? result.exceptionDetails.text}`);
        return result.result.value;
      };
      await evaluate('window.replayReady');
      return { evaluate, denied: () => denied, close: () => call('Target.closeTarget', { targetId }) };
    },
    async close() {
      try { await call('Browser.close'); } catch { /* Process may close before reply. */ }
      await terminate();
      if (!profile.startsWith(path.resolve(tmpdir()) + path.sep) || !path.basename(profile).startsWith('feel-replay-')) throw new Error('Unsafe browser profile cleanup');
      await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    },
  };
}
