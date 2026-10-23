import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { FOOD_LANES, FOOD_LANES_SOURCE, FoodLaneBot, foodLaneGeometry, foodLaneSiteQuiet,
  parseArgs, recordFoodLanes } from './slither-food-lanes.mjs';

function fixture() {
  const events = [], sent = [];
  const bot = new FoodLaneBot({ scenario: 'food-lanes' }, 0,
    { meta: event => events.push(structuredClone(event)), frame() {} }, { now: 0 });
  bot.ws = { readyState: 1, send: bytes => sent.push(Array.from(bytes)),
    close() { this.readyState = 3; } };
  bot.world.self = 1;
  const me = { id: 1, pts: [{ x: 21600, y: 21600 }], ang: 0, wang: 0,
    sp: 5.79, sct: 7, fam: 0.2, t: 100 };
  bot.world.snakes.set(1, me);
  bot.lastReceivedAt = 100;
  // These explicit received-pose fixtures test individual gates. The complete
  // route test below instead integrates the actual emitted quantized commands.
  const pose = (now, point = me.pts.at(-1), age = 0) => {
    me.pts[0] = { ...point }; me.t = now - age; bot.lastReceivedAt = now;
  };
  return { bot, me, events, sent, pose };
}

function temporaryDirectory(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-food-lanes-'));
  t.after(() => {
    assert.equal(path.dirname(directory), path.resolve(os.tmpdir()));
    assert(path.basename(directory).startsWith('slither-food-lanes-'));
    fs.rmSync(directory, { recursive: true, force: true });
  });
  return directory;
}

function replaceSocket(t, Socket) {
  const original = globalThis.WebSocket;
  globalThis.WebSocket = Socket;
  t.after(() => { globalThis.WebSocket = original; });
}

function captureRows(directory) {
  return zlib.gunzipSync(fs.readFileSync(path.join(directory, 'bot0.jsonl.gz')))
    .toString().trimEnd().split('\n').map(line => JSON.parse(line));
}

test('fixed lane geometry changes quarter phase while preserving the full-sector row', () => {
  assert(Object.isFrozen(FOOD_LANES));
  assert(Object.isFrozen(FOOD_LANES.slots));
  assert(FOOD_LANES.slots.every(Object.isFrozen));
  const geometry = foodLaneGeometry({ x: 21601, y: 21902 });
  assert.deepEqual(geometry, { cx: 21750, yA: 21937.5, yB: 22012.5,
    A: { west: { x: 21100, y: 21937.5 }, east: { x: 22400, y: 21937.5 } },
    B: { west: { x: 21100, y: 22012.5 }, east: { x: 22400, y: 22012.5 } } });
  for (const sign of [-1, 1]) {
    const leadY = 100 * Math.sin(sign > 0 ? 0 : Math.PI);
    assert.equal(Math.floor((geometry.yA + leadY) / 300), Math.floor((geometry.yB + leadY) / 300));
    assert.equal(Math.floor((geometry.yB + leadY) / 75) - Math.floor((geometry.yA + leadY) / 75), 1);
  }
  assert.equal(geometry.A.east.x - geometry.A.west.x, 1300);
});

