import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildManifest, parse, sha256, stringify } from './feel-replay/io.mjs';
import { buildSchedule } from './feel-replay/schedule.mjs';
import { assertBuildBinding } from './feel-replay/artifacts.mjs';
import { nativeSteps } from './feel-replay/native.mjs';
import { startupPointer } from './feel-replay/input.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { validateSuccessfulDraws } from './feel-replay/schema.mjs';
import { desktopSteps, desktopExpectation } from './feel-replay/desktop.mjs';

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
  const product = { commit: git('rev-parse', 'HEAD'), tree: git('write-tree') };
  const built = parse(await readFile('dist/feel-native/build.json', 'utf8'));
  assertBuildBinding(built, product);
  if (sha256(await readFile(built.executable)) !== built.sha256) throw new Error('Native executable authentication failed');
  const wasmDirectory = path.resolve(options['--wasm-run']);
  const wasmBytes = await readFile(path.join(wasmDirectory, 'canvas-wasm.json'), 'utf8');
  const wasm = parse(wasmBytes);
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
  const planned = desktopSteps(steps, delayMs, options['--ack-ms'] === 'frame');
  const expected = desktopExpectation(wasm, schedule);
  await writeFile(path.join(out, 'expected-schedule.json'), stringify(expected.schedule));
  await writeFile(path.join(out, 'expected-desktop.json'), stringify(expected.trace));
  const request = { geometry, pointer, durationMs: planned.at(-1).event.actualMs, lane: 'desktop-ordering', steps: planned };
  await writeFile(path.join(out, 'schedule.json'), stringify(schedule));
  await writeFile(path.join(out, 'presentation-plan.json'), stringify({ hz, delayMs, request }));
  await writeFile(path.join(out, 'input-receipt.json'), stringify({ inputFile, sha256: sha256(input), wasmTraceSha256: sha256(wasmBytes), built, product }));
  const invoke = (args, request) => execFileSync(built.executable, args, { input: stringify(request), encoding: 'utf8',
    windowsHide: true, maxBuffer: 256 * 1024 * 1024 });
  const artifactFiles = { 'native-executable': built.executable };
  const manifestFor = observed => buildManifest({ schedule: expected.schedule, artifactFiles, inputFile, source: product, peer: 'native',
    input: { label: wasm.manifest.input.label, sha256: null },
    geometry, policy: 'feel-perceptual-v1', completed: observed.body.final.completed });
  const raw = invoke([], request), observed = parse(raw);
  await writeFile(path.join(out, 'desktop-observations.json'), raw);
  const manifest = await manifestFor(observed);
  const desktop = { ...observed.body, manifest };
  await writeFile(path.join(out, 'desktop.json'), stringify(desktop));
  if (validateSuccessfulDraws(desktop) !== desktop.final.draws) throw new Error('Final acknowledged draw count differs');
  const comparison = compareTraces(expected.trace, desktop, expected.schedule);
  const crossHost = compareTraces(wasm, desktop, schedule);
  await writeFile(path.join(out, 'cross-host-comparison.json'), stringify(crossHost));
  await writeFile(path.join(out, 'comparison.json'), stringify(comparison));
  const summary = { lane: 'desktop-ordering', hz, delayMs, status: comparison.status,
    crossHostStatus: crossHost.status, errors: [...observed.errors, ...comparison.errors], channels: comparison.channels,
    commandEffectsUnique: new Set(desktop.commands.map(c => c.effectId)).size === desktop.commands.length,
    effectAcksUnique: new Set(observed.effects.map(e => `${e.generation}:${e.id}`)).size === observed.effects.length,
    issues: observed.lifecycle.filter(e => e.kind === 'issue').length,
    acknowledgments: observed.lifecycle.filter(e => e.kind === 'presented').length,
    queuedRedraws: observed.lifecycle.filter(e => e.kind === 'redraw' && e.queuedPicture).length,
    packets: planned.filter(s => s.packet).length,
    ingested: observed.lifecycle.filter(e => e.kind === 'ingest').length,
    maxIngressDeferralMs: Math.max(0, ...observed.lifecycle.filter(e => e.kind === 'ingest').map(e => e.atMs - e.receivedMs)),
    blockedRedraws: observed.lifecycle.filter(e => e.kind === 'blocked-redraw').length };
  await writeFile(path.join(out, 'summary.json'), stringify(summary));
  if (sha256(await readFile(inputFile)) !== sha256(input) || sha256(await readFile(built.executable)) !== built.sha256
    || git('write-tree') !== product.tree || git('diff', '--name-only')) throw new Error('Replay inputs/artifacts/source changed during run');
  console.log(JSON.stringify(summary));
  if (summary.errors.length || summary.issues !== summary.acknowledgments
    || summary.packets !== summary.ingested || !comparison.passed
    || !summary.commandEffectsUnique || !summary.effectAcksUnique
    || desktop.final.pendingEffects || desktop.final.pendingFrames) process.exitCode = 1;
} catch (error) {
  await writeFile(path.join(out, 'failure.txt'), String(error.stack));
  console.error('Desktop replay failed; see failure.txt in the requested output directory.');
  process.exitCode = 1;
}
