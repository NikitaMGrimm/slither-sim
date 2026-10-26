// Real production WebSocket host, two wire clients, no outside connections.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { recordPair } from './slither-paired-collision.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const binary = path.resolve(root, process.env.CARGO_TARGET_DIR ?? 'target', 'debug',
  process.platform === 'win32' ? 'slither-sim.exe' : 'slither-sim');

test('localhost host completes all ladder offsets and reversed head-on join cells', { timeout: 300000 }, async () => {
  assert.ok(fs.existsSync(binary), 'build first: CARGO_BUILD_JOBS=2 cargo build --locked --bin slither-sim');
  // Retain full archives, including failed runs, outside Git for diagnosis.
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-collision-'));
  console.log('retained local-only evidence:', directory);
  const config = path.join(directory, 'host.json');
  fs.writeFileSync(config, JSON.stringify({ bind: '127.0.0.1:0', seed: 7,
    players: { max: 2 }, bots: { count: 0, threads: 1 }, simulation: { world: {
      radius: 10000, vision: 20000, spawnRadius: 600, minScore: 100,
      foodCount: 0, ambientReplenishPerSecond: 0, prey: { count: 0 },
    } } }));
  const host = spawn(binary, ['host', '--config', config], {
    windowsHide: true, env: { ...process.env, RAYON_NUM_THREADS: '2' }, stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '';
  try {
    const server = await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error(`host start timeout: ${output}`)), 10000);
      host.on('error', error => { clearTimeout(timeout); reject(error); });
      host.on('exit', code => { clearTimeout(timeout); reject(Error(`host exit ${code}: ${output}`)); });
      const read = bytes => {
        output += bytes;
        const match = /ws:\/\/(127\.0\.0\.1:\d+)\/slither/.exec(output);
        if (match) { clearTimeout(timeout); resolve(match[1]); }
      };
      host.stdout.on('data', read);
      host.stderr.on('data', read);
    });
    const plan = path.join(directory, 'plan.json');
    fs.writeFileSync(plan, JSON.stringify({ dataOrigin: 'local-simulator' }));
    const results = await recordPair({ out: path.join(directory, 'raw'), server,
      seconds: 240, localCycle: true, plan });
    fs.writeFileSync(path.join(directory, 'results.json'), JSON.stringify(results, null, 2));
    console.log(JSON.stringify(results.map(r => ({ trial: r.trial, ...r.spec,
      qualified: r.qualified, reasons: r.reasons, offset: r.targetOffset,
      achieved: r.achievedMinOffset, deaths: r.deaths, outcome: r.outcome }))));
    assert.equal(results.length, 12, 'one full schedule without replacements');
    const qualified = results.filter(row => row.qualified);
    // This is a real-time acquisition controller; staging can legitimately
    // abort under concurrent CI load. Keep those rows and require witnesses
    // for EVERY offset, both probe identities, and all head-on cells.
    for (const factor of [1.3, 1.1, .9, .7])
      assert.ok(qualified.some(r => r.spec.kind === 'ladder' && r.spec.factor === factor), `missing offset ${factor}`);
    for (const probe of [0, 1]) for (const outcome of ['contact', 'survival'])
      assert.ok(qualified.some(r => r.spec.kind === 'ladder' && r.spec.probe === probe && r.outcome === outcome),
        `missing probe ${probe} ${outcome}`);
    assert.equal(qualified.filter(r => r.spec.kind === 'head-on').length, 4);
    for (const row of results) {
      if (!row.qualified) { assert.ok(row.reasons.length); continue; }
      const expected = row.spec.kind === 'ladder' ? row.spec.factor < 1 : true;
      assert.equal(row.outcome, expected ? 'contact' : 'survival', `trial ${row.trial}`);
      if (row.spec.kind === 'ladder') {
        assert.equal(row.deaths[row.spec.probe], expected);
        assert.equal(row.deaths[1 - row.spec.probe], false);
      } else {
        // Byte headings cannot express exactly opposing rays (251 is odd).
        // A qualified approximate crossing need not be a mutual strike. The
        // exact mutual-contact/order assertion lives in collision_curve.rs.
        assert.equal(row.deaths.filter(Boolean).length, 1, 'current model records one survivor');
      }
    }
    const streams = [0, 1].map(i => zlib.gunzipSync(fs.readFileSync(path.join(directory, 'raw', `bot${i}.jsonl.gz`))).toString());
    for (const stream of streams) {
      const lines = stream.trimEnd().split('\n'), rows = lines.map(x => JSON.parse(x));
      const end = rows.at(-1);
      assert.equal(end.event, 'capture-end');
      assert.equal(end.collectorSourcesUnchanged, true);
      assert.equal(end.integrity.sha256, createHash('sha256').update(lines.slice(0, -1).join('\n') + '\n').digest('hex'));
      assert.ok(rows.some(r => r[0] === 'i')); assert.ok(rows.some(r => r[0] === 'o'));
      let sockets = 0;
      for (const row of rows) {
        if (row.event === 'connect') sockets++;
        if (row.event === 'close') sockets--;
        assert.ok(sockets >= 0 && sockets <= 1, 'no replacement before close');
      }
      assert.equal(sockets, 0);
    }
  } finally {
    fs.writeFileSync(path.join(directory, 'host.log'), output);
    host.kill();
    await new Promise(resolve => host.exitCode !== null ? resolve() : host.once('exit', resolve));
  }
});
