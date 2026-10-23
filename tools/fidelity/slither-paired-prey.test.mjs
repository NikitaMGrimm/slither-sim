import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';
import { Bot, World } from './slither-headless.mjs';
import { CROSSING, PAIR_PREY, PairPreyState, PairPreyBot, snapshot, preyOffset,
  crossingOffset, crossingProjection, safePreyWaypoint, followSteering, parseArgs,
  recordPair } from './slither-paired-prey.mjs';

const pose = (id, x, y, prey = [], extra = {}) =>
  ({ id, life: 1, x, y, anchorAgeMs: 0, peerVisible: true, prey, ...extra });
const prey = (id, t = 100, generation = 1, x = 21600, y = 21600) =>
  ({ id, t, generation, x, y, sp: 6.06 });
const worlds = [new World(), new World()];
const nearX = 22000 + CROSSING.nearOffset;
const nearY = 21600 + CROSSING.nearOffset;

test('crossing near waypoint enters the 1000-unit circle without sharing the reference offset', () => {
  assert.ok(CROSSING.nearOffset * Math.SQRT2 < 1000);
  assert.ok(CROSSING.nearOffset - CROSSING.followerOffset >= 200);
});

test('crossing endpoint observation hold keeps its waypoint and cannot count another endpoint', () => {
  const state = new PairPreyState('crossing', 'y');
  worlds[0].grd = worlds[1].grd = 21600;
  worlds[0].radius = worlds[1].radius = 21000;
  const sample = (t, offset) => state.update(t, [
    pose(1, 22000, 21600 + CROSSING.followerOffset, [prey(7, t, 1, 22000, 21600)]),
    pose(2, 22000, 21600 + offset),
  ], worlds);
  sample(100, CROSSING.nearOffset);
  sample(200, CROSSING.nearOffset);
  assert.equal(state.crossing.endpointVisits, 1);
  assert.equal(state.crossing.completedLegs, 0);
  assert.deepEqual(crossingOffset(prey(7, 200, 1, 22000, 21600), state.crossing, 1),
    { x: 22000, y: 21600 + CROSSING.nearOffset });
  const until = 2200;
  sample(until - 1, CROSSING.farOffset);
  assert.equal(state.crossing.endpointVisits, 1);
  assert.equal(state.crossing.completedLegs, 0);
  sample(until, CROSSING.nearOffset);
  assert.deepEqual(crossingOffset(prey(7, until, 1, 22000, 21600), state.crossing, 1),
    { x: 22000, y: 21600 + CROSSING.farOffset });
  sample(until + 100, CROSSING.farOffset);
  assert.equal(state.crossing.completedLegs, 1);
  assert.equal(state.crossing.lastEndpoint.start.t, 200);
  assert.equal(state.crossing.lastEndpoint.end.t, until + 100);
  sample(until + 101, CROSSING.nearOffset);
  assert.equal(state.crossing.completedLegs, 1);
  assert.equal(state.crossing.holdOffset, CROSSING.farOffset);
});

test('near holds keep checking the future far arena margin before release', () => {
  for (const driftTime of [1200, 2200]) {
    const state = new PairPreyState('crossing', 'diagonal-positive');
    worlds[0].grd = worlds[1].grd = 21600;
    worlds[0].radius = worlds[1].radius = 21000;
    const sample = (t, coordinate) => state.update(t, [
      pose(1, coordinate + 450, coordinate + 450, [prey(7, t, 1, coordinate, coordinate)]),
      pose(2, coordinate + CROSSING.nearOffset, coordinate + CROSSING.nearOffset),
    ], worlds);
    sample(100, 34300); sample(200, 34300);
    assert.equal(state.crossing.holdOffset, CROSSING.nearOffset);
    assert.equal(state.crossing.holdUntil, 2200);
    const farMargin = 21000 - Math.SQRT2 * (34400 + CROSSING.farOffset - 21600);
    assert.ok(farMargin < CROSSING.minArenaGap - CROSSING.farOffset);
    sample(driftTime, 34400);
    assert.equal(state.target, null);
    assert.equal(state.reason, 'rim_margin_lost');
  }
});

test('hold release cannot credit the opposite endpoint before its control update', () => {
  const state = new PairPreyState('crossing', 'y');
  const sample = (t, offset) => state.update(t, [
    pose(1, 22000, 22050, [prey(7, t, 1, 22000, 21600)]),
    pose(2, 22000, 21600 + offset),
  ], worlds);
  sample(100, CROSSING.nearOffset); sample(200, CROSSING.nearOffset);
  const release = 200 + CROSSING.endpointHoldMs;
  sample(release, CROSSING.farOffset);
  assert.equal(state.crossing.holdUntil, null);
  assert.equal(state.crossing.holdOffset, null);
  assert.equal(state.crossing.completedLegs, 0);
  assert.equal(state.crossing.endpointVisits, 1);
  assert.deepEqual(crossingOffset(prey(7, release, 1, 22000, 21600), state.crossing, 1),
    { x: 22000, y: 21600 + CROSSING.farOffset });
  sample(release + 25, CROSSING.farOffset);
  assert.equal(state.crossing.completedLegs, 1);
  assert.equal(state.crossing.lastEndpoint.end.t, release + 25);
});

test('pair snapshots retain the raw snake anchor age', () => {
  const bot = new PairPreyBot({ scenario: 'movement' }, 0, { meta() {} });
  const peer = new PairPreyBot({ scenario: 'movement' }, 1, { meta() {} });
  bot.lives = 1;
  bot.ws = { readyState: 1 };
  bot.world.self = 1;
  bot.world.snakes.set(1, { id: 1, pts: [{ x: 21600, y: 21600 }], t: 75 });
  assert.equal(snapshot(bot, peer, 100).anchorAgeMs, 25);
});