test('point robot completes eight commanded passes with fixed split and excluded approaches/transfers', () => {
  const { bot, me, events, sent, pose } = fixture();
  for (let now = 100; now < 240000 && bot.routeStatus === 'active'; now += 25) {
    pose(now);
    bot.tick(now);
    if (bot.routeStatus !== 'active') break;
    const delivered = sent.findLast(bytes => bytes.length === 1 && bytes[0] <= 250);
    assert(delivered, 'the robot needs an actually delivered heading command');
    const target = delivered[0] * Math.PI * 2 / 251;
    const error = Math.atan2(Math.sin(target - me.ang), Math.cos(target - me.ang));
    me.ang += Math.max(-0.06, Math.min(0.06, error));
    me.wang = target;
    const distance = me.sp / 32 * 25;
    me.pts[0].x += Math.cos(me.ang) * distance;
    me.pts[0].y += Math.sin(me.ang) * distance;
  }
  assert.equal(bot.routeStatus, 'completed', JSON.stringify(bot.summary()));
  assert.equal(bot.summary().completedPasses, 8);
  const ends = events.filter(event => event.event === 'food-lane-phase-end');
  assert.deepEqual(ends.map(event => event.kind),
    ['approach', 'pass', 'pass', 'transfer', 'pass', 'pass', 'pass', 'pass', 'transfer', 'pass', 'pass']);
  const passes = ends.filter(event => event.kind === 'pass');
  assert.deepEqual(passes.map(event => event.scheduledSlot), [1, 2, 3, 4, 5, 6, 7, 8]);
  assert.deepEqual(passes.map(event => event.lane), ['A', 'A', 'B', 'B', 'B', 'B', 'A', 'A']);
  assert.deepEqual(passes.map(event => event.direction),
    ['positive', 'negative', 'positive', 'negative', 'positive', 'negative', 'positive', 'negative']);
  assert.deepEqual(passes.map(event => event.split),
    ['calibration', 'calibration', 'calibration', 'calibration', 'holdout', 'holdout', 'holdout', 'holdout']);
  for (const phase of ends) {
    assert.equal(phase.completed, true);
    assert(phase.endpointDistance <= 55);
    assert(phase.achievedEnd.anchorAgeMs <= 150);
    if (phase.kind !== 'pass') {
      assert.equal(phase.scheduledSlot, null);
      assert.equal(phase.split, 'excluded');
    } else {
      assert(Math.hypot(phase.achievedStart.anchor.x - phase.expectedStart.x,
        phase.achievedStart.anchor.y - phase.expectedStart.y) <= 55);
    }
  }
  // A route witness retains measured start/end rather than assigned waypoints.
  assert(passes.some(event => event.achievedEnd.anchor.x !== event.target.x));
  assert(sent.some(bytes => bytes[0] === 254));
  assert(!sent.some(bytes => bytes[0] === 253));
  assert.equal(bot.ws, null);
  assert.equal(events.filter(event => event.event === 'food-lane-route-end').length, 1);
});

test('only fresh received anchors advance endpoints, never an extrapolated head', () => {
  const { bot, me, pose } = fixture();
  bot.tick(100);
  const west = bot.geometry.A.west;
  pose(300, west, 151);
  bot.tick(300);
  assert.equal(bot.lanePhase.kind, 'approach');
  pose(400, { x: west.x - 60, y: west.y }, 100);
  me.ang = 0; me.sp = 20;
  bot.tick(400);
  assert.equal(bot.lanePhase.kind, 'approach');
  assert(Math.hypot(bot.world.head(me, 400).x - west.x, bot.world.head(me, 400).y - west.y) <= 55);
  pose(500, west, 150);
  bot.tick(500);
  assert.equal(bot.lanePhase.kind, 'pass');
  assert.equal(bot.lanePhase.scheduledSlot, 1);
  assert.equal(bot.summary().completedPasses, 0);
});

test('control metadata describes actual rate-limited sends and received physical heading', () => {
  const { bot, me, events, sent, pose } = fixture();
  me.ang = 0.3;
  bot.tick(100);
  let control = events.findLast(event => event.event === 'food-lane-control');
  assert.equal(control.ang, 0.3);
  assert.equal(control.physicalHeading, 0.3);
  assert.equal(control.anchorAgeMs, 0);
  assert.equal(control.desiredBoost, false);
  assert.equal(control.heldBoost, false);
  assert.equal(control.explicitBoostOff, true);
  assert(control.sentControls.some(packet => packet.bytes[0] === 254));
  assert(control.sentControls.some(packet => packet.bytes[0] === bot.lastHeading));
  assert.equal(control.headingSent, true);
  const count = sent.length;
  pose(110);
  bot.tick(110);
  control = events.findLast(event => event.event === 'food-lane-control');
  assert.equal(sent.length, count);
  assert.deepEqual(control.sentControls, []);
  assert.equal(control.headingSent, false);
  assert.equal(control.sentHeading, bot.lastHeading);
  assert.notEqual(control.intendedHeading, null);
});

