// Attempt 3 outer guard. The capture and analysis modules remain unchanged.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { verifySources } from './capture.mjs';
import { reserveOrdinal } from './selection.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const ignored = new Set(['TIME-WAIT', 'CLOSED', 'LISTEN', 'BOUND']);
export function classify(snapshot, endpoint, beforeLaunch = false) {
  if (!snapshot.windows?.ok || !snapshot.remote?.ok) throw Error('connection census unavailable');
  const owned = new Set(snapshot.remote.ownedPids.map(String));
  const conflicts = snapshot.windows.connections.map(c => ({ machine: 'controller', ...c }));
  let ours = 0;
  for (const line of snapshot.remote.ss.split('\n').filter(s => s.trim())) {
    const fields = line.trim().split(/\s+/);
    if (fields.length < 5) throw Error('malformed connection census');
    if (ignored.has(fields[0])) continue;
    const peer = fields[4].replace(/^\[::ffff:(.*)\]:(\d+)$/, '$1:$2');
    if (peer !== endpoint) throw Error('unexpected census endpoint');
    const pids = [...line.matchAll(/pid=(\d+)/g)].map(m => m[1]);
    if (pids.length && pids.every(p => owned.has(p))) ours++;
    else conflicts.push({ machine: 'separate', line, pids,
      commandLines: pids.map(p => snapshot.remote.commandLines[p] ?? 'unavailable') });
  }
  if (ours > 2 || beforeLaunch && ours) conflicts.push({ machine: 'separate', reason: 'own connection bound', ours });
  return { conflicts, ours };
}

export async function preflight(check, record, { now = () => performance.now(), sleep = pause } = {}) {
  for (let index = 0; index <= 4; index++) {
    const state = await check();
    record({ event: 'preflight-check', index, passed: !state.conflicts.length, conflicts: state.conflicts });
    if (!state.conflicts.length) return;
    if (index === 4) throw Error('exclusivity re-check budget exhausted');
    const deadline = now() + 900000;
    record({ event: 'preflight-wait', index, seconds: 900 });
    for (;;) {
      const remaining = deadline - now();
      if (remaining <= 0) break;
      await sleep(Math.min(30000, remaining));
    }
  }
}

