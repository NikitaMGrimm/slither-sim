import test from 'node:test';
import assert from 'node:assert/strict';
import { desktopSteps, desktopExpectation } from './feel-replay/desktop.mjs';
import { validateSuccessfulDraws } from './feel-replay/schema.mjs';

test('draw accounting counts only receipted preparations once in sample order', () => {
  const sample = (phase, committed, count) => ({ phase, status: { committed }, quality: { successfulDraws: count } });
  const trace = { samples: [sample('draw', true, 1), sample('draw', false, 1),
    sample('update', false, 1), sample('draw', true, 2)] };
  assert.equal(validateSuccessfulDraws(trace), 2);
  trace.samples[0].quality.successfulDraws = 2;
  assert.throws(() => validateSuccessfulDraws(trace), /samples\[0\].quality.successfulDraws/);
});

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

test('desktop Setup receipts are frozen independently and unexpected dropped/duplicate acknowledgements fail', async () => {
  const { syntheticFixture } = await import('./feel-replay/synthetic.mjs');
  const { compareTraces } = await import('./feel-replay/compare.mjs');
  const { schedule, trace } = syntheticFixture();
  // The first draw is explicitly an authored packet Setup candidate.
  schedule.boundaries[2].eventSeq = trace.samples[2].eventSeq = 0;
  schedule.boundaries[1].eventSeq = trace.samples[1].eventSeq = 0;
  const expected = desktopExpectation(trace, schedule);
  assert.deepEqual(expected.schedule.pictures, [1]);
  assert.deepEqual(schedule.pictures, [0, 1]);
  assert.equal(trace.final.draws, 2); assert.equal(expected.trace.final.draws, 1);
  assert(compareTraces(expected.trace, expected.trace, expected.schedule).passed);
  const missing = structuredClone(expected.trace); missing.samples[4].status.committed = false;
  assert(!compareTraces(expected.trace, missing, expected.schedule).passed);
  const extra = structuredClone(expected.trace); extra.samples[2].status.committed = true;
  assert(!compareTraces(expected.trace, extra, expected.schedule).passed);
});