test('first eligible site is selected once; quiet/rim gates and acquisition deadline are bounded', () => {
  const { bot, me, pose } = fixture();
  bot.world.snakes.set(2, { ...me, id: 2, pts: [{ x: 21630, y: 21600 }] });
  bot.tick(100);
  assert.equal(bot.geometry, null);
  bot.world.snakes.delete(2);
  bot.world.radius = 1000;
  pose(200); bot.tick(200);
  assert.equal(bot.geometry, null);
  bot.world.radius = 21000;
  pose(300); bot.tick(300);
  const geometry = structuredClone(bot.geometry);
  pose(400, { x: 41000, y: 41000 }); bot.tick(400);
  assert.equal(bot.routeStatus, 'aborted');
  pose(500); bot.tick(500);
  assert.deepEqual(bot.geometry, geometry);
  const missing = fixture();
  missing.bot.world.snakes.clear();
  missing.bot.tick(90000);
  assert.equal(missing.bot.routeReason, 'acquire_timeout');
  assert(missing.bot.summary().slots.every(slot => slot.status === 'missing'));
});

const acquisitionObstacles = {
  'far head with a near received body': ({ A, cx }) =>
    [{ x: cx - 2000, y: A.west.y + 2000 }, { x: cx, y: A.west.y + 200 },
      { x: A.east.x + 2000, y: A.east.y + 2000 }],
  'body segment crossing lanes with far vertices': ({ A, B, cx }) =>
    [{ x: cx, y: A.west.y - 2000 }, { x: cx, y: B.west.y + 2000 }],
  'lane middle outside all endpoint clearance discs': ({ B, cx }) =>
    [{ x: cx, y: B.west.y + 700 }],
  'approach interior outside lane and head clearance discs': ({ A }, head) => {
    const dx = A.west.x - head.x, dy = A.west.y - head.y, length = Math.hypot(dx, dy);
    return [{ x: (head.x + A.west.x) / 2 - dy / length * 799,
      y: (head.y + A.west.y) / 2 + dx / length * 799 }];
  },
  'west transfer interior outside endpoint clearance discs': ({ A, B }) =>
    [{ x: A.west.x - 799.5, y: (A.west.y + B.west.y) / 2 }],
};

for (const [name, obstacle] of Object.entries(acquisitionObstacles)) {
  test(`acquisition waits for ${name}`, () => {
    const { bot, me, events, pose } = fixture();
    const geometry = foodLaneGeometry(me.pts[0]), head = bot.world.head(me, 100);
    const pts = obstacle(geometry, head);
    const sites = [geometry.A.west, geometry.A.east, geometry.B.west, geometry.B.east, head];
    assert(sites.every(site => Math.hypot(pts.at(-1).x - site.x, pts.at(-1).y - site.y) > 800),
      'the former head-site check would accept this obstacle');
    bot.world.snakes.set(2, { ...me, id: 2, pts, sp: 0 });
    for (const now of [100, 200]) {
      pose(now); bot.tick(now);
      assert.equal(bot.geometry, null);
      assert.equal(bot.lanePhase, null);
      assert.equal(bot.routeStatus, 'active');
      const control = events.findLast(event => event.event === 'food-lane-control');
      assert.equal(control.phase, 'acquire');
      assert.equal(control.quiet, false);
      assert.equal(control.rimSafe, true);
      assert(bot.summary().slots.every(slot => slot.status === 'missing'));
    }
    assert(!events.some(event => event.event === 'food-lane-site'));
    bot.world.snakes.delete(2);
    pose(300); bot.tick(300);
    assert.deepEqual(bot.geometry, geometry);
    assert.equal(bot.lanePhase.kind, 'approach');
  });
}

test('quietness includes the current extrapolated tip and its connecting segment', () => {
  const geometry = { A: { west: { x: 0, y: 0 }, east: { x: 1300, y: 0 } },
    B: { west: { x: 0, y: 75 }, east: { x: 1300, y: 75 } } };
  const { bot, me } = fixture();
  const other = { ...me, id: 2, pts: [{ x: 650, y: -820 }], ang: Math.PI / 2, sp: 9.6, t: 0 };
  bot.world.snakes.set(2, other);
  assert.equal(foodLaneSiteQuiet(bot.world, 0, geometry, geometry.A.west), true);
  assert.equal(foodLaneSiteQuiet(bot.world, 100, geometry, geometry.A.west), false);
  const world = { self: 1, snakes: new Map([[2, other]]), head: (snake, now) => {
    assert.equal(snake, other);
    assert.equal(now, 123);
    return { x: 650, y: 2000 };
  } };
  other.pts = [{ x: 650, y: -2000 }];
  assert.equal(foodLaneSiteQuiet(world, 123, geometry, geometry.A.west), false,
    'far anchor and tip still have a connecting segment crossing the corridors');
});

