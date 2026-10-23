import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';
import { Writable } from 'node:stream';
import { createHash } from 'node:crypto';
import { Bot, World, Recorder, headingDanger } from './slither-headless.mjs';
import { PAIR_SNAKE as C, DIRECTIONS, PairSnakeState, PairSnakeBot, railPoint,
  railProjection, snapshot, chooseSite, siteInside, safeSteering, parseArgs, recordPair } from './slither-paired-snake.mjs';

const center = { x: 21600, y: 21600 };
const pose = (id, x, y, extra = {}) => ({ id, life: 1, x, y, anchorAgeMs: 0, peerVisible: true, ...extra });
const worlds = () => [new World(), new World()];
const poses = (direction, sign, offset, extraA = {}, extraB = {}) => {
  const point = railPoint(center, direction, sign, offset);
  return [pose(1, center.x, center.y, extraA), pose(2, point.x, point.y, extraB)];
};
function start(direction = 'x', sign = 1) {
  const events = [], state = new PairSnakeState(direction, (row, recipient) => events.push({ ...row, recipient }));
  const ws = worlds();
  state.update(100, poses(direction, sign, 600), ws);
  assert.equal(state.phase, 'crossing');
  return { state, events, ws };
}

test('all rail directions use Euclidean distances and both B-side signs', () => {
  for (const direction of Object.keys(DIRECTIONS)) for (const sign of [-1, 1]) {
    const { state } = start(direction, sign);
    assert.equal(state.crossing.sign, sign);
    assert.deepEqual(state.crossing.center, center);
    for (const offset of [800, 2300]) {
      const point = railPoint(center, direction, sign, offset);
      assert.ok(Math.abs(Math.hypot(point.x - center.x, point.y - center.y) - offset) < 1e-9);
      const p = railProjection(center, direction, sign, point);
      assert.ok(Math.abs(p.along - offset) < 1e-9);
      assert.ok(Math.abs(p.crossTrack) < 1e-9);
    }
  }
});

test('rendezvous requires actual mutual body presence before selecting a fixed site', () => {
  const state = new PairSnakeState('x'), ws = worlds();
  state.update(100, poses('x', 1, 600, {}, { peerVisible: false }), ws);
  assert.equal(state.phase, 'rendezvous'); assert.equal(state.crossing, null);
  state.update(200, poses('x', 1, 700), ws);
  assert.equal(state.phase, 'rendezvous');
  state.update(300, poses('x', 1, 600), ws);
  assert.equal(state.crossing.selectedAt, 300);
  assert.equal(state.crossing.endpointVisits, 0);
});

test('endpoints are exact, B-only, initial approach excluded; holds include reversal control update', () => {
  const { state, events, ws } = start();
  const update = (t, offset, a = {}, b = {}) => state.update(t, poses('x', 1, offset, a, b), ws);
  update(200, 800);
  assert.equal(events.length, 1); assert.equal(events[0].recipient, 1);
  assert.equal(events[0].event, 'paired-snake-endpoint');
  assert.deepEqual(events[0].crossingEndpoint.start, null);
  assert.equal(events[0].crossingEndpoint.completed, false);
  assert.equal(events[0].crossingEndpoint.end.t, 200);
  assert.equal(state.crossing.holdUntil, 2200);
  update(2199, 2300); assert.equal(events.length, 1);
  update(2200, 2300); assert.equal(events.length, 1);
  assert.equal(state.crossing.holdUntil, null);
  update(2225, 2300, {}, { peerVisible: false });
  assert.equal(events.length, 2); assert.equal(events[1].crossingEndpoint.completed, true);
  assert.equal(events[1].crossingEndpoint.start.t, 200);
  assert.equal(events[1].crossingEndpoint.end.t, 2225);
  assert.deepEqual(events[1].crossingEndpoint.end.pairIdentities, ['0/1/1', '1/1/2']);
  update(4225, 800); assert.equal(events.length, 2);
  update(4250, 800); assert.equal(events.length, 3);
  assert.equal(events[2].completedLegs, 2); assert.equal(events[2].feasibilityPassed, true);
  update(4300, 800); assert.equal(events.length, 3);
});

test('endpoint gates require both fresh anchors, raw reference radius and rail geometry', () => {
  for (const [a, b, offset] of [
    [{ x: center.x + 201 }, {}, 800],
    [{}, { y: center.y + 181 }, 800], [{}, {}, 921],
  ]) {
    const { state, events, ws } = start();
    state.update(200, poses('x', 1, offset, a, b), ws);
    assert.equal(events.length, 0);
  }
  const { state, events, ws } = start();
  state.update(200, poses('x', 1, 920, { anchorAgeMs: 150 }, { anchorAgeMs: 150, y: center.y + 180 }), ws);
  assert.equal(events.length, 1);
});

