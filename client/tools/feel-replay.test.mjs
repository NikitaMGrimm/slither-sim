import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compareTraces, crossTargetPolicyVersion } from './feel-replay/compare.mjs';
import { buildManifest, floatBits, parse, readTrace, scheduleHash, sha256, stringify } from './feel-replay/io.mjs';
import { orderEvents, validateManifest, validateNumericalPolicy, validateSchedule, validateTrace } from './feel-replay/schema.mjs';
import { syntheticFixture, syntheticNumericalPolicy } from './feel-replay/synthetic.mjs';

const cli = fileURLToPath(new URL('./feel-replay.mjs', import.meta.url));
function peers(cross = false) {
  const { schedule, trace: left } = syntheticFixture();
  const right = structuredClone(left);
  right.manifest.peer = cross ? 'native' : 'webgpu-wasm';
  if (cross) {
    left.manifest.numericalPolicy = structuredClone(syntheticNumericalPolicy);
    right.manifest.numericalPolicy = structuredClone(syntheticNumericalPolicy);
  }
  return { schedule, left, right, mode: cross ? 'cross-target' : 'same-core' };
}
function run(p) { return compareTraces(p.left, p.right, p.schedule, { mode: p.mode }); }
function failure(report, message) {
  assert.equal(report.passed, false);
  assert.match(JSON.stringify(report), message);
}
function next64(value) {
  const bytes = Buffer.alloc(8); bytes.writeDoubleBE(value);
  bytes.writeBigUInt64BE(bytes.readBigUInt64BE() + 1n);
  return bytes.readDoubleBE();
}
function next32(value, steps = 1) {
  const bytes = Buffer.alloc(4); bytes.writeFloatBE(value);
  bytes.writeUInt32BE(bytes.readUInt32BE() + steps);
  return bytes.readFloatBE();
}

test('synthetic deterministic self-comparison and Wasm peers are exact', () => {
  const p = peers();
  assert.equal(compareTraces(p.left, p.left, p.schedule).status, 'exact-match');
  assert.equal(run(p).status, 'exact-match');
  assert.equal(stringify(p.left), stringify(parse(stringify(p.left))));
});
test('v2 retains skipped draw boundaries but counts only acknowledged pictures', () => {
  const { schedule, trace } = syntheticFixture();
  const skipped = trace.samples.filter(s => s.phase === 'draw').at(-1);
  Object.assign(skipped.status, { prepared: true, submitted: false, committed: false });
  skipped.quality.successfulDraws = 1;
  trace.final.draws = 1;
  assert.equal(validateTrace(trace, schedule), trace);
  trace.final.draws = 2;
  assert.throws(() => validateTrace(trace, schedule), /incomplete final boundary\/counters/);
});
test('every sample reports the cumulative committed draw count, including updates and skips', () => {
  const { schedule, trace } = syntheticFixture();
  assert.equal(validateTrace(trace, schedule), trace);
  for (const index of [0, 2, 3, 4]) {
    const changed = parse(stringify(trace));
    changed.samples[index].quality.successfulDraws++;
    assert.throws(() => validateTrace(changed, schedule), /quality.successfulDraws: expected cumulative committed draw count/);
  }
  const skipped = trace.samples[4];
  Object.assign(skipped.status, { submitted: false, committed: false });
  trace.final.draws = 1;
  assert.throws(() => validateTrace(trace, schedule), /quality.successfulDraws: expected cumulative committed draw count/);
  skipped.quality.successfulDraws = 1;
  assert.equal(validateTrace(trace, schedule, [0]), trace);
  assert.throws(() => validateTrace(trace, schedule, []), /picture acknowledgement mismatch/);
  assert.throws(() => validateTrace(trace, schedule, [0, 0]), /duplicate ID/);
  assert.throws(() => validateTrace(trace, schedule, [2]), /no matching draw/);
});
test('stable equal-time schedule is packet -> input -> frame, preserving source order', () => {
  const events = ['frame', 'input', 'packet', 'packet', 'input'].map((kind, i) => ({ kind, sourceId: String(i), atMs: 0 }));
  assert.deepEqual(orderEvents(events).map(e => e.sourceId), ['2', '3', '1', '4', '0']);
  const { schedule } = syntheticFixture();
  [schedule.events[0], schedule.events[1]] = [schedule.events[1], schedule.events[0]];
  assert.throws(() => validateSchedule(schedule), /packet -> input -> frame/);
});
test('a dangling pre-draw reference fails even when identical traces have no draws', () => {
  const { schedule, trace } = syntheticFixture();
  schedule.boundaries = schedule.boundaries.filter(b => b.phase !== 'draw');
  schedule.boundaries[1].drawId = 0;
  trace.samples = trace.samples.filter(s => s.phase !== 'draw');
  trace.samples[1].drawId = 0;
  Object.assign(trace.final, { samples: 3, draws: 0, lastBoundaryId: 'update-1' });
  // Hash the authored invalid schedule without invoking its validator.
  trace.manifest.scheduleSha256 = sha256(stringify(schedule));
  const report = compareTraces(trace, trace, schedule);
  failure(report, /schedule.boundaries\[1\].drawId: unknown draw/);
  assert.equal(report.status, 'invalid');
  assert.throws(() => validateSchedule(schedule), /schedule.boundaries\[1\].drawId: unknown draw/);
});
test('a pre-draw reference passes when a later draw realizes its ordinal', () => {
  const { schedule, trace } = syntheticFixture();
  schedule.boundaries[1].drawId = 0;
  trace.samples[1].drawId = 0;
  trace.manifest.scheduleSha256 = scheduleHash(schedule);
  assert.equal(validateSchedule(schedule), schedule);
  assert.equal(compareTraces(trace, trace, schedule).status, 'exact-match');
});
test('bit encoding preserves signed zero, subnormals and finite extremes before JSON', () => {
  const numbers = [-0, 0, Number.MIN_VALUE, -Number.MIN_VALUE, Number.MAX_VALUE, next64(1)];
  const decoded = parse(stringify(numbers));
  numbers.forEach((x, i) => assert(Object.is(x, decoded[i])));
  assert.equal(floatBits(decoded[0]), '8000000000000000');
  for (const x of [NaN, Infinity, undefined]) assert.throws(() => stringify(x));
  assert.throws(() => parse('{"encoding":"feel-f64-v1","value":0}'), /bare JSON number/);
  assert.throws(() => parse('{"encoding":"feel-f64-v1","value":{"$f64":"7ff0000000000000"}}'), /non-finite/);
});