test('interior intersections have zero clearance even when all endpoint projections are far', () => {
  // Longer synthetic corridors isolate intersection math from the 800-unit margin.
  const geometry = { A: { west: { x: 0, y: 0 }, east: { x: 4000, y: 0 } },
    B: { west: { x: 0, y: 75 }, east: { x: 4000, y: 75 } } };
  const { bot, me } = fixture();
  bot.world.snakes.set(2, { ...me, id: 2, sp: 0,
    pts: [{ x: 2000, y: -2000 }, { x: 2000, y: 2000 }] });
  assert.equal(foodLaneSiteQuiet(bot.world, 100, geometry, geometry.A.west), false);
});

test('quietness handles isolated points, repeated points, collinear overlap and degenerate approach', () => {
  const { bot, me } = fixture();
  const geometry = foodLaneGeometry(me.pts[0]), head = geometry.A.west;
  const cases = [
    [{ x: geometry.cx, y: geometry.yA }],
    [head, head],
    [{ x: geometry.A.west.x - 1000, y: geometry.yA },
      { x: geometry.A.east.x + 1000, y: geometry.yA }],
  ];
  for (const pts of cases) {
    bot.world.snakes.set(2, { ...me, id: 2, pts, sp: 0 });
    assert.equal(foodLaneSiteQuiet(bot.world, 100, geometry, head), false);
  }
  const point = { x: geometry.cx, y: geometry.yB + 801 };
  bot.world.snakes.set(2, { ...me, id: 2, pts: [point, point], sp: 0 });
  assert.equal(foodLaneSiteQuiet(bot.world, 100, geometry, head), true);
});

test('800-unit clearance is strict; a safe full body acquires and own body is excluded', () => {
  for (const clearance of [799.999, 800, 800.001]) {
    const { bot, me, events } = fixture();
    const geometry = foodLaneGeometry(me.pts[0]);
    const pts = [{ x: geometry.A.west.x - 1000, y: geometry.yB + clearance },
      { x: geometry.A.east.x + 1000, y: geometry.yB + clearance }];
    bot.world.snakes.set(2, { ...me, id: 2, pts, sp: 0 });
    bot.tick(100);
    if (clearance <= 800) {
      assert.equal(bot.geometry, null);
      assert.equal(events.findLast(event => event.event === 'food-lane-control').quiet, false);
    } else {
      assert.deepEqual(bot.geometry, geometry);
      assert.equal(bot.lanePhase.kind, 'approach');
    }
  }
  const { bot, me } = fixture();
  const geometry = foodLaneGeometry(me.pts[0]);
  me.pts = [geometry.A.west, geometry.B.east, me.pts[0]];
  assert.equal(foodLaneSiteQuiet(bot.world, 100, geometry, bot.world.head(me, 100)), true);
  bot.tick(100);
  assert.deepEqual(bot.geometry, geometry);
});

test('avoidance, stale anchors, gaps and phase deadlines abort without replacing missing slots', () => {
  for (const reason of ['avoidance', 'stale_anchor', 'frame_gap', 'phase_timeout']) {
    const { bot, me, pose } = fixture();
    bot.tick(100);
    pose(200, bot.geometry.A.west); bot.tick(200);
    let now = 300;
    pose(now);
    if (reason === 'avoidance') bot.world.snakes.set(2,
      { ...me, id: 2, pts: [{ x: me.pts[0].x + 50, y: me.pts[0].y }] });
    if (reason === 'stale_anchor') me.t = now - 501;
    if (reason === 'frame_gap') bot.lastReceivedAt = now - 601;
    if (reason === 'phase_timeout') { now = 30200; pose(now); }
    bot.tick(now);
    assert.equal(bot.routeReason, reason);
    assert.equal(bot.summary().slots[0].status, 'aborted');
    assert(bot.summary().slots.slice(1).every(slot => slot.status === 'missing'));
    bot.tick(now + 1000);
    assert.equal(bot.slotIndex, 0);
  }
});