test('pair rendezvous starts with both lives and waits for actual mutual visibility', () => {
  const state = new PairPreyState();
  state.update(100, [pose(1, 20000, 21000), null], worlds);
  assert.equal(state.phase, 'waiting');
  state.update(200, [pose(1, 20000, 21000), pose(2, 22000, 21000)], worlds);
  assert.equal(state.phase, 'rendezvous');
  assert.deepEqual(state.identities, ['0/1/1', '1/1/2']);
  assert.equal(state.meetingTargets[0].x, 21000);
  state.update(300, [pose(1, 20700, 21000, [], { peerVisible: false }),
    pose(2, 21300, 21000)], worlds);
  assert.equal(state.phase, 'rendezvous');
  state.update(400, [pose(1, 20700, 21000), pose(2, 21300, 21000)], worlds);
  assert.equal(state.phase, 'search');
  assert.deepEqual(state.searchCenter, { x: 21000, y: 21000 });
});

test('target selection prefers actually shared fresh prey and excludes rim prey', () => {
  const state = new PairPreyState();
  worlds[0].grd = worlds[1].grd = 21600;
  worlds[0].radius = worlds[1].radius = 21000;
  const a = pose(1, 21200, 21600, [prey(7), prey(8, 100, 1, 42500, 21600)]);
  const b = pose(2, 21800, 21600, [prey(7)]);
  state.update(100, [a, b], worlds);
  assert.equal(state.phase, 'follow');
  assert.deepEqual(state.target, { id: 7, generations: [1, 1] });
  assert.equal(state.pairDistance, 600);
  assert.equal(Math.max(Math.abs(preyOffset(prey(7), 21600, -1).x - 21600),
    Math.abs(preyOffset(prey(7), 21600, -1).y - 21600)), 450);
});

test('one observer losing prey does not assert shared visibility or drop the other view', () => {
  const state = new PairPreyState();
  state.update(100, [pose(1, 21200, 21600, [prey(7)]),
    pose(2, 21800, 21600, [prey(7)])], worlds);
  state.update(1200, [pose(1, 21200, 21600, []),
    pose(2, 21800, 21600, [prey(7, 1150)])], worlds);
  assert.equal(state.target.id, 7);
  assert.equal(state.phase, 'follow');
  state.update(3700, [pose(1, 21200, 21600, []), pose(2, 21800, 21600, [])], worlds);
  assert.equal(state.target, null);
  assert.equal(state.reason, 'prey_lost');
});

test('prey generation and life changes clear target before reacquisition', () => {
  const state = new PairPreyState();
  state.update(100, [pose(1, 21200, 21600, [prey(7)]),
    pose(2, 21800, 21600, [prey(7)])], worlds);
  state.update(200, [pose(1, 21200, 21600, [prey(7, 200, 2)]),
    pose(2, 21800, 21600, [])], worlds);
  assert.equal(state.target.id, 7);
  assert.deepEqual(state.target.generations, [2, null]);
  state.update(300, [pose(1, 21200, 21600, [prey(7, 300, 2)]),
    pose(2, 21800, 21600, [], { life: 2 })], worlds);
  assert.deepEqual(state.identities, ['0/1/1', '1/2/2']);
  assert.equal(state.target.id, 7);
  assert.deepEqual(state.target.generations, [2, null]);
  state.update(400, [null, pose(2, 21800, 21600, [])], worlds);
  assert.equal(state.target, null);
  assert.equal(state.phase, 'waiting');
});

test('large pair separation returns to rendezvous and clears selected prey', () => {
  const state = new PairPreyState();
  state.update(100, [pose(1, 21200, 21600, [prey(7)]),
    pose(2, 21800, 21600, [prey(7)])], worlds);
  state.update(200, [pose(1, 20000, 21600, [prey(7, 200)]),
    pose(2, 22000, 21600, [prey(7, 200)])], worlds);
  assert.equal(state.phase, 'rendezvous');
  assert.equal(state.target, null);
  assert.equal(state.reason, 'pair_separated');
});

test('steering routes around prey and respects other snake and rim avoidance', () => {
  const bot = new PairPreyBot({ scenario: 'movement' }, 1, { meta() {} });
  bot.world.self = 2;
  bot.world.grd = 21600;
  const me = { id: 2, pts: [{ x: 21000, y: 21600 }], sct: 6,
    ang: 0, sp: 5.7, t: 0 };
  bot.world.snakes.set(2, me);
  bot.world.prey.set(7, prey(7));
  const state = { phase: 'follow', target: { id: 7, generations: [null, 1] } };
  assert.deepEqual(safePreyWaypoint({ x: 21000, y: 21600 }, prey(7),
    { x: 22050, y: 21600 }), { x: 21000, y: 22050 });
  assert.deepEqual(safePreyWaypoint({ x: 21700, y: 22050 }, prey(7),
    { x: 22050, y: 21600 }), { x: 22050, y: 22050 },
  'stay outside the prey square until reaching the assigned side');
  let choice = { a: 0, danger: 0 };
  let intent = followSteering(bot, state, 100, me, choice);
  assert.equal(intent.desiredBoost, false);
  assert.equal(intent.targetPoint.y, 22050);
  assert.equal(choice.a, Math.PI / 2);
  bot.world.snakes.set(3, { id: 3, pts: [{ x: 21000, y: 21700 }],
    sct: 6, ang: 0, sp: 0, t: 0 });
  choice = { a: 0, danger: 0 };
  intent = followSteering(bot, state, 100, me, choice);
  assert.equal(intent.interruption, 'avoidance');
  assert.equal(choice.a, 0);
});

