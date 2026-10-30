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
  const row = ['draw', null, [0, 0, 0, 0, 1, -0, 0, 0, 0, 0, 0, 1, 1, ...Array(124).fill(0)]];
  const ack = { seq: 0, sourceId: 'packet-0', atMs: 0, event: trace.events[0], observations: [row], commands: [],
    geometry: trace.manifest.geometry, lag: { active: false, pingPending: false, pongAgeMs: 0 }, highQuality: true, failed: false, sendFailed: false };
  assembly.append(ack);
  const result = assembly.finish({ atMs: 0, pendingEffects: 0, pendingFrames: 0, completed: true, unexpectedNetwork: false });
  assert.equal(result.samples.length, 1); assert(Object.is(result.samples[0].camera.x, -0));
  assert(Object.is(parse(stringify(result)).samples[0].camera.x, -0));
  assert.throws(() => assembly.append(ack), /Duplicate/);
  assert.equal(compareTraces(result, result, schedule).passed, false);
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