const faults = [
  ['missing timestamp', p => { delete p.right.events[1].actualMs; }, /events\[1\].actualMs.*required time/],
  ['missing final boundary', p => { delete p.right.final; }, /trace.final.*required object/],
  ['signed zero', p => { p.right.samples[1].own.offsets.y = -0; }, /signed zero mismatch/],
  ['command byte', p => { p.right.commands[0].bytes[0] = 11; }, /commands\[0\].bytes\[0\].*exact\/discrete mismatch/],
  ['event reordering', p => { [p.right.events[0], p.right.events[1]] = [p.right.events[1], p.right.events[0]]; }, /source order\/identity/],
  ['truncation', p => { p.right.samples.pop(); }, /boundary count 4, expected 5/],
  ['duplicate event', p => { p.right.events[1] = structuredClone(p.right.events[0]); }, /trace.events: duplicate ID/],
  ['extra draw', p => { p.right.samples.push(structuredClone(p.right.samples.at(-1))); }, /extra draw/],
  ['one f64 ULP', p => { p.right.samples[1].own.rawHead.x = next64(1); }, /non-exact float/],
  ['sample reordering', p => { [p.right.samples[1], p.right.samples[2]] = [p.right.samples[2], p.right.samples[1]]; }, /boundary order\/identity/],
  ['missing command timestamp', p => { delete p.right.commands[0].ackMs; }, /ackMs.*required time/],
  ['missing state field', p => { delete p.right.samples[1].camera.pending; }, /camera.pending.*required uint/],
  ['unsafe command', p => { p.right.commands[0].bytes = [115, 1, 2]; }, /unsafe command/],
  ['unexpected network', p => { p.right.final.unexpectedNetwork = true; }, /unexpected network/],
  ['failed send', p => { p.right.commands[0].succeeded = false; }, /failed send/],
  ['failed present', p => { p.right.samples[2].status.presentFailed = true; }, /send\/present failure/],
  ['missing own', p => { p.right.samples[1].own = null; }, /missing own/],
  ['non-finite timestamp', p => { p.right.events[0].actualMs = Infinity; }, /required time \(finite\)/],
];
for (const [label, mutate, message] of faults) test(`synthetic ${label} fails specifically`, () => {
  const p = peers(); mutate(p); failure(run(p), message);
});
test('completeness gates before numeric comparison, including two identically truncated peers', () => {
  const p = peers();
  p.left.samples.pop(); p.right.samples.pop();
  const report = run(p);
  failure(report, /boundary count/);
  assert.deepEqual(report.nonExactFloats, []);
});
test('same-core one f32 ULP fails and all non-exact channels are retained', () => {
  const p = peers();
  for (const sample of p.right.samples) sample.camera.ring[0] = next32(0);
  const report = run(p);
  assert.equal(report.passed, false);
  assert.equal(report.nonExactFloats.length, 5);
});
test('core ring cleanup sentinel and double-precision camera offsets remain representable', () => {
  const p = peers();
  for (const trace of [p.left, p.right]) {
    trace.samples[1].own.rings.head.pending = -1;
    trace.samples[1].own.rings.angle.pending = -1;
    trace.samples[1].own.rings.length.pending = -1;
    trace.samples[1].camera.offset.x = next64(1);
  }
  assert.equal(run(p).status, 'exact-match');
  p.right.samples[1].own.rings.head.values.pop();
  failure(run(p), /required 159 values/);
});
test('finite extremes retain every mismatch even when subtraction overflows', () => {
  const p = peers();
  p.left.samples[1].own.rawHead.x = Number.MAX_VALUE;
  p.right.samples[1].own.rawHead.x = -Number.MAX_VALUE;
  const report = run(p);
  failure(report, /negative overflow/);
  assert.equal(parse(stringify(report)).nonExactFloats[0].leftBits, floatBits(Number.MAX_VALUE));
});
test('incomplete rings/pupils and unfinished final work fail completeness', () => {
  for (const [mutate, message] of [
    [p => { p.right.samples[1].camera.ring.pop(); }, /required 124 values/],
    [p => { p.right.samples[1].own.projected.css.pupils.pop(); }, /required 2 values/],
    [p => { p.right.final.pendingEffects = 1; }, /incomplete final boundary/],
    [p => { p.right.final.pendingFrames = 1; }, /incomplete final boundary/],
    [p => { p.right.samples[1].own.offsets.x = next64(1); }, /required f32/],
  ]) { const p = peers(); mutate(p); failure(run(p), message); }
});