test('shared search keeps both observers on coordinated opposite lanes', () => {
  const state = new PairPreyState();
  state.phase = 'search';
  state.searchCenter = { x: 21600, y: 21600 };
  const bots = [0, 1].map(index => {
    const bot = new PairPreyBot({ scenario: 'movement' }, index, { meta() {} });
    bot.world.self = index + 1;
    bot.world.snakes.set(index + 1, { id: index + 1,
      pts: [{ x: 21500 + index * 200, y: 21600 }], sct: 6,
      ang: 0, sp: 5.7, t: 0 });
    return bot;
  });
  const points = bots.map(bot => followSteering(bot, state, 100,
    bot.world.snakes.get(bot.world.self), { a: 0 }).targetPoint);
  assert.ok(Math.abs(Math.hypot(points[0].x - points[1].x,
    points[0].y - points[1].y) - 480) < 1e-8);
  assert.equal(followSteering(bots[0], state, 100,
    bots[0].world.snakes.get(1), { a: 0 }).navigationSource, 'shared_search');
});

test('observer without a local packet follows peer coordinates without claiming visibility', () => {
  const state = new PairPreyState();
  state.update(100, [pose(1, 21200, 21600, []),
    pose(2, 21800, 21600, [prey(7)])], worlds);
  const events = [], bot = new PairPreyBot({ scenario: 'movement' }, 0,
    { meta(row) { events.push(row); }, frame() {} });
  bot.pair = state;
  bot.lives = 1;
  bot.ws = { readyState: 1, send() {} };
  bot.lastReceivedAt = 1e9;
  bot.world.self = 1;
  bot.world.grd = 21600;
  bot.world.snakes.set(1, { id: 1, pts: [{ x: 21200, y: 21600 }],
    sct: 6, fam: .2, ang: 0, sp: 5.7, t: 0 });
  bot.tick(150);
  const sample = events.find(e => e.event === 'paired-prey-state');
  assert.equal(sample.targetVisible, false);
  assert.equal(sample.targetX, null);
  assert.equal(sample.navigationSource, 'peer');
  assert.equal(sample.navigationPreyObserver, 1);
  assert.equal(sample.navigationPreyAgeMs, 50);
  assert.notEqual(sample.intendedHeading, null);
});

test('at the assigned prey offset the observer deliberately orbits', () => {
  const bot = new PairPreyBot({ scenario: 'movement' }, 1, { meta() {} });
  bot.world.self = 2;
  bot.world.grd = 21600;
  const me = { id: 2, pts: [{ x: 22050, y: 21600 }],
    sct: 6, ang: 0, sp: 5.7, t: 100 };
  bot.world.snakes.set(2, me);
  bot.world.prey.set(7, prey(7));
  const state = { phase: 'follow', target: { id: 7, generations: [null, 1] },
    navigationPrey: { ...prey(7), observer: 1 } };
  const choice = { a: 0, danger: 0 };
  const intent = followSteering(bot, state, 100, me, choice);
  assert.equal(intent.targetPoint.x, 22050);
  assert.equal(intent.navigationSource, 'local');
  assert.equal(intent.intendedHeading, Math.PI / 2);
  assert.equal(choice.a, Math.PI / 2);
});

test('observer metadata distinguishes target visibility and logs actual sent control', () => {
  const events = [], bot = new PairPreyBot({ scenario: 'movement' }, 0,
    { meta(row) { events.push(row); }, frame() {} });
  bot.ws = { readyState: 1, send() {} };
  bot.lastReceivedAt = 1e9;
  bot.world.self = 1;
  bot.world.snakes.set(1, { id: 1, pts: [{ x: 21000, y: 21600 }],
    sct: 6, fam: .2, ang: 0, sp: 5.7, t: 0 });
  bot.pair = { phase: 'follow', target: { id: 7, generations: [null, 1] },
    pairDistance: 600, identities: ['0/1/1', '1/1/2'] };
  bot.tick(100);
  const missing = events.find(e => e.event === 'paired-prey-state');
  assert.equal(missing.targetVisible, false);
  assert.equal(missing.targetX, null);
  assert.equal(missing.sentHeading, bot.lastHeading);
  bot.world.prey.set(7, prey(7, 150));
  bot.tick(200);
  const visible = events.filter(e => e.event === 'paired-prey-state').at(-1);
  assert.equal(visible.targetVisible, true);
  assert.equal(visible.targetGeneration, 1);
  assert.equal(visible.targetAgeMs, 50);
  assert.equal(visible.targetSpeed, 6.06);
});

test('crossing freezes tangent axis and advances only from measured near/far endpoints', () => {
  const state = new PairPreyState('crossing');
  const sample = (time, bx, by, p = prey(7, time, 1, 22000, 21600), bPrey = []) =>
    state.update(time, [pose(1, 22000, 22050, [p]),
      pose(2, bx, by, bPrey)], worlds);
  sample(100, 22000, 22100);
  assert.equal(state.phase, 'follow');
  assert.equal(state.crossing.axis, 'y');
  assert.equal(state.crossing.sign, 1);
  assert.equal(state.crossing.trial, 1);
  assert.equal(state.crossing.selectedAt, 100);
  assert.deepEqual(crossingOffset(prey(7, 100, 1, 22000, 21600), state.crossing, 0),
    { x: 22000, y: 22050 });
  assert.deepEqual(crossingOffset(prey(7, 100, 1, 22000, 21600), state.crossing, 1),
    { x: 22000, y: nearY });
  sample(200, 22000, 22100); // 500 is outside the near650 endpoint tolerance.
  assert.equal(state.crossing.leg, 'near');
  sample(300, 22000, nearY);
  assert.equal(state.crossing.leg, 'far');
  assert.equal(state.crossing.completedLegs, 0);
  sample(2400, 22000, 23100);
  assert.equal(state.crossing.leg, 'far');
  sample(2500, 22000, 23450);
  assert.equal(state.crossing.leg, 'near');
  assert.equal(state.crossing.completedLegs, 1);
  assert.equal(state.crossing.axis, 'y');
  assert.equal(CROSSING.maxPairSeparation, 2400);
});

