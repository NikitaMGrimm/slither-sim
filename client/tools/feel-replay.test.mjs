// Authored synthetic fault matrix. No recordings or game servers.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compareTraces } from './feel-replay/compare.mjs';
import { buildManifest, parse, readTrace, scheduleHash, sha256, stringify } from './feel-replay/io.mjs';
import { orderEvents, validateSchedule, validateTrace } from './feel-replay/schema.mjs';
import { syntheticFixture } from './feel-replay/synthetic.mjs';
const cli = fileURLToPath(new URL('./feel-replay.mjs', import.meta.url));
function peers() { const { schedule, trace: left } = syntheticFixture(); return { schedule, left, right: structuredClone(left) }; }
const run = p => compareTraces(p.left, p.right, p.schedule);
test('v3 retains skipped draw boundaries but counts only acknowledged pictures', () => {
  const { schedule, trace } = syntheticFixture();
  const skipped = trace.samples.filter(s => s.phase === 'draw').at(-1);
  Object.assign(skipped.status, { prepared: true, submitted: false, committed: false });
  skipped.quality.successfulDraws = 1; skipped.acknowledgedMs = null;
  trace.final.draws = 1; schedule.pictures = [0];
  assert.equal(validateTrace(trace, schedule), trace);
  trace.final.draws = 2;
  assert.throws(() => validateTrace(trace, schedule), /incomplete final boundary\/counters/);
});
test('every sample reports the cumulative committed draw count, including updates and skips', () => {
  const { schedule, trace } = syntheticFixture();
  assert.equal(validateTrace(trace, schedule), trace);
  for (const index of [0, 2, 3, 4]) {
    const changed = parse(stringify(trace));
    changed.samples[index].quality.successfulDraws++;
    assert.throws(() => validateTrace(changed, schedule), /quality.successfulDraws: expected cumulative committed draw count/);
  }
  const skipped = trace.samples[4];
  Object.assign(skipped.status, { submitted: false, committed: false });
  trace.final.draws = 1; skipped.acknowledgedMs = null; schedule.pictures = [0];
  assert.throws(() => validateTrace(trace, schedule), /quality.successfulDraws: expected cumulative committed draw count/);
  skipped.quality.successfulDraws = 1;
  assert.equal(validateTrace(trace, schedule, [0]), trace);
  assert.throws(() => validateTrace(trace, schedule, []), /picture acknowledgement mismatch/);
  assert.throws(() => validateTrace(trace, schedule, [0, 0]), /duplicate ID/);
  assert.throws(() => validateTrace(trace, schedule, [2]), /no matching draw/);
});
test('stable equal-time schedule is packet -> input -> frame, preserving source order', () => {
  const events = ['frame', 'input', 'packet', 'packet', 'input'].map((kind, i) => ({ kind, sourceId: String(i), atMs: 0 }));
  assert.deepEqual(orderEvents(events).map(e => e.sourceId), ['2', '3', '1', '4', '0']);
  const { schedule } = syntheticFixture();
  [schedule.events[0], schedule.events[1]] = [schedule.events[1], schedule.events[0]];
  assert.throws(() => validateSchedule(schedule), /packet -> input -> frame/);
});
test('a dangling pre-draw reference fails even when identical traces have no draws', () => {
  const { schedule, trace } = syntheticFixture();
  schedule.boundaries = schedule.boundaries.filter(b => b.phase !== 'draw');
  schedule.boundaries[1].drawId = 0; schedule.pictures = [];
  trace.samples = trace.samples.filter(s => s.phase !== 'draw');
  trace.samples[1].drawId = 0;
  Object.assign(trace.final, { samples: 3, draws: 0, lastBoundaryId: 'update-1' });
  // Hash the authored invalid schedule without invoking its validator.
  trace.manifest.scheduleSha256 = sha256(stringify(schedule));
  const report = compareTraces(trace, trace, schedule);
  assert.match(JSON.stringify(report), /schedule.boundaries\[1\].drawId: unknown draw/);
  assert.equal(report.status, 'invalid');
  assert.throws(() => validateSchedule(schedule), /schedule.boundaries\[1\].drawId: unknown draw/);
});
test('a pre-draw reference passes when a later draw realizes its ordinal', () => {
  const { schedule, trace } = syntheticFixture();
  schedule.boundaries[1].drawId = 0;
  trace.samples[1].drawId = 0;
  trace.manifest.scheduleSha256 = scheduleHash(schedule);
  assert.equal(validateSchedule(schedule), schedule);
  assert.equal(compareTraces(trace, trace, schedule).status, 'perceptual-match');
});

