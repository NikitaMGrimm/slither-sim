import assert from 'node:assert/strict';
import { syntheticSetup } from '../web/synthetic-scene.mjs';

// Authored maximum-retention packets through the real copied Canvas Wasm ABI.
// No image source, renderer, reference input, browser or socket is used.
export function snakeCapacitySmoke(wasm, { exhaustive = false } = {}) {
  const u16 = n => [n >>> 8 & 255, n & 255];
  const u24 = n => [n >>> 16 & 255, n >>> 8 & 255, n & 255];
  const spawn = (id, anchors) => Uint8Array.from([115, ...u16(id), ...u24(0), 48, ...u24(0), ...u16(128),
    ...u24(0), 0, ...u24(32550 * 5), ...u24(32550 * 5), 0, 0, 0,
    ...u24(30000 * 5), ...u24(32550 * 5),
    ...Array.from({ length: anchors - 2 }, () => [129, 127]).flat(), ...u16(0)]);
  // The fast cases separately prove the live-ID and detail-pressure rules.
  // Explicit manual exhaustive mode retains the full anchor-product proof.
  const cases = exhaustive ? [[512, 4096]] : [[512, 26], [17, 4096]];
  for (const [population, anchors] of cases) {
    const scene = new wasm.AnimatedScene(0);
    try {
      scene.apply_frame(syntheticSetup(), false);
      scene.apply_frame(spawn(1, anchors), false);
      const own = scene.own_body();
      for (let id = 2; id <= population; id++) {
        scene.apply_frame(spawn(id, anchors), false);
        const state = JSON.parse(scene.snapshot());
        assert.ok(state.retainedBodyPoints <= (state.snakes > 256 || state.snakes * anchors > 65536 ? 32768 : 131072));
        assert.equal(state.own.physicalPoints, anchors);
        assert.equal(state.own.logicalSegments, anchors);
        assert.deepEqual(scene.own_body(), own);
      }
      const before = scene.snapshot();
      assert.equal(JSON.parse(before).snakes, population);
      if (population === 512) {
        assert.throws(() => scene.apply_frame(spawn(513, anchors), false), /scene collection limit/);
        assert.equal(scene.snapshot(), before, '513th live appearance rejects the whole transaction');
      }
      scene.set_zoom(0.01);
      scene.prepare_draw_with_style(960, 540, 1, true);
      assert.deepEqual(scene.own_body(), own, 'numeric draw never compacts the own body');
    } finally { scene.free(); }
  }
  return { cases: cases.map(([snakes, anchors]) => ({ snakes, receivedAnchors: snakes * anchors })), synthetic: true, network: false };
}