test('crossing does not advance at B endpoint until the reference geometry is ready', () => {
  const state = new PairPreyState('crossing');
  const a = (time, y) => pose(1, 22000, y, [prey(7, time, 1, 22000, 21600)]);
  state.update(100, [a(100, 22050), pose(2, 22000, 22100)], worlds);
  state.update(200, [a(200, 21800), pose(2, 22000, nearY)], worlds);
  assert.equal(state.crossing.leg, 'near');
  assert.equal(state.crossing.completedLegs, 0);
  state.update(300, [a(300, 22050), pose(2, 22000, nearY)], worlds);
  assert.equal(state.crossing.leg, 'far');
});

test('initial approach arms a leg without claiming an achieved opposite starting endpoint', () => {
  const state = new PairPreyState('crossing');
  const sample = (time, y) => state.update(time,
    [pose(1, 22000, 22050, [prey(7, time, 1, 22000, 21600)]),
      pose(2, 22000, y)], worlds);
  sample(100, 22100);
  sample(200, nearY);
  assert.equal(state.crossing.completedLegs, 0);
  sample(200 + CROSSING.endpointHoldMs, nearY);
  assert.equal(state.crossing.completedLegs, 0);
  sample(2300, 23450);
  assert.equal(state.crossing.completedLegs, 1);
});

test('crossing endpoints require fresh known anchors and primary prey', () => {
  for (const [aAge, bAge, preyAge] of [[151, 0, 0], [0, 151, 0],
    [undefined, 0, 0], [0, null, 0], [0, 0, 351], [0, 0, -1]]) {
    const state = new PairPreyState('crossing');
    const a = (time, age) => pose(1, 22000, 22050,
      [prey(7, time, 1, 22000, 21600)], { anchorAgeMs: age });
    state.update(100, [a(100, 0), pose(2, 22000, 22100)], worlds);
    state.update(500, [a(500 - preyAge, aAge),
      pose(2, 22000, nearY, [], { anchorAgeMs: bAge })], worlds);
    assert.equal(state.crossing.leg, 'near');
    assert.equal(state.crossing.endpointVisits, 0);
    state.update(600, [a(250, 150),
      pose(2, 22000, nearY, [], { anchorAgeMs: 150 })], worlds);
    assert.equal(state.crossing.leg, 'far', 'inclusive frozen endpoint boundaries');
    assert.equal(state.crossing.endpointVisits, 1);
  }
});

test('endpoint metadata preserves exact start/end witnesses once on the route observer', () => {
  const state = new PairPreyState('crossing');
  const sample = (time, y, life = 1) => state.update(time,
    [pose(1, 22000, 22050, [prey(7, time, 1, 22000, 21600)]),
      pose(2, 22000, y, [], { life })], worlds);
  sample(100, 22100);
  sample(200, nearY);
  const events = [], bot = new PairPreyBot({ scenario: 'movement' }, 1,
    { meta(row) { events.push(row); }, frame() {} });
  bot.pair = state; bot.lives = 1;
  bot.ws = { readyState: 1, send() {} }; bot.lastReceivedAt = 1e9;
  bot.world.self = 2; bot.world.grd = 21600;
  const setHead = (y, t) => bot.world.snakes.set(2,
    { id: 2, pts: [{ x: 22000, y }], sct: 6, fam: .2, ang: 0, sp: 5.7, t });
  setHead(nearY, 200); bot.tick(250); bot.tick(350);
  let endpoints = events.filter(e => e.crossingEndpoint);
  assert.equal(endpoints.length, 1);
  assert.equal(endpoints[0].crossingEndpoint.completed, false);
  assert.equal(endpoints[0].crossingEndpoint.start, null);
  assert.equal(endpoints[0].crossingEndpoint.end.t, 200);
  assert.equal(endpoints[0].crossingLeg, 'far');
  assert.equal(endpoints[0].crossingHeldOffset, CROSSING.nearOffset);
  assert.equal(endpoints[0].intendedOffset, CROSSING.nearOffset);
  assert.equal(endpoints[0].crossingHoldUntil, 2200);
  bot.tick(2190);
  sample(2200, nearY); setHead(nearY, 2200); bot.tick(2215);
  const released = events.at(-1);
  assert.equal(released.crossingHoldUntil, null);
  assert.equal(released.crossingHeldOffset, null);
  assert.equal(released.intendedOffset, CROSSING.farOffset);
  assert.equal(released.crossingEndpoint, undefined);
  sample(2400, 23450);
  setHead(23450, 2400); bot.tick(2450);
  endpoints = events.filter(e => e.crossingEndpoint);
  assert.equal(endpoints.length, 2);
  const leg = endpoints[1].crossingEndpoint;
  assert.equal(leg.completed, true);
  assert.equal(leg.start.t, 200); assert.equal(leg.end.t, 2400);
  assert.equal(leg.start.offset, CROSSING.nearOffset); assert.equal(leg.end.offset, 1850);
  assert.equal(endpoints[1].completedLegs, 1);
  assert.equal(endpoints[1].endpointVisits, 2);
  assert.deepEqual(leg.end.pairIdentities, ['0/1/1', '1/1/2']);
  sample(2500, 22100, 2);
  assert.equal(state.crossing.completedLegs, 0);
  assert.equal(state.crossing.lastEndpoint, null);
  assert.equal(state.crossing.holdUntil, null);
  assert.equal(state.crossing.holdOffset, null);
});

