import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';
import { getEventListeners } from 'node:events';
import { createServer } from 'node:http';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { solveChallenge, splitFrame, loginPacket, World, Bot, Recorder, PREY_PROBE, BOUNDARY_PROBE, VISIBILITY_PROBE, SPEED_PROBE, visibilityEndpoints, DEATH_OBSERVATION_MS, chooseHeading, parseArgs, record, captureSourceIdentity, createCaptureBatch, finalizeCaptureBatch } from './slither-headless.mjs';
import { recordPair as recordDeath, parseArgs as parseDeath } from './slither-paired-death.mjs';
import { recordPair as recordPrey, parseArgs as parsePrey } from './slither-paired-prey.mjs';
import { recordFeedingShape, parseArgs as parseFeeding } from './slither-feeding-shape.mjs';

test('CLI requires an explicit valid endpoint before creating output or using the network', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-endpoint-'));
  t.after(() => {
    assert.equal(path.dirname(directory), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  });
  const preload = path.join(directory, 'offline.mjs');
  fs.writeFileSync(preload, 'globalThis.fetch = () => { throw Error("unexpected fetch"); }; globalThis.WebSocket = class { constructor() { throw Error("unexpected socket"); } };');
  for (const server of [null, '127.0.0.1:0', '127.0.0.1:65536', '127.0.0.1:444:extra', 'invalid:444']) {
    const out = path.join(directory, 'capture');
    const result = spawnSync(process.execPath, ['--import', pathToFileURL(preload).href,
      fileURLToPath(new URL('./slither-headless.mjs', import.meta.url)), '--out', out,
      ...(server ? ['--server', server] : [])], { encoding: 'utf8', timeout: 5000 });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr, /--server/);
    assert.doesNotMatch(result.stderr, /unexpected fetch|unexpected socket/);
    assert.equal(fs.existsSync(out), false);
  }
});

test('collector CLIs accept an explicit HTTP origin and reject discovery options', () => {
  for (const parse of [parseArgs, parseDeath, parsePrey, parseFeeding]) {
    assert.equal(parse(['--out', 'unused', '--origin', 'http://localhost:8080']).origin, 'http://localhost:8080');
    for (const origin of ['ws://localhost:8080', 'http://localhost/path', 'http://user@localhost', 'http://localhost?x=1', 'http://localhost\r\nX: value'])
      assert.throws(() => parse(['--out', 'unused', '--origin', origin]), /HTTP\(S\) origin/);
    assert.throws(() => parse(['--out', 'unused', '--survey', '2']));
    assert.throws(() => parse(['--out', 'unused', '--min-weight', '1']));
  }
});

test('native WebSocket sends the configured Origin to a loopback server', { timeout: 5000 }, async t => {
  const server = createServer(), sockets = new Set();
  let receive;
  server.on('upgrade', (request, socket) => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
    socket.on('error', error => {
      // The client stops once the upgrade headers have been inspected.
      if (error.code !== 'ECONNRESET') throw error;
    });
    receive(request);
    socket.end('HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n');
  });
  t.after(() => {
    for (const socket of sockets) socket.destroy();
    return new Promise(resolve => server.close(resolve));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  for (const origin of [null, 'http://localhost:8080']) {
    const incoming = new Promise(resolve => { receive = resolve; });
    const bot = new Bot({ scenario: 'forage', origin }, 0, { meta() {}, frame() {} });
    try {
      bot.connect({ ip: '127.0.0.1', port: server.address().port });
      const request = await incoming;
      assert.equal(request.url, '/slither');
      assert.equal(request.headers.origin ?? null, origin);
    } finally { bot.stop(); }
  }
});

test('all collectors send only the configured Origin and preserve it in provenance', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-origin-'));
  const original = globalThis.WebSocket;
  t.after(() => {
    globalThis.WebSocket = original;
    assert.equal(path.dirname(directory), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  });
  const runners = [record, recordDeath, recordPrey, recordFeedingShape];
  for (const [index, run] of runners.entries()) {
    for (const origin of [null, 'http://localhost:8080']) {
      const controller = new AbortController(), connections = [];
      globalThis.WebSocket = class {
        readyState = 1;
        constructor(url, options) {
          connections.push({ url, options });
          queueMicrotask(() => controller.abort('offline-test'));
        }
        send() {}
        close() { this.readyState = 3; }
      };
      const out = path.join(directory, `${index}-${origin ? 'origin' : 'default'}`);
      const options = { out, server: '127.0.0.1:444', seconds: 60, origin };
      if (run === record) {
        fs.mkdirSync(out);
        await record({ ...options, bots: 1, scenario: 'forage' },
          { ip: '127.0.0.1', port: 444 }, 'bot', 1, controller.signal);
      } else await run(options, controller.signal);
      assert(connections.length > 0);
      for (const connection of connections) {
        assert.equal(connection.url, 'ws://127.0.0.1:444/slither');
        assert.deepEqual(connection.options, origin ? { headers: { Origin: origin } } : undefined);
      }
      const header = JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(out, 'bot0.jsonl.gz')))
        .toString().split('\n')[0]);
      assert.equal(header.captureProvenance.options.origin, origin);
    }
  }
});

function speedBot(sct = 6) {
  const events = [], sent = [];
  const bot = new Bot({ scenario: 'speedprobe' }, 0, {
    meta(e) { events.push(e); }, frame() {},
  });
  bot.ws = { readyState: 1, send(bytes) { sent.push(Array.from(bytes)); }, close() {} };
  bot.world.self = 1;
  const me = { id: 1, pts: [{ x: 21600, y: 21600 }],
    ang: 0, wang: 0, sp: 5.7, sct, fam: 0.5, t: 100 };
  bot.world.snakes.set(1, me);
  bot.lastReceivedAt = 100;
  return { bot, me, events, sent };
}

test('frame splitting rejects truncated packets instead of accepting partial data',()=>{
  for(const bytes of [[],[0,7],[0,7,0],[0,7,0,0],[0,7,0,3,101],[0,7,0,1,112,35,101]]){
    assert.throws(()=>splitFrame(Uint8Array.from(bytes)),/frame|packet/);
  }
});

test('a malformed received frame interrupts acquisition while preserving raw evidence',async t=>{
  const original=globalThis.WebSocket;
  t.after(()=>{globalThis.WebSocket=original;});
  globalThis.WebSocket=class {close(){}};
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),'slither-malformed-frame-'));
  t.after(()=>fs.rmSync(directory,{recursive:true,force:true}));
  const file=path.join(directory,'bot.jsonl.gz');
  const rec=new Recorder(file,{schema:1});
  const bot=new Bot({scenario:'forage'},0,rec);
  bot.connect({ip:'1.1.1.1',port:444});
  assert.doesNotThrow(()=>bot.ws.onmessage({data:Uint8Array.from([0,7,0,3,101]).buffer}));
  const error=await rec.failure;
  assert.match(error.message,/invalid packet length/);
  await assert.rejects(finalizeCaptureBatch({bots:[bot],completed:false,error}),/invalid packet length/);
  const rows=zlib.gunzipSync(fs.readFileSync(file)).toString().trim().split('\n').map(JSON.parse);
  assert.equal(rows.at(-1).event,'capture-interrupted');
  assert.ok(rows.some(row=>Array.isArray(row)&&row[2]===Buffer.from([0,7,0,3,101]).toString('base64')));
});

test('speed probe can wait for a larger discriminating size without boosting', () => {
  const opts = parseArgs(['--out', 'unused', '--scenario', 'speedprobe', '--speed-min-segments', '24']);
  const { bot, me, events, sent } = speedBot(23);
  bot.opts.speedMinSegments = opts.speedMinSegments;
  bot.tick(100);
  assert.equal(events.at(-1).interruption, 'insufficient_mass');
  assert.equal(events.at(-1).heldBoost, false);
  assert(!sent.some(p => p[0] === 253));
  me.sct = 24;
  bot.tick(125);
  assert.equal(events.at(-1).phase, 'settle');
  for (const value of ['1', '3.5', 'NaN']) {
    assert.throws(() => parseArgs(['--out', 'unused', '--speed-min-segments', value]), /integer >=2/);
  }
});

test('speed probe forages for mass then settles, boosts and coasts on one heading', () => {
  const { bot, me, events, sent } = speedBot(5);
  const step = now => { me.t = now; bot.lastReceivedAt = now; bot.tick(now); };
  step(100);
  assert.equal(events.at(-1).interruption, 'insufficient_mass');
  me.sct = 6;
  step(200);
  assert.equal(events.at(-1).phase, 'settle');
  assert.equal(events.at(-1).desiredBoost, false);
  step(4700);
  assert.equal(events.at(-1).phase, 'boost');
  assert.equal(events.at(-1).heldBoost, true);
  assert(sent.some(p => p[0] === 253));
  step(5550);
  assert.equal(events.at(-1).phase, 'coast');
  assert.equal(events.at(-1).heldBoost, false);
  assert(sent.some(p => p[0] === 254));
  step(10050);
  assert.equal(events.at(-1).phase, 'forage_wait');
  assert(events.some(e => e.event === 'speed-trial-end' && e.completed));
  step(11550);
  assert.equal(events.at(-1).phase, 'settle');
  assert.equal(events.at(-1).trial, 2);
  assert.equal(SPEED_PROBE.coastMs, 4500);
  assert.equal(SPEED_PROBE.settleMs, 4500);
});

test('speed probe chooses inward corridor and releases boost on danger', () => {
  const { bot, me, events, sent } = speedBot();
  me.pts[0] = { x: bot.world.grd + bot.world.radius - 1000, y: bot.world.grd };
  bot.tick(100);
  assert.equal(bot.speedTrial.heading, Math.PI);
  assert.equal(events.at(-1).phase, 'settle');
  me.t = 4600; bot.lastReceivedAt = 4600;
  bot.tick(4600);
  assert.equal(bot.boost, true);
  bot.world.snakes.set(2, { id: 2, pts: [{ x: me.pts[0].x - 200, y: me.pts[0].y }],
    ang: 0, sp: 0, sct: 2, t: 4625 });
  me.t = 4625; bot.lastReceivedAt = 4625;
  bot.tick(4625);
  assert.equal(bot.speedTrial, null);
  assert.equal(bot.boost, false);
  assert(sent.some(p => p[0] === 254));
  assert(events.some(e => e.event === 'speed-trial-end' && !e.completed &&
    ['avoidance', 'occupied_corridor'].includes(e.reason)));
});

test('speed growth probe records an unboosted baseline before growing and refreshing cruise', () => {
  const { bot, me, events, sent } = speedBot(2);
  bot.opts.speedMinSegments = 2;
  bot.opts.speedGrowthSegments = 12;
  const step = now => { me.t = now; bot.lastReceivedAt = now; bot.tick(now); };
  step(100);
  assert.equal(bot.speedTrial.unboostedBaseline, true);
  step(4600);
  assert.equal(bot.speedTrial.phase, 'coast');
  assert.equal(bot.boost, false);
  step(9100);
  assert.equal(bot.speedGrowthTarget, 14);
  assert(events.some(event => event.event === 'speed-growth-target' &&
    event.baselineSegments === 2 && event.targetSegments === 14));
  me.sct = 13;
  step(10600);
  assert.equal(bot.speedTrial, null);
  assert.equal(events.at(-1).interruption, 'insufficient_mass');
  assert(!sent.some(packet => packet[0] === 253));
  me.sct = 14;
  step(10700);
  assert.equal(bot.speedTrial.unboostedBaseline, false);
  assert.equal(events.at(-1).growthTargetSegments, 14);
  step(15200);
  assert.equal(bot.speedTrial.phase, 'boost');
  assert.equal(bot.boost, true);
  step(16050);
  assert.equal(bot.speedTrial.phase, 'coast');
  assert.equal(bot.boost, false);
  step(20550);
  assert.equal(bot.speedTrial, null);
});

