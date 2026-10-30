import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createSceneServer } from '../scene-server.mjs';
const root = fileURLToPath(new URL('../..', import.meta.url));
const modules = ['peer', 'clock', 'json', 'schema', 'schedule', 'input'];
export async function replayServer() {
  const server = createSceneServer();
  const original = server.listeners('request')[0]; server.removeAllListeners('request');
  const files = new Map(modules.map(name => [`/tools/feel-replay/${name}.mjs`, `tools/feel-replay/${name}.mjs`]));
  for (const name of ['local-host', 'local-input']) files.set(`/apps/wgpu-slice/web/${name}.mjs`, `apps/wgpu-slice/web/${name}.mjs`);
  for (const [prefix, dir, name] of [['/game/pkg/', 'canvas', 'slither_client'], ['/gpu/', 'gpu', 'slither_wgpu_slice']]) {
    for (const suffix of ['.js', '_bg.wasm']) files.set(prefix + name + suffix, `dist/feel-replay/${dir}/${name}${suffix}`);
  }
  server.on('request', async (request, response) => {
    const address = server.address();
    if (request.headers.host !== `127.0.0.1:${address.port}` || request.method !== 'GET') { response.writeHead(403).end(); return; }
    const url = request.url;
    const page = /^\/replay\/(canvas-wasm|webgpu-wasm)\.html$/.exec(url);
    if (page) {
      response.setHeader('Content-Security-Policy', "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; worker-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'");
      response.setHeader('Content-Type', 'text/html');
      response.end(`<!doctype html><meta charset="utf-8"><link rel="stylesheet" href="/game/host.css"><style>html,body{margin:0;overflow:hidden}#game{width:100vw;height:100vh}aside{position:fixed;left:0;top:0;z-index:2147483647}</style>
        ${page[1] === 'canvas-wasm' ? '<section id="slither-client-root"><p id="slither-client-status"></p><button id="slither-client-reload" hidden>Reload</button></section>' : '<canvas id="game"></canvas>'}
        <aside><div id="replay-overlay"></div><input id="replay-input"><textarea id="replay-textarea"></textarea><div id="replay-editable" contenteditable></div><button id="replay-button">Button</button></aside>
        <script type="module" src="/tools/feel-replay/peer.mjs"></script>`); return;
    }
    // Generated bindings import their authored HUD contract via a hashed snippet.
    const snippet = /^\/game\/pkg\/(snippets\/slither-client-[a-f0-9]{16}\/web\/stock-hud-contract\.mjs)$/.exec(url);
    const gpuSnippet = /^\/gpu\/(snippets\/slither-client-[a-f0-9]{16}\/web\/stock-hud-contract\.mjs)$/.exec(url);
    const file = files.get(url) ?? (snippet ? `dist/feel-replay/canvas/${snippet[1]}` : gpuSnippet ? `dist/feel-replay/gpu/${gpuSnippet[1]}` : null);
    if (!file) { original(request, response); return; }
    try {
      const bytes = await readFile(path.join(root, file));
      response.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
      response.setHeader('Cache-Control', 'no-store'); response.end(bytes);
    } catch { response.writeHead(404).end(); }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const port = server.address().port;
  if ([8766, 18884, 18885].includes(port)) { server.close(); throw new Error('Excluded ephemeral port'); }
  return { origin: `http://127.0.0.1:${port}`, close: () => new Promise(resolve => { server.close(resolve); server.closeAllConnections(); }) };
}
