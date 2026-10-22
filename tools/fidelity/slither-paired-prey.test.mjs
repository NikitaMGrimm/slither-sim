import test from 'node:test';
import assert from 'node:assert/strict';
import { World } from './slither-headless.mjs';
import { CROSSING, PairPreyState, PairPreyBot, snapshot, preyOffset,
  crossingOffset, safePreyWaypoint, followSteering, parseArgs, recordPair } from './slither-paired-prey.mjs';

const pose = (id, x, y, prey = [], extra = {}) =>
  ({ id, life: 1, x, y, peerVisible: true, prey, ...extra });
const prey = (id, t = 100, generation = 1, x = 21600, y = 21600) =>
  ({ id, t, generation, x, y, sp: 6.06 });
const worlds = [new World(), new World()];

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
    { x: 22000, y: 22400 });
  sample(200, 22000, 22200); // 600 on the selected axis is short of near800.
  assert.equal(state.crossing.leg, 'near');
  sample(300, 22000, 22400);
  assert.equal(state.crossing.leg, 'far');
  assert.equal(state.crossing.completedLegs, 1);
  sample(400, 22000, 23100);
  assert.equal(state.crossing.leg, 'far');
  sample(500, 22000, 23450);
  assert.equal(state.crossing.leg, 'near');
  assert.equal(state.crossing.completedLegs, 2);
  assert.equal(state.crossing.axis, 'y');
  assert.equal(CROSSING.maxPairSeparation, 2400);
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
  state.update(200, [a(200), pose(2, 22000, 22400, [])], worlds);
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
  assert.equal(row.intendedOffset, 800);
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