test('speed growth probe does not advance after interruption and resets growth on reconnect', () => {
  const { bot, me, events } = speedBot(2);
  bot.opts.speedMinSegments = 2;
  bot.opts.speedGrowthSegments = 12;
  bot.tick(100);
  bot.tick(800);
  assert.equal(bot.speedGrowthTarget, null);
  assert(events.some(event => event.event === 'speed-trial-end' && !event.completed));
  me.t = 2300; bot.lastReceivedAt = 2300;
  bot.tick(2300);
  assert.equal(bot.speedTrial.unboostedBaseline, true);
  bot.speedGrowthTarget = 14;
  const original = globalThis.WebSocket;
  globalThis.WebSocket = class { close() {} };
  try {
    bot.connect({ ip: '127.0.0.1', port: 1 });
    assert.equal(bot.speedGrowthTarget, null);
    bot.stop();
  } finally { globalThis.WebSocket = original; }
});

test('speed growth probe validates opt-in configuration', () => {
  const args = ['--out', 'unused', '--scenario', 'speedprobe'];
  assert.equal(parseArgs(args).speedGrowthSegments, 0);
  assert.equal(parseArgs([...args, '--speed-growth-segments', '12']).speedGrowthSegments, 12);
  for (const value of ['-1', '1.5', 'NaN', 'Infinity']) {
    assert.throws(() => parseArgs([...args, '--speed-growth-segments', value]), /integer >=0/);
  }
  assert.throws(() => parseArgs(['--out', 'unused', '--speed-growth-segments', '12']), /requires speedprobe/);
});

test('fixed speed headings alternate exact outgoing bytes without substituting inward trials', () => {
  const { bot, me, events, sent } = speedBot(2);
  bot.opts.speedMinSegments = 2;
  bot.opts.speedHeadingBytes = [147, 197];
  const step = now => { me.t = now; bot.lastReceivedAt = now; bot.tick(now); };
  step(100);
  assert.equal(events.at(-1).sentHeading, 147);
  assert(sent.some(packet => packet.length === 1 && packet[0] === 147));
  assert.equal(bot.speedTrial.requestedByte, 147);
  bot.endSpeedTrial(125, 'test_interrupt', me);
  step(1625);
  assert.equal(events.at(-1).sentHeading, 197);
  assert.equal(bot.speedTrial.requestedByte, 197);
  bot.endSpeedTrial(1650, 'test_interrupt', me);
  me.pts[0] = { x: bot.world.grd - bot.world.radius + 500, y: bot.world.grd };
  step(3150);
  assert.equal(bot.speedTrial, null);
  assert.equal(events.at(-1).interruption, 'rim_corridor');
  me.pts[0] = { x: bot.world.grd, y: bot.world.grd };
  step(3175);
  assert.equal(bot.speedTrial.requestedByte, 147);
});

test('fixed speed heading configuration rejects ambiguous or out-of-range values', () => {
  const args = ['--out', 'unused', '--scenario', 'speedprobe'];
  assert.deepEqual(parseArgs(args).speedHeadingBytes, []);
  assert.deepEqual(parseArgs([...args, '--speed-heading-bytes', '147,197']).speedHeadingBytes, [147, 197]);
  for (const value of ['-1', '251', '1.5', 'NaN', '1,,2', '1,', ' 1']) {
    assert.throws(() => parseArgs([...args, '--speed-heading-bytes', value]), /integers in 0\.\.250/);
  }
  assert.throws(() => parseArgs(['--out', 'unused', '--speed-heading-bytes', '147']), /requires speedprobe/);
});

test('fixed speed heading midpoint encodes every protocol command without a boundary underflow', () => {
  for (let requestedByte = 0; requestedByte <= 250; requestedByte++) {
    const { bot, events } = speedBot(2);
    bot.opts.speedMinSegments = 2;
    bot.opts.speedHeadingBytes = [requestedByte];
    bot.tick(100);
    assert.equal(events.at(-1).sentHeading, requestedByte);
  }
});

test('speed probe logs stale frames and resets trial state on reconnect', () => {
  const { bot, me, events } = speedBot();
  bot.tick(100);
  bot.lastReceivedAt = 100; me.t = 800;
  bot.tick(800);
  assert.equal(events.find(e => e.event === 'speed-trial-end').reason, 'frame_gap');
  const original = globalThis.WebSocket;
  globalThis.WebSocket = class { close() {} };
  try {
    bot.connect({ ip: '127.0.0.1', port: 1 });
    assert.equal(bot.speedTrial, null);
    assert.equal(bot.speedIntent, null);
    assert.equal(bot.speedTrials, 0);
    bot.stop();
  } finally { globalThis.WebSocket = original; }
  assert.throws(() => parseArgs(['--out', 'unused', '--scenario', 'speedprobe', '--bots', '2']), /one client/);
  assert.throws(() => parseArgs(['--out', 'unused', '--scenario', 'speedprobe', '--survey', '2']), /unknown option/);
  assert.equal(parseArgs(['--out', 'unused', '--scenario', 'forage']).scenario, 'forage');
});

function visibilityBot(sct = 2, fam = 0) {
  const events = [], sent = [];
  const bot = new Bot({ scenario: 'visibilityprobe' }, 0, {
    meta(e) { events.push(e); }, frame() {},
  });
  bot.ws = { readyState: 1, send(bytes) { sent.push(Array.from(bytes)); }, close() {} };
  bot.world.self = 1;
  const me = { id: 1, pts: [{ x: 21600, y: 21600 }],
    ang: 0, wang: 0, sp: 5.7, sct, fam, t: 100 };
  bot.world.snakes.set(1, me);
  bot.lastReceivedAt = 100;
  return { bot, me, events, sent };
}

test('visibility endpoints and repeated axis reversals lead to a diagonal transfer', () => {
  const c = { x: 21600, y: 21600 };
  const axis = visibilityEndpoints(c, 'axis');
  const diagonal = visibilityEndpoints(c, 'diagonal');
  assert.deepEqual(axis.positive, { x: 22250, y: 21600 });
  assert.deepEqual(axis.negative, { x: 20950, y: 21600 });
  assert(Math.abs(Math.hypot(diagonal.positive.x - c.x, diagonal.positive.y - c.y) - 650) < 1e-9);
  assert.equal(VISIBILITY_PROBE.halfSpan, 650);
  const { bot, me, events } = visibilityBot();
  const advance = (now, point) => {
    me.pts[0] = { ...point }; me.t = now; bot.lastReceivedAt = now;
    const choice = { a: 0, danger: 0 };
    bot.probeVisibility(now, me, choice);
    return choice;
  };
  assert.equal(advance(100, c).a, 0);
  assert.equal(bot.phase, 'axis_cruise_positive_1');
  for (const [now, p, phase] of [[200, axis.positive, 'axis_cruise_negative_2'],
    [300, axis.negative, 'axis_cruise_positive_3'],
    [400, axis.positive, 'axis_cruise_negative_4'],
    [500, axis.negative, 'transfer'],
    [600, c, 'diagonal_cruise_positive_1']]) {
    advance(now, p);
    assert.equal(bot.phase, phase);
  }
  assert.equal(events.filter(e => e.event === 'visibility-phase-end' && e.completed).length, 5);
  assert.deepEqual(events.find(e => e.phase === 'diagonal_cruise_positive_1').endpoints, diagonal);
});

test('visibility controls log server clock, estimated head, anchor age and sent inputs', () => {
  const { bot, events, sent } = visibilityBot();
  bot.serverClock = 12345;
  bot.tick(100);
  const c = events.find(e => e.event === 'visibility-control');
  assert.equal(c.phase, 'axis_cruise_positive_1');
  assert.deepEqual(c.center, { x: 21600, y: 21600 });
  assert.deepEqual(c.target, { x: 22250, y: 21600 });
  assert.equal(c.serverClock, 12345);
  assert.equal(c.anchorAgeMs, 0);
  assert.equal(c.sct, 2);
  assert.equal(c.fam, 0);
  assert.equal(c.sp, 5.7);
  assert.equal(c.headingSettled, true);
  assert.equal(c.realizedHeadingError, 0);
  assert.equal(c.desiredBoost, false);
  assert.equal(c.sentHeading, 0);
  assert.equal(c.headingSent, true);
  assert.equal(c.headSource, 'last_anchor_extrapolation');
  assert(sent.some(p => p[0] === 0));
});

test('visibility avoidance, stale anchors and timeouts censor controlled legs', () => {
  const { bot, me, events } = visibilityBot();
  const choice = { a: Math.PI / 2, danger: 0 };
  bot.probeVisibility(100, me, choice);
  bot.world.snakes.set(2, { id: 2, pts: [{ x: 21800, y: 21600 }],
    ang: 0, sp: 0, sct: 2, t: 100 });
  bot.lastReceivedAt = 200; me.t = 200;
  bot.probeVisibility(200, me, choice);
  assert.equal(bot.visibility, null);
  assert.equal(events.at(-1).reason, 'avoidance');
  assert.equal(events.at(-1).completed, false);
  bot.world.snakes.delete(2);
  bot.lastReceivedAt = 1800; me.t = 1800;
  bot.probeVisibility(1800, me, choice);
  assert.equal(bot.phase, 'diagonal_cruise_positive_1');
  bot.lastReceivedAt = 2401;
  bot.probeVisibility(2401, me, choice);
  assert.equal(events.at(-1).reason, 'stale_anchor');
  bot.lastReceivedAt = 4000; me.t = 4000; me.sct = 5;
  bot.probeVisibility(4000, me, choice);
  assert.equal(bot.visibility?.id, 3);
  assert.equal(bot.phase, 'axis_boost_positive_1');
  bot.lastReceivedAt = 35001; me.t = 35001;
  bot.probeVisibility(35001, me, choice);
  assert.equal(events.at(-1).reason, 'phase_timeout');
});

