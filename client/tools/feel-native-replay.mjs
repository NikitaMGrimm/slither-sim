import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildManifest, floatBits, parse, sha256, stringify } from './feel-replay/io.mjs';
import { buildSchedule } from './feel-replay/schedule.mjs';
import { assertBuildBinding } from './feel-replay/artifacts.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { assertWasmTraceBinding, channelResults, nativeSteps } from './feel-replay/native.mjs';
import { inputNormalization, startupPointer } from './feel-replay/input.mjs';
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
  const product = { commit: git('rev-parse', 'HEAD'), tree: git('write-tree'), core: git('write-tree', '--prefix=client/src/') };
  assertBuildBinding(built, product);
  if (sha256(await readFile(built.executable)) !== built.sha256) throw new Error('Native executable authentication failed');
  const wasmDirectory = path.resolve(options['--wasm-run']);
  const wasmBytes = await readFile(path.join(wasmDirectory, 'canvas-wasm.json'), 'utf8');
  assertWasmTraceBinding(parse(await readFile(path.join(wasmDirectory, 'summary.json'), 'utf8')), wasmBytes);
  const wasm = parse(wasmBytes);
  // A2's source-tree and generated-artifact hashes were authenticated by its build
  // receipt before that run. A different candidate must regenerate the Wasm run.
  if (stringify(wasm.manifest.product) !== stringify(product)) throw new Error('Wasm product binding differs; regenerate A2 with this staged candidate');
  const inputFile = path.resolve(options['--record']), input = await readFile(inputFile), record = JSON.parse(input);
  if (sha256(input) !== wasm.manifest.input.sha256) throw new Error('Wasm input binding differs');
  const { schedule, deliveries } = buildSchedule(record, { expectedCommands: Number(options['--commands']), appearanceAtMs: Number(options['--appearance-at']) });
  if (stringify(schedule) !== await readFile(path.join(wasmDirectory, 'schedule.json'), 'utf8')) throw new Error('Frozen schedules differ');
  await writeFile(path.join(out, 'schedule.json'), stringify(schedule));
  await writeFile(path.join(out, 'input-receipt.json'), stringify({ inputFile, sha256: sha256(input), wasmTraceSha256: sha256(wasmBytes), nativeBuild: built }));
  const geometry = wasm.manifest.geometry;
  const area = { ...geometry.css, dpr: geometry.dpr }, pointer = startupPointer(record, area);
  const startup = ['clock-zero', 'empty-timers-and-raf', 'high-quality', 'play',
    `startup-pointer:${pointer.source}:${floatBits(pointer.x)}:${floatBits(pointer.y)}`,
    `input-normalization:${inputNormalization}`, 'scheduled-startup-raf'];
  if (stringify(startup) !== stringify(wasm.manifest.startup)) throw new Error('Startup binding differs');
  const request = stringify({ geometry, pointer, steps: nativeSteps(schedule, deliveries, area), durationMs: schedule.durationMs });
  const invoke = (args, input) => execFileSync(built.executable, args, { input, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, windowsHide: true });
  const raw = invoke([], request);
  await writeFile(path.join(out, 'native-observations.json'), raw);
  const observed = parse(raw);
  if (stringify(observed) !== raw) throw new Error('Rust/Node A1 codec bytes differ');
  const artifactFiles = { 'native-executable': built.executable };
  for (const file of git('ls-files', '-z').split('\0').filter(Boolean)) artifactFiles[`source/${file}`] = file;
  const manifest = await buildManifest({ schedule, artifactFiles, inputFile, product,
    input: { label: wasm.manifest.input.label, sha256: null }, peer: 'native',
    toolVersions: [{ name: 'rustc', version: built.rustc }, { name: 'node', version: process.version }],
    entropy: { seed: '17', requestStreamSha256: sha256(stringify(observed.random)) }, startup,
    geometry, qualityRequest: 'high', presentation: { gpu: null, mode: 'cpu-boundary-sink', maxFrameLatency: null },
    numericalPolicy: wasm.manifest.numericalPolicy });
  // Rust writes both complete traces. Node never decimal-serializes observed floats.
  const first = invoke(['--assemble'], stringify({ body: observed.body, manifest }));
  await writeFile(path.join(out, 'native.json'), first);
  const repeatedRaw = invoke([], request);
  await writeFile(path.join(out, 'native-repeat-observations.json'), repeatedRaw);
  const repeated = parse(repeatedRaw);
  const repeatManifest = { ...manifest, entropy: { seed: '17', requestStreamSha256: sha256(stringify(repeated.random)) } };
  const second = invoke(['--assemble'], stringify({ body: repeated.body, manifest: repeatManifest }));
  await writeFile(path.join(out, 'native-repeat.json'), second);
  const native = parse(first), repeat = compareTraces(native, parse(second), schedule);
  const comparison = compareTraces(wasm, native, schedule, { mode: 'cross-target' });
  const exactComparison = compareTraces(wasm, native, schedule, { mode: 'cross-target', exactOnly: true });
  const { channels, rows } = channelResults(wasm, native, comparison);
  await writeFile(path.join(out, 'native-manifest.json'), stringify(manifest));
  await writeFile(path.join(out, 'repeat-comparison.json'), stringify({ ...repeat, byteExact: first === second, observationsByteExact: raw === repeatedRaw }));
  await writeFile(path.join(out, 'comparison.json'), stringify(comparison));
  await writeFile(path.join(out, 'exact-comparison.json'), stringify(exactComparison));
  await writeFile(path.join(out, 'differences.json'), stringify(rows));
  const summary = { status: comparison.status, errors: comparison.errors, repeat: repeat.status, byteExact: first === second,
    observationErrors: observed.errors, mismatches: comparison.mismatches.length, nonExactFloats: comparison.nonExactFloats.length,
    counts: { events: native.events.length, commands: native.commands.length, samples: native.samples.length, draws: native.final.draws }, channels };
  await writeFile(path.join(out, 'summary.json'), stringify(summary));
  if (sha256(await readFile(inputFile)) !== sha256(input) || sha256(await readFile(built.executable)) !== built.sha256) throw new Error('Replay inputs/artifact changed during run');
  console.log(JSON.stringify(summary));
  if (!comparison.passed || !repeat.exact || first !== second || raw !== repeatedRaw || observed.errors.length) process.exitCode = 1;
} catch (error) {
  await writeFile(path.join(out, 'failure.txt'), String(error.stack));
  console.error('Native replay failed; see failure.txt in the requested output directory.'); process.exitCode = 1;
}
