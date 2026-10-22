import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';
import { World } from './slither-headless.mjs';
import { PairState, PairBot, bodyGeometry, nearbyCollisionBody, pairSnapshot, recordPair, parseArgs } from './slither-paired-death.mjs';

const pose = (extra = {}) => ({ life: 1, id: 1, segments: 20, fam: .2,
  speed: 6, heading: 0, x: 1000, y: 1000, peerVisible: true,
  geometry: { medianSpacing: 8, extentFromHead: 100 },
  collisionBody: { snakeId: 2, x: 1100, y: 1000, distance: 100 }, ...extra });

test('compact contact requires measured contraction, stable mass and a local body', () => {
  const state = new PairState(20), observer = pose({ id: 2, segments: 10, x: 1350 });
  state.update(0, pose({ segments: 19 }), observer);
  assert.equal(state.phase, 'forage');
  state.update(100, pose(), observer);
  assert.equal(state.phase, 'prepare');
  assert.equal(state.shape, 'compact');
  state.update(11000, pose({ fam: .3 }), observer);
  state.update(12200, pose({ fam: .3 }), observer);
  state.update(13000, pose({ fam: .3, geometry: { medianSpacing: 22 } }), observer);
  state.update(14000, pose({ fam: .3, collisionBody: null }), observer);
  assert.equal(state.phase, 'prepare');
  state.update(16100, pose({ fam: .3 }), observer);
  assert.equal(state.phase, 'contact');
});

test('far clients rendezvous at fixed perpendicular targets before preparation', () => {
  const state = new PairState(20);
  const first = pose({ x: 1000, y: 2000 }), second = pose({ id: 2, x: 7000, y: 2000 });
  state.update(100, first, second);
  assert.equal(state.phase, 'rendezvous');
  assert.deepEqual(state.meetingTargets, [{ x: 4000, y: 1850 }, { x: 4000, y: 2150 }]);
  state.update(30100, pose({ x: 2000, y: 2000 }), second);
  assert.equal(state.phase, 'rendezvous', 'preparation deadline does not run during the meeting');
  assert.equal(state.headDistance, 5000);
  assert.deepEqual(state.meetingTargets, [{ x: 4000, y: 1850 }, { x: 4000, y: 2150 }]);
  assert.equal(state.victimIndex, 0);
  const rotated = new PairState(20);
  rotated.update(100, pose({ x: 2000, y: 1000 }), pose({ id: 2, x: 2000, y: 7000 }));
  assert.deepEqual(rotated.meetingTargets, [{ x: 2150, y: 4000 }, { x: 1850, y: 4000 }]);
});

test('meeting requires mutual visibility and resets preparation and mass clocks', () => {
  const state = new PairState(20);
  state.update(100, pose(), pose({ id: 2, x: 6000 }));
  state.update(20100, pose({ x: 3000 }), pose({ id: 2, x: 3300, peerVisible: false }));
  assert.equal(state.phase, 'rendezvous');
  state.update(25100, pose({ x: 3000, peerVisible: false }), pose({ id: 2, x: 3300 }));
  assert.equal(state.phase, 'rendezvous');
  state.update(30100, pose({ x: 3000 }), pose({ id: 2, x: 3401 }));
  assert.equal(state.phase, 'rendezvous');
  state.update(40100, pose({ x: 3000 }), pose({ id: 2, x: 3400 }));
  assert.equal(state.phase, 'prepare');
  assert.equal(state.started, 40100);
  assert.equal(state.lastGrowth, 40100, 'stable meeting mass earns no preparation credit');
  state.update(52099, pose({ x: 3000 }), pose({ id: 2, x: 3300 }));
  assert.equal(state.phase, 'prepare');
  state.update(52100, pose({ x: 3000 }), pose({ id: 2, x: 3300 }));
  assert.equal(state.phase, 'contact');
});

