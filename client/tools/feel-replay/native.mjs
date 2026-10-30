import { normalizeInput } from './schedule.mjs';
import { mouseReplay } from './input.mjs';
import { admitsBrowserInput } from '../../apps/wgpu-slice/web/input-targeting.mjs';

// Native has no DOM. Reuse A2's CSS/physical-mouse normalization and production
// admission predicate, retaining the observed compatibility event independently
// of the actual LocalGame method calls. This lane does not test OS input routing.
export function nativeSteps(schedule, deliveries, area) {
  let actions, pendingMouseUp;
  const dispatch = mouseReplay(n => {
    // A2 constructs DOM events from CSS options. MouseEvent getters floor these
    // doubles; PointerEvent getters preserve them. The production physical/f32
    // mouse_client_position boundary must not be applied to CSS a second time.
    if (n.type.startsWith('mouse')) n = { ...n, x: Math.floor(n.x), y: Math.floor(n.y) };
    const editor = ['input', 'textarea', 'editable'].includes(n.target), menu = n.target === 'button';
    const admitted = admitsBrowserInput(n, { editor, menu });
    let prevented = false;
    if (['pointerdown', 'mousedown', 'keydown'].includes(n.type)) pendingMouseUp = undefined;
    if (admitted) {
      if (n.type === 'pointermove' || n.type === 'mousemove' || n.type === 'mousedown' && n.button === 0) {
        actions.push({ kind: 'pointer', x: n.x, y: n.y, down: n.type === 'mousedown' });
        prevented = n.type === 'mousedown';
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