test('ordinary finite JSON round-trips cadence edges, unrestricted keys and extreme numbers', () => {
  const values = [Number.MIN_VALUE, Number.MAX_VALUE, 1000 / 60, 32.99999999999999, 33, 33.00000000000001, 0.9999999999999999];
  assert.deepEqual(parse(stringify({ '0': values, 'é😀': 'finite' }))['0'], values);
  assert.equal(parse(stringify(-0)), 0);
  for (const value of [Infinity, -Infinity, NaN, undefined]) assert.throws(() => stringify(value));
  assert.throws(() => parse('{"value":1e999}'), /finite/);
  assert.throws(() => parse('{'), SyntaxError);
});

// Use 40 own draws so percentile and quarter rules are independently testable.
function history() {
  const p = peers(), base = p.left.samples[2];
  p.schedule.events = Array.from({ length: 40 }, (_, i) => ({ sourceId: 'frame-' + i, kind: 'frame', atMs: i * 20 }));
  p.schedule.boundaries = p.schedule.events.map((e, i) => ({ id: e.sourceId + '-draw', eventSeq: i, phase: 'draw', frameId: i, drawId: i, atMs: e.atMs, eligibility: 'own' }));
  p.schedule.pictures = p.schedule.boundaries.map(b => b.drawId);
  p.schedule.durationMs = 780; p.schedule.expectedCommands = 0;
  p.left.events = p.schedule.events.map((e, seq) => ({ ...p.left.events[2], seq, sourceId: e.sourceId, scheduledMs: e.atMs, actualMs: e.atMs, effectIds: [] }));
  p.left.samples = p.schedule.boundaries.map((b, i) => ({ ...structuredClone(base), ...b, acknowledgedMs: b.atMs, quality: { ...base.quality, successfulDraws: i + 1 } }));
  p.left.commands = [];
  p.left.final = { ...p.left.final, atMs: 780, events: 40, commands: 0, samples: 40, draws: 40, lastBoundaryId: p.schedule.boundaries.at(-1).id };
  p.left.manifest.scheduleSha256 = scheduleHash(p.schedule);
  p.right = structuredClone(p.left);
  return p;
}
const setPath = (sample, channel, delta) => {
  if (channel === 'head') sample.own.projected.css.head.x += delta;
  if (channel === 'camera') sample.camera.x += delta * sample.geometry.backing.width / sample.geometry.css.width;
  if (channel === 'body') sample.own.body[0].x += delta * sample.geometry.backing.width / sample.geometry.css.width;
  if (channel === 'pupils') sample.own.projected.css.pupils[0].x += delta;
  if (channel === 'heading') sample.own.correctedHeading += delta * Math.PI / 180;
  if (channel === 'phase') sample.own.opacity += delta;
  if (channel === 'zoom') sample.camera.zoom += delta;
};
for (const [channel, p95, max] of [['head', 0.5, 1], ['camera', 0.5, 1], ['body', 0.5, 1], ['pupils', 0.5, 1], ['heading', 0.25, 0.5], ['phase', 0.01, 0.02]]) {
  for (const percentile of [false, true]) for (const factor of [0.99999, 1.00001]) test(channel + ' ' + (percentile ? 'p95' : 'max') + ' edge ' + factor, () => {
    const p = history();
    if (channel === 'phase') for (const t of [p.left, p.right]) for (const s of t.samples) s.own.opacity = 0.3;
    for (const s of percentile ? p.right.samples : p.right.samples.slice(0, 1)) setPath(s, channel, (percentile ? p95 : max) * factor);
    assert.equal(run(p).passed, factor < 1, JSON.stringify(run(p).mismatches));
  });
}
for (const [label, zoom, width, limit] of [['absolute', 1, 960, 0.0005], ['relative', 0.1, 960, 0.0001], ['edge', 1, 10000, 1 / Math.hypot(5000, 270)]]) {
  for (const factor of [0.99999, 1.00001]) test('zoom ' + label + ' edge ' + factor, () => {
    const p = history();
    for (const t of [p.left, p.right]) for (const s of t.samples) { s.camera.zoom = zoom; s.geometry.css.width = width; }
    p.right.samples[0].camera.zoom += limit * factor;
    assert.equal(run(p).passed, factor < 1, JSON.stringify(run(p).mismatches));
  });
}
for (const hz of [50, 144]) for (const factor of [0.99999, 1.00001]) test('transition frame limit ' + hz + ' Hz ' + factor, () => {
  const p = history();
  for (const e of p.schedule.events) e.atMs *= 50 / hz;
  for (const b of p.schedule.boundaries) b.atMs *= 50 / hz;
  p.schedule.durationMs *= 50 / hz;
  for (const t of [p.left, p.right]) {
    t.events.forEach((e, i) => { e.actualMs = e.scheduledMs = p.schedule.events[i].atMs; });
    t.samples.forEach((s, i) => { s.atMs = s.acknowledgedMs = p.schedule.boundaries[i].atMs; });
    t.final.atMs = p.schedule.durationMs; t.manifest.scheduleSha256 = scheduleHash(p.schedule);
  }
  p.right.samples[0].own.spawnProgress += Math.min(20, 1000 / hz) * 0.015 / 8 * factor;
  // Phase has its own stricter p95; one changed row leaves that percentile zero.
  assert.equal(run(p).mismatches.some(m => m.path.endsWith('.transition')), factor > 1, JSON.stringify(run(p).mismatches));
});
test('shortest heading wrap passes and an excessive wrapped error fails', () => {
  const p = history(); p.left.samples[0].own.gaze = 2 * Math.PI - 0.001; p.right.samples[0].own.gaze = 0.001;
  assert(run(p).passed);
  p.right.samples[0].own.gaze = 0.02; assert(!run(p).passed);
});
test('phase endpoints and semantic decisions remain exact', () => {
  const p = peers(); p.right.samples[1].own.deathProgress = 1e-6; assert(!run(p).passed);
  p.right = structuredClone(p.left); p.right.samples[1].own.boost = true; assert(!run(p).passed);
});
test('Euclidean vector budget and independent camera/head prevent cancellation', () => {
  const p = history(); p.right.samples[0].own.projected.css.head = { x: 0.8, y: 0.8 }; assert(!run(p).passed);
  p.right = structuredClone(p.left);
  for (const s of p.right.samples) { s.camera.x += 2; s.own.correctedHead.x += 2; }
  assert(!run(p).passed); assert(run(p).mismatches.some(m => m.path.endsWith('camera.position')));
});
test('500 ms bias is flagged, signed mean retained, quarter RMS growth fails', () => {
  const p = history(); for (const s of p.right.samples) s.camera.zoom += 0.0003;
  let report = run(p); assert(report.passed); assert(report.channels.some(c => c.bias500ms));
  assert(report.channels.find(c => c.channel.endsWith('zoom.absolute')).signedMean > 0);
  p.right = structuredClone(p.left);
  for (const s of p.right.samples.slice(-10)) s.camera.zoom += 0.0003;
  report = run(p); assert(!report.passed); assert(report.channels.some(c => c.rmsGrowth));
});
for (const [label, mutate] of [
  ['dropped event', t => t.events.pop()], ['duplicate event', t => t.events.push(t.events[0])],
  ['dropped sample', t => t.samples.splice(1, 1)], ['duplicated draw', t => t.samples.push(t.samples[2])],
  ['missing ack', t => { t.samples[2].status.committed = false; }],
  ['duplicate ack count', t => { t.final.draws++; }],
  ['changed byte', t => { t.commands[0].bytes[0]++; }],
  ['changed send time', t => { t.commands[0].sendMs = t.commands[0].ackMs = 0.001; }],
  ['changed decision time', t => { t.commands[0].decisionMs = t.commands[0].sendMs = t.commands[0].ackMs = 0.001; }],
  ['reordered events', t => t.events.reverse()], ['changed input', t => { t.events[1].coordinates.x++; }],
  ['missing tail', t => { t.samples.pop(); t.final.samples--; }], ['unfinished', t => { t.final.completed = false; }],
  ['incomplete manifest', t => { t.manifest.completed = false; }], ['pending work', t => { t.final.pendingFrames++; }],
  ['missing field', t => { delete t.samples[1].own.pupils.left; }],
  ['invalid ID', t => { t.samples[1].frameId = 2 ** 53; }], ['nonfinite', t => { t.samples[1].camera.x = NaN; }],
]) test(label + ' fails', () => { const p = peers(); mutate(p.right); assert(!run(p).passed); });
test('visual signed zero and non-f32 output values use the envelopes', () => {
  const p = peers(); p.left.samples[1].own.offsets.y = 0; p.right.samples[1].own.offsets.y = -0;
  p.right.samples[1].own.offsets.x += 1e-8;
  assert(run(p).passed);
});
test('bias needs 500 ms of signed mean and enough draws for RMS quarters', () => {
  const p = history();
  for (let i = 0; i < 25; i++) p.right.samples[i].camera.zoom += 0.0003;
  assert(!run(p).channels.find(c => c.channel.endsWith('zoom.absolute')).bias500ms);
  p.right.samples[25].camera.zoom += 0.0003;
  assert(run(p).channels.find(c => c.channel.endsWith('zoom.absolute')).bias500ms);
});
test('finite extremes still produce a serializable failure report', () => {
  const p = peers(); p.left.samples[1].camera.x = -Number.MAX_VALUE; p.right.samples[1].camera.x = Number.MAX_VALUE;
  const report = run(p); assert(!report.passed); assert.doesNotThrow(() => stringify(report));
});
test('two identically truncated peers cannot agree away a missing tail', () => {
  const p = peers(); p.left.samples.pop(); p.right = structuredClone(p.left); assert.equal(run(p).status, 'invalid');
});
test('different source/binary identities are allowed, tape and schedule stay bound', () => {
  const p = peers(); p.right.manifest.source.commit = 'f'.repeat(40); p.right.manifest.artifacts[0].sha256 = 'e'.repeat(64);
  assert(run(p).passed); p.right.manifest.scheduleSha256 = '0'.repeat(64); assert(!run(p).passed);
});
test('manifest hashing, trace reader and CLI use temporary synthetic files', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'feel-replay-synthetic-'));
  try {
    const p = peers(), artifact = path.join(dir, 'artifact'), input = path.join(dir, 'input');
    await writeFile(artifact, 'SYNTHETIC artifact'); await writeFile(input, 'SYNTHETIC input');
    const { artifacts, ...fields } = p.left.manifest;
    const manifest = await buildManifest({ ...fields, schedule: p.schedule, artifactFiles: { synthetic: artifact }, inputFile: input });
    assert.equal(manifest.artifacts[0].sha256, sha256(await readFile(artifact)));
    assert.equal(manifest.input.sha256, sha256(await readFile(input)));
    const files = ['schedule', 'left', 'right'].map(n => path.join(dir, n + '.json'));
    for (const [i, v] of [p.schedule, p.left, p.right].entries()) await writeFile(files[i], stringify(v));
    assert.deepEqual(await readTrace(files[1], p.schedule), p.left);
    const execute = () => spawnSync(process.execPath, [cli, ...files], { encoding: 'utf8', timeout: 10000 });
    assert.equal(execute().status, 0);
    p.right.commands[0].bytes[0]++; await writeFile(files[2], stringify(p.right)); assert.equal(execute().status, 1);
    await writeFile(files[2], '{'); assert.equal(execute().status, 1);
  } finally { await rm(dir, { recursive: true, force: true }); }
});
