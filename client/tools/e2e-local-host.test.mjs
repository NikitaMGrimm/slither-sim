import test from 'node:test';
import assert from 'node:assert/strict';
import { argumentsFor, allowedPort, readyEndpoint, assertSummary, runWithCleanup } from './e2e-local-host.mjs';

test('arguments require a host, accept bounded seconds and prebuilt paths, reject ambiguous input', () => {
  assert.equal(argumentsFor(['--host-bin', 'host']).seconds, 8);
  const options = argumentsFor(['--host-bin', 'host', '--client-bin', 'client', '--seconds', '5.5']);
  assert.equal(options.seconds, 5.5);
  assert.match(options.clientBin, /client$/);
  for (const args of [[], ['--host-bin'], ['--wat', 'host'], ['--host-bin', 'host', '--host-bin', 'other'],
    ...['0', '-1', '4.9', '61', 'NaN', 'Infinity', ''].map(s => ['--host-bin', 'host', '--seconds', s])]) {
    assert.throws(() => argumentsFor(args), { stage: 'arguments' });
  }
});

test('port policy rejects forbidden, zero and malformed ports', () => {
  for (const port of [0, -1, 65536, 1.5, NaN, '49152', 8766, 18884, 18885]) assert.equal(allowedPort(port), false);
  for (const port of [32768, 49152, 65535]) assert.equal(allowedPort(port), true);
});

test('Battledome requires full-map pictures without skips and preserves measured subscription counts', () => {
  const options = argumentsFor(['--host-bin', 'host', '--battledome', '500', '--scratch', 'scratch']);
  assert.equal(options.battledome, 500);
  assert.equal(options.seconds, 20);
  assert.throws(() => argumentsFor(['--host-bin', 'host', '--battledome', '501']), { stage: 'arguments' });
  const summary = { ...complete(), fullMap: true, mapFitSamples: 12, bestMapOverflowPixels: 0, worstMapOverflowPixels: 0,
    mapCameraMoved: true, mapCameraMaxDisplacementWorldUnits: 20,
    pictureSkips: 0, peakReceivedSnakes: 501, peakPreparedSnakes: 501 };
  assert.equal(assertSummary(summary, 19, true), summary);
  for (const count of [7, 256, 499]) {
    assert.throws(() => assertSummary({ ...summary, peakReceivedSnakes: count }, 19, true), { stage: "population" });
    assert.throws(() => assertSummary({ ...summary, peakPreparedSnakes: count }, 19, true), { stage: "population" });
  }
  assert.equal(argumentsFor(["--host-bin", "host", "--battledome", "500", "--vision", "1500"]).vision, 1500);
  assert.throws(() => assertSummary({ ...summary, fullMap: false }, 19, true), { stage: 'full-map' });
  for (const patch of [{mapFitSamples: 0}, {worstMapOverflowPixels: null}, {worstMapOverflowPixels: 3.56}]) {
    assert.throws(() => assertSummary({ ...summary, ...patch }, 19, true), { stage: 'full-map' });
  }
  assert.throws(() => assertSummary({ ...summary, pictureSkips: 1 }, 19, true), { stage: 'picture-skips' });
});

test('Battledome rejects stationary post-zoom summaries despite earlier snake movement', () => {
  const summary = { ...complete(), fullMap: true, mapFitSamples: 3, bestMapOverflowPixels: 0, worstMapOverflowPixels: 0,
    pictureSkips: 0, peakReceivedSnakes: 501, peakPreparedSnakes: 501,
    mapCameraMoved: true, mapCameraMaxDisplacementWorldUnits: 20 };
  assert.equal(assertSummary(summary, 19, true), summary);
  for (const patch of [{mapCameraMoved: false, mapCameraMaxDisplacementWorldUnits: 0},
    {mapCameraMaxDisplacementWorldUnits: 0}, {mapCameraMaxDisplacementWorldUnits: null}, {mapFitSamples: 1}]) {
    assert.throws(() => assertSummary({ ...summary, ...patch }, 19, true), {stage: 'camera-motion'});
  }
  assert.throws(() => assertSummary({ ...summary, bestMapOverflowPixels: 0, worstMapOverflowPixels: 4,
    fullMap: false }, 19, true), {stage: 'full-map'}, 'authored [3.56, 0, 4] must fail');
});