test('hard stale bounds abandon approach, holds and travel without joining later endpoints', () => {
  for (const index of [0, 1]) for (const age of [500.001, -1, NaN, Infinity, -Infinity]) {
    const unavailable = poses('x', 1, 600);
    unavailable[index].anchorAgeMs = age;
    const freshState = new PairSnakeState('x');
    freshState.update(100, unavailable, worlds());
    assert.equal(freshState.crossing, null);
    assert.equal(freshState.reason, 'own_anchor_stale');
    assert.equal(freshState.meetingTargets, null);
    for (const stage of ['approach', 'hold', 'travel']) {
      const { state, events, ws } = start();
      if (stage !== 'approach') state.update(200, poses('x', 1, 800), ws);
      if (stage === 'travel') state.update(2200, poses('x', 1, 800), ws);
      const invalid = poses('x', 1, stage === 'travel' ? 1550 : 800);
      invalid[index].anchorAgeMs = age;
      const t = stage === 'travel' ? 2300 : 300;
      state.update(t, invalid, ws);
      assert.equal(state.crossing, null);
      const ended = events.at(-1);
      assert.equal(ended.event, 'paired-snake-trial-end');
      assert.equal(ended.reason, 'own_anchor_stale');
      assert.equal(ended.feasibilityPassed, false);
      const count = events.filter(e => e.event === 'paired-snake-endpoint').length;
      state.update(t + 100, poses('x', 1, 2300), ws);
      state.update(t + 2200, poses('x', 1, 800), ws);
      assert.equal(events.filter(e => e.event === 'paired-snake-endpoint').length, count);
      assert.equal(state.crossing, null, 'fresh distant endpoints cannot bridge the abandoned trial');
    }
  }
});

test('soft stale boundaries preserve route and count once per pause episode', () => {
  for (const index of [0, 1]) for (const age of [150.001, 151, 430, 500]) {
    const { state, events, ws } = start();
    const c = state.crossing;
    c.cleanLegStreak = 1;
    const invalid = poses('x', 1, 600);
    invalid[index].anchorAgeMs = age;
    state.update(600, invalid, ws);
    state.update(625, invalid, ws);
    assert.equal(state.crossing, c); assert.deepEqual(c.center, center);
    assert.equal(c.interruptions, 1); assert.equal(c.pauseInterruptions, 1);
    assert.equal(c.cleanLegStreak, 0); assert.equal(state.pairDistance, null);
    assert.equal(state.poseStatus, 'paused'); assert.equal(state.pauseStartedAt, 600);
    assert.deepEqual(events.map(e => e.event), ['paired-snake-pause']);
    assert.equal(events[0].recipient, undefined);
    assert.equal(events[0].pairAnchorAgesMs[index], age);
    state.update(650, poses('x', 1, 600), ws);
    assert.equal(state.poseStatus, 'ready'); assert.equal(state.pauseStartedAt, null);
    assert.equal(events[1].event, 'paired-snake-resume');
    assert.equal(events[1].pauseStartedAt, 600); assert.equal(events[1].recipient, undefined);
    assert.equal(c.interruptions, 1);
    state.noteControl('avoidance_deflection');
    state.update(850, invalid, ws);
    assert.equal(c.interruptions, 3); assert.equal(c.pauseInterruptions, 2);
    assert.equal(c.avoidanceInterruptions, 1);
  }
  const state = new PairSnakeState('x');
  state.update(600, poses('x', 1, 600, { anchorAgeMs: 151 }), worlds());
  assert.equal(state.crossing, null); assert.equal(state.meetingTargets, null);
});

test('reacquisition requires both receipts strictly after entry, including the 1000 ms boundary', () => {
  for (const elapsed of [1000, 1000.001]) {
    const { state, events, ws } = start();
    state.update(1000, poses('x', 1, 600, { anchorAgeMs: 151 }), ws);
    state.update(1050, poses('x', 1, 600, { anchorAgeMs: 0 }, { anchorAgeMs: 50 }), ws);
    assert.equal(state.poseStatus, 'paused', 'receipt exactly at entry cannot resume');
    // Alternating freshness prevents coherent reacquisition without exceeding 500 ms.
    for (let t = 1200; t <= 1800; t += 200)
      state.update(t, poses('x', 1, 600, { anchorAgeMs: 200 }, { anchorAgeMs: 0 }), ws);
    state.update(1000 + elapsed, poses('x', 1, 600), ws);
    if (elapsed === 1000) {
      assert.equal(state.poseStatus, 'ready'); assert.equal(state.crossing.trial, 1);
      assert.equal(events.at(-1).event, 'paired-snake-resume');
    } else {
      assert.equal(state.crossing, null);
      assert.equal(events.at(-1).reason, 'paired_reacquire_timeout');
      assert.equal(events.some(e => e.event === 'paired-snake-resume'), false);
    }
  }
});

test('paused hold keeps its deadline and releases on first ready update without endpoint credit', () => {
  const { state, events, ws } = start();
  state.update(200, poses('x', 1, 800), ws);
  const c = state.crossing, startEndpoint = c.legStart;
  state.update(2100, poses('x', 1, 800, { anchorAgeMs: 151 }), ws);
  state.update(2200, poses('x', 1, 2300, { anchorAgeMs: 251 }), ws);
  assert.equal(c.holdUntil, 2200); assert.equal(c.holdOffset, 800);
  assert.equal(c.legStart, startEndpoint); assert.equal(c.legStarted, 200);
  state.update(2300, poses('x', 1, 2300), ws);
  assert.equal(c.holdUntil, null); assert.equal(c.holdOffset, null);
  assert.equal(c.endpointVisits, 1); assert.equal(c.leg, 'far');
  state.update(2325, poses('x', 1, 2300), ws);
  const end = events.at(-1).crossingEndpoint;
  assert.equal(end.start, startEndpoint); assert.equal(end.interrupted, true);
  assert.equal(end.pauseInterruptionCount, 1); assert.equal(c.feasibilityPassed, false);
});

