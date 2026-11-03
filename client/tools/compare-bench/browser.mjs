import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import { spawn } from 'node:child_process';
import WebSocket from 'ws';
import { setTimeout as delay } from 'node:timers/promises';

export class CDP {
  constructor(ws) {
    this.ws = ws; this.id = 0; this.pending = new Map(); this.events = [];
    ws.on('message', bytes => {
      const m = JSON.parse(bytes);
      if (m.id) { const p = this.pending.get(m.id); this.pending.delete(m.id); m.error ? p?.reject(Error(JSON.stringify(m.error))) : p?.resolve(m.result); }
      else this.events.push(m);
    });
    ws.on('close', () => { for (const p of this.pending.values()) p.reject(Error('CDP closed')); this.pending.clear(); });
  }
  static async open(url) {
    const ws = new WebSocket(url); await new Promise((resolve, reject) => { ws.once('open', resolve); ws.once('error', reject); });
    return new CDP(ws);
  }
  async send(method, params = {}) {
    const id = ++this.id;
    return Promise.race([new Promise((resolve, reject) => { this.pending.set(id, { resolve, reject }); this.ws.send(JSON.stringify({ id, method, params })); }), delay(10000, undefined, { ref: false }).then(() => { if (this.pending.delete(id)) throw Error(`CDP timeout: ${method}`); })]);
  }
  async evaluate(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw Error(`NTL adapter preflight: ${r.exceptionDetails.text}: ${r.exceptionDetails.exception?.description}`);
    return r.result.value;
  }
}
// Authored compatibility shell, no official page, source or assets. If the mod
// needs a new DOM/bootstrap contract, preflight fails; never fetch the public page.
const shell = `<!doctype html><html><head><title>Loopback frame pacing</title></head><body>
<div id="login"><div id="logo"></div><div id="nick_holder"><input id="nick" maxlength="24"></div>
<div id="lastscore"></div><div id="playh"></div><div id="tips"></div></div>
${['smh','ocho','cstx','csk','cskh','etco','etcoh','plq','clq','etcod','nbg'].map(id => `<div id="${id}"></div>`).join('')}
<script>window.getData=(url,handler)=>fetch(url).then(r=>r.text()).then(t=>handler.f(t,'success')).catch(()=>handler.f('','error'));</script>
</body></html>`;

export function adapterFlags(adapter) {
  if (!adapter || adapter === 'default') return [];
  const m = adapter.match(/^luid_0x([0-9a-f]{8})_0x([0-9a-f]{8})$/i);
  if (!m) throw Error('chromeLuid must be default or an exact DXGI luid_0xHHHHHHHH_0xLLLLLLLL');
  return ['--use-gl=angle', '--use-angle=d3d11', `--use-adapter-luid=${parseInt(m[1], 16) | 0},${parseInt(m[2], 16) >>> 0}`];
}

