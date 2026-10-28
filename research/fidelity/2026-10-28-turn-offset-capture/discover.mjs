// A single inert list request, using the existing authored decoder; no sockets.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { decodeList } from '../../../tools/fidelity/collision-delay-scout.mjs';
import { verifySources } from './capture.mjs';

export async function discover(root) {
  verifySources();
  if (!fs.existsSync(path.join(root, 'guard-active.json'))) throw Error('exclusive guard required');
  const config = JSON.parse(fs.readFileSync(path.join(root, 'discovery-private.json')));
  const write = (name, value) => fs.writeFileSync(path.join(root, name), JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
  write('discovery-reservation.json', { requestedWall: Date.now(), attempts: 1 });
  const response = await fetch(config.url, { redirect: 'error', signal: AbortSignal.timeout(15000) });
  const chunks = []; let length = 0;
  for await (const chunk of response.body) {
    chunks.push(chunk); length += chunk.length;
    if (length > 262144) throw Error('discovery response exceeds bound');
  }
  const raw = Buffer.concat(chunks);
  fs.writeFileSync(path.join(root, 'discovery-raw.txt'), raw, { flag: 'wx' });
  if (!response.ok) throw Error('discovery HTTP failure');
  const entries = decodeList(raw.toString('utf8'));
  write('discovery-decoded-private.json', entries);
  const active = entries.filter(e => e.active);
  const selected = active.find(e => e.ip === config.preferred.ip && e.port === config.preferred.port) ?? active[0];
  if (!selected) throw Error('no active discovery entries');
  write('endpoint-private.json', { ip: selected.ip, port: selected.port });
  write('discovery-result-private.json', { sha256: createHash('sha256').update(raw).digest('hex'),
    entries: entries.length, active: active.length, selected, preferredUsed: selected.ip === config.preferred.ip && selected.port === config.preferred.port });
  console.log(JSON.stringify({ discoveryComplete: true, entries: entries.length, active: active.length }));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  discover(process.argv[2]).catch(error => { console.error(error); process.exitCode = 1; });
