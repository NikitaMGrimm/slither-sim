// Authored local rehearsal evidence only. Never feeds model/candidate scoring.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { createHash } from 'node:crypto';

function prefix(file) {
  const bytes = zlib.gunzipSync(fs.readFileSync(file), { maxOutputLength: 256 * 1024 * 1024 });
  const lines = bytes.toString('utf8').split('\n').filter(Boolean);
  const final = JSON.parse(lines.at(-1));
  const end = bytes.lastIndexOf(Buffer.from(lines.at(-1)));
  const digest = createHash('sha256').update(bytes.subarray(0, end)).digest('hex');
  if (final.integrity?.sha256 !== digest || final.integrity.lines !== lines.length - 1 ||
      final.integrity.payloadBytes !== end || !final.collectorSourcesUnchanged) throw Error('rehearsal archive integrity failed');
  return { interrupted: final.event === 'capture-interrupted', sha256: createHash('sha256').update(fs.readFileSync(file)).digest('hex') };
}
const root = process.argv[2], instance = process.argv[3], mode = process.argv[4];
if (!['baseline', 'conflict'].includes(mode)) throw Error('invalid rehearsal mode');
const events = fs.readFileSync(path.join(root, `guard-${instance}-private.jsonl`), 'utf8').trim().split('\n').map(JSON.parse);
const result = JSON.parse(fs.readFileSync(path.join(root, 'invocation-1/result-private.json')));
const samples = events.filter(e => e.event === 'namespace-census');
const paired = samples.filter(e => e.ours === 2 && e.connections.every(c => c.attribution === 'dedicated-container-namespace'));
const conflicts = events.filter(e => e.event === 'connection-conflict');
if (!paired.length || samples.some(e => e.ours > 2) || !result.socketsClosed) throw Error('namespace attribution rehearsal failed');
if (mode === 'baseline' && (result.elapsedSeconds < 300 || conflicts.length)) throw Error('baseline did not demonstrate five conflict-free minutes');
if (mode === 'conflict' && !conflicts.some(e => e.conflicts.some(c => c.machine === 'separate-host-namespace')))
  throw Error('deliberate host connection was not detected');
const archives = [0, 1].map(i => prefix(path.join(root, `invocation-1/bot${i}.jsonl.gz`)));
if (mode === 'conflict' && (!fs.existsSync(path.join(root, 'STUDY-STOP.json')) || archives.some(a => !a.interrupted)))
  throw Error('conflict did not stop and seal');
const summary = { origin: 'authored-local-rehearsal', officialRequests: 0, mode,
  elapsedSeconds: result.elapsedSeconds, positiveTwoConnectionSamples: paired.length,
  samplesWithOptionalInodeOwners: paired.filter(e => e.connections.every(c => c.owners.length)).length,
  maximumNamespaceConnections: Math.max(...samples.map(e => e.ours)),
  hostConflictDetected: conflicts.some(e => e.conflicts.some(c => c.machine === 'separate-host-namespace')),
  bothSocketsClosed: result.socketsClosed, sealedArchives: archives,
  permanentStop: fs.existsSync(path.join(root, 'STUDY-STOP.json')) };
fs.writeFileSync(path.join(root, 'rehearsal-leg-summary.json'), JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify(summary));