test('rendezvous timeout and reconnect are bounded and reset targets', () => {
  const state = new PairState(20), second = pose({ id: 2, x: 6000 });
  state.update(100, pose(), second);
  state.update(90099, pose(), second);
  assert.equal(state.phase, 'rendezvous');
  state.update(90100, pose(), second);
  assert.equal(state.phase, 'abandon');
  assert.equal(state.reason, 'rendezvous_timeout');
  state.update(90200, pose(), null);
  assert.equal(state.phase, 'forage');
  assert.equal(state.meetingTargets, null);
  assert.equal(state.headDistance, null);
  state.update(90300, pose(), pose({ id: 3, life: 2, x: 9000 }));
  assert.equal(state.phase, 'rendezvous');
  assert.equal(state.trial, 2);
  assert.equal(state.started, 90300);
  assert.equal(state.shape, 'stretched');
  assert.equal(state.meetingTargets[0].x, 5000);
  state.update(90400, pose(), pose({ id: 4, life: 3, x: 7000 }));
  assert.equal(state.phase, 'rendezvous');
  assert.equal(state.trial, 3, 'reconnect during the meeting starts a new trial');
  assert.equal(state.started, 90400);
  assert.equal(state.meetingTargets[0].x, 4000);
});

test('observer reconnect resets preparation credit; next attempt uses stretched gate', () => {
  const state = new PairState(20), victim = pose(), observer = pose({ id: 2, segments: 10, x: 1350 });
  state.update(0, victim, observer);
  state.update(6000, victim, null);
  assert.equal(state.phase, 'forage');
  state.update(13000, victim, pose({ life: 2, id: 2, segments: 10 }));
  assert.equal(state.phase, 'prepare');
  assert.equal(state.shape, 'stretched');
  assert.equal(state.started, 13000);
  assert.equal(state.lastGrowth, 13000);
  state.update(26000, victim, pose({ life: 2, id: 2, segments: 10 }));
  assert.equal(state.phase, 'prepare');
  state.update(27000, pose({ geometry: { medianSpacing: 23 } }), pose({ life: 2, id: 2, segments: 10 }));
  assert.equal(state.phase, 'contact');
  state.update(28000, null, observer);
  assert.equal(state.phase, 'forage');
});

test('first eligible client is pinned through growth and loss until its life ends', () => {
  const state = new PairState();
  assert.equal(state.minSegments, 30);
  state.update(0, pose({ segments: 25 }), pose({ id: 2, segments: 29 }));
  assert.equal(state.role(0), 'forager');
  assert.equal(state.role(1), 'forager');
  state.update(100, pose({ segments: 25 }), pose({ id: 2, segments: 30 }));
  assert.equal(state.role(1), 'victim');
  assert.equal(state.role(0), 'observer');
  assert.equal(state.victimIdentity, '1/1/2');
  assert.equal(state.observerIdentity, '0/1/1');
  state.update(1000, pose({ segments: 50 }), pose({ id: 2, segments: 29 }));
  assert.equal(state.victimIndex, 1, 'growth or shrinking cannot exchange roles');
  assert.equal(state.trial, 1);
  state.update(2000, pose({ segments: 50 }), pose({ id: 2, life: 2, segments: 2 }));
  assert.equal(state.victimIndex, 0, 'same snake ID in a new connection is a new life');
  assert.equal(state.trial, 2);
  assert.equal(state.shape, 'stretched');
  assert.equal(state.lastGrowth, 2000);
  state.update(3000, null, null);
  assert.equal(state.trial, null);
  assert.equal(state.victimIdentity, null);
});

test('same-sample threshold ties use bot0 and missing observer cannot start a trial', () => {
  const state = new PairState();
  state.update(0, null, pose({ segments: 35 }));
  assert.equal(state.phase, 'forage');
  state.update(100, pose({ segments: 30 }), pose({ segments: 40 }));
  assert.equal(state.victimIndex, 0);
  assert.notEqual(state.victimIdentity, state.observerIdentity,
    'different clients remain distinct even when life and snake IDs match');
});