for (const [label, get, set, cap] of [
  ['world', s => s.own.rawHead.x, (s, x) => { s.own.rawHead.x = x; }, 1e-9],
  ['radians', s => s.own.rawHeading, (s, x) => { s.own.rawHeading = x; }, 1e-9],
  ['zoom', s => s.camera.zoom, (s, x) => { s.camera.zoom = x; }, 1e-12],
]) test(`cross-target ${label} passes at cap and fails immediately outside`, () => {
  const p = peers(true);
  set(p.left.samples[1], 0); set(p.right.samples[1], cap);
  assert.equal(get(p.right.samples[1]), cap);
  const report = run(p);
  assert.equal(report.status, 'numerical-match'); assert.equal(report.exact, false);
  assert.equal(report.nonExactFloats.length, 1);
  set(p.right.samples[1], next64(cap));
  failure(run(p), /non-exact float/);
});
test('cross-target f32 offsets pass one ULP, fail two, including negative offsets', () => {
  for (const sign of [1, -1]) {
    const p = peers(true);
    p.left.samples[1].own.offsets.x = sign;
    p.right.samples[1].own.offsets.x = sign * next32(1);
    assert.equal(run(p).status, 'numerical-match');
    p.right.samples[1].own.offsets.x = sign * next32(1, 2);
    failure(run(p), /non-exact float/);
  }
});
for (const cross of [false, true]) test(`discrete, ring cursor, command and timing mismatches fail in ${cross ? 'cross-target' : 'same-core'}`, () => {
  for (const mutate of [
    p => { p.right.samples[1].camera.follow = false; },
    p => { p.right.samples[1].own.rings.head.index = 1; },
    p => { p.right.samples[1].own.rings.head.pending = 1; },
    p => { p.right.samples[1].camera.index = 1; },
    p => { p.right.samples[1].camera.pending = 1; },
    p => { p.right.commands[0].bytes[0] = 11; },
    p => { p.right.commands[0].ackMs = Number.MIN_VALUE; },
    p => { p.right.events[0].actualMs = -0; },
  ]) {
    const p = peers(cross); mutate(p); assert.equal(run(p).passed, false);
  }
});

