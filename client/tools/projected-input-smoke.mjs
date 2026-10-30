import assert from 'node:assert/strict';
import { bundle, syntheticSetup } from '../web/synthetic-scene.mjs';
import { createClientInput } from '../web/client-input.mjs';

// Authored packet/input cases at the exact 16 CSS px dead-zone boundary.
// Frozen cameras come from the host-trig baseline, never the candidate head.
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
  return { cases: cases.length, evidence: 'real Wasm prediction/anchors, pointer projection and exact sent bytes; no network' };
}
