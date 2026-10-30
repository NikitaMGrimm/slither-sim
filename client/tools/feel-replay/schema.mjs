// Authored offline contract, version 1. No packet bodies or runtime adapters.
const nullable = schema => ({ nullable: schema });
const list = (schema, length) => ({ list: schema, ...(length === undefined ? {} : { length }) });
const choice = (...values) => ({ choice: values });
const point = type => ({ x: type, y: type });
const extent = { width: 'uint', height: 'uint' };
const ring = (slots, channels = 1) => ({ index: 'uint', pending: 'int', values: list('f32-exact', slots * channels) });
const geometry = { css: extent, backing: extent, surface: extent, dpr: 'positive' };
const boundary = {
  id: 'string', eventSeq: 'uint', phase: choice('update', 'draw', 'prepare', 'submit', 'commit'),
  frameId: 'uint', drawId: nullable('uint'), atMs: 'time',
  eligibility: choice('own', 'pre-appearance'),
};
export const scheduleSchema = {
  schema: choice(1), label: 'string', durationMs: 'time',
  events: list({ sourceId: 'string', kind: choice('packet', 'input', 'frame'), atMs: 'time' }),
  boundaries: list(boundary), expectedCommands: 'uint',
};
export const numericalPolicySchema = nullable({
  cause: 'string', worldCap: 'positive', angleCap: 'positive', zoomCap: 'positive', f32Ulps: 'uint',
});
export const manifestSchema = {
  schema: choice(1), product: { commit: 'gitHash', tree: 'gitHash', core: 'string' },
  artifacts: list({ name: 'string', sha256: 'hash' }),
  toolVersions: list({ name: 'string', version: 'string' }),
  input: { label: 'string', sha256: nullable('hash') }, scheduleSha256: 'hash',
  peer: choice('canvas-wasm', 'webgpu-wasm', 'native'),
  entropy: { seed: 'string', requestStreamSha256: 'hash' }, startup: list('string'),
  geometry, qualityRequest: 'string',
  presentation: { gpu: nullable('string'), mode: nullable('string'), maxFrameLatency: nullable('uint') },
  numericalPolicy: numericalPolicySchema,
};
const own = {
  token: 'string', rawHead: point('world'), correctedHead: point('world'),
  rawHeading: 'radians', correctedHeading: 'radians', wantedHeading: 'radians', gaze: 'radians',
  pupils: { left: point('number'), right: point('number') },
  offsets: { x: 'f32', y: 'f32', fraction: 'f32', heading: 'f32' },
  speed: 'number', visualSpeed: 'number', boost: 'boolean', boostPhase: 'number',
  size: { segments: 'uint', fraction: 'number', length: 'number' },
  // Stored rings stay exact even in the numerical lane; only current offsets get a ULP cap.
  rings: { head: ring(53, 3), angle: ring(26), length: ring(53) },
  projected: { backing: { head: point('number'), pupils: list(point('number'), 2) },
    css: { head: point('number'), pupils: list(point('number'), 2) } },
};
export const traceSchema = {
  schema: choice(1), manifest: manifestSchema,
  events: list({
    seq: 'uint', sourceId: 'string', kind: choice('packet', 'input', 'frame'),
    scheduledMs: 'time', actualMs: 'time', target: 'string', type: 'string',
    coordinates: nullable(point('number')), code: nullable('string'),
    trusted: 'boolean', repeat: 'boolean', buttons: 'uint', focused: 'boolean',
    visibility: choice('visible', 'hidden'), effectIds: list('string'),
  }),
  commands: list({
    seq: 'uint', eventSeq: 'uint', frameId: 'uint', effectId: 'string',
    bytes: list('byte'), decisionMs: 'time', sendMs: 'time', ackMs: 'time', succeeded: 'boolean',
  }),
  samples: list({
    ...boundary,
    delta: { raw: 'number', lagged: 'number', steps: 'uint', doubledSteps: 'uint' },
    lag: { active: 'boolean', multiplier: 'number', pingPending: 'boolean', pongAgeMs: 'time' },
    own: nullable(own),
    camera: { x: 'world', y: 'world', offset: point('world'), index: 'uint', pending: 'uint',
      ring: list('number', 124), follow: 'boolean', zoom: 'zoom', targetZoom: 'zoom' },
    quality: { requested: 'string', active: 'string', successfulDraws: 'uint' }, geometry,
    status: { prepared: 'boolean', submitted: 'boolean', committed: 'boolean', retries: 'uint',
      sendFailed: 'boolean', presentFailed: 'boolean' },
  }),
  final: { atMs: 'time', events: 'uint', commands: 'uint', samples: 'uint',
    draws: 'uint', pendingEffects: 'uint', pendingFrames: 'uint',
    lastBoundaryId: 'string', completed: 'boolean', unexpectedNetwork: 'boolean' },
};

