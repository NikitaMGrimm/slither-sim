import { normalizeInput } from './schedule.mjs';

export const inputNormalization = 'chromium-primary-mouse-compat-v1';

export function startupPointer(record, area) {
  if (record.startupPointer !== undefined) {
    // A recorded resting mouse identifies the legacy integer CSS position. The
    // optional CSS form avoids a normalized-coordinate round trip in new tapes.
    const p = record.startupPointer;
    const n = p.coordinateSpace === 'css' ? p : normalizeInput({ type: 'mousemove', ...p }, area);
    if (![n.x, n.y].every(Number.isFinite) || n.x < 0 || n.y < 0 || n.x > area.width || n.y > area.height) throw new Error('Invalid startup pointer');
    return { source: 'tape-resting-mouse', x: Math.trunc(n.x), y: Math.trunc(n.y) };
  }
  return { source: 'integer-css-center', x: Math.trunc(area.width / 2), y: Math.trunc(area.height / 2) };
}

// Pointer Events 3 §11.2 and Chromium PointerEventManager::SendMousePointerEvent;
// browser-peers.md links the canonical sources and browser-synthetic.json.
// This is DOM transport normalization, with no gameplay/input-owner policy.
export function mouseReplay(dispatch, suppressed = () => {}) {
  let preventMouse = false, buttons = 0;
  return input => {
    if (!input.type.startsWith('mouse') || input.encoding === 'dom-event') return dispatch(input);
    if (input.button !== 0) throw new Error('Physical mouse replay supports the primary button');
    const type = input.type.replace('mouse', 'pointer');
    if (type === 'pointerdown') buttons = 1;
    if (type === 'pointerup') buttons = 0;
    const pointer = { ...input, type, buttons: input.buttonsExplicit ? input.buttons : buttons,
      button: type === 'pointermove' ? -1 : input.button };
    // Chromium also clears suppression when a hover move proves the button is up.
    if (type === 'pointermove' && pointer.buttons === 0) preventMouse = false;
    let observed = dispatch(pointer);
    if (type === 'pointerdown' && observed.prevented) preventMouse = true;
    const compatibility = { ...input, buttons: pointer.buttons, x: Math.trunc(input.x), y: Math.trunc(input.y) };
    if (!preventMouse) observed = dispatch(compatibility);
    else suppressed(compatibility);
    if (type === 'pointerup') preventMouse = false;
    return observed;
  };
}
