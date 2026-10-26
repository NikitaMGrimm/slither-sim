// Linux timing instrument around the existing forage collector; controls are unchanged.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { monitorEventLoopDelay, PerformanceObserver } from 'node:perf_hooks';
import { Bot, record, parseArgs, createCaptureBatch, captureSourceIdentity, runCollectorCli }
  from '../../../tools/fidelity/slither-headless.mjs';

const self = fileURLToPath(import.meta.url);
const repo = path.resolve(path.dirname(self), '../../..');
const sha = p => createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const source = captureSourceIdentity(import.meta.url);

export function hostSample() {
  const cpu = fs.readFileSync('/proc/stat', 'utf8').split('\n')[0].trim().split(/\s+/).slice(1).map(Number);
  const cgroup = Object.fromEntries(fs.readFileSync('/sys/fs/cgroup/cpu.stat', 'utf8').trim()
    .split('\n').map(line => { const [k, v] = line.split(' '); return [k, Number(v)]; }));
  return { cpuJiffies: cpu, loadavg: fs.readFileSync('/proc/loadavg', 'utf8').trim(), cgroup };
}

export function failureClose(bot) {
  // A normal death may rejoin after the old socket's close. Handshake/path failures never retry.
  return !bot.stopped && !bot.deathObservation;
}

async function main(outerSignal) {
  const [planPath, output, roundId] = process.argv.slice(2);
  const plan = JSON.parse(fs.readFileSync(planPath, 'utf8'));
  for (const pin of plan.sourcePins) {
    if (sha(path.join(repo, pin.path)) !== pin.sha256) throw new Error(`changed source: ${pin.path}`);
  }
  if (fs.existsSync(output)) throw new Error('output already exists');
  fs.mkdirSync(output, { recursive: true });
  const controller = new AbortController();
  let failure = null;
  const fail = error => { failure ??= error; controller.abort(error); };
  const abort = () => controller.abort(outerSignal.reason);
  outerSignal.addEventListener('abort', abort, { once: true });
  if (outerSignal.aborted) abort();
  const live = new Set();
  const originalConnect = Bot.prototype.connect, originalReceive = Bot.prototype.receive;
  Bot.prototype.connect = function(server) {
    if (controller.signal.aborted) return;
    originalConnect.call(this, server);
    live.add(this);
    const close = this.ws.onclose, error = this.ws.onerror;
    this.ws.onclose = e => {
      const failed = failureClose(this);
      close(e);
      if (failed) {
        clearTimeout(this.reconnectTimer);
        fail(new Error(`unexpected close ${server.ip}:${server.port} code=${e.code}; no retry`));
      }
    };
    this.ws.onerror = e => { error(e); fail(new Error(`socket error ${server.ip}:${server.port}; no retry`)); };
  };
  let receiveMaxMs = 0, gc = [], timerGapMaxMs = 0, timerLateMaxMs = 0;
  Bot.prototype.receive = function(bytes) {
    const start = performance.now();
    try { return originalReceive.call(this, bytes); }
    finally { receiveMaxMs = Math.max(receiveMaxMs, performance.now() - start); }
  };
  const observer = new PerformanceObserver(list => {
    gc.push(...list.getEntries().map(e => ({ start: e.startTime, duration: e.duration, kind: e.detail.kind })));
  });
  observer.observe({ entryTypes: ['gc'] });
  const loop = monitorEventLoopDelay({ resolution: 1 });
  loop.enable();
  let previous = performance.now(), since = previous, cpu = process.cpuUsage();
  const probe = setInterval(() => {
    const now = performance.now(), gap = now - previous;
    timerGapMaxMs = Math.max(timerGapMaxMs, gap);
    timerLateMaxMs = Math.max(timerLateMaxMs, gap - 1);
    previous = now;
  }, 1);
  const emit = () => {
    const now = performance.now();
    const row = { event: 'study-timing', since, until: now, utc: new Date().toISOString(),
      timerGapMaxMs, timerLateMaxMs, receiveMaxMs, gc,
      loop: { count: loop.count, maxMs: loop.count ? loop.max / 1e6 : 0,
        p50Ms: loop.percentile(50) / 1e6, p99Ms: loop.percentile(99) / 1e6 },
      host: hostSample(), cpuMicroseconds: process.cpuUsage(cpu), memory: process.memoryUsage() };
    fs.appendFileSync(path.join(output, 'load.jsonl'), JSON.stringify(row) + '\n');
    for (const bot of live) if (!bot.rec.sealed && !bot.rec.error) bot.rec.meta({ ...row,
      backpressureWrites: bot.rec.backpressureWrites, maxQueuedBytes: bot.rec.maxQueuedBytes,
      socketBufferedAmount: bot.ws?.bufferedAmount ?? null,
      gzipWritable: bot.rec.gz.writableLength, outputWritable: bot.rec.output.writableLength });
    since = now; cpu = process.cpuUsage();
    receiveMaxMs = timerGapMaxMs = timerLateMaxMs = 0; gc = []; loop.reset();
  };
  const telemetry = setInterval(emit, 1000);
  try {
    if (roundId === 'gauge') {
      // No game sockets: a fixed preflight records Linux's ordinary timer resolution.
      await new Promise(resolve => setTimeout(resolve, 30000));
      emit();
      return 'no-network gauge';
    }
    const batches = plan.batches.filter(b => b.round === roundId);
    if (batches.length !== 2) throw new Error('round must contain exactly two frozen batches');
    const results = await Promise.allSettled(batches.map(batch => {
      const out = path.join(output, batch.id);
      fs.mkdirSync(out);
      const opts = parseArgs(['--out', out, '--server', batch.endpoint, '--seconds', String(batch.seconds),
        '--bots', '1', '--scenario', 'forage', '--nick', '(Bot) timing research',
        '--origin', plan.origin, '--plan', planPath]);
      const [ip, port] = batch.endpoint.split(':');
      const provenance = createCaptureBatch(opts, 1);
      const decorated = { ...provenance, provenance: index => ({ ...provenance.provenance(index),
        studyBatchId: batch.id, timingDriverSha256: source.collectorSha256 }) };
      return record(opts, { ip, port: Number(port) }, 'network', 1, controller.signal, decorated)
        .catch(error => { fail(error); throw error; });
    }));
    failure ??= results.find(r => r.status === 'rejected')?.reason;
    if (failure) throw failure;
    return results.map(r => r.value);
  } finally {
    emit(); clearInterval(telemetry); clearInterval(probe); loop.disable(); observer.disconnect();
    outerSignal.removeEventListener('abort', abort);
    Bot.prototype.connect = originalConnect; Bot.prototype.receive = originalReceive;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === self) runCollectorCli(main);
