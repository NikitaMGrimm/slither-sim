import test from 'node:test';
import assert from 'node:assert/strict';
import { nativeSteps } from './feel-replay/native.mjs';
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
  assert.deepEqual(explicitMouse.actions, [{ kind: 'pointer', x: 672, y: 216, down: false }]);
  const physicalMouse = steps.find(s => s.event.sourceId === 'input-2');
  assert.deepEqual(physicalMouse.actions, [
    { kind: 'pointer', x: 720, y: 216, down: false },
    { kind: 'pointer', x: 720, y: 216, down: false },
  ]);
  assert.equal(physicalMouse.event.type, 'mousemove');
  assert.deepEqual(steps.find(s => s.event.sourceId === 'input-5').actions, []);
  assert.deepEqual(steps.find(s => s.event.sourceId === 'input-6').actions, [{ kind: 'key', code: 'ArrowLeft', down: false }]);
  assert.deepEqual(steps.filter(s => s.control).map(s => s.control), ['close', 'play']);
  assert.equal(steps.filter(s => s.packet).length, record.packets.length);
  assert.deepEqual(steps.find(s => s.packet).packet, Array.from(Buffer.from(record.packets[0].body, 'base64')));
});

test('native fractional input follows mouse presses, both move admissions and CSS DOM getter precision', () => {
  // Authored mapping-only case, using A2 CSS initialization rather than physical
  // winit pixels. DOM MouseEvent floors 0.999999999 to 0; an extra f32 cast would
  // incorrectly admit 1. PointerEvent retains the isolated fractional movement.
  const area = { width: 961, height: 541, dpr: 1.25 };
  const inputs = [
    { type: 'pointerdown', x: 0.7, y: 0.4 },
    { type: 'mousedown', x: 0.7, y: 0.4, encoding: 'dom-event' },
    { type: 'pointermove', x: 0.7, y: 0.4 },
    { type: 'mousemove', x: 0.999999999 / area.width, y: 0.999999999 / area.height, encoding: 'dom-event' },
    { type: 'mousedown', x: 0.75, y: 0.4 },
    { type: 'mousemove', x: 0.751, y: 0.401 },
    { type: 'mouseup', x: 0.751, y: 0.401 },
    { type: 'mousedown', x: 0.7, y: 0.4, button: 1, encoding: 'dom-event' },
    { type: 'mousemove', x: 0.7, y: 0.4, target: 'textarea' },
  ];
  const schedule = { events: inputs.map((_, i) => ({ sourceId: `authored-${i}`, kind: 'input', atMs: i })) };
  const steps = nativeSteps(schedule, inputs, area);
  const point = (x, y, down = false) => ({ kind: 'pointer', x, y, down });
  assert.deepEqual(steps[0].actions, []);
  assert.deepEqual(steps[1].actions, [point(672, 216, true)]);
  assert.deepEqual(steps[1].event.coordinates, { x: 672, y: 216 });
  assert.deepEqual(steps[2].actions, [point(0.7 * 961, 0.4 * 541)]);
  assert.deepEqual(steps[3].actions, [point(0, 0)]);
  // An uncanceled pointerdown must allow the compatibility mousedown through.
  assert.equal(steps[4].event.type, 'mousedown');
  assert.deepEqual(steps[4].actions, [point(720, 216, true)]);
  assert.deepEqual(steps[5].actions, [point(0.751 * 961, 0.401 * 541), point(721, 216)]);
  assert.deepEqual(steps[5].event.coordinates, { x: 721, y: 216 });
  assert.deepEqual(steps[6].actions, [{ kind: 'release' }]);
  assert.deepEqual(steps[7].actions, []);
  assert.deepEqual(steps[8].actions, []);
});

test('native and browser comparisons use the same perceptual policy', () => {
  const { schedule, trace: left } = syntheticFixture();
  const right = structuredClone(left);
  right.manifest.peer = 'native';
  right.manifest.source.commit = 'e'.repeat(40);
  right.samples[1].camera.x += 0.1;
  const report = compareTraces(left, right, schedule);
  assert.equal(report.passed, true);
  assert(report.channels.find(c => c.channel.endsWith('.camera.position')).max > 0);
});
