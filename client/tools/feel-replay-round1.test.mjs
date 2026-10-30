// Authored synthetic review regressions. No browser/build/server is launched.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { EventEmitter } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { rebuildArtifacts, verifyArtifacts } from './feel-replay/artifacts.mjs';
import { processTermination } from './feel-replay/browser.mjs';
import { inputNormalization, mouseReplay, startupPointer } from './feel-replay/input.mjs';
import { parse } from './feel-replay/io.mjs';
import { syntheticFixture } from './feel-replay/synthetic.mjs';
import { traceAssembly } from './feel-replay/assembly.mjs';
import { compareTraces } from './feel-replay/compare.mjs';

test('odd viewport startup delivers identical integer CSS coordinates through both families', () => {
  const p = startupPointer({}, { width: 1857, height: 1041 });
  assert.deepEqual(p, { source: 'integer-css-center', x: 928, y: 520 });
  const admitted = {};
  mouseReplay(n => { admitted[n.type] = [n.x, n.y]; return { prevented: false }; })({ type: 'mousemove', button: 0, ...p });
  assert.deepEqual(admitted.pointermove, admitted.mousemove);
  assert.deepEqual(startupPointer({ startupPointer: { x: 0.25, y: 0.5 } }, { width: 960, height: 540 }), { source: 'tape-resting-mouse', x: 240, y: 270 });
  assert.deepEqual(startupPointer({ startupPointer: { coordinateSpace: 'css', x: 317, y: 219 } }, { width: 961, height: 541 }), { source: 'tape-resting-mouse', x: 317, y: 219 });
});
test('named Chromium mouse normalization retains fractional pointer and integer compatibility coordinates', () => {
  const rows = [];
  mouseReplay(n => { rows.push(n); return { prevented: false }; })({ type: 'mousemove', button: 0, target: 'surface', x: 127.75, y: 91.5 });
  assert.equal(inputNormalization, 'chromium-primary-mouse-compat-v1');
  assert.deepEqual(rows.map(n => [n.type, n.target, n.x, n.y]), [['pointermove', 'surface', 127.75, 91.5], ['mousemove', 'surface', 127, 91]]);
  assert.equal(rows[0].button, -1);
});
test('only canceled pointerdown suppresses compatibility events, until release', () => {
  const rows = [], suppressed = [];
  const dispatch = mouseReplay(n => { rows.push(n.type); return { prevented: n.type === 'pointerdown' || n.type === 'pointermove' }; }, n => suppressed.push(n.type));
  const move = { type: 'mousemove', button: 0, x: 10.5, y: 20.75 };
  dispatch(move); dispatch({ ...move, type: 'mousedown' }); dispatch(move); dispatch({ ...move, type: 'mouseup' }); dispatch(move);
  assert.deepEqual(rows, ['pointermove', 'mousemove', 'pointerdown', 'pointermove', 'pointerup', 'pointermove', 'mousemove']);
  assert.deepEqual(suppressed, ['mousedown', 'mousemove', 'mouseup']);
});
test('explicit DOM compatibility events are not expanded twice', () => {
  const rows = [];
  mouseReplay(n => { rows.push(n.type); return {}; })({ type: 'mousemove', encoding: 'dom-event' });
  assert.deepEqual(rows, ['mousemove']);
});

async function artifactsTest(run) {
  const directory = await mkdtemp(path.join(tmpdir(), 'feel-artifacts-'));
  try {
    await mkdir(path.join(directory, 'canvas')); await mkdir(path.join(directory, 'gpu'));
    await writeFile(path.join(directory, 'canvas/binding.js'), '// Authored synthetic binding\n');
    await writeFile(path.join(directory, 'gpu/binding.wasm'), 'SYNTHETIC placeholder, not Wasm');
    await run(directory);
  } finally {
    assert.equal(path.dirname(directory), path.resolve(tmpdir()));
    assert(path.basename(directory).startsWith('feel-artifacts-'));
    await rm(directory, { recursive: true, force: true });
  }
}
const product = { tree: 'synthetic-tree', core: 'synthetic-core' };
test('replaced generated binding fails authentication even with unchanged source identity', async () => artifactsTest(async directory => {
  await rebuildArtifacts(directory, async () => product);
  const receipt = parse(await readFile(path.join(directory, 'build.json'), 'utf8'));
  await verifyArtifacts(directory, receipt, product);
  await writeFile(path.join(directory, 'canvas/binding.js'), '// Replaced synthetic binding\n');
  await assert.rejects(verifyArtifacts(directory, receipt, product), /authentication/);
}));
test('interrupted rebuild invalidates the previous receipt before any mutation', async () => artifactsTest(async directory => {
  await rebuildArtifacts(directory, async () => product);
  await assert.rejects(rebuildArtifacts(directory, async () => {
    await assert.rejects(readFile(path.join(directory, 'build.json')), { code: 'ENOENT' });
    await writeFile(path.join(directory, 'gpu/binding.wasm'), 'SYNTHETIC interrupted output');
    throw new Error('Synthetic interrupted build');
  }), /interrupted/);
  await assert.rejects(readFile(path.join(directory, 'build.json')), { code: 'ENOENT' });
}));

for (const mode of ['already-signaled', 'signal-during-kill', 'already-exited']) test(`cleanup completes after ${mode}`, { timeout: 1000 }, async () => {
  const child = new EventEmitter(); Object.assign(child, { exitCode: null, signalCode: null });
  let kills = 0;
  child.kill = () => { kills++; child.signalCode = 'SIGTERM'; child.emit('exit', null, 'SIGTERM'); };
  const terminate = processTermination(child);
  if (mode === 'already-signaled') { child.signalCode = 'SIGTERM'; child.emit('exit', null, 'SIGTERM'); }
  if (mode === 'already-exited') { child.exitCode = 0; child.emit('exit', 0, null); }
  await terminate();
  assert.equal(kills, mode === 'signal-during-kill' ? 1 : 0);
});
test('explicit recorder error makes a trace incomplete without decoding a partial row', () => {
  const { schedule, trace } = syntheticFixture();
  const assembly = traceAssembly(schedule, trace.manifest);
  assembly.append({ seq: 0, event: trace.events[0], commands: [], observations: [], pictures: [], observationErrors: ['update: unsupported face layout'] });
  const result = assembly.finish({ atMs: 10, completed: true, pendingEffects: 0, pendingFrames: 0, unexpectedNetwork: false });
  assert.equal(result.final.completed, false);
  assert.equal(result.samples.length, 0);
  assert.equal(compareTraces(result, result, schedule).status, 'invalid');
});
