// Explicit loopback-only engineering run. Full recordings survive failures.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawn } from 'node:child_process';
import { recordTail } from './slither-collision-delay.mjs';

test('finite tail crossing pipeline on production loopback host', {
  skip: !process.env.SLITHER_DELAY_E2E, timeout: 650000,
}, async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-delay-'));
  console.log('retained local-only evidence:', directory);
  const config = path.join(directory, 'host.json'), plan = path.join(directory, 'plan.json');
  fs.writeFileSync(config, JSON.stringify({ bind: '127.0.0.1:0', seed: 7, players: { max: 2 },
    bots: { count: 0, threads: 1 }, simulation: { delivery: 'official', world: {
      radius: 10000, vision: 20000, spawnRadius: 600, minScore: 100, foodCount: 0,
      ambientReplenishPerSecond: 0, prey: { count: 0 } } } }));
  fs.writeFileSync(plan, JSON.stringify({ dataOrigin: 'local-simulator' }));
  const binary = path.resolve(process.env.CARGO_TARGET_DIR ?? 'target', 'debug', process.platform === 'win32' ? 'slither-sim.exe' : 'slither-sim');
  const host = spawn(binary, ['host', '--config', config], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  try {
    const server = await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error(output)), 15000);
      host.on('error', reject);
      const read = bytes => {
        output += bytes;
        const match = /ws:\/\/(127\.0\.0\.1:\d+)\/slither/.exec(output);
        if (match) { clearTimeout(timeout); resolve(match[1]); }
      };
      host.stdout.on('data', read); host.stderr.on('data', read);
    });
    const specs = [{ arm: 'control', offset: -240 }, { arm: 'delay', offset: -240 },
      { arm: 'delay', offset: 0 }, { arm: 'control', offset: 0 }];
    const results = await recordTail({ out: path.join(directory, 'raw'), server, plan, specs,
      delayMs: Number(process.env.SLITHER_DELAY_MS ?? 150), seconds: 540, connectGapMs: 1000 });
    console.log(JSON.stringify(results));
    assert.equal(results.length, specs.length);
    assert.ok(results.every(row => row.qualified), 'all offline qualification gates must pass');
    for (const arm of ['control', 'delay']) {
      assert.ok(results.some(r => r.arm === arm && r.outcome === 'survival'));
      assert.ok(results.some(r => r.arm === arm && r.outcome === 'contact'));
    }
  } finally {
    fs.writeFileSync(path.join(directory, 'host.log'), output);
    host.kill();
    await new Promise(resolve => host.exitCode !== null ? resolve() : host.once('exit', resolve));
  }
});