test('crossing selection keeps the primary outside the flight box on the assigned side', () => {
  const state = new PairPreyState('crossing');
  const close = prey(7, 100, 1, 22000, 22000);
  const safe = prey(8, 100, 1, 22000, 21600);
  state.update(100, [pose(1, 22000, 22400, [close, safe]),
    pose(2, 22000, 22500, [close, safe])], worlds);
  assert.equal(state.target.id, 8, 'skip the closer prey without assigned-axis clearance');
  const opposite = new PairPreyState('crossing');
  opposite.update(100, [pose(1, 22000, 21500, [safe]),
    pose(2, 22000, 22100, [safe])], worlds);
  assert.equal(opposite.target, null, 'do not route the primary through the prey to reach B side');
  const diagonal = new PairPreyState('crossing');
  diagonal.update(100, [pose(1, 22500, 21800, [safe]),
    pose(2, 22000, 22100, [safe])], worlds);
  assert.equal(diagonal.target, null, 'box distance alone does not protect the selected axis');
  const insideB = new PairPreyState('crossing');
  insideB.update(100, [pose(1, 22000, 22050, [safe]),
    pose(2, 22000, 21800, [safe])], worlds);
  assert.equal(insideB.target, null, 'B must also begin outside the selected-axis box');
});

test('crossing telemetry marks only measured geometry outside the flight box as ready', () => {
  const state = new PairPreyState('crossing');
  const p = prey(7, 100, 1, 22000, 21600);
  state.update(100, [pose(1, 22000, 22050, [p], { anchorAgeMs: 20 }),
    pose(2, 22000, 22400, [], { anchorAgeMs: 35 })], worlds);
  const events = [], bot = new PairPreyBot({ scenario: 'movement' }, 1,
    { meta(row) { events.push(row); }, frame() {} });
  bot.pair = state; bot.lives = 1;
  bot.ws = { readyState: 1, send() {} }; bot.lastReceivedAt = 1e9;
  bot.world.self = 2; bot.world.grd = 21600;
  bot.world.snakes.set(2, { id: 2, pts: [{ x: 22000, y: 22400 }],
    sct: 6, fam: .2, ang: 0, sp: 5.7, t: 100 });
  bot.tick(150);
  let row = events.find(e => e.event === 'paired-prey-state');
  assert.equal(row.primaryPreyAxisDistance, 450);
  assert.equal(row.secondaryPreyBoxDistance, 800);
  assert.equal(row.secondaryPreyAxisDistance, 800);
  assert.equal(row.secondaryPreyCrossTrack, 0);
  assert.equal(row.primaryAnchorAgeMs, 20);
  assert.equal(row.secondaryAnchorAgeMs, 35);
  assert.equal(row.navigationPreyAgeMs, 50);
  assert.equal(row.crossingGeometryReady, true);
  state.update(200, [pose(1, 22000, 21800, [prey(7, 200, 1, 22000, 21600)]),
    pose(2, 22000, 22400, [])], worlds);
  bot.tick(250);
  row = events.at(-1);
  assert.equal(row.primaryPreyBoxDistance, 200);
  assert.equal(row.crossingGeometryReady, false);
  state.update(300, [pose(1, 22000, 22050, [prey(7, 300, 1, 22000, 21600)]),
    pose(2, 22400, 22400, [])], worlds);
  bot.tick(350);
  row = events.at(-1);
  assert.equal(row.secondaryPreyAxisDistance, 800);
  assert.equal(row.secondaryPreyCrossTrack, 400);
  assert.equal(row.crossingGeometryReady, false);
});

test('crossing allows only observer B reentry while primary generation stays fixed', () => {
  const state = new PairPreyState('crossing');
  const a = (t, generation = 1) => pose(1, 22000, 22050,
    [prey(7, t, generation, 22000, 21600)]);
  state.update(100, [a(100), pose(2, 22000, 22100, [prey(7)])], worlds);
  state.update(200, [a(200), pose(2, 22000, nearY, [])], worlds);
  assert.equal(state.crossing.leg, 'far');
  state.update(300, [a(300), pose(2, 22000, 22500,
    [prey(7, 300, 2, 22000, 21600)])], worlds);
  assert.equal(state.target.generations[0], 1);
  assert.equal(state.target.generations[1], 2);
  assert.equal(state.crossing.reentries, 1);
  assert.equal(state.crossing.trial, 1);
  state.update(400, [a(400, 2), pose(2, 22000, 22500, [])], worlds);
  assert.equal(state.crossing.trial, 2, 'primary generation change starts a new trial');
  assert.equal(state.crossing.completedLegs, 0);
  assert.equal(state.target.generations[0], 2);
  assert.equal(state.crossing.holdUntil, null);
});