test('lifecycle and safety resets precede stale pause and cannot bridge old centers', () => {
  for (const cause of ['pair_unavailable', 'pair_identity_changed', 'rim_margin_lost',
    'pair_separated', 'crossing_leg_timeout', 'capture_stop']) {
    const { state, events, ws } = start();
    state.update(200, poses('x', 1, 800), ws);
    state.update(300, poses('x', 1, 800, { anchorAgeMs: 151 }), ws);
    let next = poses('x', 1, 600, { anchorAgeMs: 200 });
    if (cause === 'pair_unavailable') next[0] = null;
    if (cause === 'pair_identity_changed') next[1].life++;
    if (cause === 'rim_margin_lost') ws[0].radius = 2999;
    if (cause === 'pair_separated') next = poses('x', 1, 3001);
    if (cause === 'capture_stop') state.reset(400, cause);
    else state.update(cause === 'crossing_leg_timeout' ? 30201 : 400, next, ws);
    const ended = events.filter(e => e.event === 'paired-snake-trial-end');
    assert.equal(ended.length, 1); assert.equal(ended[0].reason, cause);
    assert.equal(ended[0].crossingHoldUntil, 2200); assert.equal(ended[0].legInterruptionCount, 1);
    assert.equal(state.crossing, null); assert.equal(state.meetingTargets, null);
    assert.equal(state.pauseStartedAt, null);
    assert.equal(events.some(e => e.event === 'paired-snake-resume'), false);
  }
  for (const field of ['life', 'id']) {
    const { state, events, ws } = start();
    state.update(300, poses('x', 1, 600, { anchorAgeMs: 151 }), ws);
    const next = poses('x', 1, 600); next[1][field]++;
    state.update(400, next, ws);
    assert.equal(events.find(e => e.event === 'paired-snake-trial-end').reason, 'pair_identity_changed');
    assert.equal(state.crossing.trial, 2); assert.equal(state.crossing.legStart, null);
  }
});

test('leg deadlines apply in clean holds and before a late otherwise-valid endpoint', () => {
  for (const hold of [true, false]) {
    const { state, events, ws } = start();
    if (hold) state.update(200, poses('x', 1, 800), ws);
    state.update((hold ? 200 : 100) + C.legTimeoutMs + 1, poses('x', 1, 800), ws);
    assert.equal(state.crossing, null); assert.equal(events.at(-1).reason, 'crossing_leg_timeout');
  }
});

test('stale own or paired reference skips heading delivery despite fresh non-position traffic', () => {
  for (const stalePeer of [false, true]) {
    const { state } = start();
    const events = [], a = makeBot(0, state, events), b = makeBot(1, state, events);
    a.bot.peers = b.bot.peers = [a.bot, b.bot];
    a.bot.lastReceivedAt = 10000;
    a.me.t = stalePeer ? 10000 : 100;
    state.update(10000, poses('x', 1, 600,
      stalePeer ? {} : { anchorAgeMs: 9900 }, stalePeer ? { anchorAgeMs: 9900 } : {}), worlds());
    a.bot.tick(10000);
    assert.equal(a.sent.filter(bytes => bytes.length === 1 && bytes[0] <= 250).length, 0);
    assert.equal(events.find(e => e.event === 'paired-snake-control-skipped').reason, 'own_anchor_stale');
    assert.equal(a.bot.boost, false);
    a.me.t = a.bot.lastReceivedAt = 10025;
    state.update(10025, poses('x', 1, 600, { peerVisible: false }), worlds());
    assert.equal(state.phase, 'rendezvous');
    assert.equal(state.reason, null);
    a.bot.tick(10025);
    assert.ok(a.sent.some(bytes => bytes.length === 1 && bytes[0] <= 250),
      'fresh recovery resumes rendezvous controls without joining old endpoints');
  }
});

test('an interrupted achieved leg is preserved but cannot pass the near-far-near scale gate', () => {
  const { state, events, ws } = start();
  state.update(200, poses('x', 1, 800), ws);
  state.update(2200, poses('x', 1, 800), ws);
  state.noteControl('avoidance_deflection');
  state.update(2300, poses('x', 1, 2300), ws);
  state.update(4300, poses('x', 1, 2300), ws);
  state.update(4400, poses('x', 1, 800), ws);
  assert.equal(events[1].crossingEndpoint.completed, true);
  assert.equal(events[1].crossingEndpoint.interrupted, true);
  assert.equal(state.crossing.completedLegs, 2);
  assert.equal(state.crossing.feasibilityPassed, false);
  state.update(6400, poses('x', 1, 800), ws);
  state.update(6500, poses('x', 1, 2300), ws);
  assert.equal(state.crossing.feasibilityPassed, false, 'clean far-near-far is not near-far-near');
  state.update(8500, poses('x', 1, 2300), ws);
  state.update(8600, poses('x', 1, 800), ws);
  assert.equal(state.crossing.feasibilityPassed, true);
});

