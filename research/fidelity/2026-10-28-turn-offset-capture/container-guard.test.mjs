import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { classify, preflight, snapshotReader, run } from './container-guard.mjs';
const sample = ss => ({ windows: { ok: true, connections: [] },
  remote: { ok: true, ss, ownedPids: [21, 22], commandLines: { 99: 'authored foreign command' } } });
test('fresh preflight permits no connection and admits only two owned connections during capture', () => {
  assert.equal(classify(sample(''), 'peer:7', true).conflicts.length, 0);
  const line = 'ESTAB 0 0 local:8 peer:7 users:(("node",pid=22,fd=4))';
  assert.equal(classify(sample(line), 'peer:7').ours, 1);
  assert(classify(sample(line), 'peer:7', true).conflicts.length);
  assert(classify(sample([line,line,line].join('\n')), 'peer:7').conflicts.length);
});
test('authored child seals before completion; post-launch conflict interrupts without replacement', async () => {
  for (const conflict of [false, true]) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'authored-container-'));
    let timer;
    try {
      fs.writeFileSync(path.join(dir, 'endpoint-private.json'), JSON.stringify({ ip: [127,0,0,1].join('.'), port: 9 }));
      const fake = path.join(dir, 'authored-child.mjs');
      fs.writeFileSync(fake, `import fs from 'node:fs'; import path from 'node:path';
        const root=process.argv[2]; fs.writeFileSync(path.join(root,'started'),'authored');
        const finish=completed=>{fs.mkdirSync(path.join(root,'invocation-1'));
          fs.writeFileSync(path.join(root,'invocation-1/result-private.json'),JSON.stringify({completed,socketsClosed:true,elapsedSeconds:.1,reason:'authored'}));};
        ${conflict ? `const t=setInterval(()=>{if(fs.existsSync(path.join(root,'STOP'))){clearInterval(t);finish(false);}},20);` : `finish(true);`}`);
      let sequence = 0;
      const publish = () => {
        const s = sample('');
        if (conflict && fs.existsSync(path.join(dir, 'started'))) s.windows.connections = [{ pid: 99, commandLine: 'authored foreign command' }];
        fs.writeFileSync(path.join(dir, 'connection-snapshot-private.json'), JSON.stringify({ ...s, instance: 'authored', sequence: ++sequence }));
      };
      publish(); timer = setInterval(publish, 50);
      if (conflict) await assert.rejects(run(dir, 'authored', { captureFile: fake }), /conflict/);
      else await run(dir, 'authored', { captureFile: fake });
      assert.equal(fs.readFileSync(path.join(dir, 'invocations.jsonl'), 'utf8').trim().split('\n').length, 1);
      assert.equal(JSON.parse(fs.readFileSync(path.join(dir, 'invocation-1/result-private.json'))).completed, !conflict);
      assert.equal(fs.existsSync(path.join(dir, 'STUDY-STOP.json')), conflict);
      assert(!fs.existsSync(path.join(dir, '.container-capture.lock')));
      assert(!fs.existsSync(path.join(dir, 'guard-active.json')));
    } finally {
      clearInterval(timer);
      assert(dir.startsWith(path.resolve(os.tmpdir()) + path.sep));
      fs.rmSync(dir, { recursive: true });
    }
  }
});
test('conflicts retain command identities; unrelated processes are never inputs', () => {
  const s = sample('SYN-SENT 0 0 local:8 peer:7 users:(("other",pid=99,fd=4))');
  assert.equal(classify(s, 'peer:7').conflicts[0].commandLines[0], 'authored foreign command');
  s.windows.connections.push({ pid: 40, commandLine: 'authored controller command' });
  assert.equal(classify(s, 'peer:7').conflicts.length, 2);
});
test('missing attribution conflicts; closing connections count; time-wait does not', () => {
  assert(classify(sample('FIN-WAIT-1 0 0 local:8 peer:7'), 'peer:7').conflicts.length);
  assert.equal(classify(sample('TIME-WAIT 0 0 local:8 peer:7'), 'peer:7').conflicts.length, 0);
  const unavailable = sample(''); unavailable.remote.ok = false;
  assert.throws(() => classify(unavailable, 'peer:7'));
  assert.throws(() => classify(sample('broken'), 'peer:7'));
});
test('four 15-minute rechecks precede exhaustion and generic errors never retry', async () => {
  let tick = 0, checks = 0;
  await assert.rejects(preflight(async () => { checks++; return { conflicts: [{}] }; }, () => {},
    { now: () => tick, sleep: async ms => { tick += ms; } }), /exhausted/);
  assert.equal(checks, 5); assert.equal(tick, 3600000);
  checks = 0;
  await assert.rejects(preflight(async () => { checks++; throw Error('source failure'); }, () => {}));
  assert.equal(checks, 1);
});
test('preflight later success preserves every failure without shortening wait', async () => {
  let tick = 0, checks = 0; const records = [];
  await preflight(async () => ({ conflicts: ++checks < 3 ? [{}] : [] }), r => records.push(r),
    { now: () => tick, sleep: async ms => { tick += ms; } });
  assert.equal(tick, 1800000); assert.equal(checks, 3);
  assert.equal(records.filter(r => r.event === 'preflight-check').length, 3);
});
test('monitor lease uses local monotonic receipt and rejects stale or backward snapshots', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'authored-guard-'));
  try {
    const file = path.join(dir, 'snapshot.json'); let tick = 0;
    const read = snapshotReader(file, 'new', () => tick);
    assert.equal(read(), null);
    fs.writeFileSync(file, JSON.stringify({ instance: 'old', sequence: 10 })); assert.equal(read(), null);
    fs.writeFileSync(file, JSON.stringify({ instance: 'new', sequence: 1 })); assert.equal(read().sequence, 1);
    tick = 15001; assert.throws(read, /expired/);
    fs.writeFileSync(file, JSON.stringify({ instance: 'new', sequence: 2 })); assert.equal(read().sequence, 2);
    fs.writeFileSync(file, JSON.stringify({ instance: 'new', sequence: 1 })); assert.throws(read, /sequence/);
  } finally { fs.rmSync(dir, { recursive: true }); }
});
