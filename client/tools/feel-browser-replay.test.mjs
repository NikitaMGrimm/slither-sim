// Authored synthetic checks; no browser, build, server or external traffic.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { buildSchedule, coordinate, normalizeInput, packetTypes, targetFor } from './feel-replay/schedule.mjs';
import { virtualClock } from './feel-replay/clock.mjs';
import { traceAssembly } from './feel-replay/assembly.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { parse, stringify } from './feel-replay/io.mjs';
import { syntheticFixture } from './feel-replay/synthetic.mjs';
import { validateTrace } from './feel-replay/schema.mjs';
import { differences } from './feel-replay/differences.mjs';
import { assertBuildBinding } from './feel-replay/artifacts.mjs';

const clone = value => parse(stringify(value));
test('stale prebuilt artifacts fail closed against the staged source tree', () => {
  const source = { tree: 'a', core: 'b' };
  assert.doesNotThrow(() => assertBuildBinding(source, { ...source, commit: 'c' }));
  assert.throws(() => assertBuildBinding({ ...source, core: 'old' }, source), /stale/);
  assert.throws(() => assertBuildBinding({ ...source, tree: 'old' }, source), /stale/);
});
test('schedule is frozen before peers, ties retain unsplit packet order and complete tail', () => {
  const record = JSON.parse(readFileSync(new URL('./feel-replay/browser-synthetic.json', import.meta.url)));
  const { schedule, deliveries } = buildSchedule(record, { expectedCommands: 7, appearanceAtMs: 3 });
  assert.equal(schedule.events.at(-1).atMs, record.durationMs);
  assert.equal(schedule.boundaries.at(-1).atMs, record.durationMs);
  assert.equal(deliveries.filter(x => x?.body).length, record.packets.length);
  assert.equal(schedule.boundaries.filter(b => b.phase === 'draw' && schedule.events[b.eventSeq].kind === 'packet').length, 4);
  assert.equal(schedule.boundaries.filter(b => b.phase === 'update' && schedule.events[b.eventSeq].kind === 'packet').length, 8);
  assert(schedule.boundaries.some(b => b.eligibility === 'between-lives'));
  assert(schedule.boundaries.filter(b => b.atMs >= 942).every(b => b.eligibility === 'own'));
  assert.deepEqual(packetTypes(btoa(String.fromCharCode(0, 1, 97, 33, 112))), [97, 112]);
  assert.throws(() => packetTypes('AA=='), /Invalid envelope/);
});
test('DOM normalization preserves both families and maps explicit peer targets', () => {
  assert.equal(normalizeInput({ type: 'mousemove', x: 0.25, y: 0.5 }).x, 240);
  assert.equal(normalizeInput({ type: 'pointerdown', x: 0, y: 0 }).buttons, 1);
  assert.match(targetFor('canvas-wasm', 'surface'), /slither-client-root/);
  assert.equal(targetFor('webgpu-wasm', 'surface'), '#game');
  for (const peer of ['canvas-wasm', 'webgpu-wasm']) for (const target of ['overlay', 'input', 'textarea', 'editable', 'button']) assert.equal(targetFor(peer, target), `#replay-${target}`);
  assert.throws(() => normalizeInput({ type: 'mousemove', x: 1.1, y: 0 }), /normalized pointer/);
});

test('authored fractional control retains subpixel dispatches and a complete frozen schedule', () => {
  const record = JSON.parse(readFileSync(new URL('./feel-replay/browser-fractional-synthetic.json', import.meta.url)));
  assert.match(record.provenance, /AUTHORED SYNTHETIC/);
  const { schedule, deliveries } = buildSchedule(record, { expectedCommands: 4, appearanceAtMs: 3 });
  assert.deepEqual([schedule.events.length, schedule.boundaries.length, schedule.durationMs], [31, 45, 300]);
  assert.equal(schedule.boundaries.filter(b => b.phase === 'draw').length, 22);
  for (const input of deliveries.filter(row => /^(pointer|mouse)/.test(row?.type))) {
    const point = normalizeInput(input, { width: 961, height: 541 });
    assert.notEqual(point.x, Math.floor(point.x));
    assert.notEqual(point.y, Math.floor(point.y));
  }
  assert.equal(record.inputs[1].encoding, 'dom-event');
  assert.equal(record.inputs[2].encoding, undefined);
});
test('authored isolated-pointer fixture has no mousemove paired with its first movement', () => {
  const record = JSON.parse(readFileSync(new URL('./feel-replay/browser-pointer-only-synthetic.json', import.meta.url)));
  assert.match(record.provenance, /AUTHORED SYNTHETIC/);
  const { schedule } = buildSchedule(record, { expectedCommands: 4, appearanceAtMs: 3 });
  assert.deepEqual([schedule.events.length, schedule.boundaries.length, schedule.durationMs], [30, 45, 300]);
  assert.deepEqual(record.inputs.filter(input => input.at === 10).map(input => input.type), ['pointermove']);
});

