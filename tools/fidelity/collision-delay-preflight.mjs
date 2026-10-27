// Study-specific, offline-only identifiability check. No network imports.
// The two histories are independently simulated so collision censoring cannot
// remove the counterfactual trajectory used by the rewind positive control.
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import zlib from 'node:zlib';
import { World, splitFrame, splitPayload } from './slither-headless.mjs';

export function interval(rows, floor = 4) {
  const live = rows.filter(r => r.qualified);
  const misses = live.filter(r => !r.contact).map(r => r.offsetRange?.[0] ?? r.offset);
  const hits = live.filter(r => r.contact).map(r => r.offsetRange?.[1] ?? r.offset);
  if (!misses.length || !hits.length) return null;
  const low = Math.max(...misses), high = Math.min(...hits);
  return low <= high ? [low - floor, high + floor] : null;
}

export function analyzePair(files) {
  const members = files.map(file => {
    const text = zlib.gunzipSync(fs.readFileSync(file)).toString('utf8');
    const lines = text.trimEnd().split('\n'), rows = lines.map(line => JSON.parse(line));
    const end = rows.at(-1), header = rows[0];
    const prefix = createHash('sha256').update(lines.slice(0, -1).join('\n') + '\n').digest('hex');
    if (end.integrity?.sha256 !== prefix) throw Error('invalid raw prefix receipt');
    const times = rows.map(row => Array.isArray(row) ? row[1] : row.t);
    const issues = [];
    if (times.some((t, i) => !Number.isFinite(t) || i > 0 && t < times[i - 1])) issues.push('record_clock_regressed');
    if (end.frames !== rows.filter(row => Array.isArray(row) && row[0] === 'i').length) issues.push('incoming_frame_count_mismatch');
    if (end.event !== 'capture-end' || !end.collectorSourcesUnchanged) issues.push('incomplete_or_changed_source');
    return { rows, header, end, issues, archiveSha256: createHash('sha256').update(fs.readFileSync(file)).digest('hex') };
  });
  if (members.length !== 2 || members[0].header.captureProvenance.captureBatchId !==
      members[1].header.captureProvenance.captureBatchId || members[0].header.captureProvenance.timeOrigin !==
      members[1].header.captureProvenance.timeOrigin) throw Error('pair clock/membership mismatch');
  const events = members.flatMap((member, index) => member.rows.slice(1, -1).map((row, ordinal) => ({ row, index, ordinal,
    at: Array.isArray(row) ? row[1] : row.t }))).sort((a, b) => a.at - b.at || a.index - b.index || a.ordinal - b.ordinal);
  const worlds = [0, 1].map(() => new World({ serverBody: true }));
  const frames = [0, 0], raw = [false, false], pending = [null, null], rtts = [], summaries = [];
  let active = null;
  for (const { row, index, at } of events) {
    const world = worlds[index];
    if (row.event === 'connect') { worlds[index] = new World({ serverBody: true }); frames[index] = 0; pending[index] = null; }
    if (row.event === 'tail-launch' && index === 0) active = { spec: row, offsets: [], ages: [], controls: [] };
    if (row.event === 'tail-result' && index === 0) {
      const contact = row.outcome === 'contact';
      const qualified = row.qualified && ['contact', 'survival'].includes(row.outcome) && active?.offsets.length >= 4;
      summaries.push({ ...row, qualified, contact, offsetRange: qualified ?
        [Math.min(...active.offsets), Math.max(...active.offsets)] : null,
        geometrySamples: active?.offsets.length ?? 0, maximumAnchorAgeMs: active?.ages.length ? Math.max(...active.ages) : null });
      active = null;
    }
    if (!Array.isArray(row)) continue;
    const bytes = Buffer.from(row[2], 'base64');
    if (row[0] === 'o') {
      if (bytes.length === 1 && bytes[0] === 251) pending[index] = at;
      continue;
    }
    if (row[0] !== 'i') continue;
    if (++frames[index] === 1) raw[index] = bytes[0] === 54;
    for (const packet of raw[index] ? splitPayload(bytes) : splitFrame(bytes)) {
      if (packet[0] === 112 && pending[index] != null) { rtts.push(at - pending[index]); pending[index] = null; }
      world.packet(packet, at);
    }
    if (!active) continue;
    const snakes = worlds.map(w => w.snakes.get(w.self));
    if (worlds.some(w => w.dead) || snakes.some(s => !s?.pts.length)) continue;
    const poses = snakes.map((s, i) => ({ ...worlds[i].head(s, at), heading: s.ang,
      speed: s.sp / .032, age: at - s.t, tail: s.pts[0] }));
    const [r, p] = poses, heading = active.spec.heading;
    const angle = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
    if (poses.some(s => s.age < 0 || s.age > 350) || Math.abs(angle(r.heading, heading)) > .12 ||
        Math.abs(angle(p.heading, heading + Math.PI / 2)) > .12) continue;
    const along = (x, y) => x * Math.cos(heading) + y * Math.sin(heading);
    const cross = (x, y) => -x * Math.sin(heading) + y * Math.cos(heading);
    const y = cross(p.x - r.x, p.y - r.y);
    if (y < -100 || y > -20) continue;
    const pv = { x: Math.cos(p.heading) * p.speed, y: Math.sin(p.heading) * p.speed };
    const rv = { x: Math.cos(r.heading) * r.speed, y: Math.sin(r.heading) * r.speed };
    const crossing = -y / cross(pv.x - rv.x, pv.y - rv.y);
    if (!Number.isFinite(crossing) || crossing < 0 || crossing > 1) continue;
    active.offsets.push(along(p.x - r.tail.x, p.y - r.tail.y) +
      along(pv.x - rv.x, pv.y - rv.y) * crossing);
    active.ages.push(...poses.map(pose => pose.age));
  }
  rtts.sort((a, b) => a - b);
  const rtt95 = rtts.length ? rtts[Math.ceil(rtts.length * .95) - 1] : null;
  const complete = members.every(m => m.issues.length === 0);
  if (!complete) summaries.forEach(row => { row.qualified = false; });
  return { synthetic: members[0].header.dataOrigin === 'local-simulator',
    archives: members.map(m => ({ sha256: m.archiveSha256, terminal: m.end.event,
      sourcesUnchanged: m.end.collectorSourcesUnchanged, issues: m.issues })), summaries, wireRtt95Ms: rtt95,
    timingLimit: 'wire RTT excludes artificial queues; receipt clocks do not identify simultaneous authoritative state',
    complete };
}