test('visibility boost fallback logs low mass while pickups remain a covariate', () => {
  const { bot, me, events } = visibilityBot();
  let choice = { a: 0, danger: 0 };
  bot.probeVisibility(100, me, choice);
  bot.visibility.block = 2;
  bot.startVisibilityPhase(200, me, { x: 21600, y: 21600 });
  bot.lastReceivedAt = 200; me.t = 200;
  bot.probeVisibility(200, me, choice);
  assert.equal(bot.visibilityIntent.mode, 'boost');
  assert.equal(bot.visibilityIntent.desiredBoost, false);
  assert.equal(bot.visibilityIntent.boostEligible, false);
  assert.equal(bot.visibilityIntent.effectiveMode, 'cruise');
  assert.equal(bot.visibilityIntent.boostFallback, 'insufficient_mass');
  assert.equal(events.at(-1).reason, 'insufficient_mass');
  me.fam = 0.5; me.t = 225; bot.lastReceivedAt = 225;
  // Ordinary pickup during acceleration remains in the same experimental leg.
  bot.probeVisibility(225, me, choice);
  assert.equal(bot.visibility?.id, 1);
  assert.equal(bot.visibilityIntent.deltaMass, 0.5);
  assert.equal(events.at(-1).event, 'visibility-mass-change');
  assert.equal(bot.visibilityIntent.desiredBoost, true);
  assert.equal(bot.visibility.massChangeCount, 1);
  assert.equal(bot.visibility.massDeltaTotal, 0.5);
  me.fam = 0.49; me.t = 250; bot.lastReceivedAt = 250;
  bot.probeVisibility(250, me, choice);
  assert.equal(bot.visibility.massChangeCount, 1);
  assert(Math.abs(bot.visibility.massDeltaTotal - 0.49) < 1e-12);
  bot.lastReceivedAt = 8226; me.t = 8226;
  bot.probeVisibility(8226, me, choice);
  assert.equal(bot.visibilityIntent.desiredBoost, false);
  assert.equal(bot.visibilityIntent.boostFallback, 'boost_window_complete');

  const original = globalThis.WebSocket;
  globalThis.WebSocket = class { close() {} };
  try {
    bot.connect({ ip: '127.0.0.1', port: 1 });
    assert.equal(bot.visibility, null);
    assert.equal(bot.visibilityIntent, null);
    assert.equal(bot.visibilityRouteId, 0);
    assert.equal(bot.visibilityNextBlock, 0);
    bot.stop();
  } finally { globalThis.WebSocket = original; }
  assert.throws(() => parseArgs(['--out', 'unused', '--scenario', 'visibilityprobe', '--bots', '2']), /one client/);
  assert.throws(() => parseArgs(['--out', 'unused', '--scenario', 'visibilityprobe', '--survey', '2']), /unknown option/);
  assert.equal(parseArgs(['--out', 'unused', '--scenario', 'forage']).scenario, 'forage');
});

test('censored routes rotate axis, diagonal and boost blocks without claiming completed legs', () => {
  const { bot, me, events } = visibilityBot();
  let now = 100;
  const choice = { a: 0, danger: 0 };
  const expected = ['axis_cruise_positive_1', 'diagonal_cruise_positive_1',
    'axis_boost_positive_1', 'diagonal_boost_positive_1', 'axis_cruise_positive_1'];
  for (let i = 0; i < expected.length; i++) {
    me.t = now; bot.lastReceivedAt = now;
    bot.probeVisibility(now, me, choice);
    if (bot.visibilityIntent.phase === 'acquire') {
      assert.equal(bot.visibilityIntent.interruption, 'acquire_boost_mass');
      now += VISIBILITY_PROBE.boostPrepareMaxMs + 25;
      me.t = now; bot.lastReceivedAt = now;
      bot.probeVisibility(now, me, choice);
    }
    assert.equal(bot.phase, expected[i]);
    if (i === expected.length - 1) break;
    const offset = i % 2 === 0 ? { x: 200, y: 0 } : { x: 142, y: 142 };
    bot.world.snakes.set(2, { id: 2, pts: [{ x: 21600 + offset.x, y: 21600 + offset.y }],
      ang: 0, sp: 0, sct: 2, t: now + 25 });
    now += 25; me.t = now; bot.lastReceivedAt = now;
    bot.probeVisibility(now, me, choice);
    assert.equal(bot.visibility, null);
    assert.equal(events.at(-1).completed, false);
    bot.world.snakes.delete(2);
    now += VISIBILITY_PROBE.reacquireCooldownMs + 25;
  }
  assert.equal(events.filter(e => e.event === 'visibility-phase-end' && e.completed).length, 0);
  assert.equal(events.filter(e => e.event === 'visibility-boost-fallback' && e.reason === 'insufficient_mass').length, 2);
});

test('boost block waits for a reserve, then starts or times out into marked cruise', () => {
  const { bot, me, events } = visibilityBot();
  const choice = { a: 0, danger: 0 };
  bot.visibilityNextBlock = 2;
  bot.probeVisibility(100, me, choice);
  assert.equal(bot.visibility, null);
  assert.equal(bot.visibilityIntent.interruption, 'acquire_boost_mass');
  me.sct = 5; me.t = 200; bot.lastReceivedAt = 200;
  bot.probeVisibility(200, me, choice);
  assert.equal(bot.visibility?.block, 2);
  assert.equal(bot.visibilityIntent.desiredBoost, true);
  bot.endVisibilityLeg(225, 'avoidance');

  bot.visibilityNextBlock = 3;
  me.sct = 2; me.fam = 0; me.t = 2000; bot.lastReceivedAt = 2000;
  bot.probeVisibility(2000, me, choice);
  assert.equal(bot.visibilityIntent.interruption, 'acquire_boost_mass');
  const timeout = 2000 + VISIBILITY_PROBE.boostPrepareMaxMs;
  me.t = timeout; bot.lastReceivedAt = timeout;
  bot.probeVisibility(timeout, me, choice);
  assert.equal(bot.visibility?.block, 3);
  assert.equal(bot.visibilityIntent.effectiveMode, 'cruise');
  assert.equal(bot.visibilityIntent.boostFallback, 'insufficient_mass');
  assert(events.some(e => e.event === 'visibility-boost-prepare-timeout'));
});

test('completed cruise block enters mass preparation before a boost block', () => {
  const { bot, me, events } = visibilityBot();
  const choice = { a: 0, danger: 0 };
  bot.probeVisibility(100, me, choice);
  bot.visibility.block = 1;
  bot.visibility.leg = 3;
  bot.startVisibilityPhase(200, me, { x: 21600, y: 21600 });
  const target = { ...bot.visibility.target };
  me.pts[0] = target;
  me.t = 300; bot.lastReceivedAt = 300;
  bot.probeVisibility(300, me, choice);
  assert.equal(bot.visibility, null);
  assert.equal(bot.visibilityNextBlock, 2);
  assert.equal(bot.visibilityIntent.interruption, 'acquire_boost_mass');
  assert(events.some(e => e.event === 'visibility-boost-prepare-start'));
  const end = events.find(e => e.event === 'visibility-phase-end');
  assert.equal(end.completed, true);
  assert.equal(end.reason, 'endpoint_reached');
});

test('visibility rim acquisition times out and a lateral detour invalidates the leg', () => {
  const { bot, me, events } = visibilityBot();
  me.pts[0] = { x: bot.world.grd + bot.world.radius - 1000, y: bot.world.grd };
  const choice = { a: 0, danger: 0 };
  bot.probeVisibility(100, me, choice);
  assert.equal(bot.visibility, null);
  assert.equal(bot.visibilityIntent.interruption, 'rim_margin');
  assert.equal(choice.a, Math.PI);
  let closes = 0;
  bot.ws.close = () => { closes++; };
  me.t = 45100; bot.lastReceivedAt = 45100;
  bot.probeVisibility(90100, me, choice);
  assert.equal(closes, 1);
  assert.equal(events.at(-1).event, 'visibility-reacquire-timeout');

  me.pts[0] = { x: 21600, y: 21600 };
  me.t = 90200; bot.lastReceivedAt = 90200;
  bot.probeVisibility(90200, me, choice);
  assert.equal(bot.visibility?.id, 1);
  me.pts[0] = { x: 21700, y: 21850 };
  me.t = 90300; bot.lastReceivedAt = 90300;
  bot.probeVisibility(90300, me, choice);
  assert.equal(events.at(-1).reason, 'off_path');
  assert.equal(events.at(-1).completed, false);
});

// Inverse of the client's version-string decoding, so the test needs no captured packet.
function encode(code) {
  let out = '', k = 23;
  for (const byte of Buffer.from(code, 'latin1')) {
    for (const nibble of [byte >> 4, byte & 15]) {
      out += String.fromCharCode(97 + (nibble + k) % 26);
      k += 17;
    }
  }
  return out;
}

test('challenge reply matches a recorded stock-client handshake', () => {
  const code = `var a = "=9Wz#";a+="M'n-|LqN6J:'4v26Thf[;u";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}`;
  assert.equal(Buffer.from(solveChallenge(encode(code))).toString('latin1'), '<8RuUKOfPuJdUNAXBSrGZWhqUSq');
});

test('unframed community challenge selects raw packets without inventing server time', () => {
  const code = `var a = "=9Wz#";a+="M'n-|LqN6J:'4v26Thf[;u";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}`;
  const sent = [], events = [];
  const bot = new Bot({ scenario: 'forage', nick: 'probe' }, 0, {
    frame() {}, meta(event) { events.push(event); },
  });
  bot.ws = { readyState: 1, send(bytes) { sent.push(Array.from(bytes)); } };
  bot.receive(Uint8Array.from(Buffer.from('6' + encode(code), 'latin1')));
  bot.receive(Uint8Array.from([112]));
  const setup = new Uint8Array(30);
  setup.set([97, 0, 127, 38]); // Synthetic grid center 32550.
  setup[23] = 19;
  setup[24] = 42;
  bot.receive(Uint8Array.from([0, setup.length, ...setup, 33, 112]));
  assert.equal(bot.joins, 1);
  assert.equal(bot.world.grd, 32550);
  assert.equal(bot.rawFraming, true);
  assert.equal(bot.serverClock, null);
  assert.equal(bot.awaitingPong, false);
  assert.equal(sent.length, 2);
  assert.equal(sent[1][0], 115);
  assert(events.some(event => event.event === 'unframed-challenge'));
});

test('challenge solver rejects executable additions and malformed payloads', () => {
  const code = `var a = "=9Wz#";a+="M'n-|LqN6J:'4v26Thf[;u";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}`;
  for (const source of [code + ';globalThis.changed=true;',
    'idba.constructor.constructor("return process")();',
    'for (var i=0;i<27;i++){idba[i]=65;}'])
    assert.throws(() => solveChallenge(encode(source)), /Unsupported challenge/);
  for (const raw of ['', 'a', '!!', 'a'.repeat(514)])
    assert.throws(() => solveChallenge(raw), /Invalid challenge/);
});

test('small-snake movement threshold can be lowered for a community capture', () => {
  const opts = parseArgs(['--out', 'unused', '--scenario', 'movement', '--movement-min-segments', '2']);
  const events = [];
  const bot = new Bot(opts, 0, { frame() {}, meta(event) { events.push(event); } });
  bot.world.self = 1;
  const me = { id: 1, pts: [{ x: 21600, y: 21600 }], ang: 0, wang: 0,
    sp: 5.8, sct: 2, fam: 0, t: 100 };
  bot.world.snakes.set(1, me);
  bot.ws = { readyState: 1, send() {} };
  bot.lastReceivedAt = 100;
  bot.tick(100);
  assert(events.some(event => event.event === 'phase' && event.minSegments === 2));
  assert.throws(() => parseArgs(['--out', 'unused', '--movement-min-segments', '1']), /integer >=2/);
});

test('frames split into logical packets after the elapsed-ms header', () => {
  const frame = Uint8Array.from([0, 7, 0, 3, 101, 1, 2, 35, 104, 0, 9]);
  assert.deepEqual(splitFrame(frame).map(p => Array.from(p)), [[101, 1, 2], [104, 0, 9]]);
  assert.deepEqual(Array.from(splitFrame(Uint8Array.from([0, 7, 112]))[0]), [112]);
});

