import { normalizeInput } from './schedule.mjs';
import { mouseReplay } from './input.mjs';
import { admitsBrowserInput } from '../../apps/wgpu-slice/web/input-targeting.mjs';
import { differences } from './differences.mjs';

// Native has no DOM. Reuse A2's CSS/physical-mouse normalization and production
// admission predicate, retaining the observed compatibility event independently
// of the actual LocalGame method calls. This lane does not test OS input routing.
export function nativeSteps(schedule, deliveries, area) {
  let actions, pendingMouseUp;
  const dispatch = mouseReplay(n => {
    const editor = ['input', 'textarea', 'editable'].includes(n.target), menu = n.target === 'button';
    const admitted = admitsBrowserInput(n, { editor, menu });
    let prevented = false;
    if (['pointerdown', 'mousedown', 'keydown'].includes(n.type)) pendingMouseUp = undefined;
    if (admitted) {
      if (n.type === 'pointermove' || n.type === 'pointerdown' && n.button === 0) {
        actions.push({ kind: 'pointer', x: n.x, y: n.y, down: n.type === 'pointerdown' });
        prevented = n.type === 'pointerdown';
      } else if (['pointerup', 'mouseup'].includes(n.type)) {
        const duplicate = n.type === 'mouseup' && pendingMouseUp !== undefined && pendingMouseUp === n.button;
        pendingMouseUp = n.type === 'pointerup' ? n.button : undefined;
        if (!duplicate) actions.push({ kind: 'release' });
      }
      else if (n.type === 'keydown') {
        if (n.code === 'Enter' && !n.repeat) actions.push({ kind: 'play' });
        else if (n.code === 'Escape') actions.push({ kind: 'stop' });
        else actions.push({ kind: 'key', code: n.code, down: true });
      } else if (n.type === 'keyup') actions.push({ kind: 'key', code: n.code, down: false });
      else if (n.type === 'blur' || n.type === 'resize') actions.push({ kind: n.type });
    }
    return { type: n.type, target: n.target, coordinates: /^(pointer|mouse)/.test(n.type) ? { x: n.x, y: n.y } : null,
      code: n.code, repeat: n.repeat, buttons: n.buttons, prevented };
  });
  return schedule.events.map((e, seq) => {
    const data = deliveries[seq]; actions = [];
    let observed = { target: 'host', type: data?.control ?? e.kind, coordinates: null, code: null, repeat: false, buttons: 0 };
    const extra = {};
    if (data?.control) extra.control = data.control;
    else if (e.kind === 'packet') extra.packet = Array.from(Buffer.from(data.body, 'base64'));
    else if (e.kind === 'input') {
      const { prevented, ...fields } = dispatch({ ...normalizeInput(data, area), buttonsExplicit: data.buttons !== undefined, encoding: data.encoding });
      observed = fields;
    }
    return { event: { seq, ...e, scheduledMs: e.atMs, actualMs: e.atMs, ...observed,
      trusted: false, focused: true, visibility: 'visible', effectIds: [] }, actions, ...extra };
  }).map(({ event: { atMs, ...event }, ...step }) => ({ event, ...step }));
}

export function channelResults(left, right, report) {
  const channel = path => {
    if (!path.startsWith('samples[')) return path.split(/[.[]/)[0];
    const parts = path.replace(/^samples\[\d+\]\./, '').split(/[.[]/);
    if (parts[0] === 'own') return `own.${parts[1] ?? 'presence'}`;
    return parts[0];
  };
  const names = ['manifest', 'events', 'commands', 'boundary', 'delta', 'lag', 'camera', 'quality', 'geometry', 'status', 'final',
    ...['presence', 'token', 'expired', 'deathProgress', 'rawHead', 'correctedHead', 'rawHeading', 'correctedHeading', 'wantedHeading',
      'gaze', 'pupils', 'offsets', 'speed', 'visualSpeed', 'boost', 'boostPhase', 'size', 'rings', 'projected'].map(n => `own.${n}`)];
  const rows = differences(left, right);
  const tolerated = new Set(report.nonExactFloats.filter(r => r.tolerated).map(r => r.path));
  const failures = new Set(report.mismatches.map(r => r.path));
  const counts = new Map(names.map(n => [n, { channel: n, status: 'exact', differences: 0, numerical: 0, failing: 0 }]));
  for (const row of rows) {
    let name = channel(row.path);
    if (!counts.has(name)) name = row.path.startsWith('samples[') ? 'boundary' : name;
    if (!counts.has(name)) counts.set(name, { channel: name, status: 'exact', differences: 0, numerical: 0, failing: 0 });
    const c = counts.get(name); c.differences++;
    if (tolerated.has(row.path) && !failures.has(row.path) && !report.errors.length) c.numerical++;
    else c.failing++;
    c.status = c.failing ? 'failing' : 'numerical';
  }
  return { channels: [...counts.values()], rows };
}
