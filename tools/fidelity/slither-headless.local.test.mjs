// Short production-host admission check; all sockets stay on loopback.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Bot, loginPacket, parseServer } from './slither-headless.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const binary = path.resolve(root, process.env.CARGO_TARGET_DIR ?? 'target', 'debug',
  process.platform === 'win32' ? 'slither-sim.exe' : 'slither-sim');

test('production loopback host accepts the standard protocol-19 collector login', {
  skip: !process.env.SLITHER_LOGIN_E2E && 'set SLITHER_LOGIN_E2E=1 to run', timeout: 15000,
}, async t => {
  assert.ok(fs.existsSync(binary), 'build first: cargo build --locked --bin slither-sim');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'slither-login-'));
  const config = path.join(directory, 'host.json');
  fs.writeFileSync(config, JSON.stringify({ bind: '127.0.0.1:0', bots: { count: 0, threads: 1 },
    simulation: { world: { radius: 1500, foodCount: 0, ambientReplenishPerSecond: 0, prey: { count: 0 } } } }));
  const host = spawn(binary, ['host', '--config', config], {
    windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
  });
  const closed = new Promise(resolve => host.once('close', resolve));
  let bot, output = '';
  t.after(async () => {
    bot?.stop();
    if (host.exitCode === null) host.kill();
    await closed;
    fs.rmSync(directory, { recursive: true, force: true });
  });
  const server = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(Error(`host start timeout: ${output}`)), 10000);
    const fail = error => { clearTimeout(timer); reject(error); };
    host.once('error', fail);
    host.once('exit', code => fail(Error(`host exit ${code}: ${output}`)));
    const read = bytes => {
      output += bytes;
      const match = /ws:\/\/(127\.0\.0\.1:\d+)\/slither/.exec(output);
      if (match) { clearTimeout(timer); resolve(parseServer(match[1])); }
    };
    host.stdout.on('data', read);
    host.stderr.on('data', read);
  });
  const sent = [];
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(Error(`collector never joined: ${output}`)), 3000);
    t.after(() => clearTimeout(timer));
    bot = new Bot({ nick: 'Loopback login', scenario: 'forage' }, 0, {
      meta() {},
      reportFailure(error) { clearTimeout(timer); reject(error); },
      frame(direction, bytes) {
        if (direction === 'o') sent.push([...bytes]);
        queueMicrotask(() => {
          if (bot.joins && bot.world.snakes.has(bot.world.self)) { clearTimeout(timer); resolve(); }
        });
      },
    });
    bot.connect(server);
  });
  assert.deepEqual(sent.find(bytes => bytes[0] === 115), [...loginPacket('Loopback login')]);
  assert.equal(bot.joins, 1);
});