test('crossing uses primary coordinates for B control but logs local visibility separately', () => {
  const state = new PairPreyState('crossing');
  state.update(100, [pose(1, 22000, 22050, [prey(7, 100, 1, 22000, 21600)]),
    pose(2, 22000, 22100, [])], worlds);
  const events = [], bot = new PairPreyBot({ scenario: 'movement' }, 1,
    { meta(row) { events.push(row); }, frame() {} });
  bot.pair = state; bot.lives = 1;
  bot.ws = { readyState: 1, send() {} }; bot.lastReceivedAt = 1e9;
  bot.world.self = 2; bot.world.grd = 21600;
  bot.world.snakes.set(2, { id: 2, pts: [{ x: 22000, y: 22100 }],
    sct: 6, fam: .2, ang: 0, sp: 5.7, t: 100 });
  bot.tick(150);
  const row = events.find(e => e.event === 'paired-prey-state');
  assert.equal(row.mode, 'crossing');
  assert.equal(row.crossingLeg, 'near');
  assert.equal(row.crossingTrial, 1);
  assert.equal(row.targetVisible, false);
  assert.equal(row.navigationSource, 'primary');
  assert.equal(row.navigationPreyObserver, 0);
  assert.equal(row.intendedOffset, CROSSING.nearOffset);
  assert.equal(row.actualPreyBoxDistance, 500);
  assert.equal(row.boost, false);
});

test('crossing rejects stale primary prey and opens a new trial after leg timeout', () => {
  const state = new PairPreyState('crossing');
  const a = t => pose(1, 22000, 22050, [prey(7, t, 1, 22000, 21600)]);
  state.update(100, [a(100), pose(2, 22000, 22100)], worlds);
  state.update(800, [a(100), pose(2, 22000, 22100)], worlds);
  assert.equal(state.target, null);
  assert.equal(state.reason, 'primary_visibility_lost');
  state.update(900, [a(900), pose(2, 22000, 22100)], worlds);
  assert.equal(state.crossing.trial, 2);
  state.update(900 + CROSSING.legTimeoutMs + 1,
    [a(900 + CROSSING.legTimeoutMs + 1), pose(2, 22000, 22100)], worlds);
  assert.equal(state.crossing.trial, 3);
  assert.equal(state.crossing.completedLegs, 0);
});

test('CLI requires bounded duration, explicit server and fresh output', async () => {
  assert.deepEqual(parseArgs(['--out', 'fresh', '--server', '127.0.0.1:444']),
    { out: 'fresh', server: '127.0.0.1:444', seconds: 480, mode: 'overlap' });
  assert.equal(parseArgs(['--out', 'fresh', '--server', '127.0.0.1:444',
    '--mode', 'crossing']).mode, 'crossing');
  assert.throws(() => parseArgs(['--out', 'a', '--out', 'b']), /repeated/);
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:444', seconds: 481 }), /1..480/);
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:0', seconds: 1 }), /invalid server/);
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:444',
    mode: 'invalid' }), /mode overlap\|crossing/);
});

const crossingSample = (state, index, x, y, now) => {
  const events = [], bot = new PairPreyBot({ scenario: 'movement' }, index,
    { meta(row) { events.push(row); }, frame() {} });
  bot.pair = state; bot.lives = 1;
  bot.ws = { readyState: 1, send() {} }; bot.lastReceivedAt = 1e9;
  bot.world.self = index + 1; bot.world.grd = 21600;
  bot.world.snakes.set(index + 1, { id: index + 1, pts: [{ x, y }],
    sct: 6, fam: .2, ang: 0, sp: 5.7, t: now });
  bot.tick(now);
  return { bot, row: events.find(e => e.event === 'paired-prey-state') };
};

test('fixed diagonal crossing offsets follower, near and far by max-axis distance', () => {
  worlds[0].grd = worlds[1].grd = 21600;
  worlds[0].radius = worlds[1].radius = 21000;
  const p = prey(7, 100, 1, 22000, 21600);
  const positive = new PairPreyState('crossing', 'diagonal-positive');
  positive.update(100, [pose(1, 22450, 22050, [p]), pose(2, 22500, 22100)], worlds);
  assert.equal(positive.phase, 'follow');
  assert.equal(positive.crossing.direction, 'diagonal-positive');
  assert.equal(positive.crossing.axis, null);
  assert.equal(positive.crossing.sign, 1);
  assert.deepEqual(crossingOffset(p, positive.crossing, 0), { x: 22450, y: 22050 });
  assert.deepEqual(crossingOffset(p, positive.crossing, 1), { x: nearX, y: nearY });
  assert.deepEqual(crossingOffset(p, { ...positive.crossing, leg: 'far' }, 1),
    { x: 23850, y: 23450 });
  const negative = new PairPreyState('crossing', 'diagonal-negative');
  negative.update(100, [pose(1, 21550, 22050, [p]), pose(2, 21500, 22100)], worlds);
  assert.equal(negative.crossing.sign, -1, 'B side of the fixed diagonal selects the sign');
  assert.deepEqual(crossingOffset(p, negative.crossing, 0), { x: 21550, y: 22050 });
  assert.deepEqual(crossingOffset(p, negative.crossing, 1), { x: 22000 - CROSSING.nearOffset, y: nearY });
  assert.deepEqual(crossingOffset(p, { ...negative.crossing, leg: 'far' }, 1),
    { x: 20150, y: 23450 });
  const offRoute = crossingProjection(positive.crossing, 2300, 1400);
  assert.equal(offRoute.along, 1850);
  assert.ok(Math.abs(offRoute.crossTrack + 900 * Math.SQRT1_2) < 1e-9);
  const across = crossingProjection(negative.crossing, -1000, 1000);
  assert.equal(across.along, 1000);
  assert.equal(across.crossTrack, 0);
});