test('pair life changes end the old trial before same-update reacquisition', () => {
  const { state, events, ws } = start();
  state.update(200, poses('x', 1, 800), ws);
  const old = state.crossing.trial;
  state.update(300, poses('x', 1, 600, {}, { life: 2 }), ws);
  const ended = events.find(e => e.event === 'paired-snake-trial-end');
  assert.equal(ended.crossingTrial, old); assert.equal(ended.reason, 'pair_identity_changed');
  assert.deepEqual(ended.pairIdentities, ['0/1/1', '1/1/2']);
  assert.equal(ended.endpointVisits, 1); assert.equal(ended.crossingHoldUntil, 2200);
  assert.equal(state.crossing.trial, old + 1); assert.equal(state.crossing.endpointVisits, 0);
  assert.equal(state.crossing.holdUntil, null);
});

test('pair unavailability, separation, shrinking arena and timeout end without fabricated endpoints', () => {
  for (const reason of ['pair_unavailable', 'pair_separated', 'rim_margin_lost', 'crossing_leg_timeout']) {
    const { state, events, ws } = start();
    let samples = poses('x', 1, 600), t = 200;
    if (reason === 'pair_unavailable') samples[0] = null;
    if (reason === 'pair_separated') samples = poses('x', 1, 3001);
    if (reason === 'rim_margin_lost') ws[1].radius = 2999;
    if (reason === 'crossing_leg_timeout') t = 100 + C.legTimeoutMs + 1;
    state.update(t, samples, ws);
    assert.equal(state.crossing, null); assert.equal(events.length, 1);
    assert.equal(events[0].reason, reason); assert.equal(events[0].recipient, undefined);
    state.update(t + 1, [null, null], ws); assert.equal(events.length, 1);
  }
  const { state, events } = start();
  state.reset(200, 'capture_stop');
  assert.equal(events[0].status, 'right-censored');
});

test('site selection clamps inward, screens cached obstacles and guards far point before held endpoints', () => {
  const ws = worlds();
  const site = chooseSite(pose(1, 42400, 21600), pose(2, 42000, 21600), ws, 'x');
  assert.ok(site); assert.ok(siteInside(site.center, 'x', site.sign, ws));
  assert.ok(site.center.x < 42400);
  ws[0].snakes.set(3, { id: 3, pts: [{ ...center }] });
  const shifted = chooseSite(...poses('x', 1, 600), ws, 'x');
  assert.ok(shifted); assert.notDeepEqual(shifted.center, center);
  const { state, events, ws: pairWorlds } = start();
  state.update(200, poses('x', 1, 800), pairWorlds);
  pairWorlds[0].radius = 2999;
  state.update(300, poses('x', 1, 800), pairWorlds);
  assert.equal(events.at(-1).reason, 'rim_margin_lost');
});

function makeBot(index, state, events = []) {
  const sent = [], bot = new PairSnakeBot({ scenario: 'movement' }, index,
    { meta: row => events.push(row), frame() {} });
  const me = { id: index + 1, pts: [{ ...center }], sct: 6, fam: .2,
    ang: 0, wang: 0, sp: 5.12, t: 0 };
  bot.lives = 1; bot.world.self = me.id; bot.world.snakes.set(me.id, me);
  bot.ws = { readyState: 1, send: bytes => sent.push([...bytes]), close() {} };
  bot.lastReceivedAt = 1e9; bot.pair = state;
  return { bot, me, sent };
}

test('snapshots use raw heads and report local cache absence without inventing visibility', () => {
  const state = new PairSnakeState('x');
  const a = makeBot(0, state), b = makeBot(1, state); a.bot.peers = b.bot.peers = [a.bot, b.bot];
  a.me.t = 100; a.me.pts[0] = { x: 21700, y: 21600 };
  let s = snapshot(a.bot, b.bot, 125);
  assert.equal(s.x, 21700); assert.equal(s.anchorAgeMs, 25);
  assert.equal(s.localPeerSnakeCachePresent, false);
  a.bot.world.snakes.set(2, { ...b.me, t: 80, pts: [{ x: 22400, y: 21600 }] });
  s = snapshot(a.bot, b.bot, 125);
  assert.equal(s.localPeerSnakeBodyPositiveAgeMs, 45);
  assert.deepEqual(s.localPeerSnakeHead, { x: 22400, y: 21600 });
  a.bot.world.dead = true; assert.equal(snapshot(a.bot, b.bot, 125), null);
  a.bot.world.dead = false; a.bot.ws.readyState = 3;
  assert.equal(snapshot(a.bot, b.bot, 125), null);
});

test('safe steering prefers goal progress, logs deflection/blocking and retains partner danger', () => {
  const state = new PairSnakeState('x'), { bot, me } = makeBot(0, state);
  bot.world.snakes.set(2, { id: 2, pts: [{ x: 21700, y: 21600 }], sct: 6, sp: 0, ang: 0, t: 100 });
  const choice = { a: Math.PI };
  const intent = safeSteering(bot, 100, me, choice, 0);
  assert.equal(intent.interruption, 'avoidance_deflection');
  assert.equal(intent.controlStatus, 'deflected'); assert.notEqual(choice.a, Math.PI);
  assert.ok(headingDanger(bot.world, 100, me, center, choice.a, 1 + 4 / 106) <= .2);
  bot.world.snakes.get(2).pts = [{ ...center }];
  assert.equal(safeSteering(bot, 100, me, choice, 0).interruption, 'avoidance_blocked');
  const { state: active, events, ws } = start();
  active.noteControl('avoidance_deflection'); active.update(200, poses('x', 1, 800), ws);
  assert.equal(events[0].crossingEndpoint.interrupted, true);
});

