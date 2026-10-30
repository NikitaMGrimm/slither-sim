import assert from 'node:assert/strict';
import { bundle, syntheticSetup } from '../web/synthetic-scene.mjs';
import { createClientInput } from '../web/client-input.mjs';

// Authored packet/input cases at the exact 16 CSS px dead-zone boundary.
// Fixed camera/event values come from the host-trig baseline, never candidate geometry.
export function projectedInputSmoke(wasm) {
  const u16 = v => [v >>> 8 & 255, v & 255];
  const u24 = v => [v >>> 16 & 255, v >>> 8 & 255, v & 255];
  const cases = [
    { lane: 'prediction', heading: 2408767, direction: 29831, byte: 36, outside: 185,
      camera: [26.038464309663556, 32.95448947251917], pointer: [400, 284] },
    { lane: 'directional anchor', heading: 1989851, direction: 24643, byte: 29, outside: 127,
      camera: [-59.777280663852935, 59.0142077506337], pointer: [384, 300] },
  ];
  for (const test of cases) {
    const scene = new wasm.AnimatedScene(0);
    const input = createClientInput({ InputPolicy: wasm.DesktopInput });
    try {
      const spawn = [115, 0, 0, ...u24(test.heading), 48, ...u24(test.heading),
        ...u16(65535), 0, 0, 0, 0, ...u24(0), ...u24(0), 0, 0, 255,
        ...u24(0), ...u24(0), 127, 127, ...u16(test.direction)];
      scene.apply_frame(bundle(syntheticSetup(), spawn), false);
      scene.set_follows(false);
      if (test.lane === 'prediction') scene.advance(40, false).free();
      else scene.apply_frame(Uint8Array.of(71), false); // Cached directional anchor G.
      scene.set_camera_position(...test.camera);
      const view = scene.control_view();
      input.setConnected(true);
      input.setOwn(view.own);
      const event = (clientX, clientY, follow) => ({ clientX, clientY,
        viewportWidth: 800, viewportHeight: 600, follow, camera: view.camera,
        head: view.own.rawHead, zoom: view.zoom });
      const tick = nowMs => {
        const sends = [];
        input.tick({ nowMs, wholeSteps: 0 }, bytes => sends.push([...bytes]));
        return sends;
      };
      // A first event inside the dead zone sends the packet's wanted heading.
      input.pointer(event(400, 301, true));
      assert.deepEqual(tick(34), [[test.byte]], test.lane);
      // Actual scene prediction/anchors flow through the production projection.
      input.pointer(event(...test.pointer, view.follows));
      assert.deepEqual(tick(68), [], test.lane + ': boundary sends nothing');
      input.pointer(event(test.pointer[0] - 1, test.pointer[1] - 1, view.follows));
      assert.deepEqual(tick(102), [[test.outside]], test.lane + ': outside boundary sends');
    } finally {
      input.dispose();
      scene.free();
    }
  }
  const cos = Math.cos;
  let scene, input, usedHostCos = false;
  try {
    // Observe the semantic seam without changing its results: on this toolchain,
    // Rust and host trig currently produce identical stored f32 correction weights.
    const correctionPhase = Math.PI * 51 / 52;
    Math.cos = value => {
      usedHostCos ||= value === correctionPhase;
      return cos(value);
    };
    scene = new wasm.AnimatedScene(0);
    input = createClientInput({ InputPolicy: wasm.DesktopInput });
    const spawn = [115, 0, 0, ...u24(2408767), 48, ...u24(2408767),
      ...u16(0), 0, 0, 0, 0, ...u24(0), ...u24(0), 0, 0, 255,
      ...u24(0), ...u24(0), 127, 127, ...u16(0)];
    scene.apply_frame(bundle(syntheticSetup(), spawn,
      [61, ...u16(0), ...u16(200), ...u16(100)]), false);
    Math.cos = cos;
    const followedCameras = [];
    for (const now of [8, 16, 24]) {
      scene.advance(now, false).free();
      scene.prepare_draw(800, 600);
      followedCameras.push(scene.control_view().camera);
    }
    let view = scene.control_view();
    assert.equal(view.follows, true);
    assert.ok(view.camera.x > followedCameras[0].x, 'followed correction draws move the camera');
    input.setConnected(true);
    input.setOwn(view.own);
    const pointer = (clientX, clientY) => input.pointer({ clientX, clientY,
      viewportWidth: 800, viewportHeight: 600, follow: view.follows,
      camera: view.camera, head: view.own.rawHead, zoom: view.zoom });
    const tick = nowMs => {
      const sends = [];
      input.tick({ nowMs, wholeSteps: 0 }, bytes => sends.push([...bytes]));
      return sends;
    };
    pointer(400, 301);
    assert.deepEqual(tick(34), [[36]], 'prime wanted heading while following');
    const followed = view.camera;
    scene.set_follows(false);
    view = scene.control_view();
    assert.deepEqual(view.camera, followed, 'freeze retains the corrected camera');
    pointer(614.5834627490042, 415.2917313745021);
    assert.deepEqual(tick(68), [], 'frozen corrected camera: boundary sends nothing');
    pointer(613.5834627490042, 414.2917313745021);
    assert.deepEqual(tick(102), [[127]], 'frozen corrected camera: outside boundary sends');
    assert.ok(usedHostCos, 'frozen camera correction weights retain the host trig seam');
  } finally {
    Math.cos = cos;
    input?.dispose();
    scene?.free();
  }
  return { cases: cases.length + 1, evidence: 'real Wasm prediction/anchors/corrected frozen camera, pointer projection and exact sent bytes; no network' };
}
