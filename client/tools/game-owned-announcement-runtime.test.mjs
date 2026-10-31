// Actual default entry/gate/gameplay/DOM modules; generated Wasm views are doubles.
import { test } from 'node:test';
import { runGameModuleTest } from './game-module-test-fixture.mjs';

test('default announcement publication uses Rust views without a second draw or timer', async () => {
  await runGameModuleTest(async ({ assert, createGraph }) => {
    const graph = createGraph({ hudNative: true, ownedHudNative: true, ownedPlayNative: true,
      leaderboardNative: true, minimapNative: true,
      prepareWasm(api, run) {
        api.LeaderboardScoreProjector = run('(class { project() { throw Error("No board expected"); } free() {} })');

      } });
    graph.hudWasm();graph.stubLogin();
    const entry = await graph.load('bootstrap.mjs'), runtime = await entry.gamePageRuntime;
    const root = graph.f.document.getElementById('slither-client-root'), holder = root.children[6];
    const style = node => graph.f.edit.hud.style(node.style);
    assert.equal(graph.announcements.enabled, 1); assert.equal(graph.announcements.reads, 1);
    assert.equal(style(holder).get('display'), 'none'); assert.equal(runtime.diagnostics().draws, 0);
    graph.announcements.view = { schema: 1, present: true, opacity: .4, left: 8, top: 4, width: 300, height: 228,
      blocks: [{ spaceAfter: 5, fragments: [{ kind: 'text', text: '"<tag> & text"',
        family: 'Arial, Helvetica Neue, Helvetica, sans-serif', fontSize: 17, bold: true, italic: true, alpha: 1 }] },
      { spaceAfter: 0, fragments: [{ kind: 'text', text: "- synthetic player, today's longest",
        family: 'Arial, Helvetica Neue, Helvetica, sans-serif', fontSize: 13, bold: false, italic: true, alpha: .5 }] }] };
    const [id, callback] = graph.f.raf.entries().next().value; graph.f.raf.delete(id); callback(16);
    assert.equal(graph.announcements.reads, 2); assert.equal(runtime.diagnostics().draws, 1);
    assert.equal(style(holder).get('display'), 'block'); assert.equal(style(holder).get('opacity'), '0.4');
    const diagnostic = graph.f.document.getElementById('slither-client-status');
    assert.ok(Number.parseFloat(style(diagnostic).get('top'))
      >= graph.announcements.view.top + graph.announcements.view.height,
    'the visible diagnostic status is below the announcement rectangle');
    assert.notEqual(diagnostic.hidden, true, 'diagnostic text remains available');
    assert.equal(graph.f.edit.hud.character(holder.children[0].children[0].children[0]).data, '"<tag> & text"');
    assert.equal(graph.f.intervals.size, 0); assert.equal(graph.f.raf.size, 1); assert.equal(graph.f.sockets, 0);
    runtime.dispose(); runtime.dispose(); assert.equal(root.children.length, 3); assert.equal(graph.f.raf.size, 0);
  });
});
