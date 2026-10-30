import { admitsBrowserInput, isEditorTarget } from './input-targeting.mjs';

// DOM targeting only. The owner retains steering, held-key and boost policy.
export function installLocalInput({ realm, canvas, owner, host, start, controls, signal }) {
  const document = realm.document;
  const Input = realm.HTMLInputElement, TextArea = realm.HTMLTextAreaElement;
  const listen = (node, type, fn) => node.addEventListener(type, fn, { capture: true, signal });
  const targeting = event => ({
    editor: isEditorTarget(event.target, Input, TextArea),
    menu: controls.includes(event.target),
  });
  let mouseHeld = false;
  function release() {
    if (!mouseHeld) return;
    mouseHeld = false;
    host.call(() => owner.mouse_up());
  }
  function blur() {
    mouseHeld = false;
    host.call(() => owner.blur());
  }
  function pointer(event, down) {
    if (!admitsBrowserInput(event, targeting(event))) return;
    host.call(() => {
      const r = canvas.getBoundingClientRect();
      if (r.width && r.height) {
        owner.pointer(event.clientX - r.left, event.clientY - r.top, r.width, r.height, down);
        if (down) mouseHeld = true;
      }
    });
    if (down) event.preventDefault();
  }
  // Use one movement/press family so pointer + compatibility mouse events do not
  // submit twice. Both release families may clear a press, including over UI.
  const family = typeof realm.PointerEvent === 'function' ? 'pointer' : 'mouse';
  listen(realm, `${family}move`, event => pointer(event, false));
  listen(realm, `${family}down`, event => { if (event.button === 0) pointer(event, true); });
  listen(realm, 'pointerup', release);
  listen(realm, 'mouseup', release);
  listen(realm, 'pointercancel', release);
  listen(realm, 'keydown', event => {
    if (!admitsBrowserInput(event, targeting(event))) return;
    if (event.code === 'Enter' && !event.repeat) { if (owner.can_play()) start(); }
    else if (event.code === 'Escape') host.call(() => owner.stop());
    else host.call(() => { if (owner.key(event.code, true)) event.preventDefault(); });
  });
  listen(realm, 'keyup', event => {
    const { editor, menu } = targeting(event);
    host.call(() => {
      if (owner.key(event.code, false) && !editor && !(menu && ['Space', 'Enter'].includes(event.code))) event.preventDefault();
    });
  });
  listen(realm, 'blur', blur);
  listen(document, 'visibilitychange', () => { if (document.hidden) blur(); });
}