test('sampled control fields preserve fixed center, raw reference poses and cache absence', () => {
  const { state, ws } = start();
  const events = [], a = makeBot(0, state, events), b = makeBot(1, state, events);
  a.bot.peers = b.bot.peers = [a.bot, b.bot];
  b.me.pts[0] = railPoint(center, 'x', 1, 2300); b.me.t = 200;
  state.update(200, poses('x', 1, 2300), ws);
  b.bot.tick(200);
  const row = events.find(r => r.event === 'paired-snake-state');
  assert.equal(row.phase, 'crossing'); assert.equal(row.scenario, 'paired-snake');
  assert.equal(row.localPeerSnakeCachePresent, false); assert.equal(row.actualAlong, 2300);
  assert.deepEqual(row.fixedCenter, center); assert.equal(row.primary.id, 1);
  assert.equal(row.secondary.anchorAgeMs, 0); assert.equal(row.boost, false);
  assert.equal(typeof row.sentHeading, 'number'); assert.equal(typeof row.encodedHeading, 'number');
});

test('paused control samples preserve target, raw ages and actual last byte and release boost immediately', () => {
  const { state, ws } = start();
  const rows = [], a = makeBot(0, state, rows), b = makeBot(1, state, rows);
  a.bot.peers = b.bot.peers = [a.bot, b.bot];
  for (const { bot, me } of [a, b]) { me.t = 200; bot.tick(200); }
  const delivered = a.sent.findLast(bytes => bytes.length === 1 && bytes[0] <= 250)[0];
  state.update(200, poses('x', 1, 800), ws);
  a.me.t = 200; b.me.t = 400;
  state.update(400, poses('x', 1, 800, { anchorAgeMs: 200 }), ws);
  a.bot.boost = true; a.bot.lastBoostAt = 399;
  const before = a.sent.length;
  a.bot.tick(400); b.bot.tick(400);
  assert.equal(a.sent.slice(before).some(bytes => bytes.length === 1 && bytes[0] <= 250), false);
  assert.ok(a.sent.slice(before).some(bytes => bytes[0] === 254));
  const paused = rows.findLast(r => r.event === 'paired-snake-state' && r.botIndex === 0);
  assert.equal(paused.poseStatus, 'paused'); assert.equal(paused.controlAt, 400);
  assert.equal(paused.lastDeliveredHeading, delivered); assert.equal(paused.sentHeading, delivered);
  assert.equal(paused.requestedHeading, null); assert.equal(paused.encodedHeading, null);
  assert.equal(paused.ownAnchorAgeMs, 200); assert.deepEqual(paused.pairAnchorAgesMs, [200, 0]);
  assert.equal(paused.boost, false); assert.deepEqual(paused.targetPoint, center);
  assert.equal(paused.crossingHoldUntil, 2200); assert.equal(paused.pauseInterruptionCount, 1);
  assert.deepEqual(b.bot.movementIntent.targetPoint, railPoint(center, 'x', 1, 800));
  const count = rows.filter(r => r.event === 'paired-snake-state' && r.botIndex === 0).length;
  state.update(425, poses('x', 1, 800, { anchorAgeMs: 225 }, { anchorAgeMs: 25 }), ws);
  a.bot.tick(425);
  assert.equal(rows.filter(r => r.event === 'paired-snake-state' && r.botIndex === 0).length, count);
  state.update(500, poses('x', 1, 800, { anchorAgeMs: 300 }, { anchorAgeMs: 100 }), ws);
  a.bot.tick(500);
  assert.equal(rows.filter(r => r.event === 'paired-snake-state' && r.botIndex === 0).length, count + 1);
  a.me.t = b.me.t = 525;
  state.update(525, poses('x', 1, 800), ws); a.bot.tick(525);
  assert.equal(rows.findLast(r => r.event === 'paired-snake-state').poseStatus, 'ready');
  assert.equal(state.crossing.interruptions, 1);
  a.bot.lives++;
  a.me.t = 350; b.me.t = 550;
  state.update(550, poses('x', 1, 600, { life: 2, anchorAgeMs: 200 }), ws);
  a.bot.tick(550);
  assert.equal(rows.findLast(r => r.event === 'paired-snake-state').lastDeliveredHeading, null);
});

test('pause cannot bypass stopped, death-observation or receive-timeout handling', () => {
  for (const condition of ['stopped', 'death', 'timeout']) {
    const { state, ws } = start(), rows = [];
    const a = makeBot(0, state, rows), b = makeBot(1, state, rows);
    a.bot.peers = b.bot.peers = [a.bot, b.bot];
    let closes = 0; a.bot.ws.close = () => { closes++; };
    a.me.t = 0; state.update(200, poses('x', 1, 600, { anchorAgeMs: 200 }), ws);
    if (condition === 'stopped') a.bot.stopped = true;
    if (condition === 'death') a.bot.deathObservation = { startedAt: 0 };
    if (condition === 'timeout') a.bot.lastReceivedAt = 0;
    a.bot.tick(condition === 'death' ? 2000 : condition === 'timeout' ? 15001 : 200);
    assert.equal(a.sent.length, 0); assert.equal(rows.some(r => r.event === 'paired-snake-state'), false);
    assert.equal(closes, condition === 'stopped' ? 0 : 1);
    if (condition === 'timeout') assert.ok(rows.some(r => r.event === 'receive-timeout'));
  }
});