test('foraging follows nearby food off the current heading while retaining avoidance', () => {
  const world = new World();
  world.self = 1;
  world.snakes.set(1, { id: 1, sct: 2, ang: 0, sp: 5.79, t: 0,
    pts: [{ x: 21600, y: 21600 }] });
  assert.equal(chooseHeading(world, 0).a, 0, 'empty space favors the current heading');
  for (const size of [4, 13.6]) {
    world.foods.set(1, { x: 21600, y: 21700, size });
    assert.equal(chooseHeading(world, 0).a, Math.PI / 2,
      'a safe pellet beside the snake should overcome the turn penalty');
  }
  world.snakes.set(2, { id: 2, sct: 10, ang: 0, sp: 0, t: 0,
    pts: [{ x: 21600, y: 21680 }] });
  const safe = chooseHeading(world, 0);
  assert.equal(safe.danger, 0);
  assert.notEqual(safe.a, Math.PI / 2, 'nearby food must not override body avoidance');
});

test('login packet uses the stock layout and truncates nicknames', () => {
  const p = loginPacket('x'.repeat(30));
  assert.deepEqual(Array.from(p.subarray(0, 4)), [115, 30, 1, 35]);
  assert.equal(p[25], 24);
  assert.deepEqual(Array.from(p.subarray(-2)), [0, 255]);
});

test('self rotation packets update heading, wanted heading and speed', () => {
  const w = new World();
  w.snakes.set(5, { id: 5, pts: [{ x: 0, y: 0 }], ang: 0, wang: 0, sp: 0, sct: 1, t: 0 });
  w.self = 5;
  w.packet(Uint8Array.from([100, 64, 128, 104]), 0); // 'd': ang, wang, speed/18
  const me = w.snakes.get(5);
  assert.equal(me.ang, Math.PI / 2);
  assert.equal(me.wang, Math.PI);
  assert.equal(me.sp, 104 / 18);
});

test('R packets replace only the own rsc and spawn resets it', () => {
  const w = new World();
  w.self = 1;
  const own = { id: 1, pts: [{ x: 1, y: 2 }], sct: 1, fam: .25, rsc: 0 };
  const other = { id: 2, pts: [{ x: 3, y: 4 }], sct: 1, fam: .5, rsc: 0 };
  w.snakes.set(1, own);
  w.snakes.set(2, other);
  for (const value of [9, 3, 0]) {
    assert.equal(w.packet(Uint8Array.from([82, value]), 100), true);
    assert.equal(own.rsc, value);
    assert.deepEqual(own.pts, [{ x: 1, y: 2 }]);
    assert.equal(own.sct, 1);
    assert.equal(own.fam, .25);
    assert.equal(other.rsc, 0);
  }
  assert.notEqual(w.packet(Uint8Array.from([82]), 110), true);
  assert.notEqual(w.packet(Uint8Array.from([82, 7, 8]), 120), true);
  assert.equal(own.rsc, 0);
  w.snakes.delete(1);
  assert.notEqual(w.packet(Uint8Array.from([82, 7]), 130), true);
  const spawn = new Uint8Array(31);
  spawn.set([115, 0, 1]);
  w.packet(spawn, 140);
  assert.equal(w.snakes.get(1).rsc, 0);
});

test('valid applied R packets create server-clock metadata', () => {
  const events = [];
  const bot = new Bot({}, 0, { frame() {}, meta(event) { events.push(event); } });
  bot.lives = 2;
  bot.world.self = 1;
  bot.world.snakes.set(1, { id: 1, pts: [{ x: 1, y: 2 }], sct: 1, fam: .25, rsc: 0 });
  bot.receive(Uint8Array.from([0, 7, 82, 9]));
  assert.deepEqual(events, [{ event: 'rsc', serverClock: 7, value: 9, snakeId: 1, life: 2 }]);
  bot.receive(Uint8Array.from([0, 3, 82]));
  bot.receive(Uint8Array.from([0, 3, 82, 7, 8]));
  bot.world.snakes.delete(1);
  bot.receive(Uint8Array.from([0, 3, 82, 7]));
  assert.equal(events.length, 1);
});

test('server body contraction is opt-in and ordinary anchor bookkeeping stays unchanged', () => {
  const makeWorld = serverBody => {
    const world = new World({ serverBody });
    world.self = 1;
    world.snakes.set(1, { id: 1, pts: [0, 10, 20, 30, 40].map(x => ({ x, y: 0 })),
      sct: 5, iang: 0, ang: 0, sp: 0, t: 0 });
    return world;
  };
  const ordinary = makeWorld(false), diagnostic = makeWorld(true);
  const anchor = Uint8Array.from([103, 0, 1]);
  ordinary.packet(anchor, 1);
  diagnostic.packet(anchor, 1);
  assert.deepEqual(ordinary.snakes.get(1).pts.map(point => point.x), [10, 20, 30, 40, 82]);
  assert.ok(diagnostic.snakes.get(1).pts[0].x > 10);
  assert.equal(diagnostic.snakes.get(1).pts.at(-1).x, 82);
});

test('diagnostic body contraction uses the coefficient in the server setup packet', () => {
  const world = new World({ serverBody: true });
  const setup = new Uint8Array(23);
  setup[0] = 97;
  setup[21] = 3;
  setup[22] = 32; // cst = 800 / 1000, deliberately unlike the normal .43.
  world.packet(setup, 0);
  world.self = 1;
  world.snakes.set(1, { id: 1, pts: [0, 10, 20, 30, 40].map(x => ({ x, y: 0 })),
    sct: 5, iang: 0, ang: 0, sp: 0, t: 0 });
  world.packet(Uint8Array.from([103, 0, 1]), 1);
  assert.deepEqual(world.snakes.get(1).pts.map(point => point.x), [12, 20, 30, 40, 82]);
});

test('a stopped bot cannot reconnect from an already queued retry', () => {
  const bot = new Bot({}, 0, { meta() {} });
  bot.stop();
  bot.connect({ ip: '127.0.0.1', port: 1 });
  assert.equal(bot.lives, 0);
  assert.equal(bot.ws, undefined);
});

test('spawn sampling closes only a joined session after the observation interval', () => {
  const events = [];
  const bot = new Bot({ scenario: 'spawn' }, 0, { meta(e) { events.push(e); } });
  let closes = 0;
  bot.ws = { readyState: 1, close() { closes++; } };
  bot.connectedAt = 0;
  bot.lastReceivedAt = 15001;
  bot.tick(15001);
  assert.equal(closes, 0);
  bot.world.self = 1;
  bot.tick(14999);
  assert.equal(closes, 0);
  bot.tick(15001);
  assert.equal(closes, 1);
  assert.equal(events.at(-1).event, 'spawn-sample-complete');
});

test('reconnecting resets ping, boost, heading and pellet ownership state', () => {
  const original = globalThis.WebSocket;
  globalThis.WebSocket = class { close() {} };
  try {
    const bot = new Bot({}, 0, { meta() {} });
    bot.awaitingPong = true;
    bot.boost = true;
    bot.lastHeading = 25;
    bot.deathObservation = { ended: true };
    bot.own.set(1, {});
    bot.rawFraming = true;
    bot.connectionFrames = 100;
    bot.connect({ ip: '127.0.0.1', port: 1 });
    assert.equal(bot.awaitingPong, false);
    assert.equal(bot.boost, false);
    assert.equal(bot.lastHeading, -1);
    assert.equal(bot.own.size, 0);
    assert.equal(bot.deathObservation, null);
    assert.equal(bot.rawFraming, false);
    assert.equal(bot.connectionFrames, 0);
    bot.stop();
  } finally { globalThis.WebSocket = original; }
});

test('game-over records through the close handshake without controls or a stale receipt', t => {
  const events = [], frames = [], sent = [];
  let closes = 0, now = 1000;
  t.mock.method(performance, 'now', () => now);
  const originalWebSocket = globalThis.WebSocket;
  globalThis.WebSocket = class {
    readyState = 1;
    send(bytes) { sent.push(bytes); }
    close() { closes++; }
  };
  t.after(() => { globalThis.WebSocket = originalWebSocket; });
  const bot = new Bot({ scenario: 'spawn' }, 0, {
    meta(e) { events.push(e); }, frame(...args) { frames.push(args); },
  });
  t.after(() => bot.stop());
  bot.connect({ ip: '127.0.0.1', port: 1 });
  bot.connectedAt = -20000; // The ordinary spawn deadline must not cut the window short.
  bot.world.self = 1;
  bot.world.snakes.set(1, { sct: 2 }); // Game-over may precede snake removal.
  const frame = packets => Uint8Array.from([0, 7, ...packets.flatMap(p => [0, p.length, ...p])]);
  bot.receive(frame([[118], [98, 1, 1, 10, 11, 68]]));
  const start = bot.deathObservation.startedAt;
  assert.equal(closes, 0);
  assert.equal(bot.world.foods.size, 1);
  bot.tick(start + DEATH_OBSERVATION_MS - 1);
  bot.send([253]);
  bot.receive(frame([[118], [98, 12, 13, 68]]));
  assert.equal(bot.world.foods.size, 2);
  assert.equal(bot.deathObservation.startedAt, start, 'duplicate game-over cannot extend deadline');
  assert.deepEqual(sent.map(p => Array.from(p)), [[251]]);
  assert.equal(frames.filter(f => f[0] === 'i').length, 2);
  bot.tick(start + DEATH_OBSERVATION_MS);
  bot.tick(start + DEATH_OBSERVATION_MS + 25);
  assert.equal(closes, 1);
  assert.equal(events.filter(e => e.event === 'death-observation-start').length, 1);
  assert.equal(events.filter(e => e.event === 'death-observation-end').length, 0);
  now = start + DEATH_OBSERVATION_MS + 30;
  // A queued incoming frame during closing still belongs in the final receipt.
  bot.receive(frame([[98, 14, 15, 68]]));
  assert.equal(bot.world.foods.size, 3);
  assert.deepEqual(sent.map(p => Array.from(p)), [[251]]);
  now += 10;
  bot.ws.onclose({ code: 1000, reason: '' });
  const { elapsedMs, ...end } = events.find(e => e.event === 'death-observation-end');
  assert.equal(elapsedMs, DEATH_OBSERVATION_MS + 40);
  assert.deepEqual(end, { event: 'death-observation-end', life: 1,
    reason: 'deadline', serverElapsedMs: 14, framesAfterDeath: 2 });
});

test('capture deadline marks a truncated death window and detaches callbacks', () => {
  const events = [];
  let closes = 0;
  const bot = new Bot({}, 0, { meta(e) { events.push(e); }, frame() {} });
  bot.ws = { close() { closes++; }, onmessage() {} };
  bot.receive(Uint8Array.from([0, 7, 118]));
  const ws = bot.ws;
  bot.stop();
  bot.tick(performance.now() + DEATH_OBSERVATION_MS);
  assert.equal(closes, 1);
  assert.equal(ws.onmessage, null);
  assert.equal(events.at(-1).reason, 'capture-stop');
  assert.equal(events.at(-1).framesAfterDeath, 0);
});

test('server close marks a censored death window before normal reconnect', () => {
  const original = globalThis.WebSocket;
  globalThis.WebSocket = class { readyState = 1; close() {} };
  const events = [];
  const bot = new Bot({}, 0, { meta(e) { events.push(e); }, frame() {} });
  try {
    bot.connect({ ip: '127.0.0.1', port: 1 });
    bot.receive(Uint8Array.from([0, 7, 118]));
    bot.ws.onclose({ code: 1000, reason: 'server ended session' });
    assert.equal(events.at(-2).event, 'death-observation-end');
    assert.equal(events.at(-2).reason, 'socket-close');
    assert.equal(events.at(-1).event, 'close');
    assert.ok(bot.reconnectTimer);
  } finally {
    bot.stop();
    globalThis.WebSocket = original;
  }
});

