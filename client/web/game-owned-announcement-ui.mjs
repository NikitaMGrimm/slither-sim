// Passive DOM submission. Rust supplies all announcement text, order and fade.
const invalid = () => { throw new TypeError('Invalid owned announcement UI input'); };
const html = 'http://www.w3.org/1999/xhtml';

function validate(view) {
  if (!view || view.schema !== 1 || typeof view.present !== 'boolean'
    || !Number.isFinite(view.opacity) || !Array.isArray(view.blocks) || view.blocks.length > 2) invalid();
  for (const key of ['left', 'top', 'width', 'height']) {
    if (!Number.isInteger(view[key]) || view[key] < 0 || view[key] > 65535) invalid();
  }
  for (const block of view.blocks) {
    if (!block || !Array.isArray(block.fragments) || block.fragments.length > 6
      || !Number.isInteger(block.spaceAfter) || block.spaceAfter < 0 || block.spaceAfter > 5) invalid();
    for (const fragment of block.fragments) {
      if (!fragment || !['text', 'break'].includes(fragment.kind)) invalid();
      if (fragment.kind === 'text' && (typeof fragment.text !== 'string'
        || !['Arial, Helvetica Neue, Helvetica, sans-serif', 'Lucida Sans Unicode, Lucida Grande, sans-serif'].includes(fragment.family)
        || typeof fragment.bold !== 'boolean' || typeof fragment.italic !== 'boolean'
        || !Number.isFinite(fragment.fontSize) || fragment.fontSize <= 0 || fragment.fontSize > 24
        || !Number.isFinite(fragment.alpha) || fragment.alpha < 0 || fragment.alpha > 1)) invalid();
    }
  }
  if (view.present !== (view.blocks.length > 0)) invalid();
}