test('fixed diagonal legs complete only at measured on-route endpoints', () => {
  const state = new PairPreyState('crossing', 'diagonal-positive');
  const sample = (time, bx, by) => state.update(time,
    [pose(1, 22450, 22050, [prey(7, time, 1, 22000, 21600)]), pose(2, bx, by)], worlds);
  sample(100, 22500, 22100);
  sample(150, 22800, 21600); // Box distance 800 is only 400 along the diagonal.
  assert.equal(state.crossing.leg, 'near');
  sample(200, nearX, nearY);
  assert.equal(state.crossing.leg, 'far');
  assert.equal(state.crossing.completedLegs, 0);
  assert.equal(state.crossing.actualAxisDistance, CROSSING.nearOffset);
  assert.equal(state.crossing.actualCrossTrack, 0);
  sample(2300, 24300, 23000); // Along 1850 but 636 off the route.
  assert.equal(state.crossing.leg, 'far');
  assert.equal(state.crossing.actualAxisDistance, 1850);
  sample(2350, 23850, 21600); // Max-axis 1850 on the x axis is not the diagonal far point.
  assert.equal(state.crossing.leg, 'far');
  sample(2400, 23800, 23500);
  assert.equal(state.crossing.leg, 'near');
  assert.equal(state.crossing.completedLegs, 1);
  assert.equal(state.crossing.direction, 'diagonal-positive');
});

test('fixed diagonal rejects prey whose far endpoint would leave the arena margin', () => {
  worlds[0].grd = worlds[1].grd = 21600;
  worlds[0].radius = worlds[1].radius = 21000;
  const along = (distance, offset) => 21600 + distance * Math.SQRT1_2 + offset;
  const at = (distance, a, b, time = 100) => [pose(1, along(distance, a), along(distance, a),
    [prey(7, time, 1, along(distance, 0), along(distance, 0))]),
  pose(2, along(distance, b), along(distance, b))];
  // The prey gap is 2300, above minArenaGap, but the outward diagonal far point is outside.
  const outward = new PairPreyState('crossing', 'diagonal-positive');
  outward.update(100, at(18700, 450, 500), worlds);
  assert.equal(outward.target, null);
  const adaptive = new PairPreyState('crossing');
  adaptive.update(100, at(18700, 450, 500), worlds);
  assert.equal(adaptive.target.id, 7, 'the prey gap alone would accept this prey');
  const cardinal = new PairPreyState('crossing', 'x');
  cardinal.update(100, at(18700, 450, 500), worlds);
  assert.equal(cardinal.target.id, 7, 'an outward x far point keeps the margin here');
  const inward = new PairPreyState('crossing', 'diagonal-positive');
  inward.update(100, at(18700, -450, -500), worlds);
  assert.equal(inward.crossing.sign, -1);
  const drifting = new PairPreyState('crossing', 'diagonal-positive');
  drifting.update(100, at(17000, 450, 500), worlds);
  assert.equal(drifting.target.id, 7);
  drifting.update(200, at(18700, 450, 500, 200), worlds);
  assert.equal(drifting.target, null);
  assert.equal(drifting.reason, 'rim_margin_lost');
});

test('fixed diagonal telemetry projects geometry and keeps the primary outside the flight square', () => {
  const state = new PairPreyState('crossing', 'diagonal-positive');
  state.update(100, [pose(1, 22450, 22050, [prey(7, 100, 1, 22000, 21600)], { anchorAgeMs: 20 }),
    pose(2, 22500, 22100, [], { anchorAgeMs: 35 })], worlds);
  let { row } = crossingSample(state, 1, 22500, 22100, 150);
  assert.equal(row.crossingDirection, 'diagonal-positive');
  assert.equal(row.crossingAxis, null);
  assert.equal(row.crossingSign, 1);
  assert.equal(row.intendedOffset, CROSSING.nearOffset);
  assert.equal(row.primaryPreyAxisDistance, 450);
  assert.equal(row.primaryPreyCrossTrack, 0);
  assert.equal(row.primaryPreyBoxDistance, 450);
  assert.equal(row.primaryGeometryReady, true);
  assert.equal(row.secondaryPreyAxisDistance, 500);
  assert.equal(row.secondaryPreyCrossTrack, 0);
  assert.equal(row.crossingGeometryReady, true);
  assert.equal(row.actualPreyAxisDistance, 500);
  assert.equal(row.actualPreyCrossTrack, 0);
  assert.equal(row.actualPreyBoxDistance, 500);
  state.update(200, [pose(1, 22250, 21850, [prey(7, 200, 1, 22000, 21600)]),
    pose(2, 22500, 22100)], worlds);
  ({ row } = crossingSample(state, 1, 22500, 22100, 250));
  assert.equal(row.primaryPreyBoxDistance, 250);
  assert.equal(row.primaryGeometryReady, false);
  state.update(300, [pose(1, 22700, 21800, [prey(7, 300, 1, 22000, 21600)]),
    pose(2, 22500, 22100)], worlds);
  ({ row } = crossingSample(state, 1, 22500, 22100, 350));
  assert.equal(row.primaryPreyAxisDistance, 450);
  assert.ok(row.primaryPreyBoxDistance >= CROSSING.flightBoxRadius);
  assert.equal(row.primaryGeometryReady, false, 'right max-axis distance but off the diagonal');
});