test('cross-target f32 ring storage admits one ULP only for every ring family', () => {
  for (const name of ['head', 'angle', 'length']) for (const sign of [1, -1]) {
    const p = peers(true);
    p.left.samples[1].own.rings[name].values[0] = sign;
    p.right.samples[1].own.rings[name].values[0] = sign * next32(1);
    const report = run(p);
    assert.equal(report.status, 'numerical-match');
    assert.equal(report.policyVersion, crossTargetPolicyVersion);
    assert.equal(report.nonExactFloats.length, 1);
    p.right.samples[1].own.rings[name].values[0] = sign * next32(1, 2);
    failure(run(p), /non-exact float/);
    p.right.samples[1].own.rings[name].values[0] = next64(1);
    failure(run(p), /required f32-exact/);
  }
});

test('cross-target camera ring storage admits the world cap and fails immediately beyond', () => {
  const p = peers(true);
  p.right.samples[1].camera.ring[0] = 1e-9;
  assert.equal(run(p).status, 'numerical-match');
  p.right.samples[1].camera.ring[0] = next64(1e-9);
  failure(run(p), /non-exact float/);
});

test('same-core ring floats stay exact even with a declared cross-target policy', () => {
  for (const name of ['head', 'angle', 'length', 'camera']) {
    const p = peers();
    for (const t of [p.left, p.right]) t.manifest.numericalPolicy = structuredClone(syntheticNumericalPolicy);
    if (name === 'camera') p.right.samples[1].camera.ring[0] = 1e-10;
    else p.right.samples[1].own.rings[name].values[0] = next32(0);
    const report = run(p);
    failure(report, /non-exact float/);
    assert.equal(report.nonExactFloats[0].tolerated, false);
  }
});

test('ring allowance requires a cause and retains signed-zero evidence', () => {
  for (const policy of [null, { ...syntheticNumericalPolicy, cause: '' }, { ...syntheticNumericalPolicy, cause: undefined }]) {
    const p = peers(true);
    p.left.manifest.numericalPolicy = p.right.manifest.numericalPolicy = policy;
    p.right.samples[1].camera.ring[0] = 1e-10;
    failure(run(p), policy === null ? /non-exact float/ : /cause/);
  }
  for (const camera of [true, false]) {
    const p = peers(true);
    if (camera) p.right.samples[1].camera.ring[0] = -0;
    else p.right.samples[1].own.rings.head.values[0] = -0;
    failure(run(p), /signed zero mismatch/);
  }
});

test('an exact cross-target report preserves the declared policy without accepting its allowances', () => {
  const p = peers(true);
  p.right.samples[1].camera.ring[0] = 1e-10;
  assert.equal(run(p).status, 'numerical-match');
  const report = compareTraces(p.left, p.right, p.schedule, { mode: 'cross-target', exactOnly: true });
  failure(report, /non-exact float/);
  assert.equal(report.exactOnly, true);
  assert.equal(report.nonExactFloats[0].tolerated, false);
  assert.deepEqual(p.left.manifest.numericalPolicy, syntheticNumericalPolicy);
});

