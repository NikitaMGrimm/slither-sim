// Authored acquisition/analysis gates. No model residual enters selection.
export const wrap = a => Math.atan2(Math.sin(a), Math.cos(a));
export const durations = new Map([[37, 25], [100, 60], [174, 340]]);
export const key = (count, sign) => `${count}/${sign}`;
export const primaryAgreement = cells => Object.values(cells)
  .filter(c => c.count === 37 || c.count === 100).every(c => c.observerAgreement);
export function triplet(a, b, c, count, sign) {
  if ([a, b, c].some(p => p.count !== count || p.speed > 7 || !p.qualified)) return null;
  const u = [b.x - a.x, b.y - a.y], v = [c.x - b.x, c.y - b.y];
  const chords = [Math.hypot(...u), Math.hypot(...v)];
  const gaps = [b.server - a.server, c.server - b.server];
  const angle = wrap(Math.atan2(v[1], v[0]) - Math.atan2(u[1], u[0]));
  const seconds = (gaps[0] + gaps[1]) / 2000;
  const margin = sign * wrap(b.wanted - Math.atan2(u[1] + v[1], u[0] + v[0]));
  if (chords.some(d => d < 39 || d > 45) || gaps.some(t => t <= 0 || t > 700) ||
      sign * angle / seconds <= .15 || margin < .55) return null;
  return { angle, seconds, index: b.index, server: b.server, margin, chords, gaps };
}

// One chronological run, with the original extra neighboring-triplet gates.
export class ArcSelector {
  constructor(count, sign, minimum = durations.get(count)) {
    this.count = count; this.sign = sign; this.minimum = minimum; this.reset();
  }
  reset() { this.anchors = []; this.neighbors = []; this.run = []; this.result = null; }
  push(p) {
    if (this.result) return this.result;
    this.anchors.push(p);
    if (this.anchors.length < 3) return null;
    if (this.anchors.length > 3) this.anchors.shift();
    this.neighbors.push(triplet(...this.anchors, this.count, this.sign));
    if (this.neighbors.length < 3) return null;
    if (this.neighbors.length > 3) this.neighbors.shift();
    if (this.neighbors.some(t => !t)) { this.run = []; return null; }
    this.run.push(this.neighbors[1]);
    const seconds = this.run.reduce((s, t) => s + t.seconds, 0);
    if (this.run.length >= 5 && seconds >= this.minimum) {
      this.result = { first: this.run[0].index, last: this.run.at(-1).index,
        endServer: this.anchors.at(-1).server, triplets: this.run.length, seconds,
        angle: this.run.reduce((s, t) => s + t.angle, 0) };
    }
    return this.result;
  }
}

export function decision(invocations) {
  const dev = invocations.filter(r => r.ordinal <= 2);
  const primary = [key(37, 1), key(37, -1), key(100, 1), key(100, -1)];
  const covered = new Set(dev.filter(r => r.archiveQualified !== false).flatMap(r =>
    Object.entries(r.cells ?? {}).filter(([, c]) => c.observerAgreement !== false).map(([k]) => k)));
  if (!primary.every(k => covered.has(k))) return 'inconclusive-development-coverage';
  const held = invocations.filter(r => r.ordinal >= 3);
  if (held.length !== 2 || held.some(r => !r.qualified || !primary.every(k => r.cells[k])))
    return 'inconclusive-heldout-coverage';
  const accepts = (r, name) => primary.every(k => r.cells[k].compatible.includes(name));
  if (held.every(r => !accepts(r, 'D111')) &&
      [6, 7, 8, 9, 10].some(k => held.every(r => accepts(r, `k${k}`)))) return 'reject-D111-fixed-offset-survives';
  if (held.every(r => accepts(r, 'D111') &&
      [6, 7, 8, 9, 10].every(k => !accepts(r, `k${k}`)))) return 'reject-all-fixed-offsets-D111-survives';
  return 'inconclusive-candidates';
}

export function reserveOrdinal(previous) {
  if (previous.some(r => !Number.isInteger(r.ordinal)) ||
      previous.some((r, i) => r.ordinal !== i + 1) || previous.length >= 4)
    throw Error('invalid or exhausted invocation ledger');
  return previous.length + 1;
}
