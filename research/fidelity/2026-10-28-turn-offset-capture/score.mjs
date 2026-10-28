import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { ArcSelector, durations, key, decision, primaryAgreement } from './selection.mjs';
const A = 2 * Math.asin(Math.SQRT2 / 39) + 2 * Math.PI / 65536;
export function rate(n, d = 106) {
  const sc = Math.min(6, Math.max(1, 1 + (n - 2) / d));
  return .033 / .008 * (.13 + .87 * ((7 - sc) / 6) ** 2);
}
export function scores(result, n) {
  const predictions = { D111: rate(n, 111), D106_original: rate(n),
    ...Object.fromEntries([6, 7, 8, 9, 10].map(k => [`k${k}`, rate(Math.max(2, n - k))])) };
  return Object.fromEntries(Object.entries(predictions).map(([name, q]) => {
    const error = Math.abs(Math.abs(result.angle) - q * result.seconds);
    return [name, { predictedRate: q, angleError: error,
      admitted: Object.fromEntries([0, 7, 14, 42].map(j => [j, error <= A + q * (.001 + 2 * j / 1000)])) }];
  }));
}
export function select(events, ordinal) {
  const cells = {}, selectors = new Map(), censored = [];
  const anchors = events.filter(e => e.kind === 'anchor');
  let control = null, count = null, countSince = -Infinity, life = null;
  for (const e of events) {
    if (e.kind === 'meta') {
      if (e.event === 'turn-control') control = e;
      if (e.event === 'turn-receipt') {
        if (e.life !== life || e.count !== count) {
          countSince = e.serverClock; count = e.count; life = e.life; selectors.clear();
        }
      }
      if (e.event === 'turn-hold-interrupted') selectors.clear();
      continue;
    }
    if (e.kind !== 'anchor') continue;
    const n = e.after.logical, sign = control?.sign, k = key(n, sign);
    if (!durations.has(n) || ![1, -1].includes(sign) || cells[k]) continue;
    const elapsed = e.received - (control?.t ?? -Infinity), pair = control?.pair;
    const qualified = control?.qualified && control.life === e.connection && control.target === n &&
      elapsed >= 0 && elapsed <= 75 && pair?.ready &&
      [pair.actor.age, pair.observer.age, pair.cacheAge].every(age => age + elapsed <= 300) &&
      e.server_ms - countSince >= 1000 && e.before.logical === n && e.wire_count === n;
    if (!qualified) { selectors.delete(k); continue; }
    const selector = selectors.get(k) ?? new ArcSelector(n, sign);
    selectors.set(k, selector);
    const result = selector.push({ x: e.anchor[0], y: e.anchor[1], index: e.index,
      server: e.server_ms, wanted: e.wanted, speed: e.speed_per_32ms, count: n, qualified });
    if (!result) continue;
    const following = events.find(x => x.kind === 'meta' && x.event === 'turn-receipt' &&
      x.life === e.connection && x.serverClock >= result.endServer + 1000);
    const changedSoon = events.some(x => x.kind === 'meta' && x.event === 'turn-receipt' && x.life === e.connection &&
      x.serverClock >= e.server_ms && x.serverClock < result.endServer + 1000 && x.count !== n);
    if (!following || following.count !== n || changedSoon) {
      censored.push({ cell: k, reason: 'post-count-stability', connection: e.connection, first: result.first });
      selectors.delete(k); continue;
    }
    const arc = anchors.filter(x => x.connection === e.connection && x.generation === e.generation &&
      x.index >= result.first - 2 && x.index <= result.last + 2);
    const peerOk = arc.length >= result.triplets + 4 && arc.every(x => x.observerAgreement);
    const s = scores(result, n);
    cells[k] = { count: n, sign, connection: e.connection, generation: e.generation,
      ...result, observedRate: Math.abs(result.angle) / result.seconds, scores: s,
      observerAgreement: peerOk,
      compatible: Object.keys(s).filter(name => s[name].admitted[42]),
      countRanges: Object.fromEntries(['logical', 'physical', 'dying', 'fam', 'displayed_length'].map(name =>
        [name, [Math.min(...arc.map(x => x.after[name])), Math.max(...arc.map(x => x.after[name]))]])) };
  }
  return { ordinal, cells, censored, qualified: primaryAgreement(cells) };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const input = JSON.parse(fs.readFileSync(process.argv[2]));
  const invocations = input.map(r => select(r.events, r.ordinal));
  console.log(JSON.stringify({ invocations, decision: decision(invocations) }));
}