export function fail(path, message) { throw new Error(`${path}: ${message}`); }
export function validate(value, schema, path = '$') {
  if (typeof schema === 'string') {
    const numeric = ['number', 'world', 'radians', 'zoom', 'f32', 'f32-exact', 'time', 'positive', 'int', 'uint', 'byte'];
    let ok;
    if (numeric.includes(schema)) {
      ok = typeof value === 'number' && Number.isFinite(value);
      if (schema === 'time') ok &&= value >= 0;
      if (schema === 'positive') ok &&= value > 0;
      if (schema === 'int') ok &&= Number.isSafeInteger(value);
      if (schema === 'uint' || schema === 'byte') ok &&= Number.isSafeInteger(value) && value >= 0;
      if (schema === 'byte') ok &&= value <= 255;
      if (schema === 'f32' || schema === 'f32-exact') ok &&= Object.is(Math.fround(value), value);
    } else if (schema === 'string') ok = typeof value === 'string' && value.length > 0;
    else if (schema === 'boolean') ok = typeof value === 'boolean';
    else if (schema === 'hash') ok = typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
    else if (schema === 'gitHash') ok = typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
    if (!ok) fail(path, `required ${schema}${numeric.includes(schema) ? ' (finite)' : ''}`);
  } else if (schema.nullable) {
    if (value !== null) validate(value, schema.nullable, path);
  } else if (schema.list) {
    if (!Array.isArray(value)) fail(path, 'required array');
    if (schema.length !== undefined && value.length !== schema.length) fail(path, `required ${schema.length} values, got ${value.length}`);
    for (let i = 0; i < value.length; i++) validate(value[i], schema.list, `${path}[${i}]`);
  } else if (schema.choice) {
    if (!schema.choice.some(x => Object.is(x, value))) fail(path, `expected ${schema.choice.join('|')}`);
  } else {
    if (!value || typeof value !== 'object' || Array.isArray(value)) fail(path, 'required object');
    for (const key of Object.keys(value)) if (!Object.hasOwn(schema, key)) fail(`${path}.${key}`, 'unknown field');
    for (const [key, child] of Object.entries(schema)) validate(value[key], child, `${path}.${key}`);
  }
  return value;
}

