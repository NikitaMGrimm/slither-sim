import test from 'node:test';
import assert from 'node:assert/strict';
import { desktopSteps, classifyPath, coreHistoryComparison } from './feel-replay/desktop.mjs';

const step = (kind, at) => ({ event: { kind, sourceId: `${kind}-${at}`, actualMs: at, scheduledMs: at }, actions: [] });
test('zero-delay desktop lane preserves the authored schedule', () => {
  const plan = desktopSteps([step('frame', 0), step('packet', 2), step('frame', 10)], 0);
  assert.deepEqual(plan.map(s => s.event.actualMs), [0, 2, 10]);
  assert.deepEqual(plan.filter(s => s.event.kind === 'frame').map(s => [s.ackAtMs, s.skippedRedraw]), [[0, false], [10, false]]);
});
test('blocking present queues transport/input and acknowledges before service and redraw', () => {
  const plan = desktopSteps([step('frame', 0), step('packet', 2), step('input', 3), step('frame', 10), step('frame', 40)], 40);
  assert.equal(plan.find(s => s.event.sourceId === 'frame-10').skippedRedraw, true);
  assert.deepEqual(plan.filter(s => s.event.actualMs === 40).map(s => s.acknowledgment ? 'ack' : s.event.kind), ['ack', 'packet', 'input', 'frame']);
  assert.equal(plan.find(s => s.event.kind === 'packet').event.scheduledMs, 2);
  assert.equal(plan.at(-1).event.actualMs, 80);
  assert.throws(() => desktopSteps([], -1), /delay/);
  assert.throws(() => desktopSteps([], NaN), /delay/);
});
test('classification retains camera ring errors separately from scheduling and commands', () => {
  assert.equal(classifyPath('samples[1].camera.ring[3]'), 'camera ring');
  assert.equal(classifyPath('samples[1].camera.index'), 'camera samples');
  assert.equal(classifyPath('samples[1].atMs'), 'update/draw timing or missing boundary');
  assert.equal(classifyPath('commands[0].sendMs'), 'command bytes or times');
  assert.equal(classifyPath('samples'), 'update/draw timing or missing boundary');
  assert.equal(classifyPath('samples[5]'), 'update/draw timing or missing boundary');
});
test('core history ignores receipt bookkeeping but retains every sample and command value', () => {
  const left = { commands: [{ bytes: [1], decisionMs: 2, sendMs: 2, ackMs: 2 }],
    samples: [{ phase: 'draw', atMs: 2, delta: {}, lag: {}, own: null, camera: { index: 1 }, geometry: {} }] };
  const right = structuredClone(left);
  right.samples[0].eventSeq = 99;
  right.samples[0].status = { committed: false };
  assert.equal(coreHistoryComparison(left, right).exact, true);
  right.samples[0].camera.index = 2;
  assert.equal(coreHistoryComparison(left, right).rows.length, 1);
  right.samples.push(structuredClone(right.samples[0]));
  assert.equal(coreHistoryComparison(left, right).exact, false);
});
test('next-session ingress waits for its authored reset/Connect barrier', () => {
  const play = { ...step('input', 20), control: 'play' };
  const packet = { ...step('packet', 21), packet: [112] };
  const plan = desktopSteps([step('frame', 0), play, packet, step('frame', 50)], 40);
  assert.equal(plan.find(s => s.packet).event.actualMs, 90);
  assert.equal(plan.find(s => s.packet).event.scheduledMs, 21);
});
test('one-interval FIFO uses authored cadence endpoints without floating-point missed frames', () => {
  const frames = Array.from({ length: 120 }, (_, i) => step('frame', i * (1000 / 144)));
  const plan = desktopSteps(frames, 1000 / 144, true);
  assert.equal(plan.filter(s => s.skippedRedraw).length, 0);
  assert.equal(plan.filter(s => s.acknowledgment).length, frames.length);
});