function movementBot() {
  const events = [], sent = [];
  const bot = new Bot({ scenario: 'movement' }, 0, {
    meta(e) { events.push(e); }, frame() {},
  });
  bot.ws = { readyState: 1, send(bytes) { sent.push(Array.from(bytes)); } };
  bot.lastReceivedAt = 1e9;
  bot.world.self = 1;
  bot.world.snakes.set(1, {
    id: 1, pts: [{ x: 21600, y: 21600 }], ang: 0, wang: 0, sp: 5.7,
    sct: 6, fam: 0.5, t: 0,
  });
  return { bot, events, sent };
}

test('movement probe runs bounded turn, reversal and boost phases then waits to grow', () => {
  const { bot, events, sent } = movementBot();
  for (const t of [100, 1400, 3700, 6000, 7900, 9800, 10700, 11850, 13000, 20100]) bot.tick(t);
  assert.deepEqual(events.filter(e => e.event === 'phase').map(e => e.phase), [
    'straight_cruise', 'left_cruise', 'right_cruise', 'reverse_left_cruise',
    'reverse_right_cruise', 'straight_boost', 'left_boost', 'right_boost', 'recover_forage',
  ]);
  assert.equal(events.find(e => e.event === 'movement-tier-complete')?.tier, 'small');
  const controls = events.filter(e => e.event === 'movement-control');
  assert.equal(controls[5].desiredBoost, true);
  assert.equal(controls[5].heldBoost, true);
  assert.equal(controls.at(-1).phase, 'forage_wait');
  assert.equal(controls.at(-1).tier, 'medium');
  assert.equal(controls.at(-1).heldBoost, false);
  assert(sent.some(p => p[0] === 253));
  assert(sent.some(p => p[0] === 254));
});

test('movement probe records avoidance when prescribed heading crosses another snake', () => {
  const { bot, events } = movementBot();
  bot.tick(100);
  bot.world.snakes.set(2, {
    id: 2, pts: [{ x: 21703, y: 21780 }], ang: 0, wang: 0, sp: 0,
    sct: 6, fam: 0.5, t: 0,
  });
  bot.tick(1400);
  const c = events.filter(e => e.event === 'movement-control').at(-1);
  assert.equal(c.phase, 'left_cruise');
  assert.equal(c.interruption, 'avoidance');
  assert(c.intendedDanger > 0.2);
  assert.notEqual(c.requestedHeading, Math.floor(251 * 1.35 / (2 * Math.PI)));
  assert.equal(c.heldBoost, false);
});

function boundaryBot(life = 1, sct = 6) {
  const events = [], sent = [];
  const bot = new Bot({ scenario: 'boundaryprobe' }, 0, {
    meta(event) { events.push(event); }, frame() {},
  });
  bot.lives = life;
  bot.ws = { readyState: 1, send(bytes) { sent.push(Array.from(bytes)); } };
  bot.lastReceivedAt = 1e9;
  bot.world.grd = 1000;
  bot.world.radius = 900;
  bot.world.self = 1;
  const me = { id: 1, pts: [{ x: 1500, y: 1000 }], sct, fam: .25,
    ang: 0, wang: 0, sp: 5.7, t: 0 };
  bot.world.snakes.set(1, me);
  return { bot, me, events, sent };
}

test('boundary probe approaches with food, then holds shallow and increasing outward angles', () => {
  const { bot, me, events } = boundaryBot();
  bot.world.foods.set(1, { x: 1800, y: 1100, size: 4 });
  let choice = { a: 0, danger: 0 };
  bot.probeBoundary(100, me, choice);
  assert.equal(bot.boundaryIntent.phase, 'approach');
  assert.equal(choice.a, Math.atan2(100, 300));
  me.pts[0].x = 1600;
  bot.probeBoundary(150, me, choice);
  assert.equal(bot.boundaryIntent.phase, 'acquire');
  assert.equal(choice.a, Math.PI / 2 - BOUNDARY_PROBE.acquireAngle);
  me.pts[0].x = 1870; me.pts[0].y = 1000;
  for (const [time, phase, angle] of [[200, 'shallow', .02],
    ...BOUNDARY_PROBE.outwardAngles.map((a, i) =>
      [200 + BOUNDARY_PROBE.shallowMs + i * BOUNDARY_PROBE.outwardStepMs,
        `outward_${i + 1}`, a]),
    [200 + BOUNDARY_PROBE.shallowMs + BOUNDARY_PROBE.outwardAngles.length * BOUNDARY_PROBE.outwardStepMs,
      'complete', null]]) {
    choice = { a: 0, danger: 1 };
    bot.probeBoundary(time, me, choice);
    assert.equal(bot.boundaryIntent.phase, phase);
    assert.equal(bot.boundaryIntent.angle, angle);
    assert.equal(choice.a, phase === 'complete' ? Math.PI : Math.PI / 2 - angle);
  }
  assert.deepEqual(events.filter(e => e.event === 'phase').map(e => e.phase),
    ['approach', 'acquire', 'shallow',
      ...BOUNDARY_PROBE.outwardAngles.map((_, i) => `outward_${i + 1}`), 'complete']);
  assert.equal(BOUNDARY_PROBE.entryGap, 40);
});

test('boundary probe alternates boost by life, requires mass, and flags other snakes', () => {
  const { bot, me } = boundaryBot(2, 4);
  me.pts[0].x = 1870;
  let choice = { a: 0, danger: 0 };
  bot.probeBoundary(100, me, choice);
  assert.equal(bot.wantBoost(100, me, choice), false);
  me.sct = 5;
  for (const time of [1100, 2100, 3100, 4100]) bot.probeBoundary(time, me, choice);
  assert.equal(bot.boundaryIntent.angle, BOUNDARY_PROBE.boostStartAngle);
  assert.equal(bot.wantBoost(4100, me, choice), true);
  bot.world.snakes.set(2, { id: 2, sct: 5, pts: [{ x: 1870, y: 1100 }], ang: 0, sp: 0, t: 0 });
  choice = { a: 1, danger: 0 };
  bot.probeBoundary(300, me, choice);
  assert.equal(bot.boundaryIntent.interruption, 'other_snake');
  assert.equal(bot.boundaryIntent.desiredBoost, false);
  assert.equal(choice.a, 1);
  assert.ok(bot.boundaryIntent.nearestOtherClearance < 100);
  bot.world.snakes.delete(2);
  bot.lives = 3;
  bot.probeBoundary(400, me, choice);
  assert.equal(bot.boundaryIntent.trial, 'cruise');
  assert.equal(bot.wantBoost(400, me, choice), false);
});

test('boundary control logs raw anchor and resets trial state on reconnect', () => {
  const { bot, me, events } = boundaryBot(2);
  me.pts[0].x = 1870;
  bot.serverClock = 77;
  bot.tick(100);
  const control = events.find(e => e.event === 'boundary-control');
  assert.equal(control.x, 1870);
  assert.equal(control.y, 1000);
  assert.equal(control.anchorAgeMs, 100);
  assert.equal(control.serverClock, 77);
  assert.equal(control.radius, 900);
  assert.equal(control.grd, 1000);
  assert.equal(control.sct, 6);
  assert.equal(control.fam, .25);
  assert.equal(control.heldBoost, false);
  const original = globalThis.WebSocket;
  globalThis.WebSocket = class { close() {} };
  try {
    bot.connect({ ip: '127.0.0.1', port: 1 });
    assert.equal(bot.boundaryEntryAt, null);
    assert.equal(bot.boundaryPhase, null);
    assert.equal(bot.boundaryIntent, null);
    bot.stop();
  } finally { globalThis.WebSocket = original; }
});

test('boundary probe CLI requires one client', () => {
  const args = ['--out', 'capture', '--scenario', 'boundaryprobe'];
  assert.equal(parseArgs(args).scenario, 'boundaryprobe');
  assert.throws(() => parseArgs([...args, '--bots', '2']), /one client/);
  assert.throws(() => parseArgs([...args, '--survey', '1']), /unknown option/);
});

test('boost boundary sweep saves fuel until angle 0.35 and then boosts only while eligible', () => {
  const { bot, me } = boundaryBot(2, 12);
  me.pts[0].x = 1890;
  for (let second = 0; second <= 8; second++) {
    bot.probeBoundary(100 + second * 1000, me, { a: 0, danger: 0 });
    assert.equal(bot.boundaryIntent.desiredBoost, second >= 4);
    assert.equal(bot.boundaryIntent.boostWindow, second >= 4);
  }
  assert.equal(bot.boundaryIntent.angle, .70);
  assert.equal(bot.boundaryIntent.phase, 'outward_8');
  assert.equal(BOUNDARY_PROBE.boostStartAngle, .35);
  me.sct = 4;
  bot.probeBoundary(9100, me, { a: 0, danger: 0 });
  assert.equal(bot.boundaryIntent.desiredBoost, false);
  me.sct = 5;
  bot.probeBoundary(9200, me, { a: 0, danger: 0 });
  assert.equal(bot.boundaryIntent.desiredBoost, true);
});

test('boundary steering hysteresis does not count off-rim dwell as exposure', () => {
  const { bot, me } = boundaryBot();
  me.pts[0].x = 1890;
  bot.probeBoundary(100, me, { a: 0, danger: 0 });
  me.pts[0].x = 1830; // gap 70: hold steering, but discard timed exposure.
  bot.probeBoundary(200, me, { a: 0, danger: 0 });
  bot.probeBoundary(20000, me, { a: 0, danger: 0 });
  assert.equal(bot.boundaryContact, true);
  assert.equal(bot.boundaryIntent.phase, 'shallow');
  assert.equal(bot.boundaryIntent.nearRimDwellMs, 0);
  me.pts[0].x = 1890;
  bot.probeBoundary(21000, me, { a: 0, danger: 0 });
  bot.probeBoundary(35999, me, { a: 0, danger: 0 });
  assert.equal(bot.boundaryIntent.phase, 'shallow');
  bot.probeBoundary(36000, me, { a: 0, danger: 0 });
  assert.equal(bot.boundaryIntent.phase, 'outward_1');
});

test('boundary trials cover both tangent directions independently of boost', () => {
  for (const [life, tangentSign] of [[1, 1], [2, 1], [3, -1], [4, -1]]) {
    const { bot, me } = boundaryBot(life, 12);
    me.pts[0].x = 1890;
    const choice = { a: 0, danger: 0 };
    bot.probeBoundary(100, me, choice);
    assert.equal(choice.a, tangentSign * (Math.PI / 2 - .02));
    assert.equal(bot.boundaryIntent.tangentSign, tangentSign);
    assert.equal(bot.boundaryIntent.desiredBoost, false);
  }
});

test('boundary probe censors a life that never acquires raw rim contact', () => {
  const { bot, events } = boundaryBot();
  let closes = 0;
  bot.ws.close = () => { closes++; };
  bot.connectedAt = 0;
  bot.tick(BOUNDARY_PROBE.acquireTimeoutMs - 1);
  assert.equal(closes, 0);
  assert.equal(bot.boundaryIntent.phase, 'approach');
  bot.tick(BOUNDARY_PROBE.acquireTimeoutMs);
  assert.equal(closes, 1);
  assert.equal(bot.boundaryCensored, true);
  assert.equal(events.at(-1).event, 'boundary-censored');
  assert.equal(events.at(-1).reason, 'acquire_timeout');
  bot.tick(BOUNDARY_PROBE.acquireTimeoutMs + 25);
  assert.equal(closes, 1);
});