const priority = { packet: 0, input: 1, frame: 2 };
export function orderEvents(events) {
  // Explicit source ordinal preserves arrival order within each equal-time kind.
  return events.map((event, ordinal) => ({ event, ordinal }))
    .sort((a, b) => a.event.atMs - b.event.atMs || priority[a.event.kind] - priority[b.event.kind]
      || a.ordinal - b.ordinal).map(x => x.event);
}
const same = (a, b) => Object.is(a, b);
function unique(values, path) {
  if (new Set(values).size !== values.length) fail(path, 'duplicate ID');
}
export function validateSchedule(schedule) {
  validate(schedule, scheduleSchema, 'schedule');
  if (!schedule.events.length || !schedule.boundaries.length) fail('schedule', 'events and final boundary required');
  unique(schedule.events.map(x => x.sourceId), 'schedule.events');
  unique(schedule.boundaries.map(x => x.id), 'schedule.boundaries');
  const ordered = orderEvents(schedule.events);
  schedule.events.forEach((e, i) => {
    if (ordered[i] !== e) fail(`schedule.events[${i}]`, 'expected packet -> input -> frame time order');
    if (e.atMs > schedule.durationMs) fail(`schedule.events[${i}]`, 'outside duration');
  });
  let previousEvent = -1, previousTime = 0, nextDraw = 0;
  const referencedDraws = [];
  schedule.boundaries.forEach((b, i) => {
    const event = schedule.events[b.eventSeq];
    if (!event || b.eventSeq < previousEvent || b.atMs < event.atMs || b.atMs < previousTime
      || b.atMs > schedule.durationMs) fail(`schedule.boundaries[${i}]`, 'invalid event/time order');
    if (b.phase === 'draw' && b.drawId !== nextDraw++) fail(`schedule.boundaries[${i}].drawId`, 'draw order');
    if (b.phase !== 'draw' && b.drawId !== null) {
      if (b.drawId > nextDraw) fail(`schedule.boundaries[${i}].drawId`, 'unknown draw');
      referencedDraws.push({ index: i, drawId: b.drawId });
    }
    previousEvent = b.eventSeq; previousTime = b.atMs;
  });
  for (const { index, drawId } of referencedDraws) {
    if (drawId >= nextDraw) fail(`schedule.boundaries[${index}].drawId`, 'unknown draw');
  }
  const last = schedule.boundaries.at(-1);
  if (last.eventSeq !== schedule.events.length - 1 || !same(last.atMs, schedule.durationMs)) {
    fail('schedule.boundaries', 'missing final boundary at duration and final event');
  }
  return schedule;
}
export function validateManifest(manifest) {
  validate(manifest, manifestSchema, 'manifest');
  if (!manifest.artifacts.length || !manifest.toolVersions.length || !manifest.startup.length) {
    fail('manifest', 'artifacts, tool versions and startup required');
  }
  unique(manifest.artifacts.map(x => x.name), 'manifest.artifacts');
  unique(manifest.toolVersions.map(x => x.name), 'manifest.toolVersions');
  const p = manifest.numericalPolicy;
  if (p && (p.worldCap > 1e-9 || p.angleCap > 1e-9 || p.zoomCap > 1e-12 || p.f32Ulps > 1)) {
    fail('manifest.numericalPolicy', 'exceeds world/radians 1e-9, zoom 1e-12 or f32 1 ULP caps');
  }
  return manifest;
}

