import test from 'node:test';
import assert from 'node:assert/strict';
import { ArcSelector, durations, decision, key, reserveOrdinal, primaryAgreement } from './selection.mjs';
import { Acquisition, pairStatus, TurnBot } from './capture.mjs';
import { rate, scores, select } from './score.mjs';

function bots(n = 37) {
  return [0, 1].map(i => {
    const self = i + 1, s = { id: self, sct: n, pts: [{ x: i * 550, y: 0 }], t: 2000, ang: 0, sp: 5.8 };
    return { lives: 1, connectedAt: 0, ws: { readyState: 1 }, world: {
      self, snakes: new Map([[self, s], [3 - self, { id: 3 - self, pts: [{ x: (1 - i) * 550, y: 0 }], t: 2000 }]]),
    } };
  });
}

test('paired gate needs positive, fresh same-life receipt and bounded separation', () => {
  const b = bots(); assert(pairStatus(b, 2000).ready);
  assert(!pairStatus(b, 2301).ready);
  b[1].world.snakes.delete(1); assert(!pairStatus(b, 2000).ready);
  const c = bots(); c[0].connectedAt = 2100; assert(!pairStatus(c, 2000).ready);
});
test('count changes, overshoots, safety interruptions and even direction order persist', () => {
  const state = new Acquisition(2, () => {}), b = bots();
  state.update(b, 2000, 1200000); assert.equal(state.sign, -1); assert(!state.ready);
  b.forEach(p => p.world.snakes.get(p.world.self).t = 3100);
  b[1].world.snakes.get(1).t = 3100;
  state.update(b, 3100, 1200000); assert(state.ready);
  state.anchor({ x: 0, y: 0, count: 37, speed: 5.8, wanted: 1, server: 2000 }, 3100);
  state.interrupt('avoidance'); assert.equal(state.selector.anchors.length, 0);
  b[0].world.snakes.get(1).sct = 38;
  state.update(b, 3200, 1200000); assert.equal(state.target, 100);
  assert(state.passed.has(37)); assert(!state.ready);
});
test('selector takes first sufficient signed arc; invalid neighbors reset', () => {
  for (const sign of [1, -1]) {
    const selector = new ArcSelector(37, sign, 2);
    let result;
    for (let i = 0; i < 100 && !result; i++) {
      const a = sign * .4 * i, r = 42 / (2 * Math.sin(.2));
      result = selector.push({ x: r * Math.cos(a), y: r * Math.sin(a),
        index: i, server: i * 110, wanted: a + sign * (Math.PI / 2 + 1.4), count: 37, speed: 6, qualified: true });
    }
    assert(result); assert(result.seconds >= 2); assert(result.seconds < 2.12);
    assert.equal(Math.sign(result.angle), sign);
  }
  assert.deepEqual([...durations.values()], [25, 60, 340]);
});
test('hard ledger consumes failures, limits four, and rejects holes', () => {
  assert.equal(reserveOrdinal([]), 1);
  assert.equal(reserveOrdinal([{ ordinal: 1, failed: true }]), 2);
  assert.throws(() => reserveOrdinal([{ ordinal: 2 }]));
  assert.throws(() => reserveOrdinal([1, 2, 3, 4].map(ordinal => ({ ordinal }))));
});
test('decision needs complete paired held-out replication with one shared offset', () => {
  const cells = compatible => Object.fromEntries([37, 100].flatMap(n => [1, -1].map(s => [key(n, s), { compatible }])));
  const dev = [{ ordinal: 1, cells: cells([]) }, { ordinal: 2, cells: {} }];
  assert.equal(decision([{ ordinal: 1, cells: {} }]), 'inconclusive-development-coverage');
  const held = [3, 4].map(ordinal => ({ ordinal, qualified: true, cells: cells(['k6']) }));
  assert.equal(decision([...dev, ...held]), 'reject-D111-fixed-offset-survives');
  held[1].cells = cells(['k7']); assert.equal(decision([...dev, ...held]), 'inconclusive-candidates');
  held.forEach(r => r.cells = cells(['D111']));
  held[0].cells['174/1'] = { count: 174, observerAgreement: false, compatible: [] };
  assert(primaryAgreement({ first: { count: 37, observerAgreement: true },
    optional: { count: 174, observerAgreement: false } }));
  assert(!primaryAgreement({ first: { count: 37, observerAgreement: false } }));
  assert.equal(decision([...dev, ...held]), 'reject-all-fixed-offsets-D111-survives');
  held[1].qualified = false; assert.equal(decision([...dev, ...held]), 'inconclusive-heldout-coverage');
});
test('an anchor over 75ms after its causal control resets acquisition before completion', () => {
  const state = new Acquisition(1, () => {}), b = bots();
  state.update(b, 2000, 1200000);
  b.forEach(p => p.world.snakes.get(p.world.self).t = 3100);
  b[1].world.snakes.get(1).t = 3100;
  state.update(b, 3100, 1200000);
  state.control = { t: 3100, life: 1, target: 37, qualified: true, pair: state.pair };
  const point = { x: 0, y: 0, count: 37, speed: 6, wanted: 1, server: 3200 };
  state.anchor(point, 3200);
  assert.equal(state.selector.anchors.length, 0);
  assert.equal(state.failures['stale-control-on-receipt'], undefined);
  state.anchor(point, 3150);
  assert.equal(state.selector.anchors.length, 1);
  state.anchor(point, 3200);
  assert.equal(state.selector.anchors.length, 0);
  assert.equal(state.failures['stale-control-on-receipt'], 1);
});
test('adapter preserves transport/tick implementation and never requests boost', () => {
  const rec = { meta() {} }, b = bots(), a = new Acquisition(1, () => {});
  a.ready = false; a.pair = {};
  const bot = new TurnBot({ scenario: 'movement' }, 0, rec, a, b, new Set(), () => {});
  bot.movement(2000, { id: 1, sct: 37 }, { a: 0 });
  assert.equal(bot.movementIntent.desiredBoost, false);
  assert(!Object.hasOwn(TurnBot.prototype, 'tick'));
  assert(!Object.hasOwn(TurnBot.prototype, 'send'));
});
test('frozen rates and duration margins separate all fixed offsets at both primary sizes', () => {
  for (const n of [37, 100]) {
    const q = rate(n, 111), seconds = durations.get(n);
    const result = scores({ angle: q * seconds, seconds }, n);
    assert(result.D111.admitted[42]);
    for (const k of [6, 7, 8, 9, 10]) assert(!result[`k${k}`].admitted[42]);
  }
  assert.equal(select([], 1).cells && Object.keys(select([], 1).cells).length, 0);
});
test('offline selection preserves the first native arc and censors missing positive observer coverage', () => {
  const events = [{ kind: 'meta', event: 'turn-receipt', life: 1, id: 1, count: 37, serverClock: 0 }];
  const q = rate(37, 111), step = q * .11, radius = 42 / (2 * Math.sin(step / 2));
  for (let i = 0; i < 300; i++) {
    const a = i * step, t = 2000 + i * 110;
    events.push({ kind: 'meta', event: 'turn-control', life: 1, t, target: 37, sign: 1, qualified: true,
      pair: { ready: true, actor: { age: 0 }, observer: { age: 0 }, cacheAge: 0 } });
    events.push({ kind: 'anchor', connection: 1, generation: 1, index: i, received: t, server_ms: t,
      anchor: [radius * Math.cos(a), radius * Math.sin(a)], wanted: a + Math.PI,
      before: { logical: 37 }, after: { logical: 37, physical: 38, dying: 1, fam: .2, displayed_length: 37.2 },
      wire_count: 37, speed_per_32ms: 6, observerAgreement: true });
  }
  events.push({ kind: 'meta', event: 'turn-receipt', life: 1, id: 1, count: 37, serverClock: 40000 });
  const selected = select(events, 1);
  assert.equal(selected.cells['37/1'].first, 2);
  assert(selected.cells['37/1'].compatible.includes('D111'));
  assert(selected.qualified);
  events.find(e => e.kind === 'anchor').observerAgreement = false;
  const censored = select(events, 1);
  assert.equal(censored.cells['37/1'].first, 2);
  assert(!censored.qualified);
});
