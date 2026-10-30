// Shared Canvas/WebGPU DOM admission; numeric input policy remains in Rust.
export function isEditorTarget(target, Input, TextArea) {
  return target instanceof Input
    || (typeof TextArea === 'function' && target instanceof TextArea) || !!target?.isContentEditable;
}

export function admitsBrowserInput(event, { editor = false, menu = false } = {}) {
  // Releases belong to the input owner even after focus/pointer targeting changes.
  if (['mouseup', 'pointerup', 'keyup', 'blur'].includes(event.type)) return true;
  return (!editor || event.type === 'input') && !menu;
}
