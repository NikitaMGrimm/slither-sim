import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash, randomUUID } from 'node:crypto';
import { spawn, execFileSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { presentation, csv, hostStats, wholeMap, validity, schedule, netlogProof, comparison, presentMonFailure } from './analysis.mjs';
import { ntl } from './browser.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repository = path.resolve(here, '../../..');
const prohibited = new Set([8766, 18884, 18885]);
let interrupted = false;
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sha = async file => hash(await fs.readFile(file));
const save = (file, value) => fs.writeFile(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
export function completeJsonLines(text, prefix) {
  return text.split(/\r?\n/).slice(0,-1).filter(l => l.startsWith(prefix)).map(l => JSON.parse(l.slice(prefix.length)));
}
async function treeHash(directory) {
  const rows = [];
  async function walk(dir, prefix = '') {
    for (const entry of (await fs.readdir(dir, { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name, 'en'))) {
      if (entry.name === '.git') continue;
      if (entry.isSymbolicLink()) throw Error('Input tree must not contain symlinks');
      const name = prefix + entry.name;
      if (entry.isDirectory()) await walk(path.join(dir, entry.name), name + '/');
      else rows.push([name, await sha(path.join(dir, entry.name))]);
    }
  }
  await walk(directory); return { sha256: hash(JSON.stringify(rows)), files: rows };
}
export function endpointFrom(text) {
  const found = text.match(/Listening on (ws:\/\/127\.0\.0\.1:(\d+)\/slither)(?:\r?\n|$)/);
  if (!found) return null;
  const port = +found[2]; if (!port || port > 65535 || prohibited.has(port)) throw Error('Host selected forbidden port');
  return found[1];
}
function execute(command, args, options = {}) {
  return execFileSync(command, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, windowsHide: true, ...options }).trim();
}
async function waitFor(predicate, ms, label) {
  const end = performance.now() + ms;
  while (performance.now() < end) { if (interrupted) throw Error('Interrupted'); const result = await predicate(); if (result) return result; await delay(100); }
  throw Error(`${label} timeout`);
}
async function stop(child) {
  if (!child || child.exitCode !== null) return;
  // Only the process tree launched for this attempt, never owner's browser.
  try { execute('taskkill', ['/PID', String(child.pid), '/T', '/F']); } catch {}
  await Promise.race([new Promise(resolve => child.once('close', resolve)), delay(2000)]);
}
async function windows(config, directory, label, children) {
  const file = path.join(directory, `${label}-config.json`); await save(file, config);
  const child = spawn('powershell.exe', ['-NoProfile', '-File', path.join(here, 'windows.ps1'), '-Config', file], { windowsHide: true });
  children.push(child); let out = '', err = '';
  child.stdout.on('data', b => out += b); child.stderr.on('data', b => err += b);
  const code = await new Promise((resolve, reject) => { child.once('error', reject); child.once('close', resolve); });
  await fs.writeFile(path.join(directory, `${label}.txt`), out + err);
  if (code) throw Error(`Windows ${label} failed: ${err.slice(-2000)}`);
  return out.trim().split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line));
}
async function prepare(config, root) {
  const checkout = path.join(root, 'vlither');
  try { await fs.access(path.join(checkout, '.git')); }
  catch { execute('git', ['clone', 'https://github.com/for-loop9/vlither.git', checkout]); }
  if (execute('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: checkout })) throw Error('Vlither checkout has tracked changes; preserve them and use a clean scratch root');
  execute('git', ['fetch', 'origin'], { cwd: checkout });
  const ref = config.vlitherRef || execute('git', ['symbolic-ref', 'refs/remotes/origin/HEAD'], { cwd: checkout });
  const commit = execute('git', ['rev-parse', `${ref}^{commit}`], { cwd: checkout });
  execute('git', ['checkout', '--detach', commit], { cwd: checkout });
  return { checkout, ref, commit };
}
async function inputs(config, directory) {
  const binary = path.resolve(config.binary), host = path.resolve(config.host);
  const result = { binary: { sha256: await sha(binary), path: binary }, host: { sha256: await sha(host), path: host },
    repositoryHead: execute('git', ['rev-parse', 'HEAD'], { cwd: repository }),
    sourceDirty: Boolean(execute('git', ['status', '--porcelain'], { cwd: repository })),
    presentMon: { sha256: await sha(config.presentMon) } };
  return result;
}
async function modInput(input, session) {
  const info = await fs.stat(input);
  if (info.isDirectory()) return { directory: path.resolve(input), tree: await treeHash(input) };
  const bytes = await fs.readFile(input);
  let offset = 0;
  if (bytes.subarray(0, 4).toString() === 'Cr24') {
    const version = bytes.readUInt32LE(4);
    if (version === 3) offset = 12 + bytes.readUInt32LE(8);
    else if (version === 2) offset = 16 + bytes.readUInt32LE(8) + bytes.readUInt32LE(12);
    else throw Error(`Unsupported CRX version ${version}`);
  }
  if (bytes.subarray(offset, offset + 4).toString('hex') !== '504b0304') throw Error('NTL input must be a manifest folder, ZIP, or CRX2/3 package');
  const archive = path.join(session, 'mod-package.zip'), directory = path.join(session, 'mod-extracted');
  await fs.writeFile(archive, bytes.subarray(offset), { flag: 'wx' });
  execute('powershell.exe', ['-NoProfile', '-File', path.join(here, 'unpack-mod.ps1'), '-Archive', archive, '-Destination', directory]);
  return { directory, packageSha256: hash(bytes), tree: await treeHash(directory) };
}
async function runAttempt(config, root, item, provenance, machine) {
  const directory = path.join(root, `${item.scenario}-${item.round}-${item.client}-${randomUUID()}`);
  await fs.mkdir(directory);
  const children = []; let hostLog = '', clientLog = '', browser, monitor, pm, host, native;
  let startHost = 0, startClient = 0, hostEnd = 0;
  let pmLog = '';
  const r = { schema: 'slither-frame-bench-v1', ...item, seconds: config.seconds,
    experiment: config.experiment, targetAdapterLuid: config.targetAdapterLuid,
    targetDisplay: config.targetDisplay, targetRefreshHz: config.targetRefreshHz,
    width: config.width, height: config.height, dt: .007, bots: item.scenario === 'battledome' ? 100 : 500,
    provenance, machine, networkVerified: false, windowSamples: [], views: [], census: [], stats: [],
    acceptance: 'harness smoke only; not comparative client acceptance',
    survival: 'host circular steering at 0.75*radius, boost off, socket player ignores body collisions; boundary/pit rules and bots unchanged; any death invalidates' };
  const register = child => { children.push(child); child.on('error', () => {}); };
  const cancel = () => {
    interrupted = true;
    browser?.close().catch(() => {});
    for (const child of [...children].reverse()) stop(child).catch(() => {});
  };
  process.once('SIGINT', cancel); process.once('SIGTERM', cancel);
  try {
    const scenario = JSON.parse(await fs.readFile(path.join(here, 'scenarios', `${item.scenario}.json`), 'utf8'));
    const scenarioPath = path.join(directory, 'host.json'); await save(scenarioPath, scenario);
    r.scenarioSha256 = await sha(scenarioPath);
    host = spawn(config.host, ['host', '--bind', '127.0.0.1:0', '--config', scenarioPath, '--stats', '0.5', '--frame-bench'], { windowsHide: true }); register(host);
    host.stdout.on('data', b => hostLog += b); host.stderr.on('data', b => hostLog += b);
    const endpoint = await waitFor(() => endpointFrom(hostLog) || (host.exitCode !== null ? Promise.reject(Error(`Host exited ${host.exitCode}: ${hostLog}`)) : null), 30000, 'Host readiness');
    r.endpoint = endpoint;
    if (item.client === 'ntl') {
      browser = await ntl({ ...config, directory, endpoint, register });
      native = browser.child;
      r.adapter = browser.adapter; r.chromeVersion = browser.chromeVersion;
      r.adapterRequest = config.adapter; r.vsync = 'Chrome default compositor pacing; actual SyncInterval unmeasured until ETW';
      await windows({ mode: 'setup', clientPid: native.pid, client: 'browser', scenario: item.scenario, width: config.width, height: config.height, x: config.x, y: config.y }, directory, 'setup', children);
      if (item.scenario === 'battledome') await browser.zoomOut();
      const view = await browser.view(); r.preflightView = view;
      r.vsync = 'Chrome default compositor pacing; verify PresentMon SyncInterval/mode'; r.chromeVersion = browser.chromeVersion;
    } else if (item.client === 'vlither') {
      const receipt = JSON.parse(await fs.readFile(path.join(config.output, 'vlither/build/compare-build.json'), 'utf8'));
      if (!receipt.instrumented || receipt.binarySha256 !== await sha(config.vlitherBinary)
          || receipt.wrapperSha256 !== await sha(path.join(here, 'vlither-wrapper.c'))
          || receipt.commit !== provenance.vlither.commit || provenance.vlither.dirty) throw Error('Vlither wrapper/build provenance changed; prepare and rebuild before acquisition');
      r.vlitherBuild = receipt; r.vsync = 'FIFO, enabled by labelled benchmark wrapper';
      // Fresh user.dat per attempt; assets remain in external upstream checkout.
      for (const resource of ['app', 'thermite']) await fs.symlink(path.join(config.output, 'vlither', resource), path.join(directory, resource), 'junction');
      native = spawn(config.vlitherBinary, [], { cwd: directory, env: { ...process.env,
        SLITHER_BENCH_SERVER: endpoint.replace('ws://', '').replace('/slither', ''),
        ...(item.scenario === 'battledome' ? { SLITHER_BENCH_WHOLE_MAP: '1' } : {}) }, windowsHide: false });
      register(native); native.stdout.on('data', b => clientLog += b); native.stderr.on('data', b => clientLog += b);
      await windows({ mode: 'setup', clientPid: native.pid, client: 'vlither', scenario: item.scenario, width: config.width, height: config.height, x: config.x, y: config.y }, directory, 'setup', children);
      await waitFor(() => clientLog.includes('frame-bench-client: ') || (native.exitCode !== null ? Promise.reject(Error(`Vlither exited ${native.exitCode}: ${clientLog}`)) : null), 20000, 'Vlither local join');
    } else {
      const data = path.join(directory, 'preferences'); await fs.mkdir(path.join(data, 'SlitherClient'), { recursive: true });
      await fs.writeFile(path.join(data, 'SlitherClient', 'settings.txt'), `2:1:${item.client === 'native' ? 'native' : 'original'}`);
      native = spawn(config.binary, ['--server', endpoint.replace('ws://', '').replace('/slither', ''), '--nickname', 'FrameBench', '--skin', '0'],
        { cwd: directory, env: { ...process.env, LOCALAPPDATA: data, SLITHER_FRAME_PACING: '1' }, windowsHide: false });
      register(native); native.stdout.on('data', b => clientLog += b); native.stderr.on('data', b => clientLog += b);
      await windows({ mode: 'setup', clientPid: native.pid, client: 'ours', scenario: item.scenario, width: config.width, height: config.height, x: config.x, y: config.y }, directory, 'setup', children);
      await waitFor(() => completeJsonLines(hostLog, 'frame-bench: ').some(s => s.players?.[0]?.setup)
        || (native.exitCode !== null ? Promise.reject(Error(`Client exited ${native.exitCode}: ${clientLog}`)) : null), 60000, 'Own admission');
      r.vsync = 'Fifo (source); PresentMon records actual mode and SyncInterval';
      r.networkVerified = true; // Direct explicit --server bypasses discovery; verify socket census below.
    }
    r.worldReady = await waitFor(() => {
      const census = completeJsonLines(hostLog, 'frame-bench: ').at(-1);
      if (!census) return null;
      return census.admittedBotSlots === r.bots && census.players?.[0]?.alive && census.players[0].setup ? census : null;
    }, 60000, 'All internal bot seats and living player readiness');
    await delay(config.warmupSeconds * 1000);
    // Start ETW before the fixed window; retain the guards before/after it too.
    const csvPath = path.join(directory, 'presentmon.csv');
    pm = spawn(config.presentMon, ['--session_name', `SlitherBench-${randomUUID()}`, '--output_file', csvPath,
      '--v2_metrics', '--qpc_time_ms', '--timed', String(config.seconds + 12), '--terminate_after_timed', '--no_console_stats'], { windowsHide: true }); register(pm);
    pm.stdout.on('data', b => pmLog += b); pm.stderr.on('data', b => pmLog += b);
    await delay(1000);
    if (pm.exitCode !== null) throw Error(`PresentMon failed without elevation: ${pmLog}. Owner must grant ETW permission or run collector separately; no security settings changed.`);
    startHost = hostLog.length; startClient = clientLog.length;
    const samplesPromise = windows({ mode: 'sample', clientPid: native.pid, gpuPid: browser ? browser.gpuPid : native.pid, hostPid: host.pid, seconds: config.seconds + .5 }, directory, 'window-load', children);
    let monitoring = true;
    samplesPromise.finally(() => { monitoring = false; }).catch(() => {});
    while (monitoring) {
      if (browser) { const v = await browser.view(); r.views.push({ ...v, wholeMap: wholeMap(v) }); }
      await delay(250);
    }
    r.windowSamples = await samplesPromise;
    hostEnd = hostLog.length;
    if (!r.windowSamples.length) throw Error('Window monitoring returned no samples');
    const first = r.windowSamples[0]; r.display = first.display; r.dpi = first.dpi;
    r.startMs = first.qpcMs; r.endMs = r.startMs + config.seconds * 1000;
    r.windowSamples = r.windowSamples.filter(s => s.qpcMs <= r.endMs);
    r.refreshHz = first.refreshHz;
    r.displayAdapters = machine.dxgiAdapters.filter(a => a.outputs?.includes(r.display));
    r.displayBindings = machine.displayBindings.filter(b => b.display === r.display);
    const engines = r.windowSamples.flatMap(s => s.gpuEngines ?? []);
    const renderLuids = [...new Set(engines.filter(e => /engtype_3d/i.test(e.instance) && e.utilization > .01)
      .map(e => e.instance.match(/luid_0x[0-9a-f]+_0x[0-9a-f]+/i)?.[0].toLowerCase()).filter(Boolean))];
    r.renderAdapters = renderLuids.map(luid => machine.dxgiAdapters.find(a => a.luid === luid) ?? { luid, name: 'unmapped' });
    r.presentationPath = r.renderAdapters.length && r.displayBindings.length
      ? r.renderAdapters.every(a => r.displayBindings.some(b => b.targetLuid === a.luid)) ? 'same-adapter render/output' : 'cross-adapter render/output'
      : 'unknown render/output path';
    r.adapterRequest = config.adapter ?? { chromeLuid: 'default', native: 'default', vlither: 'default' };
    r.expectedRenderLuid = config.experiment === 'primary' ? config.targetAdapterLuid
      : item.client === 'ntl' && config.adapter?.chromeLuid !== 'default' ? config.adapter?.chromeLuid : null;
    r.adapter = item.client === 'ntl' ? browser.adapter : clientLog.match(/Actual adapter (.+); MSAA/)?.[1] ?? clientLog.match(/GPU: (.+)/)?.[1];
    r.load = { hostMeanCpuCores: r.windowSamples.reduce((sum, s) => sum + s.hostCpuCores, 0) / r.windowSamples.length,
      machineMeanCpuPercent: r.windowSamples.reduce((sum, s) => sum + s.machineCpuPercent, 0) / r.windowSamples.length,
      machineMaxCpuPercent: Math.max(...r.windowSamples.map(s => s.machineCpuPercent)),
      logicalProcessors: machine.logicalProcessors };
    if (!browser) {
      r.views = completeJsonLines(clientLog.slice(startClient), 'frame-bench-client: ').map(v => {
        // Camera works in logical client backing coordinates, not attachment pixels.
        const viewport = v.viewport ?? [config.width * 96 / r.dpi, config.height * 96 / r.dpi];
        return { ...v, viewport, wholeMap: wholeMap({ ...v, viewport }) };
      });
      const network = execute('powershell.exe', ['-NoProfile', '-Command', `@(Get-NetTCPConnection -OwningProcess ${native.pid} -ErrorAction SilentlyContinue | Where-Object State -eq Established | Select-Object RemoteAddress,RemotePort) | ConvertTo-Json -Compress`]);
      await fs.writeFile(path.join(directory, 'native-sockets.json'), network);
      const peers = network ? [JSON.parse(network)].flat() : [];
      r.networkVerified = peers.length > 0 && peers.every(p => p.RemoteAddress === '127.0.0.1' && p.RemotePort === +new URL(endpoint).port);
    }
    r.stats = hostStats(hostLog.slice(startHost, hostEnd));
    r.census = completeJsonLines(hostLog.slice(startHost, hostEnd), 'frame-bench: ');
    await new Promise(resolve => pm.exitCode !== null ? resolve() : pm.once('close', resolve));
    await fs.writeFile(path.join(directory, 'presentmon-console.txt'), pmLog);
    if (pm.exitCode !== 0) throw Error(`PresentMon exit ${pm.exitCode}; ETW access/collection failed. No self-elevation attempted.`);
    if (presentMonFailure(pmLog)) throw Error(presentMonFailure(pmLog));
    const text = await fs.readFile(csvPath, 'utf8');
    const surfaces = new Map();
    const ownedPids = new Set([browser ? browser.gpuPid : native.pid]);
    // Chrome GPU process owns presents. Require an explicit single active
    // surface in this dedicated process tree; never pick the fastest surface.
    for (const row of csv(text)) {
      if (!ownedPids.has(+row.ProcessID)) continue;
      if (row.DisplayedTime === 'NA' || !(+row.DisplayedTime > 0)) continue;
      if (!(+row.CPUStartQPCTime >= r.startMs && +row.CPUStartQPCTime <= r.endMs)) continue;
      const key = `${row.ProcessID}/${row.SwapChainAddress}`; surfaces.set(key, (surfaces.get(key) ?? 0) + 1);
    }
    r.surfaces = [...surfaces];
    if (surfaces.size !== 1) throw Error(`Ambiguous/missing presentation surface (${surfaces.size}); no aggregate or automatic fastest selection`);
    const [pid, swapchain] = [...surfaces.keys()][0].split('/');
    r.metrics = presentation(text, { pid: +pid, swapchain, seconds: config.seconds, startMs: r.startMs, endMs: r.endMs });
    r.metrics.refreshPeriodMs = 1000 / r.refreshHz;
    r.metrics.p95RefreshPeriods = r.metrics.p95Ms / r.metrics.refreshPeriodMs;
    r.metrics.longestRefreshPeriods = r.metrics.longestStallMs / r.metrics.refreshPeriodMs;
    r.metrics.medianRefreshPeriods = r.metrics.medianMs / r.metrics.refreshPeriodMs;
    r.vsyncCondition = r.metrics.syncIntervals.length === 1 && r.metrics.syncIntervals[0] === '1' ? 'sync-interval-1' : 'recorded-modes-require-qualified-comparison';
  } catch (error) { r.error = error.message; }
  finally {
    try { await browser?.close(); } catch (e) { r.cleanupError = e.message; }
    for (const child of [...children].reverse()) await stop(child);
    await fs.writeFile(path.join(directory, 'host.txt'), hostLog);
    await fs.writeFile(path.join(directory, 'client.txt'), clientLog);
    await fs.writeFile(path.join(directory, 'presentmon-console.txt'), pmLog);
    if (item.client === 'ntl' && r.endpoint) {
      try { r.networkProof = netlogProof(JSON.parse(await fs.readFile(path.join(directory,'netlog.json'), 'utf8')), r.endpoint.replace('ws://', '').replace('/slither', '')); r.networkVerified = r.networkProof.verified; }
      catch (e) { r.networkError = e.message; }
    }
    r.validity = validity(r); await save(path.join(directory, 'result.json'), r);
    process.removeListener('SIGINT', cancel); process.removeListener('SIGTERM', cancel);
  }
  console.log(JSON.stringify({ directory, scenario: item.scenario, client: item.client, valid: r.validity.valid, reasons: r.validity.reasons }));
  return { directory, valid: r.validity.valid, run: r };
}
export async function main(args = process.argv.slice(2)) {
  if (args.length !== 2 || !['inspect', 'prepare', 'smoke', 'run'].includes(args[0])) throw Error('Usage: node tools/compare-bench/run.mjs inspect|prepare|smoke|run EXTERNAL_CONFIG.json');
  if (process.platform !== 'win32') throw Error('Windows only; analysis/tests are portable');
  const config = JSON.parse(await fs.readFile(args[1], 'utf8'));
  const root = path.resolve(config.output);
  config.output = root;
  if (root === repository || root.startsWith(repository + path.sep)) throw Error('Results and inputs must stay outside Git');
  await fs.mkdir(root, { recursive: true });
  if (args[0] === 'inspect') {
    const directory = path.join(root, `machine-${randomUUID()}`); await fs.mkdir(directory);
    console.log(JSON.stringify((await windows({ mode: 'machine' }, directory, 'machine', []))[0])); return;
  }
  if (args[0] === 'prepare') { console.log(JSON.stringify(await prepare(config, root))); return; }
  Object.assign(config, { width: config.width ?? 1280, height: config.height ?? 720, x: config.x ?? 0, y: config.y ?? 0,
    seconds: args[0] === 'smoke' ? 5 : config.seconds ?? 60, warmupSeconds: config.warmupSeconds ?? 5 });
  if (!Number.isInteger(config.seconds) || config.seconds < 3 || config.seconds > 180) throw Error('seconds must be 3..180');
  const clients = config.clients ?? ['original', 'native', 'ntl', 'vlither'];
  if (!Array.isArray(clients) || !clients.length || new Set(clients).size !== clients.length || clients.some(c => !['original','native','ntl','vlither'].includes(c))) throw Error('clients must be a unique nonempty subset of original, native, ntl, vlither');
  for (const key of ['width', 'height']) if (!Number.isInteger(config[key]) || config[key] < 320 || config[key] > 3840) throw Error(`${key} must be 320..3840 physical pixels`);
  config.experiment ??= 'primary';
  if (!['primary', 'secondary', 'rtx-optional'].includes(config.experiment)) throw Error('experiment must be primary, secondary or rtx-optional');
  if (args[0] === 'smoke' && config.experiment !== 'primary') throw Error('Smoke is restricted to the primary iGPU experiment');
  if (config.experiment === 'primary') {
    if (!/^luid_0x[0-9a-f]{8}_0x[0-9a-f]{8}$/i.test(config.targetAdapterLuid ?? '') || !config.targetDisplay || !config.targetRefreshHz) throw Error('Primary requires explicit targetAdapterLuid, targetDisplay and targetRefreshHz from inspect (AMD iGPU / 4K display)');
    config.adapter = { ...config.adapter, chromeLuid: config.targetAdapterLuid };
  }
  if (config.experiment === 'secondary' && config.adapter?.chromeLuid && config.adapter.chromeLuid !== 'default') throw Error('Secondary must use default adapter selection for every client');
  for (const client of ['native', 'vlither']) if (config.adapter?.[client] && config.adapter[client] !== 'default') throw Error(`${client} has no adapter selector in this binary/source adapter; no Windows graphics setting is changed. Use a matching Chrome LUID, or owner-managed per-app GPU preferences and verify engine evidence.`);
  const session = path.join(root, `session-${new Date().toISOString().replace(/[:.]/g, '-')}`); await fs.mkdir(session);
  const provenance = await inputs(config, session);
  try {
    const mod = await modInput(config.mod, session); config.mod = mod.directory;
    provenance.ntl = { manifest: JSON.parse(await fs.readFile(path.join(config.mod, 'manifest.json'), 'utf8')), ...mod };
  } catch (error) { provenance.ntl = { error: error.message }; }
  const checkout = path.join(root, 'vlither');
  try { provenance.vlither = { commit: execute('git', ['rev-parse', 'HEAD'], { cwd: checkout }), ref: config.vlitherRef ?? 'latest default branch at prepare',
    dirty: execute('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: checkout }), binarySha256: config.vlitherBinary ? await sha(config.vlitherBinary) : null }; }
  catch (error) { provenance.vlither = { error: error.message }; }
  await save(path.join(session, 'inputs.json'), provenance);
  const machine = (await windows({ mode: 'machine' }, session, 'machine', []))[0];
  if (config.experiment === 'primary') {
    const adapter = machine.dxgiAdapters.find(a => a.luid === config.targetAdapterLuid);
    const display = machine.screens.find(s => s.display === config.targetDisplay);
    if (adapter?.vendorId !== 0x1002 || !display || display.bounds[2] !== 3840 || display.bounds[3] !== 2160 || config.targetRefreshHz !== 240) throw Error('Primary matrix requires the AMD adapter and the physical 3840x2160 / 240 Hz display; inspect and correct the external config. No run was started.');
  }
  const rounds = args[0] === 'smoke' ? 1 : config.rounds ?? 3;
  if (args[0] === 'run' && rounds < 3) throw Error('Real runs require at least three rounds');
  const results = [];
  for (const item of schedule(rounds, config.clients ?? ['original', 'native', 'ntl', 'vlither'])) {
    for (const scenario of ['battledome', 'normal']) {
      if (interrupted) break;
      results.push(await runAttempt(config, session, { ...item, scenario }, provenance, machine));
    }
  }
  const comparisons = [];
  for (let i = 0; i < results.length; i++) for (let j = i + 1; j < results.length; j++) {
    const a = results[i], b = results[j];
    if (a.run.scenario === b.run.scenario && a.run.round === b.run.round) comparisons.push({ a: a.directory, b: b.directory, ...comparison(a.run, b.run) });
  }
  await save(path.join(session, 'index.json'), { mode: args[0], results: results.map(({run, ...item}) => item), comparisons });
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch(e => { console.error(e.message); process.exitCode = 1; });