test('bot1 victim steers, records dynamic identities and closes once per abandoned trial', () => {
  const state = new PairState(), events = [[], []];
  const bots = [0, 1].map(index => new PairBot({}, index, { meta: row => events[index].push(row) }));
  const snakes = [0, 1].map(index => ({ id: index + 1, sct: 30, fam: .2, ang: .2,
    x: 21000 + index * 2000, y: 21000, pts: [{ x: 21000 + index * 2000, y: 21000 }] }));
  for (const bot of bots) {
    bot.peers = bots; bot.pair = state; bot.lives = 1;
    bot.world.snakes = new Map(snakes.map(snake => [snake.id, snake]));
    bot.world.self = snakes[bot.index].id;
    bot.world.head = snake => ({ x: snake.x, y: snake.y });
  }
  state.update(0, pose({ segments: 10 }), pose({ id: 2, segments: 30 }));
  const choice = { a: 0 };
  bots[1].movement(100, snakes[1], choice);
  assert.equal(choice.a, 1.55);
  assert.equal(events[1][0].role, 'victim');
  assert.equal(events[1][0].life, 1);
  assert.equal(events[1][0].victimIdentity, '1/1/2');
  assert.equal(events[1][0].pairedIdentities, '1/1/2:0/1/1');
  bots[0].movement(100, snakes[0], { a: 0 });
  assert.equal(events[0][0].role, 'observer');
  state.reset();
  bots[1].movement(125, snakes[1], choice);
  assert.equal(events[1].at(-1).role, 'forager', 'reset is recorded before the 200ms sample interval');
  assert.equal(events[1].at(-1).phase, 'forage');
  state.update(150, pose({ segments: 25 }), pose({ id: 2, segments: 30 }));
  let closes = 0;
  bots[1].ws = { close() { closes++; } };
  state.phase = 'abandon';
  bots[1].movement(200, snakes[1], choice);
  bots[1].movement(250, snakes[1], choice);
  assert.equal(closes, 1);
  state.trial++;
  bots[1].movement(300, snakes[1], choice);
  assert.equal(closes, 2, 'new trial in same victim life can be abandoned again');
});

function meetingBots() {
  const state = new PairState(20), events = [[], []];
  const bots = [0, 1].map(index => new PairBot({}, index, { meta: row => events[index].push(row) }));
  const snakes = [0, 1].map(index => ({ id: index + 1, sct: 30, fam: .2, ang: .2,
    x: 20000 + index * 2000, y: 21000, pts: [{ x: 20000 + index * 2000, y: 21000 }] }));
  for (const bot of bots) {
    bot.peers = bots; bot.pair = state; bot.lives = 1; bot.ws = { readyState: 1 };
    bot.world.snakes = new Map(snakes.map(snake => [snake.id, snake]));
    bot.world.self = snakes[bot.index].id;
    bot.world.head = snake => ({ x: snake.x, y: snake.y });
  }
  state.update(0, pairSnapshot(bots[0], bots[1], 0), pairSnapshot(bots[1], bots[0], 0));
  return { state, events, bots, snakes };
}

test('both rendezvous roles steer symmetrically, circle near targets and sample transitions', () => {
  const { state, events, bots, snakes } = meetingBots();
  const choices = [{ a: 0 }, { a: 0 }];
  bots.forEach((bot, index) => bot.movement(100, snakes[index], choices[index]));
  assert.ok(Math.abs(choices[0].a - Math.atan2(-150, 1000)) < 1e-12);
  assert.ok(Math.abs(choices[1].a - Math.atan2(150, -1000)) < 1e-12);
  for (const [index, bot] of bots.entries()) {
    assert.equal(bot.movementIntent.desiredBoost, false);
    assert.deepEqual(events[index][0].meetingTargets, state.meetingTargets);
    assert.equal(events[index][0].headDistance, 2000);
    assert.equal(events[index][0].peerVisible, true);
    Object.assign(snakes[index], state.meetingTargets[index]);
    snakes[index].pts = [{ ...state.meetingTargets[index] }];
    bot.movement(110, snakes[index], choices[index]);
    assert.equal(bot.movementIntent.intendedHeading, 1.55, 'circle within 100 of assigned target');
  }
  state.update(125, pairSnapshot(bots[0], bots[1], 125), pairSnapshot(bots[1], bots[0], 125));
  bots.forEach((bot, index) => bot.movement(125, snakes[index], choices[index]));
  assert.equal(state.phase, 'prepare');
  assert.equal(events[0].at(-1).phase, 'prepare');
  assert.equal(events[1].at(-1).phase, 'prepare');
  assert.equal(events[0].at(-1).phaseAgeMs, 0);
});

