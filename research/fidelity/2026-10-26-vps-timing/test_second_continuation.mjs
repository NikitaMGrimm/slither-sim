import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Bot } from '../../../tools/fidelity/slither-headless.mjs';
import { EndpointRun, installSocketPolicy } from './capture-second-continuation.mjs';

test('requested post-death error/1006 closes a life and the base collector rejoins', () => {
  const saved = { WebSocket: globalThis.WebSocket, setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout };
  const timers = [], sockets = [], events = [];
  globalThis.setTimeout = (callback, ms) => { const timer = { callback, ms }; timers.push(timer); return timer; };
  globalThis.clearTimeout = timer => { if (timer) timer.cleared = true; };
  globalThis.WebSocket = class {
    readyState = 1;
    constructor() { sockets.push(this); }
    close() { this.readyState = 3; this.onerror?.({ message: '' }); this.onclose?.({ code: 1006, reason: '' }); }
  };
  const run = new EndpointRun(), sibling = new EndpointRun();
  const bot = new Bot({ scenario: 'forage' }, 0, { meta: row => events.push(row), frame: () => {} });
  const connect = Bot.prototype.connect;
  bot.connect = function(server) { connect.call(this, server); installSocketPolicy(this, run, 'test'); };
  try {
    bot.connect({ ip: '127.0.0.1', port: 444 });
    bot.connectedAt = performance.now() - 20000;
    bot.deathObservation = { startedAt: performance.now() - 1001, serverClock: null, frame: 0, ended: false };
    bot.tick(performance.now());
    assert.deepEqual(events.filter(r => ['death-observation-close-requested', 'socket-error', 'death-observation-end', 'close'].includes(r.event)).map(r => r.event),
      ['death-observation-close-requested', 'socket-error', 'death-observation-end', 'close']);
    assert.equal(run.controller.signal.aborted, false);
    assert.equal(sibling.controller.signal.aborted, false);
    assert.equal(timers.length, 1);
    assert.equal(timers[0].ms, 3000);
    assert.equal(timers[0].cleared, undefined);
    timers[0].callback();
    assert.equal(bot.lives, 2);
    assert.equal(sockets.length, 2);
    assert.equal(bot.deathObservation, null);
    // A new life's unrequested error must not inherit the previous socket's close intent.
    sockets[1].onerror({ message: 'unrequested' });
    sockets[1].onclose({ code: 1006, reason: '' });
    assert.equal(run.controller.signal.aborted, true);
    assert.equal(timers[1].cleared, true);
    assert.equal(sibling.controller.signal.aborted, false);
  } finally {
    bot.stop();
    Object.assign(globalThis, saved);
  }
});

test('unrequested errors and closes stop the endpoint even during death observation', () => {
  for (const observation of [null, { startedAt: 1 }]) {
    for (const event of ['error', 'close']) {
      let timer;
      const bot = { stopped: false, deathObservation: observation, ws: {
        onerror: () => {}, onclose: () => { timer = bot.reconnectTimer = setTimeout(() => {}, 10000); }
      } };
      const run = new EndpointRun();
      installSocketPolicy(bot, run, 'endpoint');
      if (event === 'error') bot.ws.onerror({});
      bot.ws.onclose({ code: event === 'error' ? 1006 : 1000 });
      assert.equal(run.controller.signal.aborted, true);
      assert.equal(timer._destroyed, true);
      clearTimeout(timer);
    }
  }
});

test('close code does not override recorded requested post-death intent', () => {
  for (const code of [1000, 1001, 1005, 1006, 1011]) {
    const run = new EndpointRun();
    const bot = { stopped: false, deathObservation: { closeRequestedAt: 0 }, ws: { onerror: () => {}, onclose: () => {} } };
    installSocketPolicy(bot, run, 'endpoint');
    bot.ws.onerror({}); bot.ws.onclose({ code });
    assert.equal(run.controller.signal.aborted, false);
  }
});