test('boundary probe checks a predicted other head without mixing it into raw clearance', () => {
  const { bot, me } = boundaryBot(2);
  me.pts[0].x = 1870;
  const other = { id: 2, sct: 5, pts: [{ x: 1950, y: 1500 }],
    ang: -Math.PI / 2, sp: 6, t: 0 };
  bot.world.snakes.set(2, other);
  const choice = { a: 0, danger: 0 };
  bot.probeBoundary(300, me, choice);
  assert.equal(bot.boundaryIntent.interruption, 'other_snake');
  assert.ok(bot.boundaryIntent.nearestPredictedHeadClearance < bot.boundaryIntent.nearestOtherClearance);
  assert.equal(choice.a, 0);
});

test('boundary sweep reacquires after radius expansion and restarts only the current step dwell', () => {
  const { bot, me, events } = boundaryBot();
  me.pts[0].x = 1870;
  const steer = time => bot.probeBoundary(time, me, { a: 0, danger: 0 });
  steer(100);
  steer(15100);
  assert.equal(bot.boundaryStep, 0);
  assert.equal(bot.boundaryIntent.phase, 'outward_1');
  bot.world.radius = 1000; // raw gap becomes 130, without a snake position change
  steer(25000);
  assert.equal(bot.boundaryIntent.phase, 'reacquire');
  assert.equal(bot.boundaryStep, 0);
  assert.equal(bot.boundaryDwellAt, null);
  assert.equal(events.at(-2).event, 'boundary-contact-lost');
  assert.equal(events.at(-2).reason, 'gap');
  me.pts[0].x = 1970;
  steer(26000);
  assert.equal(bot.boundaryIntent.phase, 'outward_1');
  assert.equal(events.find(e => e.event === 'boundary-reacquired')?.step, 0);
  steer(35999);
  assert.equal(bot.boundaryStep, 0);
  steer(36000);
  assert.equal(bot.boundaryStep, 1);
  assert.equal(bot.boundaryIntent.phase, 'outward_2');
});

test('other-snake interruption discards dwell and cannot falsely reacquire while blocked', () => {
  const { bot, me, events } = boundaryBot();
  me.pts[0].x = 1870;
  const steer = time => bot.probeBoundary(time, me, { a: 0, danger: 0 });
  steer(100);
  bot.world.snakes.set(2, { id: 2, sct: 5, pts: [{ x: 1870, y: 1100 }],
    ang: 0, sp: 0, t: 0 });
  steer(5000);
  assert.equal(bot.boundaryIntent.phase, 'reacquire');
  assert.equal(bot.boundaryContact, false);
  steer(6000);
  assert.equal(events.filter(e => e.event === 'boundary-reacquired').length, 0);
  assert.equal(events.filter(e => e.event === 'boundary-contact-lost').length, 1);
  bot.world.snakes.delete(2);
  steer(7000);
  assert.equal(bot.boundaryIntent.phase, 'shallow');
  assert.equal(events.filter(e => e.event === 'boundary-reacquired').length, 1);
  steer(21999);
  assert.equal(bot.boundaryStep, -1);
  steer(22000);
  assert.equal(bot.boundaryStep, 0);
});

test('prey packets retain appearance generations, coordinates and explicit speeds', () => {
  const w = new World();
  const appearance = Uint8Array.from([121, 0, 7, 0, 1, 165, 224, 1, 165, 224,
    30, 48, 0, 0, 0, 0, 0, 0, 14, 216]);
  w.packet(appearance, 100);
  assert.equal(w.prey.get(7).x, 21600);
  assert.equal(w.prey.get(7).sp, 3.8);
  const update = Uint8Array.from([106, 0, 7, 28, 32, 28, 32, 48,
    0, 0, 0, 0, 0, 0, 24, 9]);
  w.packet(update, 200);
  assert.equal(w.prey.get(7).x, 21601);
  assert.equal(w.prey.get(7).sp, 6.153);
  w.packet(Uint8Array.from([106, 0, 7, 28, 32, 28, 32, 42, 245]), 250);
  assert.equal(w.prey.get(7).sp, 10.997);
  w.packet(Uint8Array.from([121, 0, 7]), 300);
  assert.equal(w.prey.has(7), false);
  w.packet(appearance, 400);
  assert.equal(w.prey.get(7).generation, 2);
});

function preyProbeBot() {
  const events = [];
  const bot = new Bot({ scenario: 'preyprobe' }, 0, { meta(e) { events.push(e); } });
  bot.world.self = 1;
  const me = { id: 1, pts: [{ x: 21600, y: 21600 }],
    ang: 0, wang: 0, sp: 0, sct: 6, fam: 0.5, t: 0 };
  bot.world.snakes.set(1, me);
  const prey = { id: 7, generation: 1, x: 22200, y: 21600,
    sp: 3.8, wang: 0, ang: 0, t: 100 };
  bot.world.prey.set(7, prey);
  return { bot, me, prey, events };
}

test('prey probe crosses an approach/retreat radius twice and boosts only safely', () => {
  const { bot, me, prey, events } = preyProbeBot();
  const steer = time => { const choice = { a: 1, danger: 0 }; bot.probePrey(time, me, choice); return choice; };
  assert.equal(PREY_PROBE.approachStop, 280);
  assert.equal(steer(100).a, 0);
  assert.equal(bot.phase, 'approach_cruise');
  prey.x = 21950; prey.t = 150;
  assert.equal(steer(150).a, 0);
  assert.equal(bot.phase, 'approach_cruise');
  prey.x = 21880; prey.t = 200;
  assert(Math.abs(steer(200).a - Math.PI) < 1e-9);
  assert.equal(bot.phase, 'retreat_boost');
  assert.equal(bot.preyIntent.desiredBoost, true);
  prey.x = 22300; prey.t = 1200;
  steer(1200);
  assert.equal(bot.phase, 'retreat_cruise');
  assert.equal(bot.preyIntent.desiredBoost, false);
  steer(1225);
  assert.equal(bot.phase, 'approach_cruise');
  prey.x = 21880; prey.t = 1300;
  steer(1300);
  assert.equal(bot.phase, 'retreat_boost');
  prey.x = 22300; prey.t = 2400;
  steer(2400);
  steer(2425);
  assert.equal(bot.preyTarget, null);
  assert.equal(events.at(-1).reason, 'two_crossings_complete');
});

test('prey probe refuses crowded targets and resets after visibility reentry', () => {
  const { bot, me, prey, events } = preyProbeBot();
  bot.world.snakes.set(2, { id: 2, pts: [{ x: prey.x + 200, y: prey.y }],
    ang: 0, sp: 0, sct: 2, t: 0 });
  const choice = { a: 1, danger: 0 };
  bot.probePrey(100, me, choice);
  assert.equal(bot.preyTarget, null);
  assert.equal(choice.a, 1);
  bot.world.snakes.delete(2);
  bot.probePrey(200, me, choice);
  assert.equal(bot.preyTarget?.generation, 1);
  prey.generation = 2; prey.t = 225;
  bot.probePrey(225, me, choice);
  assert.equal(bot.preyTarget, null);
  assert.equal(events.at(-1).reason, 'visibility_end_or_reentry');
});

test('prey probe CLI permits only one bounded client', () => {
  assert.equal(parseArgs(['--out', 'unused', '--scenario', 'preyprobe']).bots, 1);
  assert.throws(() => parseArgs(['--out', 'unused', '--scenario', 'preyprobe', '--bots', '2']), /one client/);
});

test('distant boosted prey approach releases boost before nearby approach', () => {
  const { bot, me, prey } = preyProbeBot();
  prey.x = 22400;
  const choice = { a: 1, danger: 0 };
  bot.probePrey(100, me, choice);
  assert.equal(bot.phase, 'approach_boost');
  assert.equal(bot.preyIntent.desiredBoost, true);
  prey.x = 22150; prey.t = 200;
  bot.probePrey(200, me, choice);
  assert.equal(bot.phase, 'approach_cruise');
  assert.equal(bot.preyIntent.desiredBoost, false);
  assert.equal(bot.preyIntent.distance, 550);
});

