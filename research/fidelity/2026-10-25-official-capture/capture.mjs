// Study driver: instrument the existing collector without changing its controls.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { monitorEventLoopDelay, PerformanceObserver } from 'node:perf_hooks';
import { Bot, record, parseArgs, createCaptureBatch, captureSourceIdentity, runCollectorCli }
  from '../../../tools/fidelity/slither-headless.mjs';

const self = fileURLToPath(import.meta.url);
const source = captureSourceIdentity(import.meta.url);
const sha = p => createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const histogram = () => Array(77).fill(0);
const add = (h, ms) => h[Math.min(76, Math.max(0, Math.floor(ms)))]++;
const live = new Set();
let costs = histogram(), gc = [], gaps = histogram(), lateness = histogram();
let expected = performance.now() + 1, previous = expected - 1;
const connect = Bot.prototype.connect;
Bot.prototype.connect = function(server) { live.add(this); return connect.call(this, server); };
const receive = Bot.prototype.receive;
Bot.prototype.receive = function(bytes) {
  const start = performance.now();
  try { return receive.call(this, bytes); }
  finally { add(costs, performance.now() - start); }
};
const observer = new PerformanceObserver(list => {
  for (const e of list.getEntries()) gc.push({ start: e.startTime, duration: e.duration, kind: e.detail.kind });
});
observer.observe({ entryTypes: ['gc'] });

async function main(signal) {
  const [planPath, output, batchId] = process.argv.slice(2);
  const plan = JSON.parse(fs.readFileSync(planPath, 'utf8'));
  const batch = plan.batches.find(b => b.id === batchId);
  if (!batch) throw new Error('batch not frozen');
  for (const pin of plan.sourcePins) {
    if (sha(pin.path) !== pin.sha256) throw new Error(`changed source: ${pin.path}`);
  }
  if (sha(self) !== source.collectorSha256) throw new Error('driver changed');
  if (fs.existsSync(output)) throw new Error('output already exists');
  fs.mkdirSync(output, { recursive: true });
  const delay = monitorEventLoopDelay({ resolution: 1 });
  delay.enable();
  const probe = setInterval(() => {
    const now = performance.now();
    add(gaps, now - previous); add(lateness, now - expected);
    previous = now; expected = now + 1;
  }, 1);
  let cpu = process.cpuUsage(), since = performance.now();
  const emit = () => {
    const now = performance.now(), usage = process.cpuUsage(cpu);
    const telemetry = { event: 'study-timing', batchId, since, until: now,
      bins: 'floor(ms), indices 0..75 and 76=at-least-76ms',
      timerGapBins: gaps, timerLatenessBins: lateness, receiveCostBins: costs,
      loop: { count: delay.count, minMs: delay.min / 1e6, maxMs: delay.max / 1e6,
        p50Ms: delay.percentile(50) / 1e6, p95Ms: delay.percentile(95) / 1e6,
        p99Ms: delay.percentile(99) / 1e6 },
      gc, memory: process.memoryUsage(), cpuMicroseconds: usage };
    for (const bot of live) if (!bot.rec.sealed && !bot.rec.error) {
      bot.rec.meta({ ...telemetry, socketBufferedAmount: bot.ws?.bufferedAmount ?? null,
        backpressureWrites: bot.rec.backpressureWrites, maxQueuedBytes: bot.rec.maxQueuedBytes,
        gzipWritable: bot.rec.gz.writableLength, outputWritable: bot.rec.output.writableLength });
    }
    cpu = process.cpuUsage(); since = now;
    gaps = histogram(); lateness = histogram(); costs = histogram(); gc = []; delay.reset();
  };
  const telemetryTimer = setInterval(emit, 1000);
  const run = scenario => {
    const opts = parseArgs(['--out', output, '--server', batch.endpoint, '--seconds', String(batch.seconds),
      '--bots', '1', '--scenario', scenario, '--nick', '(Bot) timing research', '--plan', planPath]);
    const [ip, port] = batch.endpoint.split(':');
    const provenance = createCaptureBatch(opts, 1);
    const decorated = { ...provenance, provenance: index => ({ ...provenance.provenance(index),
      studyBatchId: batchId, timingDriverSha256: source.collectorSha256 }) };
    return record(opts, { ip, port: Number(port) }, scenario === 'spawn' ? 'spawn' : 'network',
      1, signal, decorated);
  };
  try {
    const results = await Promise.allSettled([run('forage'), run('spawn')]);
    const failure = results.find(r => r.status === 'rejected');
    if (failure) throw failure.reason;
    return results.map(r => r.value);
  }
  finally {
    clearInterval(telemetryTimer); clearInterval(probe); delay.disable(); observer.disconnect();
  }
}

runCollectorCli(main);
