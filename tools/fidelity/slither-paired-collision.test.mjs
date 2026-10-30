import test from 'node:test';
import assert from 'node:assert/strict';
import { C, Trial, trialSpec, radius, quality, segmentDistance, parseArgs, validateOptions, CollisionBot } from './slither-paired-collision.mjs';

const pose = (id, x, y) => ({ id, life: 1, x, y, heading: 0, speed: 180, segments: 8,
  fam: 0, boost: false, age: 200, peerAge: 200,
  body: [{ x: x - 100, y }, { x: x - 50, y }, { x, y }] });
function pair(cross = 80) {
  const poses = [pose(1, -50, cross), pose(2, 0, 0)];
  poses.forEach((p, i) => { p.peerHead = { x: poses[1 - i].x, y: poses[1 - i].y }; });
  return poses;
}
test('balanced deterministic schedule brackets threshold, swaps probe and actual first join', () => {
  const specs = Array.from({ length: 12 }, (_, i) => trialSpec(i));
  assert.deepEqual(specs.slice(0, 4).map(s => s.factor), [1.3, 1.1, .9, .7]);
  assert.equal(specs.filter(s => s.kind === 'head-on').length, 4);
  assert.deepEqual(specs.slice(8).map(s => [s.probe, s.first]), [[0, 0], [1, 0], [0, 1], [1, 1]]);
  assert.equal(radius(2), 14.5);
});
test('quality accepts ordinary anchor cadence but rejects each contaminated stream', () => {
  assert.equal(quality(pair()), null);
  for (const i of [0, 1]) for (const edit of [
    { age: 351 }, { age: -1 }, { peerAge: NaN }, { peerHead: null }, { boost: true },
    { speed: 210 }, { segments: 80 }, { fam: NaN },
  ]) {
    const p = pair(); Object.assign(p[i], edit); assert.ok(quality(p));
  }
  assert.equal(quality(pair(), null, 101), 'control_gap');
  const p = pair(); p[0].peerHead.x += 26;
  assert.equal(quality(p), 'stream_disagreement');
  const initial = pair(); const changed = pair(); changed[0].life++;
  assert.equal(quality(changed, initial), 'life_or_size_changed');
});
test('guard sees the middle of a long body edge', () => {
  assert.equal(segmentDistance({ x: 0, y: 10 }, { x: -1000, y: 0 }, { x: 1000, y: 0 }), 10);
});
test('third party, timeout and deadline can never become survival evidence', () => {
  for (const reason of ['third_party', 'staging_timeout', 'capture_deadline']) {
    const t = new Trial(0); t.end(0, reason);
    assert.equal(t.result.qualified, false); assert.equal(t.result.outcome, 'censored');
  }
  const t = new Trial(0); t.update(0, pair(), { intruder: 4 });
  assert.deepEqual(t.result.reasons, ['third_party']);
});
function running() {
  const t = new Trial(0); t.site = { x: 0, y: 0 }; t.heading = 0;
  t.offset = 1.3 * .975 * radius(8); t.phase = 'run'; t.runAt = 0;
  t.initial = pair(t.offset); return t;
}
test('ladder survival requires full target hold; contact is own game-over, never peer absence', () => {
  const t = running();
  for (let now = 0; now <= C.holdMs; now += 25) t.update(now, pair(t.offset));
  assert.equal(t.result.qualified, true); assert.equal(t.result.outcome, 'survival');
  const hit = running();
  for (let now = 0; now < 150; now += 25) hit.update(now, pair(hit.offset));
  hit.update(150, [null, null], { deaths: [true, false] });
  assert.equal(hit.result.qualified, true); assert.equal(hit.result.outcome, 'contact');
  const lost = running(); lost.update(0, pair(lost.offset)); lost.update(125, [null, null]);
  assert.equal(lost.result.qualified, false);
});
test('endpoint parser accepts supplied numeric addresses without an outside option', () => {
  assert.equal(validateOptions({ out: 'unused', server: '192.0.2.1:444' }).ip, '192.0.2.1');
  assert.throws(() => validateOptions({ out: 'unused', server: 'example.com:444' }), /IPv4/);
  assert.equal(validateOptions({ out: 'unused', server: '127.0.0.1:8766' }).ip, '127.0.0.1');
  assert.throws(() => parseArgs(['--allow-outside', 'yes']), /invalid/);
  assert.throws(() => parseArgs(['--out', 'a', '--out', 'b']), /repeated/);
});

test('a collision bot sends nothing before its snake exists', () => {
  const sent = [];
  const bot = new CollisionBot({ nick: 'probe' }, 0, { meta() {}, frame() {} });
  bot.ws = { readyState: 1, send(bytes) { sent.push(Array.from(bytes)); }, close() {} };
  bot.drive({ heading: 1, boost: false }, 1000);
  assert.deepEqual(sent, []);
  bot.world.self = 1;
  bot.world.snakes.set(1, { id: 1, pts: [{ x: 0, y: 0 }], ang: 0, wang: 0, sp: 5.7, sct: 8, fam: 0, t: 0 });
  bot.drive({ heading: 1, boost: false }, 1000);
  assert.deepEqual(sent[0], [251]);
});