test('numerical declarations reject an excessive cap before peer execution', () => {
  assert.equal(validateNumericalPolicy(syntheticNumericalPolicy), syntheticNumericalPolicy);
  for (const [key, value] of [['worldCap', 1e-8], ['angleCap', 1e-8], ['zoomCap', 1e-11], ['f32Ulps', 2]]) {
    assert.throws(() => validateNumericalPolicy({ ...syntheticNumericalPolicy, [key]: value }), /exceeds/);
  }
});
test('cross-target fails without a declared cause, with excessive caps, or wrong peer mode', () => {
  const p = peers(true);
  p.left.manifest.numericalPolicy = p.right.manifest.numericalPolicy = null;
  p.right.samples[1].own.rawHead.x = next64(1);
  failure(run(p), /non-exact float/);
  p.left.manifest.numericalPolicy = { ...syntheticNumericalPolicy, worldCap: 1e-8 };
  failure(run(p), /exceeds/);
  p.left.manifest.numericalPolicy = { ...syntheticNumericalPolicy, cause: '' };
  assert.throws(() => validateManifest(p.left.manifest), /cause/);
  failure(run({ ...peers(true), mode: 'same-core' }), /requires exactly one native/);
});
test('cross-target rejects accumulating drift within individual caps', () => {
  const p = peers(true);
  for (let i = 1; i < p.right.samples.length; i++) {
    p.left.samples[i].camera.x = 0;
    p.right.samples[i].camera.x = i * 1e-10;
  }
  failure(run(p), /accumulating numerical drift/);
});
// Authored SYNTHETIC five-frame history: each frame has an update and a draw.
function fiveFrameCameraErrors(errors) {
  const p = peers(true);
  p.schedule.durationMs = 40;
  for (let frameId = 2; frameId < 5; frameId++) {
    const eventSeq = p.schedule.events.length;
    const event = { sourceId: `frame-${frameId}`, kind: 'frame', atMs: frameId * 10 };
    p.schedule.events.push(event);
    for (const phase of ['update', 'draw']) p.schedule.boundaries.push({
      id: `${phase}-${frameId}`, eventSeq, phase, frameId,
      drawId: phase === 'draw' ? frameId : null, atMs: event.atMs, eligibility: 'own',
    });
    for (const trace of [p.left, p.right]) {
      trace.events.push({ ...structuredClone(trace.events[3]), seq: eventSeq, sourceId: event.sourceId,
        scheduledMs: event.atMs, actualMs: event.atMs });
      for (const boundary of p.schedule.boundaries.slice(-2)) trace.samples.push({
        ...structuredClone(trace.samples[boundary.phase === 'update' ? 1 : 2]), ...boundary,
        quality: { ...trace.samples[2].quality, successfulDraws: frameId + (boundary.phase === 'draw' ? 1 : 0) },
      });
    }
  }
  for (const trace of [p.left, p.right]) {
    trace.manifest.scheduleSha256 = scheduleHash(p.schedule);
    Object.assign(trace.final, { atMs: 40, events: 7, samples: 11, draws: 5, lastBoundaryId: 'draw-4' });
    trace.samples.slice(1).forEach((sample, i) => { sample.camera.x = trace === p.left ? 0 : errors[i] * 1e-10; });
  }
  return p;
}
test('cross-target update/draw plateaus cannot hide accumulating camera drift', () => {
  for (const sign of [1, -1]) {
    const p = fiveFrameCameraErrors([1, 1, 2, 2, 3, 3, 4, 4, 5, 5].map(x => sign * x));
    const report = run(p);
    failure(report, /accumulating numerical drift/);
    assert.equal(report.status, 'mismatch');
    assert.equal(report.mismatches[0].path, 'samples[5].camera.x');
  }
});
test('cross-target constant update/draw error plateaus stay numerical matches', () => {
  for (const sign of [1, -1]) {
    const report = run(fiveFrameCameraErrors(Array(10).fill(sign * 2)));
    assert.equal(report.status, 'numerical-match');
    assert.equal(report.passed, true);
    assert.equal(report.nonExactFloats.length, 10);
  }
});
test('cross-target decrease or sign change resets growth before later update/draw growth', () => {
  for (const sign of [1, -1]) {
    const p = fiveFrameCameraErrors([1, 1, 2, 2, sign, sign, 2 * sign, 2 * sign, 2 * sign, 2 * sign]);
    assert.equal(run(p).status, 'numerical-match');
    p.right.samples[9].camera.x = p.right.samples[10].camera.x = sign * 3e-10;
    const report = run(p);
    failure(report, /accumulating numerical drift/);
    assert.equal(report.status, 'mismatch');
    assert.equal(report.mismatches[0].path, 'samples[9].camera.x');
  }
});

