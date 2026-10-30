import { traceSchema, manifestSchema, validateTrace, policyVersion } from './schema.mjs';
import { scheduleHash } from './io.mjs';

const position = { unit: 'CSS px', p95: 0.5, max: 1 };
const heading = { unit: 'degrees', p95: 0.25, max: 0.5 };
const phase = { unit: 'normalized phase', p95: 0.01, max: 0.02 };
const finiteError = value => Number.isFinite(value) ? value : Math.sign(value) * Number.MAX_VALUE;
const norm = values => finiteError(Math.hypot(...values));
const rms = rows => {
  const max = rows.reduce((max, r) => Math.max(max, r.error), 0);
  return max === 0 ? 0 : finiteError(max * Math.sqrt(rows.reduce((sum, r) => sum + (r.error / max) ** 2, 0) / rows.length));
};
const wrapped = (a, b) => Math.atan2(Math.sin(b % (2 * Math.PI) - a % (2 * Math.PI)), Math.cos(b % (2 * Math.PI) - a % (2 * Math.PI))) * 180 / Math.PI;

// All peers and source revisions share this gate. No alignment or resampling.
export function compareTraces(left, right, schedule) {
  const errors = [], mismatches = [], metrics = new Map();
  let frames;
  const frameBudget = atMs => {
    const index = frames.findIndex(t => t > atMs);
    return Math.min(20, index > 0 ? frames[index] - frames[index - 1]
      : index === -1 && frames.length > 1 ? frames.at(-1) - frames.at(-2) : 20);
  };
  const exactChannels = new Map(), channelPaths = new Map();
  const result = channels => ({ policyVersion, status: errors.length ? 'invalid' : mismatches.length ? 'mismatch' : 'perceptual-match',
    passed: !errors.length && !mismatches.length, errors, mismatches, firstFailure: errors[0] ?? mismatches[0] ?? null, channels });
  for (const [name, trace] of [['left', left], ['right', right]]) {
    try {
      validateTrace(trace, schedule);
      if (trace.manifest.scheduleSha256 !== scheduleHash(schedule)) throw new Error('manifest.scheduleSha256: schedule hash mismatch');
    } catch (error) { errors.push(name + '.' + error.message); }
  }
  if (errors.length) return result([]);
  frames = [...new Set(schedule.events.filter(e => e.kind === 'frame').map(e => e.atMs))];
  // Provenance differs legitimately for a baseline and a candidate. The frozen
  // tape, schedule, geometry and policy still define the same experiment.
  for (const key of ['geometry', 'policy']) walk(left.manifest[key], right.manifest[key], manifestSchema[key], 'manifest.' + key);
  exact(left.manifest.input.sha256, right.manifest.input.sha256, 'manifest.input.sha256');
  function exact(a, b, path) {
    const name = path.split(/[.[]/)[0];
    const c = exactChannels.get(name) ?? { channel: name, exact: true, count: 0, failures: 0 };
    exactChannels.set(name, c); c.count++;
    if (JSON.stringify(a) !== JSON.stringify(b)) {
      c.failures++; mismatches.push({ path, reason: 'exact channel', left: a, right: b, threshold: 0 });
    }
  }
  function metric(path, signed, budget, sample) {
    const channel = path.replace(/^samples\[\d+\]/, 'samples[]');
    const group = metrics.get(channel) ?? { channel, ...budget, rows: [] };
    group.max = Math.min(group.max, budget.max);
    metrics.set(channel, group);
    signed = signed.map(finiteError);
    const error = norm(signed);
    group.rows.push({ path, signed, error, atMs: sample.atMs, draw: sample.phase === 'draw' && sample.eligibility === 'own' });
    if (error > budget.max) mismatches.push({ path, reason: 'max', error, threshold: budget.max });
  }
  function walk(a, b, schema, path, l, r) {
    if (schema.nullable) {
      if (a === null || b === null) exact(a, b, path);
      else walk(a, b, schema.nullable, path, l, r);
    } else if (schema.list) {
      exact(a.length, b.length, path + '.length');
      for (let i = 0; i < Math.min(a.length, b.length); i++) walk(a[i], b[i], schema.list, path + '[' + i + ']', l, r);
    } else if (l && typeof a === 'object' && a && 'x' in a && 'y' in a &&
      /\.(camera\.offset|rawHead|correctedHead|pupils\.(left|right)|body\[\d+\]|(css|backing)\.(head|pupils\[\d+\]))$/.test(path)) {
      const scale = (s, axis) => schema.x === 'css-px' || path.includes('.css.') ? 1 :
        (path.includes('.backing.') ? 1 : s.camera.zoom) * s.geometry.css[axis] / s.geometry.backing[axis];
      metric(path, ['x', 'y'].map((axis, i) => (b[axis] - a[axis]) * scale(l, i ? 'height' : 'width')), position, l);
    } else if (schema.choice || typeof schema === 'string') {
      if (l && (schema === 'radians' || path.endsWith('.offsets.heading'))) metric(path, [wrapped(a, b)], heading, l);
      else if (l && schema === 'zoom') {
        metric(path + '.absolute', [b - a], { unit: 'zoom', max: 0.0005 }, l);
        metric(path + '.relative', [(b - a) / a], { unit: 'ratio', max: 0.001 }, l);
        const edge = Math.hypot(l.geometry.css.width, l.geometry.css.height) / 2;
        metric(path + '.edge', [(b - a) / a * edge], { unit: 'CSS px', max: 1 }, l);
      } else if (l && /\.(deathProgress|spawnProgress|opacity|boostPhase)$/.test(path)) {
        if (!/boostPhase$/.test(path) && ((a === 0) !== (b === 0) || (a >= 1) !== (b >= 1))) exact(a, b, path + '.endpoint');
        metric(path, [b - a], phase, l);
        if (/\.(deathProgress|spawnProgress)$/.test(path)) {
          // Production fades advance delta*0.02 / delta*0.015, delta=ms/8.
          // This compares phase at the SAME clock; it never shifts samples.
          const rate = path.endsWith('deathProgress') ? 0.02 : 0.015;
          metric(path + '.transition', [(b - a) * 8 / rate], { unit: 'virtual ms', max: frameBudget(l.atMs) }, l);
        }
      } else exact(a, b, path);
    } else {
      // Camera translation is measured separately from projected head, so equal
      // head/camera drift cannot cancel. Offsets are visual diagnostics too.
      if (l && path.endsWith('.camera')) {
        const scale = (s, axis) => s.camera.zoom * s.geometry.css[axis] / s.geometry.backing[axis];
        metric(path + '.position', [(b.x - a.x) * scale(l, 'width'), (b.y - a.y) * scale(l, 'height')], position, l);
      }
      if (l && path.endsWith('.own.offsets')) {
        metric(path + '.position', [(b.x - a.x) * l.camera.zoom * l.geometry.css.width / l.geometry.backing.width,
          (b.y - a.y) * l.camera.zoom * l.geometry.css.height / l.geometry.backing.height], position, l);
      }
      for (const [key, child] of Object.entries(schema)) {
        if (l && ((path.endsWith('.camera') || path.endsWith('.own.offsets')) && ['x', 'y'].includes(key))) continue;
        walk(a[key], b[key], child, path ? path + '.' + key : key, l, r);
      }
    }
  }
  for (const key of ['events', 'commands', 'final']) walk(left[key], right[key], traceSchema[key], key);
  left.samples.forEach((sample, i) => walk(sample, right.samples[i], traceSchema.samples.list, 'samples[' + i + ']', sample, right.samples[i]));
  const channels = [...exactChannels.values()];
  for (const { rows, ...budget } of metrics.values()) {
    const sorted = rows.map(r => r.error).sort((a, b) => a - b);
    const p95 = sorted[Math.ceil(rows.length * 0.95) - 1];
    const mean = rows[0].signed.map((_, i) => rows.reduce((sum, r) => sum + r.signed[i] / rows.length, 0));
    const draws = rows.filter(r => r.draw), quarter = Math.floor(draws.length / 4);
    const firstQuarterRms = quarter >= 10 ? rms(draws.slice(0, quarter)) : null;
    const lastQuarterRms = quarter >= 10 ? rms(draws.slice(-quarter)) : null;
    let start = 0, bias500ms = false;
    const sum = Array(rows[0].signed.length).fill(0);
    for (let end = 0; end < rows.length; end++) {
      if (rows[end].error <= budget.max / 2) { start = end + 1; sum.fill(0); continue; }
      rows[end].signed.forEach((v, i) => { sum[i] += v / rows.length; });
      while (start < end && rows[end].atMs - rows[start + 1].atMs >= 500) {
        rows[start++].signed.forEach((v, i) => { sum[i] -= v / rows.length; });
      }
      if (rows[end].atMs - rows[start].atMs >= 500
        && norm(sum.map(v => v * rows.length / (end - start + 1))) > budget.max / 2) bias500ms = true;
    }
    if (budget.p95 !== undefined && p95 > budget.p95) {
      for (const row of rows.filter(r => r.error > budget.p95)) mismatches.push({ path: row.path, reason: 'p95', error: row.error, p95, threshold: budget.p95 });
    }
    const rmsGrowth = quarter >= 10 && lastQuarterRms - firstQuarterRms > budget.max / 2;
    if (rmsGrowth) mismatches.push({ path: rows.at(-1).path, reason: 'last-quarter RMS growth', error: lastQuarterRms - firstQuarterRms, threshold: budget.max / 2 });
    channelPaths.set(budget.channel, new Set(rows.map(r => r.path)));
    channels.push({ ...budget, count: rows.length, max: sorted.at(-1), p95, rms: rms(rows), signedMean: mean.length === 1 ? mean[0] : { x: mean[0], y: mean[1] },
      threshold: { max: budget.max, ...(budget.p95 === undefined ? {} : { p95: budget.p95 }) },
      bias500ms, firstQuarterRms, lastQuarterRms, rmsGrowth });
  }
  // Preserve chronological first failure even if an aggregate limit failed.
  mismatches.sort((a, b) => (Number(a.path.match(/^samples\[(\d+)\]/)?.[1] ?? -1) - Number(b.path.match(/^samples\[(\d+)\]/)?.[1] ?? -1)));
  for (const channel of channels) {
    const paths = channelPaths.get(channel.channel);
    if (paths) channel.firstFailure = mismatches.find(m => paths.has(m.path)) ?? null;
  }
  return result(channels);
}
