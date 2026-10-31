import assert from 'node:assert/strict';
import { test } from 'node:test';
import { HandshakeSession, unpackCommands } from '../web/handshake-session.mjs';
const probeNickname = 'Synthetic';

const packed = commands => Uint8Array.from(commands.flatMap(command => [0, 0, 0, command.length, ...command]));
const fakeWasm = () => ({ encode_login: () => Uint8Array.of(115, 0), solve_challenge: () => new Uint8Array(27), decode_frame: bytes => bytes, setup_protocol_version: command => command[1] });
const profile = { clientVersion: 1, pageEcho: new Uint8Array(20), protocolVersion: 19 };

test('synthetic handshake emits only negotiation, challenge reply, and login before setup', () => {
  const session = new HandshakeSession(fakeWasm(), profile, probeNickname);
  assert.deepEqual(session.start().map(bytes => [...bytes]), [[1], [99, 0]]);
  const reply = session.receive(packed([[54, 1]]));
  assert.equal(reply.outbound.length, 2); assert.equal(reply.complete, false);
  const setup = session.receive(packed([[97, 19], [115, 1]]));
  assert.equal(setup.complete, true); assert.equal(setup.protocolVersion, 19); assert.deepEqual(setup.outbound, []);
  assert.throws(() => session.receive(packed([[97, 19]])), /terminal/);
});

test('variable-length challenge replies are sent unchanged and empty or oversized ones fail', () => {
  for (const length of [24, 27, 256]) {
    const reply = Uint8Array.from({ length }, (_, i) => 65 + i % 26);
    const session = new HandshakeSession(fakeWasm(), profile, probeNickname, { solve: () => reply });
    session.start();
    const result = session.receive(packed([[54, 1]]));
    assert.deepEqual([...result.outbound[0]], [...reply]);
  }
  for (const length of [0, 257]) {
    const session = new HandshakeSession(fakeWasm(), profile, probeNickname, { solve: () => new Uint8Array(length) });
    session.start();
    assert.throws(() => session.receive(packed([[54, 1]])), /Invalid challenge reply/);
    assert.equal(session.state, 'failed');
  }
});

test('complete-frame validation and causal ordering precede any challenge callback', () => {
  let solved = 0;
  const session = new HandshakeSession(fakeWasm(), profile, probeNickname, { solve() { solved++; return new Uint8Array(27); } });
  assert.throws(() => session.receive(packed([[54, 1], [97, 19]])), /one challenge/);
  assert.equal(solved, 0); assert.equal(session.state, 'failed');
  assert.throws(() => unpackCommands(Uint8Array.from([...packed([[54, 1]]), 0, 0])), /Truncated/);
});

test('unexpected protocol and repeated challenges fail closed', () => {
  for (const next of [[97, 20], [54, 1]]) {
    const session = new HandshakeSession(fakeWasm(), profile, probeNickname);
    session.receive(packed([[54, 1]]));
    assert.throws(() => session.receive(packed([next])));
    assert.equal(session.state, 'failed');
  }
});