export function compare(rows, expectedShift, floor = 4) {
  const envelope = arm => {
    const selected = rows.filter(r => r.arm === arm);
    const groups = [...new Set(selected.map(r => r.repeat ?? 0))];
    const intervals = groups.map(group => interval(selected.filter(r => (r.repeat ?? 0) === group), floor));
    if (!intervals.length || intervals.some(value => !value)) return null;
    // Phase-dependent thresholds are nuisance variation, not extra independent
    // samples. Use their full envelope, never their favorable intersection.
    return [Math.min(...intervals.map(x => x[0])), Math.max(...intervals.map(x => x[1]))];
  };
  const control = envelope('control');
  const delayed = envelope('delay');
  if (!control || !delayed) return { verdict: 'inconclusive', control, delayed, shift: null };
  const shift = [delayed[0] - control[1], delayed[1] - control[0]];
  const current = shift[0] <= 0 && shift[1] >= 0;
  const rewind = shift[0] <= expectedShift && shift[1] >= expectedShift;
  return { verdict: current && !rewind ? 'no compensation' : rewind && !current ? 'compensation' : 'inconclusive',
    control, delayed, shift, expectedShift,
    uncertainty: 'deterministic identification interval with per-arm position floor; not a confidence interval' };
}

export function analyticControls({ speed = 180, delayMs = 450, floor = 4 } = {}) {
  // x is the signed miss distance of a crossing trajectory from a finite end.
  // Rewinding ONLY the reference translates that end along x by -v*delay.
  // This is a specified counterfactual, not a claim about the closed server.
  const expectedShift = -speed * delayMs / 1000;
  const outcomes = {};
  for (const model of ['current', 'peer-rewind', 'whole-state-rewind', 'tandem-peer-rewind']) {
    const rows = [];
    for (const arm of ['control', 'delay']) {
      for (let repeat = 0; repeat < 2; repeat++) {
        for (let step = 0; step < 6; step++) {
          const offset = -240 + 20 * (step * 2 + repeat);
          const boundary = -60 + (model === 'peer-rewind' && arm === 'delay' ? expectedShift : 0);
          const contact = offset >= boundary;
          rows.push({ arm, repeat, step, offset, contact, qualified: true });
        }
      }
    }
    outcomes[model] = { rows, result: compare(rows, expectedShift, floor) };
  }
  return { synthetic: true, speed, delayMs, plannedSamplesPerArm: 12, floor, expectedShift, outcomes };
}

if (process.argv[1]?.endsWith('collision-delay-preflight.mjs')) {
  const output = process.argv[2];
  if (!output || fs.existsSync(output)) throw Error('supply a new output JSON path');
  fs.writeFileSync(output, JSON.stringify(analyticControls(), null, 2) + '\n', { flag: 'wx' });
}