for (const name of ['camera', 'head', 'angle', 'length']) test(`cross-target ${name} ring accumulation is guarded per stored element in sample order`, () => {
  const ring = sample => name === 'camera' ? sample.camera.ring : sample.own.rings[name].values;
  const history = levels => {
    const p = fiveFrameCameraErrors(Array(10).fill(0));
    p.right.samples.slice(1).forEach((s, i) => {
      const level = levels[i];
      if (name === 'camera') ring(s)[0] = level * 1e-10;
      else if (level !== 0) {
        // Increasing binades grow absolute error while each sample remains one
        // f32 ULP apart. The accumulation guard is separate from the ULP cap.
        const base = 1.5 * 2 ** (Math.abs(level) - 1);
        ring(p.left.samples[i + 1])[0] = base;
        ring(s)[0] = base + Math.sign(level) * (next32(base) - base);
      }
    });
    return p;
  };
  for (const sign of [1, -1]) {
    const p = history([1, 1, 2, 2, 3, 3, 4, 4, 5, 5].map(x => x * sign));
    for (const t of [p.left, p.right]) t.samples.slice(1).forEach((s, i) => {
      const cursor = name === 'camera' ? s.camera : s.own.rings[name];
      cursor.index = i; // Cursor movement must not reset a physical element's history.
    });
    const report = run(p);
    failure(report, /accumulating numerical drift/);
    assert.match(report.mismatches[0].path, /^samples\[5\]\./);
  }
  assert.equal(run(history(Array(10).fill(2))).status, 'numerical-match');
  for (const levels of [[1, 1, 2, 2, 1, 1, 2, 2, 2, 2], [1, 1, 2, 2, -1, -1, -2, -2, -2, -2], [1, 1, 2, 2, 0, 0, 1, 1, 2, 2]]) {
    assert.equal(run(history(levels)).status, 'numerical-match');
  }
  const independent = history(Array(10).fill(0));
  independent.right.samples.slice(1).forEach((s, i) => {
    if (name === 'camera') ring(s)[i] = (i + 1) * 1e-10;
    else {
      const base = 1.5 * 2 ** i;
      ring(independent.left.samples[i + 1])[i] = base;
      ring(s)[i] = next32(base);
    }
  });
  assert.equal(run(independent).status, 'numerical-match');
});
test('hash, core, startup, geometry, entropy and input bindings fail closed', () => {
  for (const mutate of [
    p => { p.right.manifest.scheduleSha256 = '0'.repeat(64); },
    p => { p.right.manifest.product.core = 'other-core'; },
    p => { p.right.manifest.startup.reverse(); },
    p => { p.right.manifest.geometry.dpr = 2; },
    p => { p.right.manifest.entropy.requestStreamSha256 = '0'.repeat(64); },
    p => { p.right.manifest.input.label = 'other-input'; },
  ]) { const p = peers(); mutate(p); failure(run(p), /manifest/); }
});
test('reader, manifest hashing and CLI success/failure use only temporary synthetic files', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'feel-replay-synthetic-'));
  try {
    const p = peers();
    const artifact = path.join(dir, 'artifact.txt'), input = path.join(dir, 'input.txt');
    await writeFile(artifact, 'SYNTHETIC artifact\n'); await writeFile(input, 'SYNTHETIC private-input stand-in\n');
    const { artifacts, ...fields } = p.left.manifest;
    const manifest = await buildManifest({ ...fields, schedule: p.schedule, artifactFiles: { synthetic: artifact }, inputFile: input });
    assert.equal(manifest.artifacts[0].sha256, sha256(await readFile(artifact)));
    assert.equal(manifest.input.sha256, sha256(await readFile(input)));
    assert.equal(manifest.scheduleSha256, scheduleHash(p.schedule));
    assert(!stringify(manifest).includes(dir));
    assert.equal(scheduleHash(p.schedule), scheduleHash(parse(stringify(p.schedule))));
    const files = ['schedule', 'left', 'right'].map(name => path.join(dir, `${name}.json`));
    for (const [i, value] of [p.schedule, p.left, p.right].entries()) await writeFile(files[i], stringify(value));
    assert.deepEqual(await readTrace(files[1], p.schedule), p.left);
    const execute = () => spawnSync(process.execPath, [cli, ...files], { encoding: 'utf8', timeout: 10000 });
    let output = execute(); assert.equal(output.status, 0, output.stderr);
    assert.equal(parse(output.stdout).status, 'exact-match');
    p.right.commands[0].bytes[0] = 11;
    await writeFile(files[2], stringify(p.right));
    output = execute(); assert.equal(output.status, 1); assert.match(output.stdout, /commands\[0\].bytes\[0\]/);
    delete p.right.final;
    await writeFile(files[2], stringify(p.right));
    output = execute(); assert.equal(output.status, 1); assert.match(output.stderr, /trace.final/);
    assert.throws(() => validateTrace(p.right, p.schedule), /trace.final/);
  } finally { await rm(dir, { recursive: true, force: true }); }
});
