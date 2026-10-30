import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildManifest, parse, sha256, stringify } from './feel-replay/io.mjs';
import { buildSchedule } from './feel-replay/schedule.mjs';
import { assertBuildBinding } from './feel-replay/artifacts.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { nativeSteps } from './feel-replay/native.mjs';
import { startupPointer } from './feel-replay/input.mjs';
process.chdir(fileURLToPath(new URL('..', import.meta.url)));
const options = {}, args = process.argv.slice(2);
for (let i = 0; i < args.length; i += 2) {
  if (!['--record', '--wasm-run', '--out', '--commands', '--appearance-at'].includes(args[i]) || options[args[i]] !== undefined || args[i + 1] === undefined) throw new Error('Expected --record --wasm-run --out --commands --appearance-at');
  options[args[i]] = args[i + 1];
}
for (const key of ['--record', '--wasm-run', '--out', '--commands', '--appearance-at']) if (!options[key]) throw new Error(`Required ${key}`);
const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
const out = path.resolve(options['--out']), repository = git('rev-parse', '--show-toplevel');
const relative = path.relative(repository, out);
if (!relative || !relative.startsWith('..' + path.sep) && relative !== '..' && !path.isAbsolute(relative)) throw new Error('Fresh output outside repository required');
await mkdir(out);
try {
  if (git('diff', '--name-only') || git('ls-files', '--others', '--exclude-standard')) throw new Error('Stage intended source before replay');
  const built = parse(await readFile('dist/feel-native/build.json', 'utf8'));
  const product = { commit: git('rev-parse', 'HEAD'), tree: git('write-tree') };
  assertBuildBinding(built, product);
  if (sha256(await readFile(built.executable)) !== built.sha256) throw new Error('Native executable authentication failed');
  const wasmDirectory = path.resolve(options['--wasm-run']);
  const wasmBytes = await readFile(path.join(wasmDirectory, 'canvas-wasm.json'), 'utf8');
  const wasm = parse(wasmBytes);
  const inputFile = path.resolve(options['--record']), input = await readFile(inputFile), record = JSON.parse(input);
  if (sha256(input) !== wasm.manifest.input.sha256) throw new Error('Wasm input binding differs');
  const { schedule, deliveries } = buildSchedule(record, { expectedCommands: Number(options['--commands']), appearanceAtMs: Number(options['--appearance-at']) });
  if (stringify(schedule) !== await readFile(path.join(wasmDirectory, 'schedule.json'), 'utf8')) throw new Error('Frozen schedules differ');
  await writeFile(path.join(out, 'schedule.json'), stringify(schedule));
  await writeFile(path.join(out, 'input-receipt.json'), stringify({ inputFile, sha256: sha256(input), wasmTraceSha256: sha256(wasmBytes), nativeBuild: built }));
  const geometry = wasm.manifest.geometry;
  const area = { ...geometry.css, dpr: geometry.dpr }, pointer = startupPointer(record, area);
  const request = stringify({ geometry, pointer, steps: nativeSteps(schedule, deliveries, area), durationMs: schedule.durationMs });
  const invoke = (args, input) => execFileSync(built.executable, args, { input, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, windowsHide: true });
  const raw = invoke([], request);
  await writeFile(path.join(out, 'native-observations.json'), raw);
  const observed = parse(raw);
  const artifactFiles = { 'native-executable': built.executable };
  const manifest = await buildManifest({ schedule, artifactFiles, inputFile, source: product,
    input: { label: wasm.manifest.input.label, sha256: null }, peer: 'native',
    geometry, policy: 'feel-perceptual-v1', completed: observed.body.final.completed });
  const native = { ...observed.body, manifest };
  await writeFile(path.join(out, 'native.json'), stringify(native));
  const comparison = compareTraces(wasm, native, schedule);
  await writeFile(path.join(out, 'comparison.json'), stringify(comparison));
  const summary = { status: comparison.status, errors: comparison.errors,
    observationErrors: observed.errors, mismatches: comparison.mismatches.length,
    counts: { events: native.events.length, commands: native.commands.length, samples: native.samples.length, draws: native.final.draws }, channels: comparison.channels };
  await writeFile(path.join(out, 'summary.json'), stringify(summary));
  if (sha256(await readFile(inputFile)) !== sha256(input) || sha256(await readFile(built.executable)) !== built.sha256) throw new Error('Replay inputs/artifact changed during run');
  assertBuildBinding(built, { tree: git('write-tree') });
  if (git('diff', '--name-only')) throw new Error('Source changed during replay');
  console.log(JSON.stringify(summary));
  if (!comparison.passed || observed.errors.length) process.exitCode = 1;
} catch (error) {
  await writeFile(path.join(out, 'failure.txt'), String(error.stack));
  console.error('Native replay failed; see failure.txt in the requested output directory.'); process.exitCode = 1;
}