test('stale prey is censored and an unsafe intended heading yields to avoidance', () => {
  const { bot, me, prey, events } = preyProbeBot();
  const choice = { a: 1, danger: 0 };
  bot.probePrey(100, me, choice);
  bot.world.snakes.set(2, { id: 2, pts: [{ x: 21700, y: 21600 }],
    ang: 0, sp: 0, sct: 2, t: 0 });
  choice.a = 1;
  bot.probePrey(200, me, choice);
  assert.equal(bot.preyIntent.interruption, 'avoidance');
  assert.equal(bot.preyIntent.desiredBoost, false);
  assert.equal(choice.a, 1);
  bot.world.snakes.delete(2);
  bot.probePrey(3000, me, choice);
  assert.equal(bot.preyTarget, null);
  assert.equal(events.at(-1).reason, 'stale_target');
  assert.equal(prey.t, 100);
});
test('capture cancellation closes gzip without a false complete receipt', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-cancel-test-'));
  const originalConnect = Bot.prototype.connect;
  let connections = 0;
  Bot.prototype.connect = function () { connections++; this.rec.meta({ event: 'connect' }); };
  try {
    const controller = new AbortController();
    const capture = record({ out: directory, bots: 1, seconds: 60, scenario: 'forage', nick: 'Offline test' },
      { ip: '127.0.0.1', port: 0 }, 'bot', 1, controller.signal);
    controller.abort('SIGINT');
    const result = await capture;
    assert.equal(connections, 1);
    assert.equal(result.length, 1);
    assert.equal(getEventListeners(controller.signal, 'abort').length, 0);
    const rows = zlib.gunzipSync(fs.readFileSync(path.join(directory, 'bot0.jsonl.gz')))
      .toString('utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.equal(rows.at(-1).event, 'capture-interrupted');
    assert.equal(rows.at(-1).reason, 'SIGINT');
    assert.equal(rows.at(-1).requestedSeconds, 60);
    assert(!rows.some(row => row.event === 'capture-end'));
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('normal capture completion keeps the legacy terminal receipt', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-complete-test-'));
  const originalConnect = Bot.prototype.connect;
  Bot.prototype.connect = function () { this.rec.meta({ event: 'connect' }); };
  try {
    const controller = new AbortController();
    await record({ out: directory, bots: 1, seconds: 0.01, scenario: 'forage', nick: 'Offline test' },
      { ip: '127.0.0.1', port: 0 }, 'bot', 1, controller.signal);
    assert.equal(getEventListeners(controller.signal, 'abort').length, 0);
    const rows = zlib.gunzipSync(fs.readFileSync(path.join(directory, 'bot0.jsonl.gz')))
      .toString('utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.equal(rows.at(-1).event, 'capture-end');
    assert.equal(rows.at(-1).reason, undefined);
    assert.equal(rows.at(-1).requestedSeconds, undefined);
    assert(!rows.some(row => row.event === 'capture-interrupted'));
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('recorder receipt verifies the exact JSONL prefix and counts both directions', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-integrity-test-'));
  try {
    const file = path.join(directory, 'bot.jsonl.gz');
    const recorder = new Recorder(file, { schema: 1, t: 0 });
    recorder.frame('i', Uint8Array.from([0, 1, 112]), 1);
    recorder.frame('o', Uint8Array.from([251]), 2);
    recorder.meta({ event: 'test', text: 'non-ASCII: café' });
    recorder.finish({ event: 'capture-end', frames: 1 });
    assert.throws(() => recorder.meta({ event: 'too-late' }), /finalized/);
    await recorder.close();
    const payload = zlib.gunzipSync(fs.readFileSync(file));
    const lines = payload.toString('utf8').trimEnd().split('\n');
    const receipt = JSON.parse(lines.at(-1));
    const prefix = Buffer.from(lines.slice(0, -1).join('\n') + '\n');
    assert.equal(receipt.integrity.sha256, createHash('sha256').update(prefix).digest('hex'));
    assert.equal(receipt.integrity.payloadBytes, prefix.length);
    assert.equal(receipt.integrity.lines, 4);
    assert.deepEqual(receipt.integrity.counts, { i: { frames: 1, bytes: 3 }, o: { frames: 1, bytes: 1 } });
  } finally {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('paired game pong logs RTT and an unmatched pong cannot invent latency', () => {
  const events = [];
  const bot = new Bot({ scenario: 'forage' }, 0, { frame() {}, meta(event) { events.push(event); } });
  bot.ws = { readyState: 1, send() {} };
  const sentAt = bot.send([251]);
  bot.pendingPing = { t: sentAt };
  bot.awaitingPong = true;
  bot.receive(Uint8Array.from([0, 1, 112]));
  const measured = events.find(event => event.event === 'ping-rtt');
  assert.equal(measured.sentAt, sentAt);
  assert(measured.rttMs >= 0);
  assert.equal(measured.rttMs, measured.receivedAt - measured.sentAt);
  assert.equal(bot.pendingPing, null);
  assert.equal(bot.awaitingPong, false);
  bot.receive(Uint8Array.from([0, 1, 112]));
  assert.equal(events.filter(event => event.event === 'ping-rtt').length, 1);
  assert.equal(events.at(-1).event, 'ping-unmatched');
});

test('connection open timing and bounded socket errors are recorded', t => {
  const original = globalThis.WebSocket, events = [];
  globalThis.WebSocket = class { readyState = 1; send() {} close() {} };
  t.after(() => { globalThis.WebSocket = original; });
  const bot = new Bot({ scenario: 'forage' }, 0, { frame() {}, meta(event) { events.push(event); } });
  t.after(() => bot.stop());
  bot.connect({ ip: '127.0.0.1', port: 1 });
  bot.ws.onopen();
  assert(events.find(event => event.event === 'open').connectDurationMs >= 0);
  bot.ws.onerror({ message: 'x'.repeat(1000) });
  assert.equal(events.at(-1).event, 'socket-error');
  assert.equal(events.at(-1).message.length, 512);
});

test('stopping detaches socket errors and queued callbacks cannot write after finalization', async t => {
  const original = globalThis.WebSocket;
  globalThis.WebSocket = class { readyState = 1; send() {} close() {} };
  t.after(() => { globalThis.WebSocket = original; });
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-late-error-'));
  t.after(() => {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  });
  const archive = path.join(directory, 'bot.jsonl.gz');
  const recorder = new Recorder(archive, { schema: 1, t: 0 });
  const bot = new Bot({ scenario: 'forage' }, 0, recorder);
  bot.connect({ ip: '127.0.0.1', port: 1 });
  const socket = bot.ws, queuedError = socket.onerror;
  bot.pendingPing = { t: performance.now() };
  const pendingSentAt = bot.pendingPing.t;
  bot.stop();
  assert.equal(bot.pendingPing, null);
  assert.equal(socket.onerror, null);
  recorder.finish({ event: 'capture-end' });
  await recorder.close();
  assert.doesNotThrow(() => queuedError({ message: 'delayed close failure' }));
  const rows = zlib.gunzipSync(fs.readFileSync(archive)).toString('utf8').trim().split('\n').map(JSON.parse);
  assert.equal(rows.at(-1).event, 'capture-end');
  assert.equal(rows.filter(row => row.event === 'socket-error').length, 0);
  const censored = rows.filter(row => row.event === 'ping-censored');
  assert.equal(censored.length, 1);
  assert.equal(censored[0].sentAt, pendingSentAt);
  assert(censored[0].waitedMs >= 0);
});

test('recorder queue overflow is an explicit bounded acquisition failure', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-backlog-test-'));
  try {
    const recorder = new Recorder(path.join(directory, 'bot.jsonl.gz'), { schema: 1, t: 0 });
    recorder.queueLimitBytes = 1;
    assert.equal(recorder.frame('i', Uint8Array.from([0, 1, 112]), 1), false);
    const error = await recorder.failure;
    assert.match(error.message, /queue exceeds/);
    assert.equal(recorder.finish({ event: 'capture-end' }), false);
    await assert.rejects(recorder.close(), /queue exceeds/);
    assert.equal(recorder.counts.i.frames, 0);
  } finally {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('one recorder I/O failure aborts the batch without overwriting or false success', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-io-failure-test-'));
  const originalConnect = Bot.prototype.connect;
  Bot.prototype.connect = function () { this.rec.meta({ event: 'connect' }); };
  try {
    const existing = path.join(directory, 'bot1.jsonl.gz');
    fs.writeFileSync(existing, 'preserve existing bytes');
    await assert.rejects(record({ out: directory, bots: 2, seconds: 60, scenario: 'forage' },
      { ip: '127.0.0.1', port: 1 }, 'bot'), error => error.code === 'EEXIST');
    assert.equal(fs.readFileSync(existing, 'utf8'), 'preserve existing bytes');
    const rows = zlib.gunzipSync(fs.readFileSync(path.join(directory, 'bot0.jsonl.gz')))
      .toString('utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.equal(rows.at(-1).event, 'capture-interrupted');
    assert.match(rows.at(-1).reason, /EEXIST/);
    assert(!rows.some(row => row.event === 'capture-end'));
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('premature output close rejects completion rather than hanging', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-premature-close-test-'));
  try {
    const recorder = new Recorder(path.join(directory, 'bot.jsonl.gz'), { schema: 1, t: 0 });
    recorder.output.destroy();
    const error = await recorder.failure;
    assert.match(error.message, /closed before finish/);
    await assert.rejects(recorder.close(), /closed before finish/);
  } finally {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('simultaneous clients share a batch, retain unique capture IDs and pin the plan', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-provenance-test-'));
  const originalConnect = Bot.prototype.connect;
  Bot.prototype.connect = function () { this.rec.meta({ event: 'connect' }); };
  try {
    const plan = path.join(directory, 'plan.json');
    fs.writeFileSync(plan, '{"features":["frozen-before-acquisition"]}\n');
    await record({ out: directory, plan, bots: 2, seconds: 0.01, scenario: 'forage' },
      { ip: '127.0.0.1', port: 0 }, 'bot');
    const headers = [0, 1].map(index => JSON.parse(zlib.gunzipSync(
      fs.readFileSync(path.join(directory, `bot${index}.jsonl.gz`))).toString('utf8').split('\n')[0]));
    assert.equal(headers[0].captureProvenance.captureBatchId, headers[1].captureProvenance.captureBatchId);
    assert.notEqual(headers[0].captureProvenance.captureId, headers[1].captureProvenance.captureId);
    assert.equal(headers[0].captureProvenance.planSha256, createHash('sha256').update(fs.readFileSync(plan)).digest('hex'));
    assert.deepEqual(headers.map(header => header.captureProvenance.botIndex), [0, 1]);
    assert.equal(headers[0].captureProvenance.botCount, 2);
    assert.equal(headers[0].captureProvenance.options.out, undefined);
    assert.match(headers[0].captureProvenance.collectorSha256, /^[a-f0-9]{64}$/);
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('already cancelled capture creates no clients or archive', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-preabort-test-'));
  const originalConnect = Bot.prototype.connect;
  let connections = 0;
  Bot.prototype.connect = function () { connections++; };
  try {
    const controller = new AbortController();
    controller.abort('SIGTERM');
    const result = await record({ out: directory, bots: 1, seconds: 60, scenario: 'forage' },
      { ip: '127.0.0.1', port: 0 }, 'bot', 1, controller.signal);
    assert.deepEqual(result, []);
    assert.equal(connections, 0);
    assert.deepEqual(fs.readdirSync(directory), []);
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
test('CLI interrupt flushes the archive and exits 130 without network access', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-cli-stop-test-'));
  try {
    const preload = path.join(directory, 'offline-websocket.mjs');
    const output = path.join(directory, 'capture');
    fs.writeFileSync(preload, [
      'globalThis.WebSocket = class {',
      '  constructor() { this.readyState = 0; setTimeout(() => process.emit("SIGINT"), 10); }',
      '  send() {}',
      '  close() { this.readyState = 3; }',
      '};',
    ].join('\n'), { flag: 'wx' });
    const result = spawnSync(process.execPath, [
      '--import', pathToFileURL(preload).href,
      fileURLToPath(new URL('./slither-headless.mjs', import.meta.url)),
      '--out', output, '--seconds', '60', '--server', '127.0.0.1:1',
    ], { encoding: 'utf8', timeout: 5000 });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 130, result.stderr);
    assert.match(result.stdout, /interrupted/);
    const rows = zlib.gunzipSync(fs.readFileSync(path.join(output, 'bot0.jsonl.gz')))
      .toString('utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.equal(rows.at(-1).event, 'capture-interrupted');
    assert.equal(rows.at(-1).reason, 'SIGINT');
    assert(!rows.some(row => row.event === 'capture-end'));
  } finally {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
test('reentrant cancellation cannot start additional requested clients', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-reentrant-stop-test-'));
  const originalConnect = Bot.prototype.connect;
  const controller = new AbortController();
  let connections = 0;
  Bot.prototype.connect = function () { connections++; controller.abort('SIGINT'); };
  try {
    const result = await record({ out: directory, bots: 3, seconds: 60, scenario: 'forage' },
      { ip: '127.0.0.1', port: 0 }, 'bot', 3, controller.signal);
    assert.equal(connections, 1);
    assert.equal(result.length, 1);
    assert.deepEqual(fs.readdirSync(directory), ['bot0.jsonl.gz']);
    const rows = zlib.gunzipSync(fs.readFileSync(path.join(directory, 'bot0.jsonl.gz')))
      .toString('utf8').trim().split('\n').map(line => JSON.parse(line));
    assert.equal(rows.at(-1).event, 'capture-interrupted');
    assert(!rows.some(row => row.event === 'capture-end'));
    assert.equal(getEventListeners(controller.signal, 'abort').length, 0);
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

const pairedCollectors = [
  { name: 'slither-paired-death', run: recordDeath, parse: parseDeath, serverBody: true },
  { name: 'slither-paired-prey', run: recordPrey, parse: parsePrey, serverBody: undefined },
  { name: 'slither-feeding-shape', run: recordFeedingShape, parse: parseFeeding, serverBody: true },
];

function readCollectorArchive(file) {
  const payload = zlib.gunzipSync(fs.readFileSync(file));
  const lines = payload.toString('utf8').trimEnd().split('\n');
  const rows = lines.map(line => JSON.parse(line));
  const terminal = rows.at(-1), prefix = Buffer.from(lines.slice(0, -1).join('\n') + '\n');
  assert.equal(terminal.integrity.format, 'sha256-jsonl-prefix-v1');
  assert.equal(terminal.integrity.sha256, createHash('sha256').update(prefix).digest('hex'));
  assert.equal(terminal.integrity.lines, rows.length - 1);
  assert.equal(terminal.integrity.payloadBytes, prefix.length);
  for (const direction of ['i', 'o']) {
    const frames = rows.filter(row => Array.isArray(row) && row[0] === direction);
    assert.deepEqual(terminal.integrity.counts[direction], { frames: frames.length,
      bytes: frames.reduce((total, row) => total + Buffer.from(row[2], 'base64').length, 0) });
  }
  return rows;
}

test('all real collectors emit grouped provenance and exact receipts accepted by the quality gate', async context => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-collectors-success-'));
  const originalConnect = Bot.prototype.connect;
  Bot.prototype.connect = function (server) {
    this.joins = 1; this.frames = 1;
    this.rec.meta({ event: 'connect', server });
    const setup = Buffer.alloc(32);
    setup[0] = 255; setup[1] = 255; setup[2] = 97; setup[25] = 19; setup[26] = 42;
    setup.writeUIntBE(100, 29, 3);
    this.rec.frame('i', setup);
    this.rec.frame('o', Uint8Array.from([this.index]));
  };
  try {
    const plan = path.join(directory, 'plan.json');
    const planBytes = Buffer.from('{ "cohort": "offline", "dataOrigin": "local-simulator", "unicode": "雪" }\r\n');
    fs.writeFileSync(plan, planBytes);
    const allFiles = [];
    const cases = [...pairedCollectors, ...[2, 3].map(bots => ({ name: `headless-${bots}`, bots,
      run: opts => record({ ...opts, bots, seconds: .01, scenario: 'forage' },
        { ip: '127.0.0.1', port: 444 }, 'bot') }))];
    for (const collector of cases) await context.test(collector.name, async () => {
      const out = path.join(directory, collector.name);
      if (collector.bots) fs.mkdirSync(out);
      await collector.run({ out, plan, server: '127.0.0.1:444', seconds: 1 });
      const files = Array.from({ length: collector.bots ?? 2 }, (_, index) => path.join(out, `bot${index}.jsonl.gz`));
      allFiles.push(...files);
      const archives = files.map(readCollectorArchive);
      assert(archives.every(rows => rows[0].dataOrigin === 'local-simulator'));
      const headers = archives.map(rows => rows[0].captureProvenance);
      assert.equal(new Set(headers.map(header => header.captureBatchId)).size, 1);
      assert.equal(new Set(headers.map(header => header.captureId)).size, files.length);
      assert.deepEqual(headers.map(header => header.botIndex), files.map((_, index) => index));
      for (const [index, header] of headers.entries()) {
        assert.equal(header.botCount, files.length);
        assert.equal(header.options.bots, files.length);
        assert.equal(header.options.out, undefined);
        assert.equal(header.options.plan, undefined);
        assert.equal(header.options.serverBody, collector.serverBody);
        assert.equal(header.planSha256, createHash('sha256').update(planBytes).digest('hex'));
        const driver = collector.bots ? 'slither-headless' : collector.name;
        assert.equal(header.collectorSha256, createHash('sha256').update(fs.readFileSync(
          fileURLToPath(new URL(`./${driver}.mjs`, import.meta.url)))).digest('hex'));
        const terminal = archives[index].at(-1);
        assert.equal(terminal.event, 'capture-end');
        assert.equal(terminal.collectorEndSha256, header.collectorSha256);
        assert.equal(terminal.headlessEndSha256, header.headlessSha256);
        assert.equal(terminal.collectorSourcesUnchanged, true);
        assert.equal(terminal.collectorTiming.targetTickMs, 25);
      }
    });
    const audited = spawnSync('python', ['-c',
      'import json,sys; sys.path.insert(0,sys.argv[1]); from capture_quality import audit_captures; print(json.dumps(audit_captures(sys.argv[2:])))',
      fileURLToPath(new URL('.', import.meta.url)), ...allFiles], { encoding: 'utf8', timeout: 10000 });
    assert.equal(audited.status, 0, audited.stderr);
    const quality = JSON.parse(audited.stdout);
    assert.deepEqual(quality.captures.map(capture => capture.status), allFiles.map(() => 'verified'));
    assert(quality.batches.every(batch => batch.completeMembership && batch.matchedProvenance && batch.eligibleForPlannedInference));
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('paired collectors cancel, observe immediate writer failures and preserve non-atomic siblings', async context => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-collectors-failure-'));
  const originalConnect = Bot.prototype.connect;
  try {
    for (const collector of pairedCollectors) {
      for (const mode of ['cancel', 'reentrant', 'queue', 'close', 'existing', 'setup', 'tick']) {
        await context.test(`${collector.name}: ${mode}`, async () => {
          const out = path.join(directory, `${collector.name}-${mode}`);
          const controller = new AbortController(), bots = [];
          const originalError = new Error('original setup failure');
          Bot.prototype.connect = function () {
            bots.push(this); this.joins = 1;
            this.rec.meta({ event: 'connect' });
            if (this.index !== 0) return;
            if (mode === 'reentrant') controller.abort('SIGTERM');
            if (mode === 'queue') {
              this.rec.queueLimitBytes = 1;
              this.rec.frame('i', Uint8Array.from([1]));
            }
            if (mode === 'close') this.rec.output.destroy();
            if (mode === 'existing') fs.writeFileSync(path.join(out, 'bot0.jsonl.gz'), 'keep original', { flag: 'wx' });
            if (mode === 'setup') throw originalError;
            if (mode === 'tick') this.tick = () => { throw originalError; };
          };
          const started = performance.now();
          const capture = collector.run({ out, server: '127.0.0.1:444', seconds: 60 }, controller.signal);
          if (mode === 'cancel') controller.abort('SIGINT');
          if (['cancel', 'reentrant'].includes(mode)) await capture;
          else await assert.rejects(capture, error => {
            if (['setup', 'tick'].includes(mode)) return error === originalError;
            if (mode === 'existing') return error.code === 'EEXIST';
            return /queue exceeds|closed before finish/.test(error.message);
          });
          assert(performance.now() - started < 2000, 'writer failure must not wait for deadline');
          assert(bots.every(bot => bot.stopped));
          assert.equal(getEventListeners(controller.signal, 'abort').length, 0);
          if (mode === 'existing') assert.equal(fs.readFileSync(path.join(out, 'bot0.jsonl.gz'), 'utf8'), 'keep original');
          const healthy = ['queue', 'close', 'existing'].includes(mode) ? [1] : [0, 1];
          for (const index of healthy) {
            const rows = readCollectorArchive(path.join(out, `bot${index}.jsonl.gz`));
            assert.equal(rows.at(-1).event, 'capture-interrupted');
            assert(!rows.some(row => row.event === 'capture-end'));
            if (['cancel', 'reentrant'].includes(mode))
              assert.equal(rows.at(-1).reason, mode === 'cancel' ? 'SIGINT' : 'SIGTERM');
          }
        });
      }
      const before = path.join(directory, `${collector.name}-already-aborted`);
      const controller = new AbortController(); controller.abort('SIGINT');
      await collector.run({ out: before }, controller.signal);
      assert.equal(fs.existsSync(before), false);
    }
  } finally {
    Bot.prototype.connect = originalConnect;
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('plan pins raw bytes before creation and source stability includes both import-time identities', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-collector-contract-'));
  try {
    const plan = path.join(directory, 'plan.json'), driver = path.join(directory, 'driver.mjs');
    fs.writeFileSync(plan, '{ "scenario": "forage" }\r\n');
    fs.writeFileSync(driver, 'original source');
    const source = captureSourceIdentity(pathToFileURL(driver));
    const batch = createCaptureBatch({ plan, scenario: 'forage' }, 2, source);
    fs.writeFileSync(plan, '{"scenario":"forage"}');
    const provenance = batch.provenance(0);
    assert.equal(provenance.planSha256, createHash('sha256').update('{ "scenario": "forage" }\r\n').digest('hex'));
    fs.writeFileSync(driver, 'changed source');
    const file = path.join(directory, 'capture.gz');
    const recorder = new Recorder(file, { schema: 1, captureProvenance: provenance });
    await finalizeCaptureBatch({ bots: [], recorders: [recorder], source, completed: true });
    const terminal = readCollectorArchive(file).at(-1);
    assert.equal(terminal.collectorSourcesUnchanged, false);
    assert.notEqual(terminal.collectorEndSha256, provenance.collectorSha256);
    assert.equal(terminal.headlessEndSha256, provenance.headlessSha256);
    const importedChanged = { ...source, collectorSha256: terminal.collectorEndSha256, headlessSha256: '0'.repeat(64) };
    const second = path.join(directory, 'headless-changed.gz');
    await finalizeCaptureBatch({ bots: [], recorders: [new Recorder(second, { schema: 1 })],
      source: importedChanged, completed: true });
    assert.equal(readCollectorArchive(second).at(-1).collectorSourcesUnchanged, false);
    for (const collector of pairedCollectors) {
      for (const invalid of [path.join(directory, 'missing'), plan]) {
        if (invalid === plan) fs.writeFileSync(plan, 'not JSON');
        const out = path.join(directory, `${collector.name}-${path.basename(invalid)}`);
        await assert.rejects(collector.run({ out, server: '127.0.0.1:444', seconds: 1, plan: invalid }));
        assert.equal(fs.existsSync(out), false);
      }
    }
    fs.writeFileSync(plan, ' '.repeat(262145));
    assert.throws(() => createCaptureBatch({ plan }, 2), /256 KiB/);
    assert.throws(() => batch.provenance(2), /bot index/);
    for (const parser of [parseArgs, ...pairedCollectors.map(collector => collector.parse)]) {
      assert.equal(parser(['--out', 'unused', '--plan', 'raw.json']).plan, 'raw.json');
      for (const invalid of [['--unknown', '1'], ['--plan'], ['--plan', '--out', 'unused'],
        ['--out', 'a', '--out', 'b'], ['out', 'unused']]) assert.throws(() => parser(invalid));
    }
  } finally {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('paired CLI signals drain both archives without any network', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-paired-cli-stop-'));
  try {
    const preload = path.join(directory, 'offline-websocket.mjs');
    fs.writeFileSync(preload, 'globalThis.WebSocket = class { constructor() { this.readyState = 0; setTimeout(() => process.emit("SIGTERM"), 10); } send() {} close() { this.readyState = 3; } };');
    for (const collector of pairedCollectors) {
      const out = path.join(directory, collector.name);
      const result = spawnSync(process.execPath, ['--import', pathToFileURL(preload).href,
        fileURLToPath(new URL(`./${collector.name}.mjs`, import.meta.url)),
        '--out', out, '--seconds', '60', '--server', '127.0.0.1:444'], { encoding: 'utf8', timeout: 5000 });
      assert.equal(result.status, 143, result.stderr);
      for (const index of [0, 1]) {
        const rows = readCollectorArchive(path.join(out, `bot${index}.jsonl.gz`));
        assert.equal(rows.at(-1).event, 'capture-interrupted');
        assert.equal(rows.at(-1).reason, 'SIGTERM');
      }
    }
  } finally {
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