test('terminal death and reset never restart or erase completed/missing pass labels', () => {
  const { bot, pose, events } = fixture();
  bot.tick(100);
  pose(200, bot.geometry.A.west); bot.tick(200);
  pose(300, bot.geometry.A.east); bot.tick(300);
  assert.equal(bot.summary().completedPasses, 1);
  bot.receive(Uint8Array.from([0, 7, 118, 0]));
  assert.equal(bot.routeReason, 'death');
  assert.equal(bot.summary().slots[0].status, 'completed');
  assert.equal(bot.summary().slots[1].status, 'aborted');
  assert(bot.summary().slots.slice(2).every(slot => slot.status === 'missing'));
  bot.connect({ ip: '127.0.0.1', port: 444 });
  bot.tick(400);
  assert.equal(events.filter(event => event.event === 'food-lane-route-end').length, 1);
  assert.equal(bot.lives, 0);
});

test('west-lane transfer has its own 16-second deadline and no scored slot', () => {
  const { bot, pose, events } = fixture();
  bot.tick(100);
  pose(200, bot.geometry.A.west); bot.tick(200);
  pose(300, bot.geometry.A.east); bot.tick(300);
  pose(400, bot.geometry.A.west); bot.tick(400);
  assert.equal(bot.lanePhase.kind, 'transfer');
  assert.equal(bot.lanePhase.scheduledSlot, null);
  pose(16400); bot.tick(16400);
  assert.equal(bot.routeReason, 'phase_timeout');
  assert.equal(bot.summary().completedPasses, 2);
  assert(bot.summary().slots.slice(2).every(slot => slot.status === 'missing'));
  assert.equal(events.filter(event => event.event === 'food-lane-phase-end').at(-1).split, 'excluded');
});

test('another setup is a terminal reset, not a new site or life', () => {
  const { bot } = fixture();
  bot.tick(100);
  bot.joins = 1;
  const setup = new Uint8Array(33);
  setup[2] = 97; setup[4] = 84; setup[5] = 96; // grid center 21600
  setup[8] = 1; setup[9] = 44; // sector size 300
  setup[25] = 19; setup[26] = 42;
  bot.receive(setup);
  assert.equal(bot.routeReason, 'unexpected_setup');
  assert.equal(bot.summary().completedPasses, 0);
  assert.equal(bot.stopped, true);
});

test('CLI requires a fixed explicit endpoint and limits the capture to one client and 480 seconds', () => {
  const base = ['--out', 'unused', '--server', '127.0.0.1:444'];
  const opts = parseArgs(base);
  assert.equal(opts.seconds, 480);
  assert.equal(opts.scenario, 'food-lanes');
  assert.equal(opts.bots, 1);
  assert.equal(parseArgs([...base, '--seconds', '12']).seconds, 12);
  for (const args of [[], ['--out', 'unused'], [...base, '--seconds', '481'],
    [...base, '--seconds', '0'], [...base, '--seconds', 'NaN'], [...base, '--seconds', 'Infinity'],
    [...base, '--bots', '2'], [...base, '--scenario', 'movement'], [...base, '--out', 'again'],
    [...base, '--origin'], [...base, '--origin', 'file:///tmp']]) assert.throws(() => parseArgs(args));
  for (const server of ['example.com:444', '127.0.0.1:0', '127.0.0.1:65536', '127.0.0.1:444:x'])
    assert.throws(() => parseArgs(['--out', 'unused', '--server', server]));
});

test('own driver and shared headless hashes are independently pinned', () => {
  const digest = name => createHash('sha256').update(fs.readFileSync(new URL(name, import.meta.url))).digest('hex');
  assert.equal(FOOD_LANES_SOURCE.collectorSha256, digest('./slither-food-lanes.mjs'));
  assert.equal(FOOD_LANES_SOURCE.headlessSha256, digest('./slither-headless.mjs'));
  assert.equal(FOOD_LANES_SOURCE.driverPath, fileURLToPath(new URL('./slither-food-lanes.mjs', import.meta.url)));
});

