import assert from 'node:assert/strict';

// Synthetic smoke viewports use one backing pixel per CSS pixel. Only rendered
// values belong here; resource ownership, lifecycle and picture counts stay exact.
function budget(errors, p95, max, message) {
  assert(errors.length > 0 && errors.every(Number.isFinite), message);
  errors.sort((a, b) => a - b);
  assert(errors.at(-1) <= max && errors[Math.ceil(errors.length * 0.95) - 1] <= p95,
    `${message ?? 'visual budget'}: max=${errors.at(-1)}, limits p95=${p95}, max=${max}`);
}
export function screen(actual, expected, message, max = 1) {
  if (typeof actual === 'number') { actual = [actual]; expected = [expected]; }
  assert.equal(actual.length, expected.length, message);
  // Flat geometry is x/y pairs (including width/height); paths are arrays of pairs.
  const pairs = a => Array.isArray(a[0]) ? a : Array.from({ length: Math.ceil(a.length / 2) }, (_, i) => a.slice(i * 2, i * 2 + 2));
  const a = pairs(actual), b = pairs(expected);
  budget(a.map((point, i) => {
    assert.equal(point.length, b[i].length, message);
    return Math.hypot(...point.map((value, j) => value - b[i][j]));
  }), max === 2 ? 2 : 0.5, max, message);
}
export function alpha(actual, expected, message) {
  if (!Array.isArray(actual)) { actual = [actual]; expected = [expected]; }
  assert.equal(actual.length, expected.length, message);
  budget(actual.map((value, i) => {
    if (expected[i] === 0 || expected[i] === 1) assert(value === expected[i], message);
    return Math.abs(value - expected[i]);
  }), 0.01, 0.02, message);
}
export function heading(actual, expected, message, bulb = false) {
  const delta = Math.atan2(Math.sin(actual - expected), Math.cos(actual - expected));
  budget([Math.abs(delta) * 180 / Math.PI], bulb ? 2 : 0.25, bulb ? 2 : 0.5, message);
}
export function transforms(actual, expected, message) {
  assert.deepEqual(actual.map(v => v[0]), expected.map(v => v[0]), message);
  actual.forEach((value, i) => {
    if (value[0] === 'translate') screen(value.slice(1), expected[i].slice(1), message);
    else if (value[0] === 'rotate') heading(value[1], expected[i][1], message);
    else assert.deepEqual(value, expected[i], message);
  });
}