test('coordinator drains and acknowledges each identical event before the next', async () => {
  const { schedule } = syntheticFixture(), calls = [];
  const peers = ['canvas', 'wgpu'].map(name => ({ name, async step(e, data, seq) { calls.push([name, seq, data]); return { seq, sourceId: e.sourceId, atMs: e.atMs }; } }));
  const deliveries = schedule.events.map((_, i) => ({ payload: i }));
  await coordinate(schedule, deliveries, peers, (p, ack) => calls.push(['ack', p.name, ack.seq]));
  assert.deepEqual(calls.slice(0, 4), [['canvas', 0, deliveries[0]], ['ack', 'canvas', 0], ['wgpu', 0, deliveries[0]], ['ack', 'wgpu', 0]]);
  await assert.rejects(coordinate(schedule, deliveries, [{ step: async () => ({ seq: 99 }) }], () => {}), /acknowledgment/);
});
test('clock installs before bootstrap, runs ordered timers and does not invent RAFs', () => {
  const realm = { Date, performance: {} }, clock = virtualClock(realm), seen = [];
  realm.setTimeout(() => seen.push(clock.now()), 5);
  realm.requestAnimationFrame(t => seen.push(t));
  clock.advance(10); assert.deepEqual(seen, [5]); clock.frame(); assert.deepEqual(seen, [5, 10]);
  assert.equal(realm.Date.now(), 1700000000010); assert.throws(() => clock.advance(1), /backwards/);
});
test('assembly retains actual boundaries and raw signed zeros; never fills missing rows', () => {
  const { schedule, trace } = syntheticFixture();
  const assembly = traceAssembly(schedule, trace.manifest);
  const row = ['draw', null, [0, 0, 0, 0, 1, -0, 0, 0, 0, 0, 0, 1, 1, ...Array(124).fill(0)], '0'];
  const ack = { seq: 0, sourceId: 'packet-0', atMs: 0, event: trace.events[0], observations: [row], pictures: ['0'], commands: [],
    geometry: trace.manifest.geometry, lag: { active: false, pingPending: false, pongAgeMs: 0 }, highQuality: true, failed: false, sendFailed: false };
  assembly.append(ack);
  const result = assembly.finish({ atMs: 0, pendingEffects: 0, pendingFrames: 0, completed: true, unexpectedNetwork: false });
  assert.equal(result.samples.length, 1); assert(Object.is(result.samples[0].camera.x, -0));
  assert(Object.is(parse(stringify(result)).samples[0].camera.x, -0));
  assert.throws(() => assembly.append(ack), /Duplicate/);
  assert.equal(compareTraces(result, result, schedule).passed, false);
});

test('recoverable overflow keeps a prepared row without a picture; the next equal-valued draw succeeds', () => {
  const { trace } = syntheticFixture();
  const schedule = { schema: 2, label: 'SYNTHETIC recoverable DRAW_MEMORY_LIMIT', durationMs: 10, expectedCommands: 0,
    events: [0, 10].map((atMs, i) => ({ sourceId: `frame-${i}`, kind: 'frame', atMs })),
    boundaries: [0, 1].flatMap(i => ['update', 'draw'].map((phase, j) => ({
      id: `frame-${i}-${phase}-${i * 2 + j}`, eventSeq: i, phase, frameId: i,
      drawId: phase === 'draw' ? i : null, atMs: i * 10, eligibility: 'pre-appearance',
    }))) };
  const assembly = traceAssembly(schedule, trace.manifest);
  const cells = [0, 0, 0, 0, 1, -0, 0, 0, 0, 0, 0, 1, 1, ...Array(124).fill(0)];
  for (let i = 0; i < 2; i++) assembly.append({ seq: i, sourceId: `frame-${i}`, atMs: i * 10,
    event: { ...trace.events[0], seq: i, sourceId: `frame-${i}`, kind: 'frame', type: 'frame', scheduledMs: i * 10, actualMs: i * 10, effectIds: [] },
    observations: [['update', null, cells, String(i * 2)], ['draw', null, cells, String(i * 2 + 1)]],
    // Production memory skips leave failed=false and issue no picture.
    pictures: i === 0 ? [] : ['3'], commands: [], geometry: trace.manifest.geometry,
    lag: { active: false, pingPending: false, pongAgeMs: 0 }, highQuality: true, failed: false, sendFailed: false });
  const result = assembly.finish({ atMs: 10, pendingEffects: 0, pendingFrames: 0, completed: true, unexpectedNetwork: false });
  assert.deepEqual(result.samples.map(s => s.quality.successfulDraws), [0, 0, 0, 1]);
  assert.deepEqual(result.samples[1].status, { prepared: true, submitted: false, committed: false, retries: 0, sendFailed: false, presentFailed: false });
  assert.equal(result.samples[3].status.committed, true);
  assert.equal(result.final.draws, 1);
  assert.deepEqual(assembly.pictureDrawIds(), [1]);
  assert.equal(validateTrace(result, schedule, [1]), result);
  assert.throws(() => validateTrace(result, schedule, []), /picture acknowledgement mismatch/);
  const wrongQuality = clone(result); wrongQuality.samples[1].quality.successfulDraws = 1;
  assert.throws(() => validateTrace(wrongQuality, schedule), /cumulative committed draw count/);
  const falseSuccess = clone(result);
  Object.assign(falseSuccess.samples[1].status, { submitted: true, committed: true });
  for (const sample of falseSuccess.samples.slice(1)) sample.quality.successfulDraws++;
  falseSuccess.final.draws++;
  assert.throws(() => validateTrace(falseSuccess, schedule, [1]), /picture acknowledgement mismatch/);
});