test('socket close finalizes a complete raw receipt with aborted route and no reconnect', async t => {
  const directory = temporaryDirectory(t), sockets = [];
  replaceSocket(t, class {
    readyState = 1;
    constructor(url, options) {
      sockets.push({ url, options, socket: this });
      queueMicrotask(() => {
        this.onopen();
        this.onmessage({ data: Uint8Array.from([0, 7, 112]).buffer });
        this.readyState = 3;
        this.onclose({ code: 1000, reason: 'synthetic-close' });
      });
    }
    send() {}
    close() { this.readyState = 3; }
  });
  const opts = parseArgs(['--out', directory, '--server', '127.0.0.1:444',
    '--seconds', '1', '--origin', 'http://localhost:8080']);
  const summary = await recordFoodLanes(opts, { ip: '127.0.0.1', port: 444 });
  const rows = captureRows(directory), header = rows[0], end = rows.at(-1);
  assert.equal(sockets.length, 1);
  assert.equal(sockets[0].url, 'ws://127.0.0.1:444/slither');
  assert.deepEqual(sockets[0].options, { headers: { Origin: 'http://localhost:8080' } });
  assert.equal(summary.reason, 'socket_close');
  assert(summary.slots.every(slot => slot.status === 'missing'));
  assert.equal(header.scenario, 'food-lanes');
  assert.equal(header.captureProvenance.options.scenario, 'food-lanes');
  assert.equal(header.captureProvenance.options.bots, 1);
  assert.equal(header.captureProvenance.options.origin, opts.origin);
  assert.equal(header.captureProvenance.collectorSha256, FOOD_LANES_SOURCE.collectorSha256);
  assert.equal(header.captureProvenance.headlessSha256, FOOD_LANES_SOURCE.headlessSha256);
  assert.deepEqual(rows.find(row => row.event === 'food-lane-plan').config, FOOD_LANES);
  assert.equal(end.event, 'capture-end');
  assert.equal(end.foodLaneRoute.status, 'aborted');
  assert.equal(end.frames, 1);
  assert.equal(end.collectorSourcesUnchanged, true);
  assert.equal(end.collectorEndSha256, FOOD_LANES_SOURCE.collectorSha256);
  assert.equal(end.headlessEndSha256, FOOD_LANES_SOURCE.headlessSha256);
  const text = zlib.gunzipSync(fs.readFileSync(path.join(directory, 'bot0.jsonl.gz'))).toString();
  const prefix = text.slice(0, text.lastIndexOf('\n', text.length - 2) + 1);
  assert.equal(end.integrity.sha256, createHash('sha256').update(prefix).digest('hex'));
  assert.equal(sockets[0].socket.onclose, null);
});

test('capture deadline retains incomplete route labels with a complete receipt', async t => {
  const directory = temporaryDirectory(t);
  let closes = 0;
  replaceSocket(t, class { readyState = 1; send() {} close() { closes++; } });
  const opts = parseArgs(['--out', directory, '--server', '127.0.0.1:444', '--seconds', '0.02']);
  await recordFoodLanes(opts, { ip: '127.0.0.1', port: 444 });
  const end = captureRows(directory).at(-1);
  assert.equal(end.event, 'capture-end');
  assert.equal(end.foodLaneRoute.reason, 'capture_deadline');
  assert.equal(end.foodLaneRoute.completedPasses, 0);
  assert.equal(closes, 1);
});

test('malformed incoming frames are preserved with interrupted capture finalization', async t => {
  const directory = temporaryDirectory(t);
  replaceSocket(t, class {
    readyState = 1;
    constructor() { queueMicrotask(() => this.onmessage({ data: Uint8Array.from([0]).buffer })); }
    send() {}
    close() { this.readyState = 3; }
  });
  const opts = parseArgs(['--out', directory, '--server', '127.0.0.1:444', '--seconds', '1']);
  await assert.rejects(recordFoodLanes(opts, { ip: '127.0.0.1', port: 444 }), /short timing frame/);
  const rows = captureRows(directory);
  assert(rows.some(row => Array.isArray(row) && row[0] === 'i' && row[2] === 'AA=='));
  assert.equal(rows.at(-1).event, 'capture-interrupted');
  assert.equal(rows.at(-1).foodLaneRoute.reason, 'capture_error');
});

test('cancellation closes the single socket and records interrupted capture', async t => {
  const directory = temporaryDirectory(t), controller = new AbortController();
  let connections = 0;
  replaceSocket(t, class {
    readyState = 1;
    constructor() { connections++; queueMicrotask(() => controller.abort('synthetic-cancel')); }
    send() {}
    close() { this.readyState = 3; }
  });
  const opts = parseArgs(['--out', directory, '--server', '127.0.0.1:444', '--seconds', '1']);
  await recordFoodLanes(opts, { ip: '127.0.0.1', port: 444 }, controller.signal);
  const end = captureRows(directory).at(-1);
  assert.equal(connections, 1);
  assert.equal(end.event, 'capture-interrupted');
  assert.equal(end.foodLaneRoute.reason, 'capture_cancelled');
});
