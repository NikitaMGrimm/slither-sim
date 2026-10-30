import { traceSchema, validateTrace } from './schema.mjs';
import { floatBits, scheduleHash, stringify } from './io.mjs';

export const crossTargetPolicyVersion = 'feel-cross-target-v2';

// Storage types and shapes stay unchanged. Only cross-target comparison assigns
// the existing numeric caps to ring floats; cursors/counts never enter this map.
function ringFloatType(type, path) {
  if (/^samples\[\d+\]\.camera\.ring\[\d+\]$/.test(path)) return 'world';
  if (/^samples\[\d+\]\.own\.rings\.(head|angle|length)\.values\[\d+\]$/.test(path)) return 'f32';
  return type;
}

function f32Ordinal(value) {
  const bytes = Buffer.alloc(4);
  bytes.writeFloatBE(value);
  const bits = bytes.readUInt32BE();
  return bits & 0x80000000 ? 0x100000000 - bits : 0x80000000 + bits;
}
function allowance(a, b, type, policy) {
  // Sign of zero is always significant, including in the numerical lane.
  if (!policy || (a === 0 && b === 0)) return false;
  if (type === 'f32') return Math.abs(f32Ordinal(a) - f32Ordinal(b)) <= policy.f32Ulps;
  const cap = { world: policy.worldCap, radians: policy.angleCap, zoom: policy.zoomCap }[type];
  return cap !== undefined && Math.abs(a - b) <= cap;
}
export function compareTraces(left, right, schedule, { mode = 'same-core', exactOnly = false } = {}) {
  const errors = [], mismatches = [], nonExactFloats = [];
  const result = () => ({
    mode, policyVersion: mode === 'cross-target' ? crossTargetPolicyVersion : 'feel-same-core-v1', exactOnly,
    status: errors.length ? 'invalid' : mismatches.length ? 'mismatch'
      : nonExactFloats.length ? 'numerical-match' : 'exact-match',
    passed: errors.length === 0 && mismatches.length === 0,
    exact: errors.length === 0 && mismatches.length === 0 && nonExactFloats.length === 0,
    errors, mismatches, nonExactFloats,
  });
  if (!['same-core', 'cross-target'].includes(mode)) errors.push('mode: expected same-core or cross-target');
  for (const [name, trace] of [['left', left], ['right', right]]) {
    try {
      validateTrace(trace, schedule);
      if (trace.manifest.scheduleSha256 !== scheduleHash(schedule)) throw new Error('manifest.scheduleSha256: schedule hash mismatch');
    } catch (error) { errors.push(`${name}.${error.message}`); }
  }
  // Completeness is a separate first gate: do not compare eligible subsets.
  if (errors.length) return result();
  const a = left.manifest, b = right.manifest;
  const cross = (a.peer === 'native') !== (b.peer === 'native');
  if ((mode === 'cross-target') !== cross) errors.push('mode: cross-target requires exactly one native and one Wasm peer');
  for (const key of ['product', 'input', 'scheduleSha256', 'entropy', 'startup', 'geometry', 'qualityRequest', 'numericalPolicy']) {
    if (stringify(a[key]) !== stringify(b[key])) errors.push(`manifest.${key}: binding mismatch`);
  }
  if (a.peer === b.peer) {
    for (const key of ['artifacts', 'toolVersions', 'presentation']) {
      if (stringify(a[key]) !== stringify(b[key])) errors.push(`manifest.${key}: same-peer provenance mismatch`);
    }
  }
  if (errors.length) return result();
  const policy = mode === 'cross-target' && !exactOnly ? a.numericalPolicy : null;
  const drift = new Map();
  function walk(x, y, schema, path) {
    if (schema.nullable) {
      if (x === null || y === null) {
        if (x !== y) mismatches.push({ path, reason: 'absence mismatch', left: x, right: y });
      } else walk(x, y, schema.nullable, path);
    } else if (schema.list) {
      if (x.length !== y.length) mismatches.push({ path, reason: 'ordered count mismatch', left: x.length, right: y.length });
      for (let i = 0; i < Math.min(x.length, y.length); i++) walk(x[i], y[i], schema.list, `${path}[${i}]`);
    } else if (typeof schema === 'string' || schema.choice) {
      const numericFloat = ['number', 'world', 'radians', 'zoom', 'f32', 'f32-exact', 'time', 'positive'].includes(schema);
      const channel = path.replace(/^samples\[\d+\]/, 'samples[]');
      if (Object.is(x, y)) {
        if (numericFloat) drift.delete(channel);
        return;
      }
      if (numericFloat) {
        const type = mode === 'cross-target' ? ringFloatType(schema, path) : schema;
        const tolerated = allowance(x, y, type, policy);
        const delta = y - x;
        const difference = { path, reason: x === 0 && y === 0 ? 'signed zero mismatch' : 'non-exact float',
          leftBits: floatBits(x), rightBits: floatBits(y),
          delta: Number.isFinite(delta) ? delta : delta > 0 ? 'positive overflow' : 'negative overflow', tolerated };
        nonExactFloats.push(difference);
        if (!tolerated) mismatches.push(difference);
        // The channel retains the physical ring-element index, so each stored
        // element has an independent history in sample order, even as cursors move.
        // Three growing same-sign error levels in one ordered sample channel fail.
        // Repeated update/draw observations retain progress without counting as growth.
        if (tolerated && path.startsWith('samples[')) {
          const prior = drift.get(channel);
          const count = prior && Math.sign(prior.delta) === Math.sign(delta)
            && Math.abs(delta) >= Math.abs(prior.delta)
            ? prior.count + (Math.abs(delta) > Math.abs(prior.delta) ? 1 : 0) : 1;
          drift.set(channel, { delta, count });
          if (count >= 3) mismatches.push({ path, reason: 'accumulating numerical drift' });
        }
      } else mismatches.push({ path, reason: 'exact/discrete mismatch', left: x, right: y });
    } else {
      for (const [key, child] of Object.entries(schema)) walk(x[key], y[key], child, path ? `${path}.${key}` : key);
    }
  }
  for (const key of ['events', 'commands', 'samples', 'final']) walk(left[key], right[key], traceSchema[key], key);
  return result();
}
