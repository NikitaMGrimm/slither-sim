// Authored synthetic lifecycle contract checks. No browser, GPU or server.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { syntheticFixture } from './feel-replay/synthetic.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { parse, scheduleHash, stringify } from './feel-replay/io.mjs';
import { observation } from './feel-replay/observation.mjs';
import { buildSchedule } from './feel-replay/schedule.mjs';
import { readFileSync } from 'node:fs';

function lifecycle() {
  const f = syntheticFixture();
  Object.assign(f.trace.samples[2].own, { expired: true, deathProgress: 1.1, projected: null });
  f.schedule.boundaries[3].eligibility = 'between-lives';
  Object.assign(f.trace.samples[3], { eligibility: 'between-lives', own: null });
  f.trace.samples[4].own.token = 'synthetic-own-2';
  f.trace.manifest.scheduleSha256 = scheduleHash(f.schedule);
  return f;
}
test('v2 retains expired own, skips projection, and admits a declared restart with a new token', () => {
  const { schedule, trace } = lifecycle();
  assert.equal(schedule.schema, 2); assert.equal(trace.schema, 2);
  assert.equal(compareTraces(trace, trace, schedule).status, 'exact-match');
  assert.equal(parse(stringify(trace)).samples[2].own.projected, null);
});
for (const [name, mutate] of [
  ['missing live own', f => { f.trace.samples[1].own = null; }],
  ['missing expired own', f => { f.trace.samples[2].own = null; }],
  ['missing live projection', f => { f.trace.samples[1].own.projected = null; }],
  ['invented expired projection', f => { f.trace.samples[2].own.projected = f.trace.samples[1].own.projected; }],
  ['expired flag without expiry', f => { f.trace.samples[2].own.deathProgress = 0.9; }],
  ['between-lives before expiry', f => { Object.assign(f.trace.samples[2].own, { expired: false, deathProgress: 0.9, projected: f.trace.samples[1].own.projected }); }],
  ['reused token after reset', f => { f.trace.samples[4].own.token = f.trace.samples[2].own.token; }],
  ['expired token reviving without reset', f => {
    f.schedule.boundaries[3].eligibility = 'own';
    Object.assign(f.trace.samples[3], { eligibility: 'own', own: f.trace.samples[1].own });
    f.trace.manifest.scheduleSha256 = scheduleHash(f.schedule);
  }],
  ['old trace version', f => { f.trace.schema = 1; }],
]) test(`completeness rejects ${name} even in identical corrupted peers`, () => {
  const f = lifecycle(); mutate(f);
  assert.equal(compareTraces(f.trace, f.trace, f.schedule).status, 'invalid');
});
test('expired raw ABI has no invented draw cells and still decodes stored channels', () => {
  const camera = [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, ...Array(124).fill(0)];
  const cells = [...camera, 1, 1.1, ...Array(28 + 238).fill(0)];
  const { trace } = syntheticFixture();
  const o = observation(['draw', 'own-1', cells], trace.manifest.geometry);
  assert(o.own.expired); assert.equal(o.own.deathProgress, 1.1);
  assert.equal(o.own.projected, null); assert.equal(o.own.rings.head.values.length, 159);
  assert.throws(() => observation(['draw', 'own-1', cells.slice(0, -1)], trace.manifest.geometry), /Truncated/);
});
test('browser fixture freezes the close/Play/appearance gap independently of peer output', () => {
  const record = JSON.parse(readFileSync(new URL('./feel-replay/browser-synthetic.json', import.meta.url)));
  const options = { expectedCommands: 7, appearanceAtMs: 3 };
  const { schedule } = buildSchedule(record, options);
  assert(schedule.boundaries.some(b => b.atMs >= 900 && b.atMs < 942 && b.eligibility === 'between-lives'));
  record.controls.shift();
  assert.throws(() => buildSchedule(record, options), /between-lives/);
});