test('pre-site pause binds identities and cannot resume a different life', () => {
  const events = [], state = new PairSnakeState('x', row => events.push(row));
  state.update(200, poses('x', 1, 600, { anchorAgeMs: 151 }), worlds());
  assert.deepEqual(events[0].pairIdentities, ['0/1/1', '1/1/2']);
  state.update(225, poses('x', 1, 600, { life: 2 }), worlds());
  assert.equal(events.some(e => e.event === 'paired-snake-resume'), false);
  assert.equal(state.crossing.trial, 1); assert.equal(state.pauseStartedAt, null);
});

const cli = ['--out', 'unused', '--server', '127.0.0.1:444', '--direction', 'x'];
test('CLI validates required direction, bounded duration, IPv4, mode, duplicates and unknown flags', async () => {
  assert.deepEqual(parseArgs(cli), { out: 'unused', server: '127.0.0.1:444', seconds: 480, mode: 'crossing', direction: 'x' });
  assert.equal(parseArgs([...cli, '--seconds', '3600']).seconds, 3600);
  for (const seconds of ['0', '.5', '3601', 'NaN', 'Infinity']) assert.throws(() => parseArgs([...cli, '--seconds', seconds]), /1\.\.3600/);
  for (const args of [cli.slice(0, 4), [...cli, '--mode', 'overlap'], [...cli, '--direction', 'y'], [...cli, '--bots', '3'], [...cli, '--seconds']])
    assert.throws(() => parseArgs(args));
  for (const direction of ['diagonal', 'X', 'toString']) assert.throws(() => parseArgs([...cli.slice(0, 4), '--direction', direction]));
  for (const server of ['localhost:444', '127.0.0.1:0', '127.0.0.1:65536', '127.0.0.1:444:5', '127.0.0.1:4e2'])
    assert.throws(() => parseArgs(['--out', 'unused', '--server', server, '--direction', 'x']), /invalid server/);
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:444', direction: 'x', seconds: '480' }), /1\.\.3600/);
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:444', direction: 'x', origin: 'file:///tmp' }), /origin/i);
});

// Observe synthetic recorder rows in memory: these tests never open capture files.
function memoryRecorders(context) {
  const rows = [], dirs = new Set();
  const line = Recorder.prototype.line;
  context.mock.method(fs, 'existsSync', file => dirs.has(file));
  context.mock.method(fs, 'mkdirSync', file => { dirs.add(file); });
  context.mock.method(fs, 'createWriteStream', () => new Writable({ write(chunk, encoding, done) { done(); } }));
  context.mock.method(Recorder.prototype, 'line', function (row) {
    if (row.type === 'header') rows.push({ recorder: this, rows: [] });
    rows.find(r => r.recorder === this).rows.push(structuredClone(row));
    return line.call(this, row);
  });
  return rows;
}

test('paired-snake capture writes readable gzip archives for both clients', async context => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-paired-snake-'));
  context.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const out = path.join(directory, 'capture'), controller = new AbortController();
  context.mock.method(Bot.prototype, 'connect', function () {
    this.joins = 1;
    if (this.index !== 1) return;
    this.pair.update(100, poses('x', 1, 600), worlds());
    this.pair.update(200, [null, null], worlds());
    controller.abort('synthetic loss');
  });
  await recordPair({ out, server: '127.0.0.1:444', direction: 'x' }, controller.signal);
  for (const index of [0, 1]) {
    const text = zlib.gunzipSync(fs.readFileSync(path.join(out, `bot${index}.jsonl.gz`))).toString('utf8');
    const rows = text.trimEnd().split('\n').map(JSON.parse);
    assert.equal(rows[0].type, 'header');
    assert.equal(rows[0].scenario, 'paired-snake');
    assert.equal(rows[0].botIndex, index);
    const ended = rows.filter(row => row.event === 'paired-snake-trial-end');
    assert.equal(ended.length, 1);
    assert.equal(ended[0].reason, 'pair_unavailable');
    assert.equal(ended[0].botIndex, index);
    assert.equal(rows.at(-1).event, 'capture-interrupted');
    assert.equal(rows.at(-1).collectorSourcesUnchanged, true);
  }
});

