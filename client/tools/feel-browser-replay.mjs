import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildManifest, parse, sha256, stringify } from './feel-replay/io.mjs';
import { buildSchedule, coordinate } from './feel-replay/schedule.mjs';
import { traceAssembly } from './feel-replay/assembly.mjs';
import { compareTraces } from './feel-replay/compare.mjs';
import { verifyArtifacts } from './feel-replay/artifacts.mjs';
import { launchBrowser } from './feel-replay/browser.mjs';
import { replayServer } from './feel-replay/server.mjs';
import { startupPointer } from './feel-replay/input.mjs';
import { validateTrace } from './feel-replay/schema.mjs';
const root = fileURLToPath(new URL('..', import.meta.url));
process.chdir(root);
const args = process.argv.slice(2), options = {};
for (let i = 0; i < args.length; i += 2) {
  if (!['--record', '--out', '--browser', '--commands', '--appearance-at', '--width', '--height', '--dpr', '--receipt', '--hz'].includes(args[i]) || options[args[i]] !== undefined || args[i + 1] === undefined) throw new Error('Expected --record --out --browser --commands --appearance-at; optional --width --height --dpr --receipt --hz');
  options[args[i]] = args[i + 1];
}
for (const key of ['--record', '--out', '--browser', '--commands', '--appearance-at']) if (!options[key]) throw new Error(`Required ${key}`);
const out = path.resolve(options['--out']);
const repository = path.resolve(execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim());
if (out.toLowerCase() === repository.toLowerCase() || out.toLowerCase().startsWith(repository.toLowerCase() + path.sep)) throw new Error('Output must be a fresh directory outside the repository');
await mkdir(out); // No recursive/exist-ok: preserve every prior attempt.
let server, browser;
try {
  const inputFile = path.resolve(options['--record']), input = await readFile(inputFile);
  const receipts = options['--receipt'] ? [{ path: path.resolve(options['--receipt']), sha256: sha256(await readFile(options['--receipt'])) }] : [];
  await writeFile(path.join(out, 'input-receipt.json'), stringify({ inputFile, sha256: sha256(input), receipts }));
  const record = JSON.parse(input);
  const hz = Number(options['--hz'] ?? 60);
  if (!Number.isFinite(hz) || hz <= 0) throw new Error('Positive display cadence required');
  const { schedule, deliveries } = buildSchedule(record, { expectedCommands: Number(options['--commands']), appearanceAtMs: Number(options['--appearance-at']), stepMs: 1000 / hz });
  await writeFile(path.join(out, 'schedule.json'), stringify(schedule)); // Frozen before either peer.
  const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
  const product = { commit: git('rev-parse', 'HEAD'), tree: git('write-tree') };
  if (git('diff', '--name-only') || git('ls-files', '--others', '--exclude-standard')) throw new Error('Stage intended changes before replay');
  const built = parse(await readFile(path.join(root, 'dist/feel-replay/build.json'), 'utf8'));
  await verifyArtifacts(path.join(root, 'dist/feel-replay'), built, product);
  const artifactFiles = Object.fromEntries(built.artifacts.map(a => [a.name, path.join(root, 'dist/feel-replay', a.name)]));
  server = await replayServer(); browser = await launchBrowser(options['--browser']);
  const area = { width: Number(options['--width'] ?? 960), height: Number(options['--height'] ?? 540), dpr: Number(options['--dpr'] ?? 1) };
  const pointer = startupPointer(record, area);
  const peers = [];
  for (const name of ['canvas-wasm', 'webgpu-wasm']) {
    const page = await browser.page(`${server.origin}/replay/${name}.html`, area, pointer);
    peers.push({ name, page, acks: [], step: async (event, data, seq) => parse(await page.evaluate(`window.replayStepEncoded(${JSON.stringify(stringify([event, data, seq]))})`)) });
  }
  await coordinate(schedule, deliveries, peers, (peer, ack) => { peer.acks.push(ack); });
  const traces = [];
  const observationFailures = [];
  for (const peer of peers) {
    const finish = parse(await peer.page.evaluate('window.replayFinish()'));
    observationFailures.push(...finish.recorderErrors.map(e => `${peer.name}.recorder[${e.seq}]: ${e.reason}`));
    if (finish.recorderErrors.length) throw new Error(observationFailures.join('\n'));
    const randomStream = finish.random;
    const manifest = await buildManifest({ schedule, artifactFiles, inputFile, source: product,
      input: { label: 'offline schema-1 input', sha256: null }, peer: peer.name,
      geometry: peer.acks[0].geometry, policy: 'feel-perceptual-v1', completed: finish.completed });
    const assembly = traceAssembly(schedule, manifest);
    for (const ack of peer.acks) assembly.append(ack);
    const trace = assembly.finish({ atMs: finish.atMs, completed: finish.completed,
      pendingEffects: finish.pendingEffects, pendingFrames: finish.pendingFrames, unexpectedNetwork: finish.unexpectedNetwork || peer.page.denied() !== 0 });
    traces.push(trace);
    const traceBytes = stringify(trace);
    await writeFile(path.join(out, `${peer.name}.json`), traceBytes);
    await writeFile(path.join(out, `${peer.name}-receipts.json`), stringify({ receipts: finish.receipts, recorderErrors: finish.recorderErrors,
      pictureAcknowledgements: assembly.pictureDrawIds(),
      entropyRequests: randomStream, queuedRaf: finish.queuedRaf, deniedRequests: peer.page.denied() }));
    await peer.page.close();
    validateTrace(trace, schedule, assembly.pictureDrawIds());
  }
  const report = compareTraces(...traces, schedule);
  if (observationFailures.length) {
    report.errors.push(...observationFailures);
    report.status = 'invalid'; report.passed = false;
  }
  await writeFile(path.join(out, 'comparison.json'), stringify(report));
  if (sha256(await readFile(inputFile)) !== sha256(input)) throw new Error('Input changed during run');
  for (const receipt of receipts) if (sha256(await readFile(receipt.path)) !== receipt.sha256) throw new Error('Receipt changed during run');
  await verifyArtifacts(path.join(root, 'dist/feel-replay'), built, { tree: git('write-tree') });
  if (git('diff', '--name-only')) throw new Error('Source changed during replay');
  const summary = { status: report.status, errors: report.errors, mismatches: report.mismatches.length, channels: report.channels,
    counts: traces.map(t => ({ peer: t.manifest.peer, events: t.events.length, commands: t.commands.length, samples: t.samples.length, draws: t.final.draws })) };
  await writeFile(path.join(out, 'summary.json'), stringify(summary));
  console.log(JSON.stringify(summary));
  if (!report.passed) process.exitCode = 1;
} catch (error) {
  // Details are private artifacts; public stdout never includes input bodies/paths.
  await writeFile(path.join(out, 'failure.txt'), String(error.stack));
  console.error('Offline browser replay failed; see failure.txt in the requested output directory.'); process.exitCode = 1;
} finally {
  if (browser) await browser.close();
  if (server) await server.close();
}
