// Authored SYNTHETIC fixture. No recording, official asset, packet body or live peer.
import { parse, scheduleHash, stringify } from './io.mjs';

export function syntheticFixture() {
  const schedule = {
    schema: 3, label: 'synthetic contract exercise', durationMs: 10,
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
    ], pictures: [0, 1], expectedCommands: 1,
  };
  const geometry = { css: { width: 960, height: 540 }, backing: { width: 1500, height: 845 },
    surface: { width: 960, height: 540 }, dpr: 1 };
  const manifest = {
    schema: 2, source: { commit: 'a'.repeat(40), tree: 'b'.repeat(40) },
    artifacts: [{ name: 'synthetic-peer', sha256: 'c'.repeat(64) }],
    input: { label: 'synthetic-no-packets', sha256: null }, scheduleSha256: scheduleHash(schedule),
    peer: 'canvas-wasm', geometry, policy: 'feel-perceptual-v1', completed: true,
  };
  const zero = { x: 0, y: 0 };
  const own = {
    token: 'synthetic-own-1', expired: false, deathProgress: 0, spawnProgress: 0.5, opacity: 0.25, rawHead: { x: 1, y: 2 }, correctedHead: { x: 1, y: 2 },
    rawHeading: 1, correctedHeading: 1, wantedHeading: 1, gaze: 1,
    pupils: { left: zero, right: zero }, offsets: { x: 1, y: 0, fraction: 0, heading: 0 },
    speed: 2, visualSpeed: 2, boost: false, boostPhase: 0,
    size: { segments: 2, fraction: 0, length: 2 },
    body: [{ x: 1, y: 2 }],
    projected: Object.fromEntries(['backing', 'css'].map(k => [k, { head: zero, pupils: [zero, zero] }])),
  };
  let successfulDraws = 0;
  const trace = {
    schema: 3, manifest,
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
      ...b, acknowledgedMs: b.phase === 'draw' ? b.atMs : null, delta: { raw: 1, lagged: 1, steps: 1, doubledSteps: 2 },
      lag: { active: false, multiplier: 1, pingPending: false, pongAgeMs: 0 },
      own: b.eligibility === 'own' ? { ...own, body: b.phase === 'draw' ? own.body : [] } : null,
      camera: { x: 1, y: 2, offset: zero, follow: true, zoom: 1, targetZoom: 1 },
      quality: { requested: 'high', active: 'high', successfulDraws: b.phase === 'draw' ? ++successfulDraws : successfulDraws },
      geometry, status: { prepared: b.phase === 'draw', submitted: b.phase === 'draw', committed: b.phase === 'draw', retries: 0,
        sendFailed: false, presentFailed: false },
    })),
    final: { atMs: 10, events: 4, commands: 1, samples: 5, draws: 2, pendingEffects: 0, pendingFrames: 0,
      lastBoundaryId: 'draw-1', completed: true, unexpectedNetwork: false },
  };
  // Avoid shared object references when tests mutate one sample.
  return parse(stringify({ schedule, trace }));
}
