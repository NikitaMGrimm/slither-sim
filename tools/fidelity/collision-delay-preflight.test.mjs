import test from 'node:test';
import assert from 'node:assert/strict';
import { interval, compare, analyticControls } from './collision-delay-preflight.mjs';
import { DelayBot, TailTrial, intruder } from './slither-collision-delay.mjs';
import { decodeList } from './collision-delay-scout.mjs';

test('specified peer-rewind positive and current-state negative controls separate at planned N', () => {
  const result = analyticControls();
  assert.equal(result.plannedSamplesPerArm, 12);
  assert.equal(result.outcomes.current.result.verdict, 'no compensation');
  assert.equal(result.outcomes['peer-rewind'].result.verdict, 'compensation');
  // These controls intentionally demonstrate two geometries the experiment
  // cannot separate from current state; they must not be called peer rewind.
  assert.equal(result.outcomes['whole-state-rewind'].result.verdict, 'no compensation');
  assert.equal(result.outcomes['tandem-peer-rewind'].result.verdict, 'no compensation');
});

test('empty, one-sided, contradictory and overlapping boundaries stay inconclusive', () => {
  assert.equal(interval([]), null);
  assert.equal(interval([{ qualified: true, offset: 0, contact: false }]), null);
  assert.equal(interval([{ qualified: true, offset: 10, contact: false },
    { qualified: true, offset: 0, contact: true }]), null);
  assert.equal(compare([], -27).verdict, 'inconclusive');
  assert.equal(analyticControls({ floor: 40 }).outcomes['peer-rewind'].result.verdict, 'inconclusive');
});

test('artificial delay records raw ingress once and queues actual egress', () => {
  const frames = [], events = [], sent = [], times = [];
  const rec = { frame: (...args) => { frames.push(args); times.push(args[2] ?? performance.now()); },
    meta: row => { events.push(row); times.push(performance.now()); } };
  const bot = new DelayBot({ delayMs: 150 }, 0, rec);
  bot.ws = { readyState: 1, send: bytes => sent.push([...bytes]) };
  const at = bot.send([99, 0]);
  assert.equal(sent.length, 0); bot.flush(at + 149); assert.equal(sent.length, 0);
  bot.flush(at + 160); assert.deepEqual(sent, [[99, 0]]);
  assert.equal(frames.length, 1); assert.equal(frames[0][0], 'o');
  bot.receive(Uint8Array.from([0, 7, 112]));
  assert.equal(frames.length, 2);
  assert.equal(bot.frames, 1, 'terminal raw count includes frames waiting in the delay queue');
  bot.flush(performance.now() + 160);
  assert.equal(bot.frames, 1, 'applying an already recorded frame never counts it twice');
  assert.equal(frames.filter(r => r[0] === 'i').length, 1);
  assert.ok(events.some(e => e.event === 'delay-inbound-applied'));
  assert.deepEqual(times, [...times].sort((a, b) => a - b), 'metadata and raw frame clocks never regress');
});

test('persistent staging retains third-party exclusion without ending the pair', () => {
  const events = [], trial = new TailTrial({ arm: 'control', offset: 0 }, r => events.push(r));
  const make = id => ({ tap: { self: id, grd: 0, radius: 10000, snakes: new Map(), head: s => s.pts.at(-1) } });
  const bots = [make(1), make(2)];
  bots[0].tap.snakes.set(3, { id: 3, pts: [{ x: -1000, y: 10 }, { x: 1000, y: 10 }] });
  const poses = [{ x: 0, y: 0 }, { x: 50, y: 0 }];
  assert.equal(intruder(bots, poses), 3);
  assert.equal(trial.update(performance.now(), poses, poses, bots, 10), null);
  assert.equal(trial.phase, 'rendezvous'); assert.equal(trial.interruptions.length, 1);
  assert.equal(events[0].reason, 'third_party');
});

test('discovery decoder transcribes bytes without making upstream ping sockets', () => {
  const bytes = [0, 127, 0, 0, 1, ...Array(16).fill(0), 1, 188, 0, 17, 2, 0, 1];
  let encoded = 'a', offset = 0;
  for (const byte of bytes) for (const nibble of [byte >> 4, byte & 15]) {
    encoded += String.fromCharCode(97 + (nibble + offset) % 26); offset += 7;
  }
  assert.deepEqual(decodeList(encoded), [{ listedIndex: 0, active: true, ip: '127.0.0.1', port: 444, advertisedCount: 17 }]);
  assert.throws(() => decodeList(encoded.slice(0, -1)), /truncated/);
});
