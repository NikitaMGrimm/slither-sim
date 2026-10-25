import assert from 'node:assert/strict';
import { test } from 'node:test';
import { EndpointRun, installSocketPolicy } from './capture-continuation.mjs';

function bot(deathObservation = null) {
  const value = { stopped: false, deathObservation, reconnectTimer: null, ws: {} };
  value.ws.onclose = () => { value.reconnectTimer = setTimeout(() => {}, 10000); };
  value.ws.onerror = () => {};
  return value;
}

test('a failed endpoint cannot cancel its healthy sibling or schedule a retry', () => {
  const low = new EndpointRun(), second = new EndpointRun(), client = bot();
  installSocketPolicy(client, low, 'low');
  client.ws.onclose({ code: 1006 });
  assert.equal(low.controller.signal.aborted, true);
  assert.equal(second.controller.signal.aborted, false);
  assert.equal(client.reconnectTimer._destroyed, true);
  assert.match(String(low.error), /no retry/);
});

test('socket error aborts only its endpoint and preserves its first failure', () => {
  const first = new EndpointRun(), second = new EndpointRun(), client = bot();
  installSocketPolicy(client, second, 'second');
  client.ws.onerror({});
  const error = second.error;
  client.ws.onclose({ code: 1006 });
  assert.equal(second.error, error);
  assert.equal(first.controller.signal.aborted, false);
});

test('normal death close may rejoin; abnormal death close stops the endpoint', () => {
  for (const code of [1000, 1001, 1005, 1006, 1011]) {
    const run = new EndpointRun(), client = bot({});
    installSocketPolicy(client, run, 'endpoint');
    client.ws.onclose({ code });
    assert.equal(run.controller.signal.aborted, [1006, 1011].includes(code));
    clearTimeout(client.reconnectTimer);
  }
});

test('shutdown errors do not turn a completed batch into an endpoint failure', () => {
  const run = new EndpointRun(), client = bot(); client.stopped = true;
  installSocketPolicy(client, run, 'endpoint');
  client.ws.onerror({}); client.ws.onclose({ code: 1006 });
  assert.equal(run.controller.signal.aborted, false);
  clearTimeout(client.reconnectTimer);
});