export function safeCommand(bytes) {
  // Protocol >=5 gameplay controls only. Handshake/login bodies must never enter a trace.
  return (bytes.length === 1 && bytes[0] <= 254 && bytes[0] !== 252)
    || (bytes.length === 2 && bytes[0] === 252 && bytes[1] !== 0 && bytes[1] !== 128);
}
export function validateTrace(trace, schedule) {
  validateSchedule(schedule);
  validate(trace, traceSchema, 'trace');
  validateManifest(trace.manifest);
  if (trace.events.length !== schedule.events.length) fail('trace.events', `count ${trace.events.length}, expected ${schedule.events.length} (truncation/extra/duplicate event)`);
  if (trace.commands.length !== schedule.expectedCommands) fail('trace.commands', `count ${trace.commands.length}, expected ${schedule.expectedCommands}`);
  if (trace.samples.length !== schedule.boundaries.length) fail('trace.samples', `boundary count ${trace.samples.length}, expected ${schedule.boundaries.length} (truncation/extra draw)`);
  unique(trace.events.map(x => x.sourceId), 'trace.events');
  const effects = trace.events.flatMap(x => x.effectIds);
  unique(effects, 'trace.events.effectIds');
  let lastActual = 0;
  trace.events.forEach((e, i) => {
    const expected = schedule.events[i];
    if (e.seq !== i || e.sourceId !== expected.sourceId || e.kind !== expected.kind
      || !same(e.scheduledMs, expected.atMs)) fail(`trace.events[${i}]`, 'source order/identity/scheduled timestamp mismatch');
    if (e.actualMs < e.scheduledMs || e.actualMs < lastActual || e.actualMs > schedule.durationMs) fail(`trace.events[${i}].actualMs`, 'outside event order/duration');
    lastActual = e.actualMs;
  });
  unique(trace.commands.map(x => x.effectId), 'trace.commands');
  let lastSend = 0, lastEvent = 0;
  trace.commands.forEach((c, i) => {
    const event = trace.events[c.eventSeq];
    if (c.seq !== i || !event || c.eventSeq < lastEvent || !event.effectIds.includes(c.effectId)) fail(`trace.commands[${i}]`, 'command order/effect identity mismatch');
    if (!safeCommand(c.bytes)) fail(`trace.commands[${i}].bytes`, 'unsafe command; gameplay controls only');
    if (!c.succeeded || c.decisionMs < event.actualMs || c.sendMs < c.decisionMs || c.sendMs < lastSend
      || c.ackMs < c.sendMs || c.ackMs > schedule.durationMs) fail(`trace.commands[${i}]`, 'failed send or decision/send/ack time order');
    if (!schedule.boundaries.some(b => b.frameId === c.frameId)) fail(`trace.commands[${i}].frameId`, 'unknown frame');
    lastSend = c.sendMs; lastEvent = c.eventSeq;
  });
  let appeared = false;
  trace.samples.forEach((s, i) => {
    const b = schedule.boundaries[i];
    for (const key of Object.keys(boundary)) if (!same(s[key], b[key])) fail(`trace.samples[${i}].${key}`, 'boundary order/identity/time mismatch');
    if (s.atMs < trace.events[s.eventSeq].actualMs) fail(`trace.samples[${i}].atMs`, 'boundary before actual ingress');
    if (s.eligibility === 'pre-appearance') {
      if (appeared || s.own !== null) fail(`trace.samples[${i}].own`, 'pre-appearance absence required');
    } else {
      if (s.own === null) fail(`trace.samples[${i}].own`, 'missing own at eligible boundary');
      appeared = true;
      // Fixed ring geometry and signed cleanup sentinel are shared core constants:
      // src/presentation.rs Head/AngleCorrections and presentation/length.rs.
      for (const [name, slots] of [['head', 53], ['angle', 26], ['length', 53]]) {
        const r = s.own.rings[name];
        if (r.index >= slots || r.pending < -1 || r.pending > slots) fail(`trace.samples[${i}].own.rings.${name}`, 'invalid ring index/pending');
      }
    }
    if (s.camera.index >= 62 || s.camera.pending > 62) fail(`trace.samples[${i}].camera`, 'invalid camera ring index/pending');
    if (s.status.sendFailed || s.status.presentFailed) fail(`trace.samples[${i}].status`, 'send/present failure');
    if ((s.status.submitted && !s.status.prepared) || (s.status.committed && !s.status.submitted)) {
      fail(`trace.samples[${i}].status`, 'invalid prepare/submit/commit order');
    }
  });
  const f = trace.final;
  if (!f.completed || f.unexpectedNetwork || f.pendingEffects !== 0 || f.pendingFrames !== 0 || !same(f.atMs, schedule.durationMs)
    || f.events !== trace.events.length || f.commands !== trace.commands.length
    || f.samples !== trace.samples.length || f.draws !== trace.samples.filter(s => s.phase === 'draw').length
    || f.lastBoundaryId !== schedule.boundaries.at(-1).id) fail('trace.final', 'incomplete final boundary/counters or unexpected network');
  return trace;
}