test('one same-process source batch uses paired-snake options and both trial-end recipients without movement', async context => {
  const saved = memoryRecorders(context);
  const out = 'memory-paired-snake', controller = new AbortController();
  context.mock.method(Bot.prototype, 'connect', function () {
    this.joins = 1;
    if (this.index !== 1) return;
    this.pair.update(100, poses('x', 1, 600), worlds());
    this.pair.update(150, poses('x', 1, 600, { anchorAgeMs: 151 }), worlds());
    this.pair.update(175, poses('x', 1, 600), worlds());
    this.pair.update(200, [null, null], worlds());
    controller.abort('synthetic loss');
  });
  await recordPair({ out, server: '127.0.0.1:444', direction: 'x' }, controller.signal);
  const rows = saved.map(r => r.rows);
  const sources = rows.map(r => r[0].captureProvenance);
  assert.equal(sources[0].captureBatchId, sources[1].captureBatchId);
  assert.notEqual(sources[0].captureId, sources[1].captureId);
  assert.equal(sources[0].timeOrigin, sources[1].timeOrigin);
  const digest = createHash('sha256').update(fs.readFileSync(new URL('./slither-paired-snake.mjs', import.meta.url))).digest('hex');
  for (const [i, member] of rows.entries()) {
    const header = member[0], p = sources[i];
    assert.equal(header.scenario, 'paired-snake'); assert.equal(header.mode, 'crossing');
    assert.equal(header.direction, 'x'); assert.match(header.nick, /^\(Bot\)/);
    assert.equal(p.botCount, 2); assert.equal(p.botIndex, i);
    assert.equal(p.collectorSha256, digest); assert.equal(p.options.scenario, 'paired-snake');
    assert.equal(p.options.seconds, 480); assert.equal(p.options.direction, 'x');
    assert.equal(header.plan.endpointAnchorFreshMs, 150);
    assert.equal(header.plan.controlAnchorMaxAgeMs, 500);
    assert.equal(header.plan.pairedReacquireMs, 1000);
    assert.equal(member.filter(r => r.event === 'paired-snake-state').length, 0);
    const pause = member.find(r => r.event === 'paired-snake-pause');
    const resume = member.find(r => r.event === 'paired-snake-resume');
    assert.equal(pause.botIndex, i); assert.equal(pause.controlAt, 150);
    assert.equal(pause.poseStatus, 'paused'); assert.equal(pause.pauseStartedAt, 150);
    assert.deepEqual(pause.pairAnchorAgesMs, [151, 0]);
    assert.equal(resume.botIndex, i); assert.equal(resume.controlAt, 175);
    assert.equal(resume.poseStatus, 'ready'); assert.equal(resume.pauseStartedAt, 150);
    assert.equal(pause.crossingTrial, resume.crossingTrial);
    assert.deepEqual(pause.fixedCenter, resume.fixedCenter);
    const ended = member.filter(r => r.event === 'paired-snake-trial-end');
    assert.equal(ended.length, 1); assert.equal(ended[0].reason, 'pair_unavailable');
    assert.equal(ended[0].botIndex, i); assert.equal(ended[0].endedAt, 200);
    assert.equal(member.at(-1).event, 'capture-interrupted');
    assert.equal(member.at(-1).collectorSourcesUnchanged, true);
  }
});

test('duration deadline, completed closeout, no-join failure, and reconnect exhaustion retain standard outcomes', async context => {
  const saved = memoryRecorders(context);
  const delays = [];
  context.mock.method(Bot.prototype, 'connect', function () { this.joins = 1; });
  context.mock.method(globalThis, 'setTimeout', (callback, delay) => { delays.push(delay); queueMicrotask(callback); });
  for (const seconds of [undefined, 3600]) {
    const out = `memory-duration-${seconds}`;
    await recordPair({ out, server: '127.0.0.1:444', direction: 'x', ...(seconds === undefined ? {} : { seconds }) });
    for (const i of [0, 1]) {
      const rows = saved.at(-2 + i).rows;
      assert.equal(rows.at(-1).event, 'capture-end');
      assert.equal(rows.at(-1).requestedSeconds, seconds ?? 480);
    }
  }
  assert.deepEqual(delays, [480000, 3600000]);
  context.mock.method(Bot.prototype, 'connect', function () {});
  const noJoin = 'memory-no-join';
  await assert.rejects(recordPair({ out: noJoin, server: '127.0.0.1:444', direction: 'x' }), /never joined/);
  assert.equal(saved.at(-2).rows.at(-1).event, 'capture-interrupted');
  context.mock.method(globalThis, 'setTimeout', () => undefined);
  context.mock.method(globalThis, 'setInterval', callback => { queueMicrotask(callback); });
  context.mock.method(Bot.prototype, 'connect', function () { this.joins = 1; this.stopped = true; this.failures = 3; });
  const failed = 'memory-failed';
  await assert.rejects(recordPair({ out: failed, server: '127.0.0.1:444', direction: 'x' }), /reconnect attempts/);
  assert.equal(saved.at(-2).rows.at(-1).failures, 3);
  await assert.rejects(recordPair({ out: failed, server: '127.0.0.1:444', direction: 'x' }), /overwrite/);
});

