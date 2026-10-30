import { admitsBrowserInput, isEditorTarget } from './input-targeting.mjs';

// DOM targeting only. The owner retains steering, held-key and boost policy.
export function installLocalInput({ realm, canvas, owner, host, start, controls, signal, observeInput }) {
  const document = realm.document;
  const Input = realm.HTMLInputElement, TextArea = realm.HTMLTextAreaElement;
  const listen = (node, type, fn) => node.addEventListener(type, observeInput ? event => { observeInput(event, 'received'); fn(event); } : fn, { capture: true, signal });
  const targeting = event => ({
    editor: isEditorTarget(event.target, Input, TextArea),
    menu: controls.includes(event.target),
  });
  let pendingMouseUp;
  function release(event) {
    // Canvas releases boost even when no mouse press reached gameplay. Only the
    // matching compatibility mouseup may reuse a release already delivered.
    const duplicate = event.type === 'mouseup' && pendingMouseUp !== undefined && pendingMouseUp === event.button;
    pendingMouseUp = event.type === 'pointerup' ? event.button : undefined;
    observeInput?.(event, duplicate ? 'filtered-compatibility' : 'gameplay');
    if (!duplicate) host.call(() => owner.mouse_up());
  }
  function pointer(event, down) {
    if (!admitsBrowserInput(event, targeting(event))) { observeInput?.(event, 'filtered-target'); return; }
    observeInput?.(event, 'gameplay');
    host.call(() => {
      const r = canvas.getBoundingClientRect();
      if (r.width && r.height) {
        owner.pointer(event.clientX - r.left, event.clientY - r.top, r.width, r.height, down);
      }
    });
    if (down) event.preventDefault();
  }
  // Use one movement/press family so pointer + compatibility mouse events do not
  // submit twice. Both release families may clear a press, including over UI.
  const family = typeof realm.PointerEvent === 'function' ? 'pointer' : 'mouse';
  listen(realm, `${family}move`, event => pointer(event, false));
  for (const type of ['pointerdown', 'mousedown']) listen(realm, type, event => {
    pendingMouseUp = undefined;
    if (type === `${family}down` && event.button === 0) pointer(event, true);
    else observeInput?.(event, 'filtered-family-or-button');
  });
  listen(realm, 'pointerup', release);
  listen(realm, 'mouseup', release);
  listen(realm, 'pointercancel', release);
  listen(realm, 'keydown', event => {
    pendingMouseUp = undefined;
    if (!admitsBrowserInput(event, targeting(event))) { observeInput?.(event, 'filtered-target'); return; }
    observeInput?.(event, 'gameplay');
    if (event.code === 'Enter' && !event.repeat) { if (owner.can_play()) start(); }
    else if (event.code === 'Escape') host.call(() => owner.stop());
    else host.call(() => { if (owner.key(event.code, true)) event.preventDefault(); });
  });
  listen(realm, 'keyup', event => {
    observeInput?.(event, 'gameplay');
    const { editor, menu } = targeting(event);
    host.call(() => {
      if (owner.key(event.code, false) && !editor && !(menu && ['Space', 'Enter'].includes(event.code))) event.preventDefault();
    });
  });
  // Focus loss is deliberately a no-op in DesktopInput; preserve release routing.
  const blur = () => host.call(() => owner.blur());
  listen(realm, 'blur', blur);
  listen(document, 'visibilitychange', () => { if (document.hidden) blur(); });
}