test('readiness requires a complete exact loopback announcement, independent of log chunking', () => {
  const line = 'Listening on ws://127.0.0.1:49152/slither\r\n';
  assert.equal(readyEndpoint('warning\n' + line), 'ws://127.0.0.1:49152/slither');
  assert.equal(readyEndpoint(line.slice(0, -4)), null);
  assert.equal(readyEndpoint('not ' + line), null);
  for (const address of ['0.0.0.0:49152', 'localhost:49152', '192.0.2.1:49152', '127.0.0.1:0',
    '127.0.0.1:8766', '127.0.0.1:18884', '127.0.0.1:18885', '127.0.0.1:65536']) {
    assert.throws(() => readyEndpoint(`Listening on ws://${address}/slither\n`), { stage: 'readiness' });
  }
});

const complete = () => ({ websocket: true, challenge: true, loginSent: true, protocol: 19, setup: true,
  ownSnake: true, steeringCommands: 3, boostOn: true, boostOff: true, moved: true, turned: true,
  accelerated: true, decelerated: true, leaderboard: true, minimap: true, preparedFrames: 5, cleanClose: true });

test('cleanup failure preserves the original session error, stage and partial summary', async () => {
  const client = { ...complete(), cleanClose: false };
  const sessionError = Object.assign(new Error('session failed'), { stage: 'close', client });
  const cleanupError = new Error('SIGKILL timeout');
  const calls = [];
  await assert.rejects(runWithCleanup(async () => {
    calls.push('session'); throw sessionError;
  }, async () => {
    await Promise.resolve(); calls.push('cleanup'); throw cleanupError;
  }), error => {
    assert.equal(error, sessionError);
    assert.equal(error.stage, 'close');
    assert.equal(error.client, client);
    assert.equal(error.cleanupError, cleanupError);
    return true;
  });
  assert.deepEqual(calls, ['session', 'cleanup']);
});

test('cleanup is awaited after success and surfaces its own error only without a session failure', async () => {
  const result = { ok: true };
  let cleaned = false;
  assert.equal(await runWithCleanup(async () => result, async () => {
    await Promise.resolve(); cleaned = true;
  }), result);
  assert.equal(cleaned, true);
  const cleanupError = new Error('cleanup failed');
  await assert.rejects(runWithCleanup(async () => result, async () => { throw cleanupError; }), error => error === cleanupError);
  await assert.rejects(runWithCleanup(async () => { throw undefined; }, async () => { throw cleanupError; }), error => error === undefined);
});

test('summary assertions require every observed stage, including server responses and close', () => {
  const summary = complete();
  assert.equal(assertSummary(summary), summary);
  const stages = { websocket: 'handshake', challenge: 'handshake', loginSent: 'login', protocol: 'setup', setup: 'setup',
    ownSnake: 'own-snake', steeringCommands: 'steering', moved: 'steering', turned: 'steering', boostOn: 'boost',
    boostOff: 'boost', accelerated: 'boost', decelerated: 'boost', leaderboard: 'leaderboard', minimap: 'minimap',
    preparedFrames: 'presentation', cleanClose: 'close' };
  for (const [key, stage] of Object.entries(stages)) {
    const missing = complete(); delete missing[key];
    assert.throws(() => assertSummary(missing), { stage });
    assert.throws(() => assertSummary({ ...complete(), [key]: 0 }), { stage });
  }
  assert.throws(() => assertSummary({ ...complete(), protocol: 13 }), { stage: 'setup' });
  assert.equal(assertSummary({ ...complete(), protocol: 13 }, 13).protocol, 13);
  assert.throws(() => assertSummary({ ...complete(), steeringCommands: Infinity }), { stage: 'steering' });
});
