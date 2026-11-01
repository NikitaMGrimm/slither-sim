import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const clientRoot = fileURLToPath(new URL('..', import.meta.url));
const usage = 'node tools/e2e-local-host.mjs --host-bin PATH [--client-bin PATH] [--seconds 5..60] [--battledome 500] [--scratch PATH]';
const fail = (stage, message) => Object.assign(new Error(message), { stage });

export function argumentsFor(args) {
  const options = { seconds: 8, clientBin: path.join(clientRoot, 'target/wgpu-slice-check/debug',
    `slither-client-e2e${process.platform === 'win32' ? '.exe' : ''}`) };
  const seen = new Set();
  for (let i = 0; i < args.length; i += 2) {
    const key = { '--host-bin': 'hostBin', '--client-bin': 'clientBin', '--seconds': 'seconds', '--battledome': 'battledome', '--scratch': 'scratch' }[args[i]];
    const value = args[i + 1];
    if (!key || !value || value.startsWith('--') || seen.has(key)) throw fail('arguments', usage);
    seen.add(key);
    options[key] = ['seconds', 'battledome'].includes(key) ? Number(value) : path.resolve(value);
  }
  if (!options.hostBin || !Number.isFinite(options.seconds) || options.seconds < 5 || options.seconds > 60
    || options.battledome !== undefined && options.battledome !== 500) {
    throw fail('arguments', usage);
  }
  return options;
}

export function allowedPort(port) {
  return Number.isInteger(port) && port > 0 && port <= 65535 && ![8766, 18884, 18885].includes(port);
}

export function readyEndpoint(output) {
  const match = /^Listening on (\S+)\r?\n/m.exec(output);
  if (!match) return null;
  const endpoint = /^ws:\/\/127\.0\.0\.1:([1-9][0-9]*)\/slither$/.exec(match[1]);
  if (!endpoint || !allowedPort(Number(endpoint[1]))) throw fail('readiness', 'Host reported an inadmissible loopback endpoint');
  return match[1];
}

export function assertSummary(s, protocol = 19, battledome = false) {
  if (!s || typeof s !== 'object') throw fail('summary', 'Missing client summary');
  const stages = [
    ['handshake', s.websocket === true && s.challenge === true],
    ['login', s.loginSent === true],
    ['setup', s.setup === true && s.protocol === protocol],
    ['own-snake', s.ownSnake === true],
    ['steering', Number.isInteger(s.steeringCommands) && s.steeringCommands > 0 && s.moved === true && s.turned === true],
    ['boost', s.boostOn === true && s.boostOff === true && s.accelerated === true && s.decelerated === true],
    ['leaderboard', s.leaderboard === true],
    ['minimap', s.minimap === true],
    ['presentation', Number.isInteger(s.preparedFrames) && s.preparedFrames > 0],
    ['close', s.cleanClose === true],
  ];
  if (battledome) {
    stages.splice(4, 2);
    stages.push(['full-map', s.fullMap === true], ['picture-skips', s.pictureSkips === 0],
      ['population', Number.isInteger(s.peakReceivedSnakes) && s.peakReceivedSnakes > 0
        && Number.isInteger(s.peakPreparedSnakes) && s.peakPreparedSnakes > 0]);
  }
  for (const [stage, valid] of stages) if (!valid) throw fail(stage, `Client assertion failed: ${stage}`);
  return s;
}

function launch(binary, args, env = process.env) {
  const child = spawn(binary, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env });
  const listeners = new Set();
  const state = { child, stdout: '', stderr: '', listeners, result: null };
  for (const stream of ['stdout', 'stderr']) {
    child[stream].setEncoding('utf8');
    child[stream].on('data', data => {
      state[stream] = (state[stream] + data).slice(-65536);
      for (const listener of listeners) listener();
    });
  }
  state.done = new Promise(resolve => {
    child.once('error', error => { state.result = { error: error.message }; resolve(state.result); });
    child.once('close', (code, signal) => { state.result ??= { code, signal }; resolve(state.result); });
  });
  return state;
}

async function bounded(promise, ms, stage, signal) {
  let timer;
  let aborted;
  const limit = new Promise((_, reject) => {
    timer = setTimeout(() => reject(fail(stage, `${stage} timed out after ${ms / 1000}s`)), ms);
    aborted = () => reject(fail('interrupted', 'Run interrupted'));
    signal?.addEventListener('abort', aborted, { once: true });
    if (signal?.aborted) aborted();
  });
  try { return await Promise.race([promise, limit]); }
  finally { clearTimeout(timer); signal?.removeEventListener('abort', aborted); }
}

async function readiness(host, signal) {
  let listener;
  const ready = new Promise((resolve, reject) => {
    listener = () => {
      try {
        const endpoint = readyEndpoint(host.stdout) ?? readyEndpoint(host.stderr);
        if (endpoint) resolve(endpoint);
      } catch (error) { reject(error); }
    };
    host.listeners.add(listener);
    listener();
  });
  try {
    return await bounded(Promise.race([ready, host.done.then(result => {
      throw fail('readiness', `Host exited before readiness: ${JSON.stringify(result)}; ${host.stderr}`);
    })]), 20000, 'readiness', signal);
  } finally { host.listeners.delete(listener); }
}