export function snapshotReader(file, instance, now = () => performance.now()) {
  let sequence = null, observed = now(), snapshot = null;
  return () => {
    try {
      const incoming = JSON.parse(fs.readFileSync(file, 'utf8'));
      if (incoming.instance === instance && incoming.sequence !== sequence) {
        if (!Number.isSafeInteger(incoming.sequence) || incoming.sequence < 1 ||
            sequence !== null && incoming.sequence < sequence) throw Error('invalid census sequence');
        sequence = incoming.sequence; observed = now(); snapshot = incoming;
      }
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
    if (now() - observed > 15000) throw Error('connection monitor lease expired');
    return snapshot;
  };
}

function append(file, value) {
  const fd = fs.openSync(file, 'a');
  try { fs.writeSync(fd, JSON.stringify(value) + '\n'); fs.fsyncSync(fd); }
  finally { fs.closeSync(fd); }
}
function create(file, value) { fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' }); }

export async function run(root, instance, { captureFile = path.join(HERE, 'capture.mjs') } = {}) {
  if (!/^[a-zA-Z0-9_.-]+$/.test(instance)) throw Error('invalid guard instance');
  verifySources();
  const endpoint = JSON.parse(fs.readFileSync(path.join(root, 'endpoint-private.json')));
  if (!/^(\d{1,3}\.){3}\d{1,3}$/.test(endpoint.ip) || !Number.isInteger(endpoint.port) || endpoint.port < 1 || endpoint.port > 65535)
    throw Error('invalid private endpoint');
  for (const name of ['STOP', 'STUDY-STOP.json']) if (fs.existsSync(path.join(root, name))) throw Error('permanent study stop');
  const lock = path.join(root, '.container-capture.lock');
  const lockFd = fs.openSync(lock, 'wx');
  const eventFile = path.join(root, `guard-${instance}-private.jsonl`);
  const record = value => append(eventFile, { wall: Date.now(), ...value });
  let child = null, childExit = null, exited = null, ended = false, failure = null, ordinal = null, started = null;
  let interrupted = false;
  const signal = () => { interrupted = true; };
  process.on('SIGTERM', signal); process.on('SIGINT', signal);
  const read = snapshotReader(path.join(root, 'connection-snapshot-private.json'), instance);
  const check = async before => {
    if (interrupted) throw Error('guard interrupted');
    let snapshot;
    while (!(snapshot = read())) { await pause(100); if (interrupted) throw Error('guard interrupted'); }
    const state = classify(snapshot, `${endpoint.ip}:${endpoint.port}`, before);
    if (state.conflicts.length) record({ event: 'connection-conflict', snapshot, ...state });
    return state;
  };
  try {
    const ledgerPath = path.join(root, 'invocations.jsonl');
    const previous = fs.existsSync(ledgerPath) ? fs.readFileSync(ledgerPath, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse) : [];
    ordinal = reserveOrdinal(previous);
    if (ordinal >= 3) {
      const gate = JSON.parse(fs.readFileSync(path.join(root, 'heldout-gate.json')));
      if (!gate.coveredAllPrimary || !gate.freezeCommit) throw Error('held-out gate closed');
    }
    await preflight(() => check(true), record, { sleep: async ms => {
      for (let remaining = ms; remaining > 0; remaining -= 500) {
        if (interrupted) throw Error('guard interrupted');
        await pause(Math.min(500, remaining)); read();
      }
    } });
    verifySources();
    append(ledgerPath, { ordinal, reservedWall: Date.now(), instance });
    create(path.join(root, 'guard-active.json'), { ordinal, instance, pid: process.pid });
    const stdout = fs.openSync(path.join(root, `capture-${ordinal}-stdout.txt`), 'wx');
    const stderr = fs.openSync(path.join(root, `capture-${ordinal}-stderr.txt`), 'wx');
    started = performance.now();
    try { child = spawn(process.execPath, [captureFile, root], { stdio: ['ignore', stdout, stderr] }); }
    finally { fs.closeSync(stdout); fs.closeSync(stderr); }
    exited = new Promise(resolve => {
      child.once('error', error => { ended = true; failure ??= error; resolve({ error: String(error) }); });
      child.once('exit', (code, exitSignal) => { ended = true; resolve({ code, signal: exitSignal }); });
    });
    while (!ended) {
      if (performance.now() - started >= 1200000) throw Error('hard invocation deadline');
      const state = await check(false);
      if (state.conflicts.length) throw Error('post-launch connection conflict');
      if (failure) throw failure;
      if (fs.existsSync(path.join(root, 'STUDY-STOP.json'))) throw Error('capture permanent stop');
      await pause(250);
    }
    childExit = await exited;
    if (childExit.code !== 0) throw Error('capture child failed');
    const result = JSON.parse(fs.readFileSync(path.join(root, `invocation-${ordinal}/result-private.json`)));
    if (!result.socketsClosed || !result.completed) throw Error('unsealed or interrupted invocation');
    if (result.reason === 'both-default-clients-failed') throw Error('binding unreachable under unchanged client defaults');
    if (result.elapsedSeconds > 1200) throw Error('invocation exceeded bound');
    // Require a fresh post-close census, including closing TCP states, before
    // considering another invocation. The next invocation has its own preflight.
    const last = read()?.sequence;
    while (read()?.sequence === last) {
      await pause(100);
      if (interrupted || performance.now() - started >= 1200000) throw Error('post-close census deadline');
    }
    const final = await check(false);
    if (final.conflicts.length || final.ours) throw Error('connections remain after capture close');
    record({ event: 'guard-complete', ordinal, childExit, elapsedSeconds: (performance.now() - started) / 1000 });
  } catch (error) {
    failure ??= error;
    record({ event: 'guard-failed', ordinal, reason: String(error) });
    if (child && !ended) {
      if (!fs.existsSync(path.join(root, 'STOP'))) fs.writeFileSync(path.join(root, 'STOP'), String(error), { flag: 'wx' });
      const remaining = Math.max(0, 1200000 - (performance.now() - started));
      const until = performance.now() + Math.min(5000, remaining);
      while (!ended && performance.now() < until) await pause(50);
      if (!ended) child.kill('SIGKILL');
      await exited;
    }
    if (!fs.existsSync(path.join(root, 'STUDY-STOP.json')))
      create(path.join(root, 'STUDY-STOP.json'), { ordinal, reason: String(error) });
  } finally {
    record({ event: 'guard-end', ordinal, failed: !!failure, childCode: child?.exitCode, childSignal: child?.signalCode });
    const active = path.join(root, 'guard-active.json');
    if (fs.existsSync(active)) fs.unlinkSync(active);
    fs.closeSync(lockFd); fs.unlinkSync(lock);
    process.off('SIGTERM', signal); process.off('SIGINT', signal);
  }
  if (failure) throw failure;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  run(process.argv[2], process.argv[3]).catch(error => {
    const stop = path.join(process.argv[2], 'STUDY-STOP.json');
    if (!fs.existsSync(stop)) create(stop, { reason: String(error) });
    console.error(error); process.exitCode = 1;
  });
