import test from 'node:test';
import assert from 'node:assert/strict';
import { assertWasmTraceBinding, nativeSteps, channelResults } from './feel-replay/native.mjs';
import { sha256 } from './feel-replay/io.mjs';
import { buildSchedule } from './feel-replay/schedule.mjs';
import { syntheticFixture } from './feel-replay/synthetic.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { readFile } from 'node:fs/promises';

test('native CSS delivery preserves physical/compatibility observations and production target admission', async () => {
  const record = JSON.parse(await readFile(new URL('./feel-replay/browser-synthetic.json', import.meta.url)));
  const { schedule, deliveries } = buildSchedule(record, { expectedCommands: 7, appearanceAtMs: 3 });
  const steps = nativeSteps(schedule, deliveries, { width: 960, height: 540 });
  const pointer = steps.find(s => s.event.sourceId === 'input-0');
  assert.deepEqual(pointer.actions, [{ kind: 'pointer', x: 672, y: 216, down: false }]);
  const explicitMouse = steps.find(s => s.event.sourceId === 'input-1');
  assert.deepEqual(explicitMouse.actions, []);
  const physicalMouse = steps.find(s => s.event.sourceId === 'input-2');
  assert.deepEqual(physicalMouse.actions, [{ kind: 'pointer', x: 720, y: 216, down: false }]);
  assert.equal(physicalMouse.event.type, 'mousemove');
  assert.deepEqual(steps.find(s => s.event.sourceId === 'input-5').actions, []);
  assert.deepEqual(steps.find(s => s.event.sourceId === 'input-6').actions, [{ kind: 'key', code: 'ArrowLeft', down: false }]);
  assert.deepEqual(steps.filter(s => s.control).map(s => s.control), ['close', 'play']);
  assert.equal(steps.filter(s => s.packet).length, record.packets.length);
  assert.deepEqual(steps.find(s => s.packet).packet, Array.from(Buffer.from(record.packets[0].body, 'base64')));
});

test('channel report retains exact channels and never promotes unexplained floats', () => {
  const { schedule, trace: left } = syntheticFixture();
  const right = structuredClone(left);
  right.manifest.peer = 'native';
  right.samples[1].camera.x += 1e-10;
  const report = compareTraces(left, right, schedule, { mode: 'cross-target' });
  const result = channelResults(left, right, report);
  assert.equal(result.channels.find(c => c.channel === 'commands').status, 'exact');
  assert.equal(result.channels.find(c => c.channel === 'camera').status, 'failing');
  assert.equal(report.passed, false);
  assert.equal(result.rows.length, 1);
});

test('Wasm operand must be the trace its completed A2 run wrote', () => {
  // ORIGINAL SYNTHETIC bytes; only the binding is under test.
  const bytes = '{"synthetic":"wasm trace"}';
  assertWasmTraceBinding({ traceSha256: { 'canvas-wasm': sha256(bytes) } }, bytes);
  assert.throws(() => assertWasmTraceBinding({ traceSha256: { 'canvas-wasm': sha256(bytes) } }, bytes + ' '), /differs/);
  assert.throws(() => assertWasmTraceBinding({ traceSha256: { 'webgpu-wasm': sha256(bytes) } }, bytes), /regenerate A2/);
  assert.throws(() => assertWasmTraceBinding({}, bytes), /regenerate A2/);
  assert.throws(() => assertWasmTraceBinding(undefined, bytes), /regenerate A2/);
});