async function stop(state) {
  if (!state || state.result) return;
  state.child.kill();
  try { await bounded(state.done, 2000, 'cleanup'); }
  catch {
    state.child.kill('SIGKILL');
    await bounded(state.done, 2000, 'cleanup');
  }
}

export async function runWithCleanup(session, cleanup) {
  let result, sessionError, cleanupError;
  let sessionFailed = false, cleanupFailed = false;
  try { result = await session(); }
  catch (error) { sessionError = error; sessionFailed = true; }
  try { await cleanup(); }
  catch (error) { cleanupError = error; cleanupFailed = true; }
  // Keep the original stage and partial client summary if teardown also fails.
  if (sessionFailed) {
    if (cleanupFailed && sessionError instanceof Error && Object.isExtensible(sessionError)) {
      sessionError.cleanupError = cleanupError;
    }
    throw sessionError;
  }
  if (cleanupFailed) throw cleanupError;
  return result;
}

export async function runLocalHost(options) {
  const protocol = Number(process.env.SLITHER_CLIENT_E2E_PROTOCOL ?? 19);
  if (![13, 19].includes(protocol)) throw fail('arguments', 'SLITHER_CLIENT_E2E_PROTOCOL must be 13 or 19');
  const started = performance.now();
  const controller = new AbortController();
  const interrupt = () => controller.abort();
  process.once('SIGINT', interrupt);
  process.once('SIGTERM', interrupt);
  let directory, host, client;
  return runWithCleanup(async () => {
    directory = await mkdtemp(path.join(options.scratch ?? os.tmpdir(), 'slither-client-e2e-'));
    const config = path.join(directory, 'host.json');
    await writeFile(config, JSON.stringify({
      bind: '127.0.0.1:0', seed: 7, players: { max: 1 }, bots: { count: options.battledome ?? 0, threads: 2 },
      simulation: {
        world: options.battledome ? { preset: 'battledome', vision: 40000 } : { radius: 10000, dynamicRadius: false, sizeToPopulation: false,
          spawnRadius: 3000, minScore: 400, foodCount: 0, ambientReplenishPerSecond: 0,
          prey: { count: 0 }, kiosk: { enabled: false } },
        leaderboardIntervalSeconds: 0.5, minimapIntervalSeconds: 0.5,
      },
    }) + '\n');
    // Host owns the port-0 listener throughout startup: no reserve/release race.
    host = launch(options.hostBin, ['host', '--config', config, '--bind', '127.0.0.1:0']);
    const endpoint = await readiness(host, controller.signal);
    if (options.battledome) {
      await bounded(Promise.race([new Promise(resolve => setTimeout(resolve, 15000)), host.done.then(result => {
        throw fail('host-fill', `Host exited while filling: ${JSON.stringify(result)}`);
      })]), 17000, 'host-fill', controller.signal);
    }
    const server = endpoint.slice('ws://'.length, -'/slither'.length);
    client = launch(options.clientBin, ['--server', server, '--seconds', String(options.seconds)],
      { ...process.env, ...(options.battledome ? { SLITHER_CLIENT_E2E_FULL_MAP: '1' } : {}) });
    const result = await bounded(Promise.race([client.done, host.done.then(result => {
      throw fail('host', `Host exited during client session: ${JSON.stringify(result)}`);
    })]), (options.seconds + 30) * 1000, 'client', controller.signal);
    if (result.error) throw fail('client-start', result.error);
    let summary;
    try { summary = JSON.parse(client.stdout.trim()); }
    catch { throw fail('summary', `Client produced no valid JSON: ${client.stderr}; ${JSON.stringify(result)}`); }
    try {
      assertSummary(summary, protocol, Boolean(options.battledome));
      if (result.code !== 0) throw fail('client-exit', JSON.stringify(result));
    } catch (error) {
      error.client = summary;
      if (client.stderr) error.message += `; ${client.stderr.trim()}`;
      throw error;
    }
    return { ok: true, endpoint, protocolCoverage: [protocol], client: summary,
      seconds: Number(((performance.now() - started) / 1000).toFixed(3)) };
  }, async () => {
    // All launched children are direct, shell-free binaries. Reap both even if one cleanup fails.
    const results = await Promise.allSettled([stop(client), stop(host)]);
    try { if (directory) await rm(directory, { recursive: true, force: true }); }
    catch (error) { results.push({ status: 'rejected', reason: error }); }
    process.removeListener('SIGINT', interrupt);
    process.removeListener('SIGTERM', interrupt);
    const error = results.find(result => result.status === 'rejected');
    if (error) throw error.reason;
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { console.log(JSON.stringify(await runLocalHost(argumentsFor(process.argv.slice(2))))); }
  catch (error) {
    console.log(JSON.stringify({ ok: false, stage: error.stage ?? 'runner', error: error.message,
      ...(error.client ? { client: error.client } : {}) }));
    process.exitCode = 1;
  }
}