export async function ntl({ chrome, mod, directory, endpoint, width, height, x, y, register, adapter }) {
  const blocked = [];
  const proxy = http.createServer((req, res) => { blocked.push(req.url); res.writeHead(403); res.end('External network denied'); });
  proxy.on('connect', (req, socket) => { blocked.push(req.url); socket.end('HTTP/1.1 403 Forbidden\r\n\r\n'); });
  proxy.on('connection', socket => socket.on('error', error => blocked.push(`socket-error:${error.code}`)));
  proxy.on('clientError', (error, socket) => { blocked.push(`client-error:${error.code}`); socket.destroy(); });
  const pageServer = http.createServer((req, res) => { res.writeHead(200, { 'Content-Type': 'text/html' }); res.end(shell); });
  pageServer.on('connection', socket => socket.on('error', () => {}));
  pageServer.on('clientError', (_error, socket) => socket.destroy());
  await new Promise(resolve => proxy.listen(0, '127.0.0.1', resolve));
  await new Promise(resolve => pageServer.listen(0, '127.0.0.1', resolve));
  const profile = path.join(directory, 'chrome-profile'); await fs.mkdir(path.join(profile, 'Default'), { recursive: true });
  await fs.writeFile(path.join(profile, 'Default', 'Preferences'), JSON.stringify({ browser: { enabled_labs_experiments: ['local-network-access-check-websockets@2', 'local-network-access-check@2'] } }));
  const log = path.join(directory, 'netlog.json');
  const child = spawn(chrome, [
    ...adapterFlags(adapter?.chromeLuid),
    `--user-data-dir=${profile}`, '--remote-debugging-port=0', '--remote-debugging-address=127.0.0.1',
    `--load-extension=${mod}`, `--disable-extensions-except=${mod}`,
    '--host-resolver-rules=MAP slither.io 127.0.0.1, MAP slither.com 127.0.0.1, MAP * ~NOTFOUND, EXCLUDE 127.0.0.1, EXCLUDE localhost',
    `--proxy-server=http://127.0.0.1:${proxy.address().port}`, '--proxy-bypass-list=127.0.0.1;localhost;slither.io;slither.com',
    '--disable-quic', '--disable-background-networking', '--no-first-run', '--no-default-browser-check',
    '--disable-features=LocalNetworkAccessChecks,LocalNetworkAccessChecksWebSockets',
    `--log-net-log=${log}`, '--net-log-capture-mode=Everything', `--window-size=${width},${height}`, `--window-position=${x},${y}`, 'about:blank',
  ], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: false });
  register(child);
  let chromeOutput = '';
  child.stdout.on('data', b => chromeOutput += b); child.stderr.on('data', b => chromeOutput += b);
  let browser, page, closing;
  const close = async () => {
    if (closing) return closing;
    closing = (async () => {
    if (page) { await fs.writeFile(path.join(directory, 'cdp-events.json'), JSON.stringify(page.events)); page.ws.close(); }
    try { await browser?.send('Browser.close'); } catch { /* retain original failure */ }
    browser?.ws.close();
    if (child.exitCode === null) await Promise.race([new Promise(resolve => child.once('close', resolve)), delay(3000)]);
    await fs.writeFile(path.join(directory, 'denied-requests.json'), JSON.stringify(blocked));
    await fs.writeFile(path.join(directory, 'chrome-console.txt'), chromeOutput);
    proxy.closeAllConnections(); proxy.close(); pageServer.closeAllConnections(); pageServer.close();
    })();
    return closing;
  };
  try {
    let port;
    for (let i = 0; i < 100; i++) {
      try { port = +(await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; if (port) break; } catch {}
      if (child.exitCode !== null) throw Error(`Chrome exited ${child.exitCode}; use extension-capable Chrome for Testing/Chromium`);
      await delay(100);
    }
    if (!port) throw Error('Dedicated Chrome CDP startup failed');
    const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    const version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
    browser = await CDP.open(version.webSocketDebuggerUrl);
    const gpu = await browser.send('SystemInfo.getInfo');
    const processes = await browser.send('SystemInfo.getProcessInfo');
    const gpuPid = processes.processInfo.find(p => p.type === 'GPU')?.id;
    await fs.writeFile(path.join(directory, 'browser-gpu.json'), JSON.stringify({ gpu, processes, gpuPid, adapterRequest: adapter ?? null }, null, 2));
    page = await CDP.open(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
    await page.send('Network.enable'); await page.send('Page.enable');
    // Defense in depth; the deny proxy and resolver remain the network boundary.
    await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `{
      const Original=window.WebSocket, allowed=${JSON.stringify(endpoint)};
      window.WebSocket=new Proxy(Original,{construct(target,args){if(String(args[0])!==allowed)throw Error('Benchmark blocked non-loopback game socket');return Reflect.construct(target,args);}});
    }` });
    await page.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
    await page.send('Page.navigate', { url: `http://slither.io:${pageServer.address().port}/` });
    let ready = false;
    for (let i = 0; i < 100; i++) {
      ready = await page.evaluate(`Boolean(document.querySelector('#ip-server')&&document.querySelector('#connect-btn')&&document.querySelector('#nick'))`);
      if (ready) break; await delay(100);
    }
    if (!ready) throw Error('NTL DOM/bootstrap contract changed: expected #nick, #ip-server, #connect-btn in local authored shell. Public origin was not fetched.');
    await page.evaluate(`(()=>{ const set=(id,v)=>{const e=document.querySelector(id); e.value=v; e.dispatchEvent(new Event('input',{bubbles:true})); e.dispatchEvent(new Event('change',{bubbles:true}));}; set('#nick','FrameBench'); set('#ip-server',${JSON.stringify(endpoint.replace('ws://', '').replace('/slither', ''))}); document.querySelector('#connect-btn').click(); return true; })()`);
    let joined;
    for (let i = 0; i < 600; i++) {
      const setup = page.events.some(e => e.method === 'Network.webSocketFrameReceived'
        && e.params.response.opcode === 2 && Buffer.from(e.params.response.payloadData, 'base64')[2] === 97);
      joined = setup && await page.evaluate(`Boolean(window.playing&&window.snake&&Number.isFinite(window.snake.xx)&&Number.isFinite(window.gsc))`);
      if (joined) break; await delay(100);
    }
    if (!joined) throw Error('NTL failed local protocol-13 join/preflight (playing, snake.xx, gsc). See CDP/netlog; no timing accepted.');
    await delay(1000);
    await fs.writeFile(path.join(directory, 'view-contract-preflight.json'), JSON.stringify(await page.evaluate(`(()=>{
      const arrays=[]; for(const k of Object.keys(window)){try{const v=window[k]; if(Array.isArray(v))arrays.push({name:k,length:v.length,firstKeys:v.find(x=>x&&typeof x==='object')?Object.keys(v.find(x=>x&&typeof x==='object')).slice(0,40):[]});}catch{}}
      return {arrays,scalars:Object.fromEntries(['mww','mhh','grd','gsc','view_xx','view_yy','real_flux_grd','flux_grd','foods_c'].map(k=>[k,window[k]]))};
    })()`), null, 2));
    return { child, page, close, log, chromeVersion: version.Browser, gpuPid,
      adapter: JSON.stringify({ devices: gpu.gpu.devices, renderer: gpu.gpu.auxAttributes?.glRenderer }),
      async zoomOut() {
        // Actual page wheel dispatch, no mutation of mod game state or code.
        await page.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: width / 2, y: height / 2, deltaX: 0, deltaY: 12000 }); await delay(1500);
      },
      async view() {
        const v = await page.evaluate(`(()=>{
          const missing=['gsc','view_xx','view_yy','grd','real_flux_grd','flux_grd','mww','mhh','foods_c'].filter(k=>!Number.isFinite(window[k]));
          if(!Array.isArray(window.foods))missing.push('foods[]');
          if(missing.length)throw Error('NTL read-only view preflight missing: '+missing.join(', ')+'. Mod must have a verified local adapter before timing; no inferred counts.');
          if(!window.playing||!window.snake) throw Error('NTL stopped playing before view preflight');
          const z=window.gsc, w=window.mww, h=window.mhh;
          if(!Number.isFinite(w)||!Number.isFinite(h)||!Number.isFinite(window.foods_c))throw Error('NTL viewport/food contract changed');
          let visible=0,retained=0;
          for(let i=0;i<window.foods_c;i++){const f=window.foods[i];if(!f||!Number.isFinite(f.rx)||!Number.isFinite(f.ry))throw Error('NTL food contract changed: rx/ry');retained++;if(Math.abs(f.rx-window.view_xx)*z<=w/2&&Math.abs(f.ry-window.view_yy)*z<=h/2)visible++;}
          return {camera:[window.view_xx,window.view_yy],center:window.grd,radius:Math.max(window.real_flux_grd,window.flux_grd),zoom:z,viewport:[w,h],retainedSnakes:window.snakes?.length,retainedPellets:retained,visiblePellets:visible};
        })()`);
        return v;
      } };
  } catch (error) { await close(); throw error; }
}
