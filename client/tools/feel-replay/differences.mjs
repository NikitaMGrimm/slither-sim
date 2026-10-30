import { floatBits } from './codec.mjs';

// Supplementary evidence only. A1 still gates completeness/provenance first.
// Keep all state differences inspectable when entropy already fails that gate.
export function differences(left, right) {
  const rows = [];
  function visit(a, b, path) {
    if (Object.is(a, b)) return;
    if (typeof a === 'number' && typeof b === 'number') {
      rows.push({ path, kind: a === 0 && b === 0 ? 'signed-zero' : 'number', leftBits: floatBits(a), rightBits: floatBits(b) });
    } else if (Array.isArray(a) && Array.isArray(b)) {
      if (a.length !== b.length) rows.push({ path, kind: 'count', left: a.length, right: b.length });
      for (let i = 0; i < Math.max(a.length, b.length); i++) visit(a[i], b[i], `${path}[${i}]`);
    } else if (a && b && typeof a === 'object' && typeof b === 'object') {
      for (const key of new Set([...Object.keys(a), ...Object.keys(b)])) visit(a[key], b[key], path ? `${path}.${key}` : key);
    } else rows.push({ path, kind: a === undefined || b === undefined ? 'missing' : 'discrete', left: a ?? null, right: b ?? null });
  }
  for (const key of ['events', 'commands', 'samples', 'final']) visit(left[key], right[key], key);
  for (const key of ['product', 'input', 'scheduleSha256', 'entropy', 'startup', 'geometry', 'qualityRequest', 'numericalPolicy']) visit(left.manifest[key], right.manifest[key], `manifest.${key}`);
  return rows;
}
