// Controlled study scout: sequential admissions, no discovery-time ping sockets.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { Recorder, captureSourceIdentity, createCaptureBatch, finalizeCaptureBatch } from './slither-headless.mjs';
import { DelayBot } from './slither-collision-delay.mjs';

const SOURCE = captureSourceIdentity(import.meta.url);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));

// Transcription of loadSos's byte decoder, with no eval and no /ptc sockets.
// Current upstream JS/raw list are retained privately and pinned in the study.
export function decodeList(text) {
  const bytes = []; let accumulator = 0, nibble = 0, offset = 0;
  for (let i = 1; i < text.trim().length; i++) {
    const value = ((text.charCodeAt(i) - 97 - offset) % 26 + 26) % 26;
    if (value > 15) throw Error('invalid discovery nibble');
    accumulator = accumulator * 16 + value; offset += 7;
    if (++nibble === 2) { bytes.push(accumulator); accumulator = 0; nibble = 0; }
  }
  if (nibble || bytes.length % 28) throw Error('truncated discovery list');
  const result = [];
  for (let i = 0; i < bytes.length; i += 28) {
    result.push({ listedIndex: i / 28, active: bytes[i] <= 26,
      ip: bytes.slice(i + 1, i + 5).join('.'), port: bytes[i + 21] * 256 + bytes[i + 22],
      advertisedCount: bytes[i + 23] * 256 + bytes[i + 24] });
  }
  return result;
}

export async function scoutOne({ endpoint, directory, plan, local = false }) {
  fs.mkdirSync(directory, { recursive: true });
  const server = `${endpoint.ip}:${endpoint.port}`;
  const batch = createCaptureBatch({ scenario: 'collision-delay-scout', server, plan, seconds: 12 }, 1, SOURCE);
  const rec = new Recorder(path.join(directory, 'bot0.jsonl.gz'), { schema: 1,
    server: { ip: endpoint.ip, port: endpoint.port }, scenario: 'collision-delay-scout',
    t: performance.now(), wall: Date.now(), dataOrigin: local ? 'local-simulator' : 'official-server',
    captureProvenance: batch.provenance(0) });
  const bot = new DelayBot({ nick: 'Collision scout', serverBody: true, delayMs: 0,
    origin: local ? undefined : 'http://slither.io' }, 0, rec);
  let reason = null, failure = null, visible = null, setup = false, firstPopulation = null;
  const started = performance.now();
  try {
    bot.connect(endpoint);
    while (performance.now() - started < 12000) {
      const now = performance.now(); bot.flush(now);
      const w = bot.world, me = w.snakes.get(w.self);
      setup ||= bot.joins > 0;
      if (me && !w.dead) {
        const head = w.head(me, now);
        bot.drive({ heading: Math.atan2(w.grd - head.y, w.grd - head.x), boost: false }, now);
      }
      if (bot.population !== null) {
        firstPopulation = bot.population;
        visible = [...bot.tap.snakes.keys()].filter(id => id !== bot.tap.self).length;
        reason = 'leaderboard'; break;
      }
      if (!bot.ws) { reason = 'early_close'; break; }
      if (rec.error) throw rec.error;
      await wait(10);
    }
    reason ??= 'leaderboard_timeout';
    bot.ws?.close(); const closeAt = performance.now();
    while (bot.ws) {
      if (performance.now() - closeAt > 5000) throw Error('close_timeout');
      await wait(10);
    }
  } catch (e) { failure = e; }
  const result = { listedIndex: endpoint.listedIndex, firstPopulation, visibleOtherSnakes: visible,
    setup, reason, elapsedMs: performance.now() - started, completedWall: Date.now() };
  try {
    await finalizeCaptureBatch({ bots: [bot], source: SOURCE, completed: !failure, error: failure,
      metadata: { scout: result } });
  } catch (e) { failure = e; }
  result.failure = failure ? String(failure) : null;
  result.archiveSha256 = digest(fs.readFileSync(path.join(directory, 'bot0.jsonl.gz')));
  fs.writeFileSync(path.join(directory, 'result.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  return result;
}

export async function scoutList({ listFile, plan, out, maximum = 48 }) {
  if (fs.existsSync(out)) throw Error('output exists');
  const frozen = JSON.parse(fs.readFileSync(plan));
  if (!frozen.offlineGate?.passed) throw Error('offline gate is closed');
  if (digest(fs.readFileSync(listFile)) !== frozen.discovery?.listSha256) throw Error('discovery list differs from frozen plan');
  for (const name of ['slither-headless.mjs', 'slither-paired-collision.mjs', 'slither-collision-delay.mjs', 'collision-delay-scout.mjs']) {
    const relative = `tools/fidelity/${name}`;
    if (digest(fs.readFileSync(new URL(`./${name}`, import.meta.url))) !== frozen.sourceSha256?.[relative])
      throw Error(`source differs from frozen plan: ${relative}`);
  }
  fs.mkdirSync(out, { recursive: true });
  const endpoints = decodeList(fs.readFileSync(listFile, 'utf8')).filter(e => e.active);
  fs.copyFileSync(listFile, path.join(out, 'official-list.txt'));
  fs.copyFileSync(plan, path.join(out, 'plan.json'));
  const results = [];
  for (const endpoint of endpoints.slice(0, maximum)) {
    if (results.length) await wait(results.at(-1).reason === 'early_close' ? 30000 : 8000);
    const result = await scoutOne({ endpoint, plan, directory: path.join(out, `scout-${results.length + 1}`) });
    results.push(result);
    fs.appendFileSync(path.join(out, 'attempts.jsonl'), JSON.stringify(result) + '\n');
    console.log(JSON.stringify({ scout: results.length, ...result }));
    if (!result.failure && result.firstPopulation !== null && result.firstPopulation <= 6 &&
        result.visibleOtherSnakes <= 5) {
      fs.writeFileSync(path.join(out, 'selected-private.json'), JSON.stringify({ endpoint, result }, null, 2) + '\n');
      break;
    }
    if (result.failure) break;
  }
  return results;
}

if (process.argv[1]?.endsWith('collision-delay-scout.mjs')) {
  const [listFile, plan, out] = process.argv.slice(2);
  if (!listFile || !plan || !out) throw Error('list, frozen plan, fresh output directory required');
  await scoutList({ listFile, plan, out });
}