/** Inert, owned native-node lease; no timers, packet parsing or text policy. */
export function createOwnedAnnouncementUi({ document, parent, dom, construct, isCurrentDocument }) {
  for (const name of ['nodeType', 'ownerDocument', 'parentNode', 'firstChild', 'nextSibling',
    'isConnected', 'localName', 'namespaceURI', 'style', 'setText', 'setStyle']) {
    if (typeof dom?.[name] !== 'function') invalid();
  }
  for (const name of ['createElement', 'createTextNode', 'appendChild', 'removeChild']) {
    if (typeof construct?.[name] !== 'function') invalid();
  }
  if (!document || !parent || document === parent || typeof isCurrentDocument !== 'function') invalid();
  let attempted = false, mounted = false, disposed = false, holder;
  const entries = [], blocks = [], fragments = [];
  function current() {
    if (isCurrentDocument() !== true) throw new Error('Stale owned announcement document');
    if (dom.nodeType(document) !== 9 || !dom.isConnected(document)
      || dom.nodeType(parent) !== 1 || dom.ownerDocument(parent) !== document
      || dom.namespaceURI(parent) !== html || !dom.isConnected(parent)) invalid();
  }
  function owner(entry) {
    current();
    if (dom.nodeType(entry.node) !== entry.type || dom.ownerDocument(entry.node) !== document
      || (entry.type === 1 && (dom.namespaceURI(entry.node) !== html || dom.localName(entry.node) !== entry.tag
        || dom.style(entry.node) !== entry.style))) invalid();
  }
  function inspect() {
    current();
    for (const entry of entries) {
      owner(entry);
      if (dom.parentNode(entry.node) !== entry.parent
        || dom.firstChild(entry.node) !== (entry.children[0] ?? null)
        || (entry !== holder && dom.nextSibling(entry.node) !== entry.next)
        || dom.isConnected(entry.node) !== mounted) invalid();
    }
    current();
  }
  function effect(fn, ...args) { inspect(); const result = fn(...args); inspect(); return result; }
  function fresh(type, tag = 'div') {
    current();
    const node = type === 1 ? construct.createElement(document, tag) : construct.createTextNode(document, '');
    if (node === parent || node === document || entries.some(entry => entry.node === node)) invalid();
    const entry = { node, type, tag, style: type === 1 ? dom.style(node) : null, parent: null, children: [], next: null };
    owner(entry);
    if (dom.parentNode(node) !== null || dom.firstChild(node) !== null
      || dom.nextSibling(node) !== null || dom.isConnected(node) !== false
      || (entry.style && entries.some(other => other.style === entry.style))) invalid();
    entries.push(entry); return entry;
  }
  function append(target, child) {
    inspect(); construct.appendChild(target.node, child.node);
    const previous = target.children.at(-1);
    if (previous) entries.find(entry => entry.node === previous).next = child.node;
    target.children.push(child.node); child.parent = target.node; inspect();
  }
  function style(entry, values) {
    for (const [key, value] of Object.entries(values)) effect(dom.setStyle, entry.style, key, String(value));
  }
  function mount() {
    if (attempted || disposed) throw new Error('Owned announcement mount already consumed');
    attempted = true; current(); holder = fresh(1);
    for (let index = 0; index < 2; index++) {
      const block = fresh(1), slots = []; blocks.push(block); fragments.push(slots);
      for (let slot = 0; slot < 6; slot++) {
        const span = fresh(1, 'span'), text = fresh(3), br = fresh(1, 'br');
        append(span, text); append(block, span); append(block, br);
        style(span, { display: 'none' }); style(br, { display: 'none' });
        slots.push({ span, text, br, value: undefined });
      }
      append(holder, block);
      style(block, { display: 'none', margin: '0px', 'pointer-events': 'none' });
    }
    style(holder, { position: 'fixed', display: 'none', 'pointer-events': 'none', color: '#ffffff',
      'font-family': 'Arial, Helvetica Neue, Helvetica, sans-serif', 'font-size': '13px',
      'white-space': 'normal', 'overflow-wrap': 'break-word', overflow: 'hidden', 'z-index': 7 });
    inspect(); construct.appendChild(parent, holder.node); holder.parent = parent; mounted = true; inspect();
    return holder.node;
  }
  function render(view) {
    if (!mounted || disposed) throw new Error('Owned announcement is not mounted');
    inspect(); validate(view);
    style(holder, { display: view.present ? 'block' : 'none', opacity: Math.max(0, Math.min(1, view.opacity)),
      left: `${view.left}px`, top: `${view.top}px`, width: `${view.width}px`, height: `${view.height}px` });
    for (let index = 0; index < 2; index++) {
      const block = view.blocks[index];
      style(blocks[index], { display: block ? 'block' : 'none', 'margin-bottom': `${block?.spaceAfter ?? 0}px` });
      for (let slot = 0; slot < 6; slot++) {
        const fragment = block?.fragments[slot], entry = fragments[index][slot];
        style(entry.br, { display: fragment?.kind === 'break' ? 'inline' : 'none' });
        style(entry.span, { display: fragment?.kind === 'text' ? 'inline' : 'none' });
        if (fragment?.kind === 'text') {
          style(entry.span, { 'font-family': fragment.family, 'font-size': `${fragment.fontSize}px`,
            'font-weight': fragment.bold ? 'bold' : 'normal', 'font-style': fragment.italic ? 'italic' : 'normal',
            opacity: fragment.alpha });
          if (entry.value !== fragment.text) {
            effect(dom.setText, entry.text.node, fragment.text);
            entry.value = fragment.text;
          }
        }
      }
    }
  }
  function dispose() {
    if (disposed) return;
    disposed = true;
    try {
      if (holder && isCurrentDocument() === true && dom.ownerDocument(holder.node) === document
        && dom.ownerDocument(parent) === document && dom.parentNode(holder.node) === parent) {
        construct.removeChild(parent, holder.node);
      }
    } finally {
      mounted = false; entries.length = blocks.length = fragments.length = 0;
      holder = document = parent = dom = construct = isCurrentDocument = undefined;
    }
  }
  return Object.freeze({ mount, render, dispose });
}