test('diagonal waypoints fly straight only while the segment avoids the prey square', () => {
  const p = prey(7, 100, 1, 22000, 21600);
  assert.deepEqual(safePreyWaypoint({ x: 22800, y: 22400 }, p, { x: 23850, y: 23450 }),
    { x: 23850, y: 23450 });
  assert.deepEqual(safePreyWaypoint({ x: 21500, y: 22200 }, p, { x: 22450, y: 22050 }),
    { x: 22450, y: 22050 }, 'a segment along the square edge stays outside');
  assert.deepEqual(safePreyWaypoint({ x: 21400, y: 21500 }, p, { x: 22450, y: 22050 }),
    { x: 21400, y: 21150 }, 'a segment through the square takes the axis detour');
  const state = new PairPreyState('crossing', 'diagonal-positive');
  state.update(100, [pose(1, 22450, 22050, [p]), pose(2, 22500, 22100)], worlds);
  const { bot } = crossingSample(state, 1, 22500, 22100, 100);
  const intent = followSteering(bot, state, 100, bot.world.snakes.get(2), { a: 0 });
  assert.deepEqual(intent.targetPoint, { x: nearX, y: nearY });
  assert.equal(intent.intendedHeading, Math.PI / 4);
  assert.equal(intent.navigationSource, 'primary');
});

test('fixed cardinal directions reuse cardinal geometry and log the frozen direction', () => {
  const p = prey(7, 100, 1, 22000, 21600);
  const y = new PairPreyState('crossing', 'y');
  y.update(100, [pose(1, 22000, 22050, [p]), pose(2, 22000, 22100)], worlds);
  assert.equal(y.crossing.axis, 'y');
  assert.equal(y.crossing.sign, 1);
  assert.deepEqual(crossingOffset(p, y.crossing, 1), { x: 22000, y: nearY });
  const { row } = crossingSample(y, 1, 22000, 22100, 150);
  assert.equal(row.crossingDirection, 'y');
  assert.equal(row.crossingAxis, 'y');
  assert.equal(row.primaryPreyAxisDistance, 450);
  // Adaptive selection uses the tangent y axis for this prey, so it rejects x-side poses.
  const poses = [pose(1, 22450, 21600, [p]), pose(2, 22500, 21600)];
  const x = new PairPreyState('crossing', 'x');
  x.update(100, poses, worlds);
  assert.equal(x.crossing.axis, 'x');
  assert.deepEqual(crossingOffset(p, x.crossing, 1), { x: nearX, y: 21600 });
  const adaptive = new PairPreyState('crossing');
  adaptive.update(100, poses, worlds);
  assert.equal(adaptive.target, null);
  adaptive.update(200, [pose(1, 22000, 22050, [prey(7, 200, 1, 22000, 21600)]),
    pose(2, 22000, 22100)], worlds);
  assert.equal(adaptive.crossing.axis, 'y');
  assert.equal(adaptive.crossing.direction, null);
  assert.equal('crossingDirection' in crossingSample(adaptive, 1, 22000, 22100, 250).row, false);
});

test('direction is a crossing-only, validated CLI option', async () => {
  assert.deepEqual(parseArgs(['--out', 'fresh', '--server', '127.0.0.1:444',
    '--mode', 'crossing', '--direction', 'diagonal-negative']),
  { out: 'fresh', server: '127.0.0.1:444', seconds: 480, mode: 'crossing',
    direction: 'diagonal-negative' });
  assert.equal('direction' in parseArgs(['--out', 'fresh', '--server', '127.0.0.1:444',
    '--mode', 'crossing']), false);
  assert.throws(() => parseArgs(['--out', 'fresh', '--server', '127.0.0.1:444',
    '--direction', 'x']), /requires --mode crossing/);
  for (const direction of ['diagonal', 'X', 'toString'])
    assert.throws(() => parseArgs(['--out', 'fresh', '--server', '127.0.0.1:444',
      '--mode', 'crossing', '--direction', direction]), /x\|y\|diagonal-positive\|diagonal-negative/);
  assert.throws(() => parseArgs(['--mode', 'crossing', '--direction', 'x', '--direction', 'y']),
    /repeated/);
  assert.throws(() => new PairPreyState('overlap', 'x'), /requires --mode crossing/);
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:444', seconds: 1,
    direction: 'y' }), /requires --mode crossing/);
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:444', seconds: 1,
    mode: 'crossing', direction: '' }), /diagonal-negative/);
});

test('capture headers and provenance freeze direction and identify automated clients', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-paired-prey-direction-'));
  const originalConnect = Bot.prototype.connect;
  let controller;
  Bot.prototype.connect = function () { this.joins = 1; controller.abort('test'); };
  try {
    const headers = async (name, options) => {
      controller = new AbortController();
      const out = path.join(directory, name);
      await recordPair({ out, server: '127.0.0.1:444', seconds: 60, ...options }, controller.signal);
      return [0, 1].map(index => JSON.parse(zlib.gunzipSync(fs.readFileSync(
        path.join(out, `bot${index}.jsonl.gz`))).toString('utf8').split('\n')[0]));
    };
    const fixed = await headers('fixed', { mode: 'crossing', direction: 'diagonal-positive' });
    assert.deepEqual(fixed.map(h => h.nick), ['(Bot) Probe prey 0', '(Bot) Probe prey 1']);
    for (const header of fixed) {
      assert.equal(header.mode, 'crossing');
      assert.equal(header.direction, 'diagonal-positive');
      assert.deepEqual(header.plan, { ...PAIR_PREY, ...CROSSING });
      assert.equal(header.captureProvenance.options.mode, 'crossing');
      assert.equal(header.captureProvenance.options.direction, 'diagonal-positive');
    }
    for (const [name, mode] of [['adaptive', 'crossing'], ['overlap', 'overlap']]) {
      for (const header of await headers(name, { mode })) {
        assert.equal('direction' in header, false);
        assert.equal('direction' in header.captureProvenance.options, false);
        assert.match(header.nick, /^\(Bot\) Probe prey [01]$/);
      }
    }
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
