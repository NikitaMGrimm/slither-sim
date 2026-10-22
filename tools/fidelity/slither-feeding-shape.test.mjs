import test from 'node:test';
import assert from 'node:assert/strict';
import { FeedingBot, FeedingState, rawGeometry, rearBodyTarget } from './slither-feeding-shape.mjs';

const receiver = (overrides = {}) => ({ life: 1, id: 10, segments: 40, fam: .2,
  heading: 0, x: 1000, y: 1000, peerVisible: true, interruption: null,
  receivedAgeMs: 0, anchorAgeMs: 0,
  geometry: { medianSpacing: 11, distantPairs: 0, pathRatio: .95 },
  rapidNearDeath: [], collectedRapidIds: [], ...overrides });
const donor = (overrides = {}) => ({ life: 1, id: 11, segments: 3, fam: .1,
  x: 1200, y: 1000, peerVisible: true, targetSource: 'local', targetDanger: 0,
  receivedAgeMs: 0, anchorAgeMs: 0,
  target: { x: 1010, y: 1000, distance: 190, fromHead: 300, snakeId: 10 },
  ...overrides });

function contactState() {
  const state = new FeedingState();
  state.update(0, receiver(), donor());
  assert.equal(state.phase, 'contact');
  return state;
}

test('shared coordinates guide rendezvous but never permit contact', () => {
  const state = new FeedingState();
  state.update(0, receiver(), donor({ peerVisible: false, targetSource: 'shared' }));
  assert.equal(state.phase, 'rendezvous');
  state.update(100, receiver(), donor());
  assert.equal(state.phase, 'contact');
});

test('donor death starts collection and requires nearby rapid disappearance plus growth', () => {
  const state = contactState();
  state.update(100, receiver(), null, true);
  assert.equal(state.phase, 'collect');
  assert.deepEqual(state.deathSite, { x: 1200, y: 1000 });
  state.update(200, receiver({ fam: .3 }), null, true);
  assert.equal(state.phase, 'collect');
  state.update(300, receiver({ fam: .3, rapidNearDeath: [42], collectedRapidIds: [42] }), null, true);
  assert.equal(state.phase, 'hold');
  assert.equal(state.heading, 0);
  state.update(5300, receiver({ fam: .3 }), null, true);
  assert.equal(state.reason, 'straight_compressed_observed');
});

test('hold rejects saturated mass, interrupted steering, and bent paths', () => {
  for (const overrides of [{ fam: 1 }, { interruption: 'avoidance' },
    { anchorAgeMs: 510 },
    { geometry: { medianSpacing: 11, distantPairs: 0, pathRatio: .3 } }]) {
    const state = contactState();
    state.update(100, receiver(), null, true);
    state.update(200, receiver({ fam: .3, rapidNearDeath: [42], collectedRapidIds: [42] }), null, true);
    assert.equal(state.phase, 'hold');
    state.update(5300, receiver({ fam: .3, ...overrides }), null, true);
    assert.notEqual(state.reason, 'straight_compressed_observed');
  }
});

test('dead donor after contact is intentional; earlier death is null', () => {
  const state = new FeedingState();
  state.update(0, receiver(), donor({ peerVisible: false, targetSource: 'shared' }));
  state.update(100, receiver(), null, true);
  assert.equal(state.reason, 'premature_donor_death');
  const collecting = contactState();
  collecting.update(100, receiver(), null, true);
  collecting.update(200, receiver(), null, true);
  assert.equal(collecting.phase, 'collect');
});

test('rear target requires 250 separation from receiver head', () => {
  const world = { snakes: new Map([[10, { pts: [
    { x: 500, y: 0 }, { x: 600, y: 0 }, { x: 800, y: 0 },
    { x: 900, y: 0 }, { x: 980, y: 0 }, { x: 990, y: 0 }] }]]) };
  const target = rearBodyTarget(world, 10, { x: 510, y: 0 }, { x: 1000, y: 0 });
  assert.equal(target.x, 500);
  assert.ok(target.fromHead >= 250);
});

test('shape metric rejects a bent arc despite compressed spacing', () => {
  const arc = Array.from({ length: 40 }, (_, i) => ({
    x: 84 * Math.cos(i * .13), y: 84 * Math.sin(i * .13) }));
  const shape = rawGeometry(arc);
  assert.ok(shape.medianSpacing >= 10 && shape.medianSpacing <= 12);
  assert.ok(shape.pathRatio < .9);
});

test('rapid food only counts as receiver intake with its wire eater ID', () => {
  const bot = new FeedingBot({ scenario: 'movement', serverBody: true }, 0,
    { frame() {}, meta() {} });
  bot.world.self = 1;
  bot.world.snakes.set(1, { id: 1, sct: 40, fam: .2, pts: [{ x: 1000, y: 1000 }],
    ang: 0, wang: 0, sp: 5.8, t: performance.now() });
  const frame = p => Uint8Array.from([0, 1, ...p]);
  bot.receive(frame([98, 3, 3, 1, 1, 68]));
  const id = [...bot.recentRapid.keys()][0];
  assert.equal(bot.recentRapid.get(id).size, 13.6);
  bot.receive(frame([60, 3, 3, 1, 1, 0, 2]));
  assert.equal(bot.receiverEatenRapid.has(id), false);
  bot.receive(frame([98, 3, 3, 2, 2, 68]));
  const next = [...bot.recentRapid.keys()].find(value => value !== id);
  bot.receive(frame([60, 3, 3, 2, 2, 0, 1]));
  assert.equal(bot.receiverEatenRapid.has(next), true);
  bot.receive(frame([98, 3, 3, 2, 2, 68]));
  assert.equal(bot.receiverEatenRapid.has(next), false,
    'a new pellet at the same coordinate has a new identity');
  const combined = Uint8Array.from([0, 1, 0, 7, 60, 3, 3, 2, 2, 0, 1,
    38, 98, 3, 3, 2, 2, 68]);
  bot.receive(combined);
  assert.equal(bot.receiverEatenRapid.has(next), false,
    'same-frame replacement must preserve packet order');
});