test('assembly requires picture metadata and rejects duplicate, orphan and non-draw receipts', () => {
  const { schedule, trace } = syntheticFixture();
  const cells = [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, ...Array(124).fill(0)];
  const ack = { seq: 0, sourceId: 'packet-0', atMs: 0, event: trace.events[0],
    observations: [['draw', null, cells, '0']], commands: [], pictures: [], geometry: trace.manifest.geometry,
    lag: { active: false, pingPending: false, pongAgeMs: 0 }, highQuality: true, failed: false, sendFailed: false };
  assert.throws(() => traceAssembly(schedule, trace.manifest).append({ ...ack, pictures: undefined }), /acknowledgements required/);
  assert.throws(() => traceAssembly(schedule, trace.manifest).append({ ...ack, pictures: ['0', '0'] }), /duplicate picture/);
  assert.throws(() => traceAssembly(schedule, trace.manifest).append({ ...ack, observations: [...ack.observations, ...ack.observations] }), /duplicate observation/);
  for (const [phase, pictures] of [['draw', ['99']], ['update', ['0']]]) {
    const assembly = traceAssembly(schedule, trace.manifest);
    assembly.append({ ...ack, observations: [[phase, null, cells, '0']], pictures });
    assert.throws(() => assembly.finish({ completed: true }), /no matching draw observation/);
  }
  const assembly = traceAssembly(schedule, trace.manifest);
  assembly.append(ack);
  const skipped = assembly.finish({ completed: true });
  assert.equal(skipped.final.draws, 0, 'zero acknowledged pictures cannot count as successful draws');
  assert.equal(skipped.samples[0].quality.successfulDraws, 0);
  assert.equal(skipped.samples[0].status.prepared, true);
  assert.equal(skipped.samples[0].status.submitted, false);
  assert.equal(skipped.samples[0].status.committed, false);
  assembly.append({ ...ack, seq: 1, sourceId: 'input-0', event: trace.events[1], observations: [], pictures: ['0'] });
  const result = assembly.finish({ completed: true });
  assert.equal(result.samples[0].status.committed, true, 'a later acknowledgement matches the earlier drained row');
  assert.equal(result.final.draws, 1);
});
test('diagnostics retain all differences without bypassing the entropy gate', () => {
  const { schedule, trace } = syntheticFixture(), other = clone(trace);
  other.manifest.entropy.requestStreamSha256 = 'e'.repeat(64);
  other.samples[0].camera.x = -0;
  other.commands[0].bytes[0]++;
  const rows = differences(trace, other);
  assert(rows.some(r => r.path === 'manifest.entropy.requestStreamSha256'));
  assert(rows.some(r => r.path === 'commands[0].bytes[0]'));
  assert.equal(compareTraces(trace, other, schedule).passed, false);
});
for (const [name, mutate] of [
  ['dropped row', t => t.samples.splice(1, 1)],
  ['duplicate event', t => t.events.splice(1, 0, clone(t.events[0]))],
  ['moved timestamp', t => { t.events[1].actualMs += 0.25; }],
  ['altered command byte', t => { t.commands[0].bytes[0]++; }],
  ['truncated tail', t => { t.samples.pop(); t.final.completed = false; }],
]) test(`full trace comparison rejects ${name}`, () => {
  const { schedule, trace } = syntheticFixture(), changed = clone(trace); mutate(changed);
  assert.equal(compareTraces(trace, trace, schedule).passed, true);
  assert.equal(compareTraces(trace, changed, schedule).passed, false);
});
