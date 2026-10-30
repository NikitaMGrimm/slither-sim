import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildManifest, floatBits, parse, sha256, stringify } from './feel-replay/io.mjs';
import { buildSchedule } from './feel-replay/schedule.mjs';
import { assertBuildBinding } from './feel-replay/artifacts.mjs';
import { assertWasmTraceBinding, nativeSteps } from './feel-replay/native.mjs';
import { inputNormalization, startupPointer } from './feel-replay/input.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { desktopSteps, desktopComparison, coreHistoryComparison } from './feel-replay/desktop.mjs';

process.chdir(fileURLToPath(new URL('..', import.meta.url)));
const options = {}, args = process.argv.slice(2);
for (let i = 0; i < args.length; i += 2) {
  if (!['--record', '--wasm-run', '--out', '--commands', '--appearance-at', '--hz', '--ack-ms'].includes(args[i])
    || options[args[i]] !== undefined || args[i + 1] === undefined) throw new Error('Expected paired desktop replay options');
  options[args[i]] = args[i + 1];
}
for (const key of ['--record', '--wasm-run', '--out', '--commands', '--appearance-at', '--hz', '--ack-ms']) {
  if (options[key] === undefined) throw new Error(`Required ${key}`);
}
const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
const out = path.resolve(options['--out']), repository = git('rev-parse', '--show-toplevel');
const relative = path.relative(repository, out);
if (!relative || !relative.startsWith('..' + path.sep) && relative !== '..' && !path.isAbsolute(relative)) throw new Error('Fresh output outside repository required');
await mkdir(out);
try {
  if (git('diff', '--name-only') || git('ls-files', '--others', '--exclude-standard')) throw new Error('Stage intended source before replay');
  const product = { commit: git('rev-parse', 'HEAD'), tree: git('write-tree'), core: git('write-tree', '--prefix=client/src/') };
  const built = parse(await readFile('dist/feel-native/build.json', 'utf8'));
  assertBuildBinding(built, product);
  if (sha256(await readFile(built.executable)) !== built.sha256) throw new Error('Native executable authentication failed');
  const wasmDirectory = path.resolve(options['--wasm-run']);
  const wasmBytes = await readFile(path.join(wasmDirectory, 'canvas-wasm.json'), 'utf8');
  assertWasmTraceBinding(parse(await readFile(path.join(wasmDirectory, 'summary.json'), 'utf8')), wasmBytes);
  const wasm = parse(wasmBytes);
  if (stringify(product) !== stringify(wasm.manifest.product)) throw new Error('Wasm product binding differs; regenerate A2');
  const inputFile = path.resolve(options['--record']), input = await readFile(inputFile), record = JSON.parse(input);
  if (sha256(input) !== wasm.manifest.input.sha256) throw new Error('Wasm input binding differs');
  const hz = Number(options['--hz']);
  if (!Number.isFinite(hz) || hz <= 0) throw new Error('Positive display cadence required');
  const delayMs = options['--ack-ms'] === 'frame' ? 1000 / hz : Number(options['--ack-ms']);
  const { schedule, deliveries } = buildSchedule(record, { expectedCommands: Number(options['--commands']),
    appearanceAtMs: Number(options['--appearance-at']), stepMs: 1000 / hz });
  if (stringify(schedule) !== await readFile(path.join(wasmDirectory, 'schedule.json'), 'utf8')) throw new Error('Frozen schedules differ');
  const geometry = wasm.manifest.geometry, area = { ...geometry.css, dpr: geometry.dpr };
  const pointer = startupPointer(record, area), steps = nativeSteps(schedule, deliveries, area);
  const startup = ['clock-zero', 'empty-timers-and-raf', 'high-quality', 'play',
    `startup-pointer:${pointer.source}:${floatBits(pointer.x)}:${floatBits(pointer.y)}`,
    `input-normalization:${inputNormalization}`, 'scheduled-startup-raf'];
  if (stringify(startup) !== stringify(wasm.manifest.startup)) throw new Error('Startup binding differs');
  const planned = desktopSteps(steps, delayMs, options['--ack-ms'] === 'frame');
  const request = { geometry, pointer, durationMs: planned.at(-1).event.actualMs, lane: 'desktop-ordering', steps: planned };
  await writeFile(path.join(out, 'schedule.json'), stringify(schedule));
  await writeFile(path.join(out, 'presentation-plan.json'), stringify({ hz, delayMs, request }));
  await writeFile(path.join(out, 'input-receipt.json'), stringify({ inputFile, sha256: sha256(input), wasmTraceSha256: sha256(wasmBytes), built, product }));
  const invoke = (args, request) => execFileSync(built.executable, args, { input: stringify(request), encoding: 'utf8',
    windowsHide: true, maxBuffer: 256 * 1024 * 1024 });
  const artifactFiles = { 'native-executable': built.executable };
  for (const file of git('ls-files', '-z').split('\0').filter(Boolean)) artifactFiles[`source/${file}`] = file;
  const manifestFor = observed => buildManifest({ schedule, artifactFiles, inputFile, product, peer: 'native',
    input: { label: wasm.manifest.input.label, sha256: null }, toolVersions: [{ name: 'rustc', version: built.rustc }, { name: 'node', version: process.version }],
    entropy: { seed: '17', requestStreamSha256: sha256(stringify(observed.random)) }, startup: wasm.manifest.startup,
    geometry, qualityRequest: 'high', presentation: { gpu: null, mode: `desktop-ordering:${hz}Hz:ack=${delayMs}ms`, maxFrameLatency: null },
    numericalPolicy: wasm.manifest.numericalPolicy });
  const raw = invoke([], request), observed = parse(raw);
  await writeFile(path.join(out, 'desktop-observations.json'), raw);
  const repeatedRaw = invoke([], request), repeated = parse(repeatedRaw);
  await writeFile(path.join(out, 'desktop-repeat-observations.json'), repeatedRaw);
  const manifest = await manifestFor(observed), repeatedManifest = await manifestFor(repeated);
  const traceBytes = invoke(['--assemble'], { body: observed.body, manifest });
  const repeatBytes = invoke(['--assemble'], { body: repeated.body, manifest: repeatedManifest });
  await writeFile(path.join(out, 'desktop.json'), traceBytes);
  await writeFile(path.join(out, 'desktop-repeat.json'), repeatBytes);
  const desktop = parse(traceBytes);
  const equalRaw = invoke([], { geometry, pointer, durationMs: schedule.durationMs, steps });
  await writeFile(path.join(out, 'equal-observations.json'), equalRaw);
  const equalObserved = parse(equalRaw), equalManifest = await manifestFor(equalObserved);
  equalManifest.presentation.mode = 'cpu-boundary-sink';
  const equalBytes = invoke(['--assemble'], { body: equalObserved.body, manifest: equalManifest });
  await writeFile(path.join(out, 'equal-native.json'), equalBytes);
  const equal = parse(equalBytes);
  const pacedSteps = planned.filter(s => !s.skippedRedraw && !s.acknowledgment)
    .map((s, seq) => ({ ...s, event: { ...s.event, seq } }));
  const pacedRaw = invoke([], { geometry, pointer, durationMs: request.durationMs, steps: pacedSteps });
  await writeFile(path.join(out, 'paced-equal-observations.json'), pacedRaw);
  const paced = parse(pacedRaw).body;
  const coreHistory = coreHistoryComparison(paced, desktop);
  await writeFile(path.join(out, 'paced-core-differences.json'), stringify(coreHistory));
  const wasmComparison = desktopComparison(wasm, desktop), equalComparison = desktopComparison(equal, desktop);
  await writeFile(path.join(out, 'wasm-differences.json'), stringify(wasmComparison));
  await writeFile(path.join(out, 'equal-differences.json'), stringify(equalComparison));
  // Preserve the strict A1 gate, even when cadence/queueing makes its original
  // equal-schedule boundary declaration inapplicable. Diagnostics are not passes.
  const a1 = compareTraces(wasm, desktop, schedule, { mode: 'cross-target' });
  await writeFile(path.join(out, 'a1-comparison.json'), stringify(a1));
  const summary = { lane: 'desktop-ordering', hz, delayMs, byteExact: raw === repeatedRaw && traceBytes === repeatBytes,
    errors: [...observed.errors, ...equalObserved.errors, ...repeated.errors], a1Status: a1.status,
    commandEffectsUnique: new Set(desktop.commands.map(c => c.effectId)).size === desktop.commands.length,
    effectAcksUnique: new Set(observed.effects.map(e => `${e.generation}:${e.id}`)).size === observed.effects.length,
    wasm: { counts: wasmComparison.counts, classes: wasmComparison.classes },
    equal: { counts: equalComparison.counts, classes: equalComparison.classes },
    issues: observed.lifecycle.filter(e => e.kind === 'issue').length,
    acknowledgments: observed.lifecycle.filter(e => e.kind === 'presented').length,
    queuedRedraws: observed.lifecycle.filter(e => e.kind === 'redraw' && e.queuedPicture).length,
    packets: planned.filter(s => s.packet).length,
    ingested: observed.lifecycle.filter(e => e.kind === 'ingest').length,
    maxIngressDeferralMs: Math.max(0, ...observed.lifecycle.filter(e => e.kind === 'ingest').map(e => e.atMs - e.receivedMs)),
    pacedCoreExact: coreHistory.exact,
    pacedCoreClasses: coreHistory.classes,
    blockedRedraws: observed.lifecycle.filter(e => e.kind === 'blocked-redraw').length };
  await writeFile(path.join(out, 'summary.json'), stringify(summary));
  if (sha256(await readFile(inputFile)) !== sha256(input) || sha256(await readFile(built.executable)) !== built.sha256
    || git('write-tree') !== product.tree || git('diff', '--name-only')) throw new Error('Replay inputs/artifacts/source changed during run');
  console.log(JSON.stringify(summary));
  if (!summary.byteExact || summary.errors.length || summary.issues !== summary.acknowledgments
    || summary.packets !== summary.ingested || !summary.pacedCoreExact
    || !summary.commandEffectsUnique || !summary.effectAcksUnique
    || desktop.final.pendingEffects || desktop.final.pendingFrames) process.exitCode = 1;
} catch (error) {
  await writeFile(path.join(out, 'failure.txt'), String(error.stack));
  console.error('Desktop replay failed; see failure.txt in the requested output directory.');
  process.exitCode = 1;
}
