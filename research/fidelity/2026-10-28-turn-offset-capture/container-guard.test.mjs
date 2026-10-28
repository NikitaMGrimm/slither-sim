import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { classify, preflight, snapshotReader, run, boundedDelay } from './container-guard.mjs';
import { parseProc, namespaceCensus } from './namespace-census.mjs';
const sample = ss => ({ windows: { ok: true, connections: [] },
  remote: { ok: true, ss, ownedPids: [21, 22], commandLines: { 99: 'authored foreign command' } } });
test('poll and drain sleeps clamp to their remaining budget', async () => {
  for (const maximum of [250, 100, 50]) {
    let tick = 1199990; const delays = [];
    const options = { now: () => tick, sleep: async ms => { delays.push(ms); tick += ms; } };
    assert(await boundedDelay(1200000, maximum, options));
    assert.deepEqual(delays, [10]); assert.equal(tick, 1200000);
    assert(await boundedDelay(1200000, maximum, options)); assert.deepEqual(delays, [10]);
  }
});
test('fresh preflight permits no connection and admits only two owned connections during capture', () => {
  assert.equal(classify(sample(''), 'peer:7', true).conflicts.length, 0);
  const namespace = { connections: [{ attribution: 'dedicated-container-namespace', owners: [] }] };
  assert.equal(classify(sample(''), 'peer:7', false, namespace).ours, 1);
  assert(classify(sample(''), 'peer:7', true, namespace).conflicts.length);
  assert(classify(sample(''), 'peer:7', false, { connections: [{}, {}, {}] }).conflicts.length);
});
test('authored child seals before completion; post-launch conflict interrupts without replacement', async () => {
  for (const conflict of [false, true]) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'authored-container-'));
    let timer;
    try {
      fs.writeFileSync(path.join(dir, 'endpoint-private.json'), JSON.stringify({ ip: [127,0,0,1].join('.'), port: 9 }));
      fs.writeFileSync(path.join(dir, 'rehearsal-marker.json'), JSON.stringify({ origin: 'authored-local-rehearsal' }));
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
      const options = { captureFile: fake, namespaceProbe: () => ({ connections: [] }) };
      if (conflict) await assert.rejects(run(dir, 'authored', options), /conflict/);
      else await run(dir, 'authored', options);
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
  assert.equal(classify(s, 'peer:7').conflicts[0].machine, 'separate-host-namespace');
  s.windows.connections.push({ pid: 40, commandLine: 'authored controller command' });
  assert.equal(classify(s, 'peer:7').conflicts.length, 2);
});
test('host connections conflict without identity; missing optional namespace owners do not', () => {
  assert(classify(sample('FIN-WAIT-1 0 0 local:8 peer:7'), 'peer:7').conflicts.length);
  assert.equal(classify(sample('TIME-WAIT 0 0 local:8 peer:7'), 'peer:7').conflicts.length, 0);
  const unavailable = sample(''); unavailable.remote.ok = false;
  assert.throws(() => classify(unavailable, 'peer:7'));
  assert.throws(() => classify(sample('broken'), 'peer:7'));
  assert.equal(classify(sample(''), 'peer:7', false, { connections: [{ owners: [] }, { owners: [] }] }).conflicts.length, 0);
});
test('proc census handles IPv4 and mapped IPv6; namespace attribution survives missing optional fd data', () => {
  const endpoint = { ip: [127,0,0,1].join('.'), port: 7 };
  const row = (remote, state = '01') => ` 0: 00000000:0001 ${remote}:0007 ${state} 0:0 0:0 0 0 0 123`;
  const header = 'sl local_address rem_address st\n';
  assert.equal(parseProc(header + row('0100007F'), endpoint).length, 1);
  assert.equal(parseProc(header + row('0000000000000000FFFF00000100007F'), endpoint).length, 1);
  assert.equal(parseProc(header + row('0100007F', '06'), endpoint).length, 0);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'authored-proc-'));
  try {
    fs.mkdirSync(path.join(dir, 'net')); fs.mkdirSync(path.join(dir, '1'));
    fs.writeFileSync(path.join(dir, 'net/tcp'), header + row('0100007F'));
    fs.writeFileSync(path.join(dir, 'net/tcp6'), header);
    const census = namespaceCensus(endpoint, dir);
    assert.equal(census.connections.length, 1); assert.deepEqual(census.connections[0].owners, []);
    fs.unlinkSync(path.join(dir, 'net/tcp6'));
    assert.throws(() => namespaceCensus(endpoint, dir));
  } finally { assert(dir.startsWith(path.resolve(os.tmpdir()) + path.sep)); fs.rmSync(dir, { recursive: true }); }
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