function rehearse(direction, sign, sparse = false) {
  // Explicit toy physics: constant 160 units/s, finite 3 rad/s turn, no server-law claim.
  const toy = { stepMs: 25, speed: 160, turnRate: 3, maxMs: 60000 };
  const events = [], state = new PairSnakeState(direction, (row, recipient) => events.push({ ...row, recipient }));
  const pair = [makeBot(0, state), makeBot(1, state)];
  for (const { bot } of pair) bot.peers = pair.map(p => p.bot);
  const d = DIRECTIONS[direction];
  pair[0].me.pts[0] = { x: center.x + 80 * -d.y, y: center.y + 80 * d.x };
  const orbitCenter = { ...pair[0].me.pts[0] };
  pair[1].me.pts[0] = railPoint(orbitCenter, direction, sign, 600);
  pair[0].me.ang = Math.atan2(d.y, d.x);
  pair[1].me.ang = Math.atan2(sign * d.y, sign * d.x);
  const physical = pair.map(({ me }) => ({ ...me.pts[0], ang: me.ang }));
  let heldSamples = 0, sentCommands = 0, throttledUpdates = 0, pausedSamples = 0;
  for (let now = 100; now < toy.maxMs; now += toy.stepMs) {
    for (const { bot, me } of pair) {
      const phase = (now - 100) % 2000;
      const receipt = !sparse || now === 100 ||
        (phase < (bot.index ? 200 : 100) || phase >= (bot.index ? 450 : 400)) &&
        (now - 100) % (bot.index ? 125 : 100) === 0;
      if (receipt) {
        me.t = now; me.pts[0] = { x: physical[bot.index].x, y: physical[bot.index].y };
        me.ang = physical[bot.index].ang;
      }
      bot.lastReceivedAt = now; // Other traffic cannot refresh the own anchor.
      const peer = pair[1 - bot.index].me;
      bot.world.snakes.set(peer.id, { ...peer, pts: peer.pts.map(p => ({ ...p })) });
    }
    state.update(now, pair.map(({ bot }, i) => snapshot(bot, pair[1 - i].bot, now)), pair.map(p => p.bot.world));
    assert.ok(state.crossing, JSON.stringify(events));
    if (state.crossing.holdUntil !== null) heldSamples++;
    if (state.poseStatus === 'paused') pausedSamples++;
    for (const { bot, me, sent } of pair) {
      const previous = bot.lastHeading, before = sent.length;
      bot.tick(now);
      assert.equal(bot.boost, false);
      const commands = sent.slice(before).filter(b => b.length === 1 && b[0] <= 250);
      if (state.poseStatus === 'paused') assert.equal(commands.length, 0);
      sentCommands += commands.length;
      if (bot.movementIntent && bot.lastHeading === previous) throttledUpdates++;
      assert.equal(bot.movementIntent.interruption, state.poseStatus === 'paused' ? 'own_anchor_stale' : null);
      if (state.crossing.holdUntil !== null && bot.index === 1)
        assert.deepEqual(bot.movementIntent.targetPoint, railPoint(state.crossing.center,
          direction, sign, state.crossing.holdOffset));
      if (bot.index === 0) assert.ok(Math.hypot(physical[0].x - state.crossing.center.x,
        physical[0].y - state.crossing.center.y) <= C.referenceTolerance);
      // Only the quantized and rate-limited command actually sent by Bot.tick drives motion.
      const delivered = sent.findLast(bytes => bytes.length === 1 && bytes[0] <= 250);
      assert.ok(delivered, 'the robot needs an actually delivered heading command');
      const target = delivered[0] * TAU / 251;
      const actual = physical[bot.index];
      const delta = Math.atan2(Math.sin(target - actual.ang), Math.cos(target - actual.ang));
      const maxTurn = toy.turnRate * toy.stepMs / 1000;
      actual.ang += Math.max(-maxTurn, Math.min(maxTurn, delta));
      const distance = toy.speed * toy.stepMs / 1000;
      actual.x += distance * Math.cos(actual.ang); actual.y += distance * Math.sin(actual.ang);
    }
    if (state.crossing.completedLegs === 2) {
      const endpoints = events.filter(e => e.event === 'paired-snake-endpoint');
      assert.deepEqual(endpoints.map(e => e.crossingEndpoint.completed), [false, true, true]);
      assert.deepEqual(endpoints.map(e => e.crossingEndpoint.end.offset), [800, 2300, 800]);
      assert.ok(endpoints.every(e => e.recipient === 1));
      assert.ok(endpoints.every(e => [e.crossingEndpoint.end.primary, e.crossingEndpoint.end.secondary]
        .every(p => p.anchorAgeMs >= 0 && p.anchorAgeMs <= 150)));
      for (let i = 1; i < 3; i++) {
        assert.deepEqual(endpoints[i].crossingEndpoint.start, endpoints[i - 1].crossingEndpoint.end);
        assert.ok(endpoints[i].crossingEndpoint.end.t - endpoints[i].crossingEndpoint.start.t > C.endpointHoldMs);
      }
      assert.ok(heldSamples >= 160); assert.ok(sentCommands > 20); assert.ok(throttledUpdates > 20);
      assert.equal(state.crossing.sign, sign);
      assert.equal(state.crossing.feasibilityPassed, !sparse);
      if (sparse) {
        assert.ok(pausedSamples > 0);
        assert.ok(endpoints.slice(1).every(e => e.crossingEndpoint.interrupted));
        assert.ok(events.some(e => e.event === 'paired-snake-resume'));
      }
      return;
    }
  }
  assert.fail(`toy route failed: ${JSON.stringify({ direction, sign, events })}`);
}

const TAU = Math.PI * 2;
test('actual sent controls complete near-far-near in finite-turn toy physics, four directions and signs', async context => {
  for (const direction of Object.keys(DIRECTIONS)) for (const sign of [-1, 1])
    await context.test(`${direction} sign ${sign}`, () => rehearse(direction, sign));
});

test('sparse asymmetric receipts yield nominal fresh endpoints but never a strict clean round', async context => {
  for (const direction of Object.keys(DIRECTIONS)) for (const sign of [-1, 1])
    await context.test(`${direction} sign ${sign}`, () => rehearse(direction, sign, true));
});
