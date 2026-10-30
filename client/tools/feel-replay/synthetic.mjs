// Authored SYNTHETIC fixture. No recording, official asset, packet body or live peer.
import { parse, scheduleHash, stringify } from './io.mjs';

export function syntheticFixture() {
  const schedule = {
    schema: 1, label: 'synthetic contract exercise', durationMs: 10,
    events: [
      { sourceId: 'packet-0', kind: 'packet', atMs: 0 },
      { sourceId: 'input-0', kind: 'input', atMs: 0 },
      { sourceId: 'frame-0', kind: 'frame', atMs: 0 },
      { sourceId: 'frame-1', kind: 'frame', atMs: 10 },
    ],
    boundaries: [
      { id: 'packet-0-after', eventSeq: 0, phase: 'update', frameId: 0, drawId: null, atMs: 0, eligibility: 'pre-appearance' },
      { id: 'update-0', eventSeq: 2, phase: 'update', frameId: 0, drawId: null, atMs: 0, eligibility: 'own' },
      { id: 'draw-0', eventSeq: 2, phase: 'draw', frameId: 0, drawId: 0, atMs: 0, eligibility: 'own' },
      { id: 'update-1', eventSeq: 3, phase: 'update', frameId: 1, drawId: null, atMs: 10, eligibility: 'own' },
      { id: 'draw-1', eventSeq: 3, phase: 'draw', frameId: 1, drawId: 1, atMs: 10, eligibility: 'own' },
    ], expectedCommands: 1,
  };
  const geometry = { css: { width: 960, height: 540 }, backing: { width: 1500, height: 845 },
    surface: { width: 960, height: 540 }, dpr: 1 };
  const manifest = {
    schema: 1, product: { commit: 'a'.repeat(40), tree: 'b'.repeat(40), core: 'synthetic-core' },
    artifacts: [{ name: 'synthetic-peer', sha256: 'c'.repeat(64) }],
    toolVersions: [{ name: 'synthetic-tool', version: '1' }],
    input: { label: 'synthetic-no-packets', sha256: null }, scheduleSha256: scheduleHash(schedule),
    peer: 'canvas-wasm', entropy: { seed: '17', requestStreamSha256: 'd'.repeat(64) },
    startup: ['clock-zero', 'pointer-center', 'play'], geometry, qualityRequest: 'high',
    presentation: { gpu: null, mode: null, maxFrameLatency: null }, numericalPolicy: null,
  };
  const zero = { x: 0, y: 0 };
  const own = {
    token: 'synthetic-own-1', rawHead: { x: 1, y: 2 }, correctedHead: { x: 1, y: 2 },
    rawHeading: 1, correctedHeading: 1, wantedHeading: 1, gaze: 1,
    pupils: { left: zero, right: zero }, offsets: { x: 1, y: 0, fraction: 0, heading: 0 },
    speed: 2, visualSpeed: 2, boost: false, boostPhase: 0,
    size: { segments: 2, fraction: 0, length: 2 },
    rings: Object.fromEntries([['head', 159], ['angle', 26], ['length', 53]]
      .map(([k, count]) => [k, { index: 0, pending: 0, values: Array(count).fill(0) }])),
    projected: Object.fromEntries(['backing', 'css'].map(k => [k, { head: zero, pupils: [zero, zero] }])),
  };
  const trace = {
    schema: 1, manifest,
    events: schedule.events.map((e, seq) => ({
      seq, sourceId: e.sourceId, kind: e.kind, scheduledMs: e.atMs, actualMs: e.atMs,
      target: e.kind === 'input' ? 'canvas' : 'host', type: e.kind === 'input' ? 'pointermove' : e.kind,
      coordinates: e.kind === 'input' ? { x: 480, y: 270 } : null, code: null,
      trusted: false, repeat: false, buttons: 0, focused: true, visibility: 'visible',
      effectIds: seq === 2 ? ['synthetic-send-0'] : [],
    })),
    commands: [{ seq: 0, eventSeq: 2, frameId: 0, effectId: 'synthetic-send-0', bytes: [10],
      decisionMs: 0, sendMs: 0, ackMs: 0, succeeded: true }],
    samples: schedule.boundaries.map(b => ({
      ...b, delta: { raw: 1, lagged: 1, steps: 1, doubledSteps: 2 },
      lag: { active: false, multiplier: 1, pingPending: false, pongAgeMs: 0 },
      own: b.eligibility === 'own' ? own : null,
      camera: { x: 1, y: 2, offset: zero, index: 0, pending: 0, ring: Array(124).fill(0), follow: true, zoom: 1, targetZoom: 1 },
      quality: { requested: 'high', active: 'high', successfulDraws: b.drawId === null ? 0 : b.drawId + 1 },
      geometry, status: { prepared: true, submitted: true, committed: true, retries: 0,
        sendFailed: false, presentFailed: false },
    })),
    final: { atMs: 10, events: 4, commands: 1, samples: 5, draws: 2, pendingEffects: 0, pendingFrames: 0,
      lastBoundaryId: 'draw-1', completed: true, unexpectedNetwork: false },
  };
  // Avoid shared object references when tests mutate one sample.
  return parse(stringify({ schedule, trace }));
}
export const syntheticNumericalPolicy = {
  cause: 'SYNTHETIC: authored platform-math perturbation, not a measured cause',
  worldCap: 1e-9, angleCap: 1e-9, zoomCap: 1e-12, f32Ulps: 1,
};
