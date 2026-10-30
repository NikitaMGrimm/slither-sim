// Authored DOM-operation doubles; no native browser or Wasm acceptance.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createOwnedAnnouncementUi } from '../web/game-owned-announcement-ui.mjs';
import { createGateTestDom } from './game-gate-test-fixture.mjs';

function fixture() {
  const native = createGateTestDom({ hud: true, ownedHud: true });
  const document = new native.Document(), parent = document.createElement('main'); document.appendChild(parent);
  let current = true, fail = false, writes = 0;
  const getter = (prototype, name) => {
    const get = Object.getOwnPropertyDescriptor(prototype, name).get;
    return value => Reflect.apply(get, value, []);
  };
  const dom = Object.fromEntries(['nodeType', 'ownerDocument', 'parentNode', 'firstChild', 'nextSibling', 'isConnected']
    .map(name => [name, getter(native.Node.prototype, name)]));
  for (const name of ['localName', 'namespaceURI']) dom[name] = getter(native.Element.prototype, name);
  dom.style = getter(native.hud.HTMLElement.prototype, 'style');
  dom.setText = (node, text) => { if (fail) throw new Error('synthetic text failure'); node.data = text; writes++; };
  dom.setStyle = (style, name, value) => style.setProperty(name, value);
  const construct = { createElement: (doc, tag) => doc.createElement(tag), createTextNode: (doc, text) => doc.createTextNode(text),
    appendChild: (node, child) => node.appendChild(child), removeChild: (node, child) => node.removeChild(child) };
  const ui = createOwnedAnnouncementUi({ document, parent, dom, construct, isCurrentDocument: () => current });
  return { ui, parent, document, style: node => native.hud.style(dom.style(node)),
    writes: () => writes, current: value => { current = value; }, fail: value => { fail = value; } };
}
const span = (text, fontSize = 13, bold = false, alpha = .5) => ({ kind: 'text', text,
  family: 'Arial, Helvetica Neue, Helvetica, sans-serif', fontSize, bold, italic: true, alpha });
const view = () => ({ schema: 1, present: true, opacity: .6, left: 8, top: 4, width: 300, height: 228,
  blocks: [{ spaceAfter: 5, fragments: [span('"', 17, true), span('<tag> &  text', 17, true, 1), span('"', 17, true)] },
    { spaceAfter: 0, fragments: [span('- '), span('name', 13, true, .75), span(", today's longest"),
      { kind: 'break' }, span('with a length of '), span('10', 13, true, .65)] }] });

test('passive lease submits literal spans, explicit breaks and multiplies alpha through the holder', () => {
  const f = fixture(); assert.equal(f.parent.children.length, 0);
  const holder = f.ui.mount(); assert.equal(f.style(holder).get('display'), 'none');
  f.ui.render(view());
  assert.equal(f.style(holder).get('opacity'), '0.6'); assert.equal(f.style(holder).get('left'), '8px');
  assert.equal(f.style(holder).get('white-space'), 'normal');
  assert.equal(f.style(holder).get('overflow-wrap'), 'break-word');
  assert.equal(f.style(holder).get('overflow'), 'hidden');
  assert.equal(f.style(holder).get('top'), '4px');
  assert.equal(f.style(holder).get('width'), '300px');
  assert.equal(f.style(holder).get('height'), '228px');
  assert.deepEqual(holder.children[0].children.filter((_, i) => i % 2 === 0).slice(0, 3)
    .map(node => node.children[0].data), ['"', '<tag> &  text', '"']);
  const quote = holder.children[0].children[0], message = holder.children[0].children[2];
  assert.equal(quote.localName, 'span'); assert.equal(message.children.length, 1);
  assert.equal(f.style(quote).get('font-size'), '17px');
  assert.equal(f.style(quote).get('font-style'), 'italic');
  assert.equal(f.style(quote).get('opacity'), '0.5');
  assert.equal(f.style(message).get('opacity'), '1');
  assert.equal(f.style(holder.children[1].children[7]).get('display'), 'inline');
  assert.equal(f.style(holder.children[0]).get('margin-bottom'), '5px');
  assert.equal(f.writes(), 8); f.ui.render({ ...view(), opacity: .5 });
  assert.equal(f.writes(), 8, 'fade-only publication retains existing Text nodes without rewriting content');
  f.ui.render({ ...view(), opacity: -1, blocks: view().blocks.slice(0, 1) });
  assert.equal(f.style(holder).get('opacity'), '0'); assert.equal(f.style(holder.children[1]).get('display'), 'none');
  f.ui.dispose(); f.ui.dispose(); assert.equal(f.parent.children.length, 0);
});

test('invalid plans fail before writes; replaced topology and stale documents fail closed', () => {
  const f = fixture(), holder = f.ui.mount();
  assert.throws(() => f.ui.render({ ...view(), blocks: [view().blocks[0], { fragments: [span(null)], spaceAfter: 0 }] }));
  assert.equal(f.style(holder).get('display'), 'none');
  holder.children[0].appendChild(f.document.createTextNode('intruder'));
  assert.throws(() => f.ui.render(view())); f.ui.dispose();
  const stale = fixture(); stale.ui.mount(); stale.current(false);
  assert.throws(() => stale.ui.render(view()), /Stale/); stale.ui.dispose();
  assert.equal(stale.parent.children.length, 1, 'no writes/removal in a stale document');
});

test('branch replacement hides stale spans and breaks while node count stays fixed for maximum text', () => {
  const f = fixture(), holder = f.ui.mount();
  const count = node => 1 + (node.children ?? []).reduce((n, child) => n + count(child), 0);
  const nodes = count(holder);
  f.ui.render(view());
  for (const fragments of [
    [span('- today\'s longest'), { kind: 'break' }, span('with a length of '), span('10', 13, true, .65)],
    [span("Today's longest was "), span('ÿ name', 13, true, .75), { kind: 'break' }, span('with a length of '), span('10', 13, true, .65)],
    [span("Today's longest: "), span('10', 13, true, .75)],
    [span('ÿ'.repeat((1 << 20) - 8), 17, true, 1)],
  ]) {
    f.ui.render({ ...view(), blocks: [{ spaceAfter: 0, fragments }] });
    assert.equal(count(holder), nodes);
    assert.equal(f.style(holder.children[1]).get('display'), 'none');
    for (let slot = fragments.length; slot < 6; slot++) {
      assert.equal(f.style(holder.children[0].children[slot * 2]).get('display'), 'none');
      assert.equal(f.style(holder.children[0].children[slot * 2 + 1]).get('display'), 'none');
    }
  }
  f.ui.dispose();
});

test('submission failures propagate and explicit disposal removes the owned mount', () => {
  const f = fixture(); f.ui.mount(); f.fail(true);
  assert.throws(() => f.ui.render(view()), /synthetic text failure/);
  f.ui.dispose(); assert.equal(f.parent.children.length, 0);
});
