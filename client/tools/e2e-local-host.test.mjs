import test from 'node:test';
import assert from 'node:assert/strict';
import { argumentsFor, allowedPort, readyEndpoint, assertSummary } from './e2e-local-host.mjs';

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
  assert.throws(() => assertSummary({ ...complete(), steeringCommands: Infinity }), { stage: 'steering' });
});