test('rendezvous retains peer body and rim avoidance for both roles', () => {
  for (const index of [0, 1]) {
    const { bots, snakes } = meetingBots();
    const heading = Math.atan2(index ? 150 : -150, index ? -1000 : 1000);
    // A peer body 250 units ahead would be excluded by the compact-prep rule.
    snakes[1 - index].pts = [{ x: snakes[index].x + 250 * Math.cos(heading),
      y: snakes[index].y + 250 * Math.sin(heading) }];
    const choice = { a: .7 };
    bots[index].movement(100, snakes[index], choice);
    assert.equal(bots[index].movementIntent.interruption, 'avoidance');
    assert.equal(choice.a, .7);
    snakes[1 - index].pts = [];
    bots[index].world.radius = 300;
    bots[index].movement(125, snakes[index], choice);
    assert.equal(bots[index].movementIntent.interruption, 'avoidance');
    assert.equal(choice.a, .7);
  }
});

test('peer visibility comes from the local body cache and a live peer connection', () => {
  const { bots, snakes } = meetingBots();
  assert.equal(pairSnapshot(bots[0], bots[1], 0).peerVisible, true);
  bots[0].world.snakes.delete(snakes[1].id);
  assert.equal(pairSnapshot(bots[0], bots[1], 0).peerVisible, false);
  bots[0].world.snakes.set(snakes[1].id, { ...snakes[1], pts: [] });
  assert.equal(pairSnapshot(bots[0], bots[1], 0).peerVisible, false);
  bots[0].world.snakes.set(snakes[1].id, snakes[1]);
  bots[1].world.dead = true;
  assert.equal(pairSnapshot(bots[0], bots[1], 0).peerVisible, false);
  assert.equal(pairSnapshot(bots[1], bots[0], 0), null);
  bots[1].world.dead = false;
  bots[1].ws.readyState = 3;
  assert.equal(pairSnapshot(bots[0], bots[1], 0).peerVisible, false);
  assert.equal(pairSnapshot(bots[1], bots[0], 0), null);
});

test('unreachable compact shape and stalled contact abandon at bounded times', () => {
  const state = new PairState(20), observer = pose({ id: 2 });
  state.update(0, pose({ geometry: { medianSpacing: 30 } }), observer);
  state.update(29999, pose({ geometry: { medianSpacing: 30 } }), observer);
  assert.equal(state.phase, 'prepare');
  state.update(30000, pose({ geometry: { medianSpacing: 30 } }), observer);
  assert.equal(state.phase, 'abandon');
  assert.equal(state.reason, 'shape_or_body_unavailable');
  state.update(31000, pose({ life: 2, geometry: { medianSpacing: 25 } }), observer);
  assert.equal(state.shape, 'stretched');
  state.update(43000, pose({ life: 2, geometry: { medianSpacing: 25 } }), observer);
  assert.equal(state.phase, 'contact');
  state.update(48000, pose({ life: 2, geometry: { medianSpacing: 25 } }), observer);
  assert.equal(state.phase, 'abandon');
  assert.equal(state.reason, 'contact_timeout');
});

test('body geometry and local collision target use actual points', () => {
  const points = [0, 10, 20, 30].map(x => ({ x, y: 0 }));
  assert.deepEqual(bodyGeometry(points, { x: 40, y: 0 }),
    { medianSpacing: 10, extentFromHead: 40 });
  const world = { snakes: new Map([[1, { id: 1, pts: points }],
    [2, { id: 2, pts: [{ x: 280, y: 0 }, { x: 130, y: 0 }] }]]) };
  assert.equal(nearbyCollisionBody(world, { id: 1 }, { x: 40, y: 0 })?.distance, 90);
  assert.equal(nearbyCollisionBody(world, { id: 1 }, { x: 40, y: 0 }, 80), null);
});

test('opt-in server body replay matches independent fresh snapshot fixtures', () => {
  const fixture = JSON.parse(fs.readFileSync(new URL('../../tests/fixtures/server-body-snapshots.json', import.meta.url)));
  for (const entry of fixture.cases) {
    const world = new World({ serverBody: true });
    world.packet(Buffer.from(entry.initial_packet_hex, 'hex'), 0);
    for (const step of entry.steps) world.packet(Buffer.from(step.packet_hex, 'hex'), 0);
    const points = world.snakes.get(entry.snake_id)?.pts;
    assert.equal(points?.length, entry.expected_body.length);
    const error = Math.max(...points.map((point, index) =>
      Math.hypot(point.x - entry.expected_body[index][0],
        point.y - entry.expected_body[index][1])));
    assert.ok(error <= 1.25, `snake ${entry.snake_id}: ${error}`);
  }
});

test('runner rejects unbounded durations before opening any connection', async () => {
  await assert.rejects(recordPair({ out: 'unused', server: '127.0.0.1:1', seconds: 481 }),
    /1\.\.480/);
});

test('duration typos and missing or repeated options cannot silently start a default run', () => {
  for (const args of [['--second', '1'], ['--seconds'],
    ['--seconds', '--out', 'unused'], ['--seconds', '1', '--seconds', '600']]) {
    assert.throws(() => parseArgs(args), /invalid or repeated/);
  }
  assert.deepEqual(parseArgs(['--out', 'unused', '--server', '127.0.0.1:1', '--seconds', '1']),
    { out: 'unused', server: '127.0.0.1:1', seconds: 1, minSegments: 30 });
  assert.equal(parseArgs(['--min-segments', '20']).minSegments, 20);
});

const archiveEvents = file => zlib.gunzipSync(fs.readFileSync(file)).toString('utf8')
  .trim().split('\n').map(line => JSON.parse(line)).filter(row => row.type === 'meta');

test('synchronous connection setup failure closes both archives with failure markers', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-paired-setup-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const oldWebSocket = globalThis.WebSocket;
  globalThis.WebSocket = class { constructor() { throw Error('synthetic connection failure'); } };
  try {
    await assert.rejects(recordPair({ out: path.join(directory, 'capture'),
      server: '127.0.0.1:444', seconds: 1 }), /synthetic connection failure/);
  } finally { globalThis.WebSocket = oldWebSocket; }
  for (const index of [0, 1]) {
    const events = archiveEvents(path.join(directory, 'capture', `bot${index}.jsonl.gz`));
    assert.equal(events.filter(row => row.event === 'capture-interrupted').length, 1);
    assert.equal(events.filter(row => row.event === 'capture-end').length, 0);
    assert.equal(events.find(row => row.event === 'capture-interrupted').joins, 0);
  }
});

test('exhausted zero-join clients end early and report failed archives', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-paired-exhausted-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const oldWebSocket = globalThis.WebSocket, oldSetTimeout = globalThis.setTimeout;
  globalThis.WebSocket = class {
    constructor() {
      this.readyState = 0;
      queueMicrotask(() => this.onclose?.({ code: 1006, reason: 'synthetic failure' }));
    }
    close() {}
  };
  // Shorten only the inherited reconnect backoff; keep the five-second run
  // deadline intact so this test proves the runner exits on exhaustion.
  globalThis.setTimeout = (callback, delay, ...args) =>
    oldSetTimeout(callback, delay > 5000 ? 0 : delay, ...args);
  const started = performance.now();
  try {
    await assert.rejects(recordPair({ out: path.join(directory, 'capture'),
      server: '127.0.0.1:444', seconds: 5 }), /exhausted reconnect attempts/);
  } finally {
    globalThis.WebSocket = oldWebSocket;
    globalThis.setTimeout = oldSetTimeout;
  }
  assert.ok(performance.now() - started < 4000, 'should not wait for the run deadline');
  for (const index of [0, 1]) {
    const events = archiveEvents(path.join(directory, 'capture', `bot${index}.jsonl.gz`));
    assert.equal(events.filter(row => row.event === 'capture-interrupted').length, 1);
    assert.equal(events.filter(row => row.event === 'capture-end').length, 0);
    assert.equal(events.find(row => row.event === 'capture-interrupted').joins, 0);
  }
});
