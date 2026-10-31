import test from 'node:test';
import assert from 'node:assert/strict';
import { analyze } from './latency-trace.mjs';

// Authored CPU stage sequence, not network/display evidence.
function fixture() {
  const rows = [];
  const row = (stage, id, related = 0, detail = 0) => rows.push({ thread: 0, event: rows.length,
    ns: String(rows.length * 1000), stage, generation: 1, id: String(id), related: String(related), detail: String(detail) });
  row('poll', 1); row('read', 2, 1, 100); row('message', 3, 2, 2); row('deferred', 3);
  row('apply_begin', 3); row('apply_end', 3);
  row('build_begin', 1, 3); row('build_end', 1, 3);
  row('acquire_begin', 12); row('acquire_end', 12); row('issued', 1, 12, 1); row('retried', 1); row('issued', 1, 12, 1);
  row('sync_begin', 1); row('sync_end', 1);
  row('submit_begin', 1, 10); row('submit_end', 1, 10); row('submit_begin', 1, 11); row('submit_end', 1, 11);
  row('pre_present_notify', 1); row('present_begin', 1); row('present_end', 1); row('presented', 1);
  row('callback', 4); row('admitted', 4); row('effect', 5, 0, 1); row('decision', 5, 4, 3);
  row('backpressure', 5); row('accepted', 5); row('ack_effect', 5); row('flushed', 5);
  row('acquire_begin', 6); row('acquire_end', 6); row('thread_end', 0);
  return rows;
}
function encode(rows, header = {}) {
  const counters = new Map();
  return [JSON.stringify({ format: 'latency-trace-v1', run: 'synthetic', clockDomain: 'process-instant-1-1',
    sourceRevision: 'a'.repeat(40), complete: true, dropped: 0, threads: 1, ...header }),
  ...rows.map(r => { const event = counters.get(r.thread) ?? 0; counters.set(r.thread, event + 1); return JSON.stringify({ ...r, event }); }), JSON.stringify({ end: true, records: rows.length })].join('\n');
}
test('joins retained retries, last submit, accepted backpressure and eventual flush', () => {
  const result = analyze(encode(fixture()));
  assert.equal(result.complete, true);
  assert.equal(result.distributions.buildToLastSubmit.count, 1);
  assert.equal(result.distributions.buildToLastSubmit.p50Ms, .011);
  assert.equal(result.distributions.admissionToFlushed.p95Ms, .006);
  assert.equal(result.distributions.acquireWait.p50Ms, .001);
});
test('rejects overflow, false completion, missing end and missing thread retirement', () => {
  for (const header of [{ dropped: 1 }, { complete: false }]) assert.throws(() => analyze(encode(fixture(), header)), /incomplete/);
  assert.throws(() => analyze(encode(fixture()).split('\n').slice(0, -1).join('\n')), /completion/);
  assert.throws(() => analyze(encode(fixture().slice(0, -1))), /thread completion/);
});
test('rejects duplicate/orphan IDs and missing packet, picture, control and flush completions', () => {
  const rows = fixture();
  for (const stage of ['apply_end', 'build_end', 'presented', 'ack_effect', 'flushed']) {
    assert.throws(() => analyze(encode(rows.filter(r => r.stage !== stage))), /orphan|incomplete|ownership|completion/);
  }
  const duplicate = [...rows]; duplicate.splice(3, 0, { ...rows[2] });
  assert.throws(() => analyze(encode(duplicate)), /duplicate/);
  const orphan = structuredClone(rows); orphan.find(r => r.stage === 'decision').related = '999';
  assert.throws(() => analyze(encode(orphan)), /orphan/);
  const watermark = structuredClone(rows); watermark.find(r => r.stage === 'build_end').related = '999';
  assert.throws(() => analyze(encode(watermark)), /watermark/);
  assert.throws(() => analyze(encode(rows.filter(r => r.stage !== 'retried'))), /ownership/);
});
test('a Setup build nested in application keeps distinct start/end intervals', () => {
  const rows = fixture(), applyEnd = rows.splice(rows.findIndex(r => r.stage === 'apply_end'), 1)[0];
  rows.splice(rows.findIndex(r => r.stage === 'build_end') + 1, 0, applyEnd);
  rows.forEach((r, i) => r.ns = String(i * 1000));
  const result = analyze(encode(rows));
  assert.equal(result.distributions.applyStartToBuildEnd.count, 1);
  assert.equal(result.distributions.applyEndToBuildEnd, undefined);
});

test('readable first-pending wakes join across threads; writable-only wakes are not read readiness', () => {
  const rows = fixture();
  rows.splice(1, 0, { thread: 0, ns: '500', stage: 'poll_ready', generation: 1, id: '1', related: '1', detail: '0' });
  const ready = { thread: 1, ns: '100', stage: 'ready', generation: 1, id: '1', related: '0', detail: '1' };
  rows.push(ready, { ...ready, ns: '200', stage: 'thread_end', id: '0', detail: '0' });
  const result = analyze(encode(rows, { threads: 2 }));
  assert.equal(result.distributions.readyToRead.p95Ms, .000999);
  ready.detail = '2';
  assert.equal(analyze(encode(rows, { threads: 2 })).distributions.readyToRead, undefined);
});

test('a handshake read can be retained into a UI-thread message', () => {
  const rows = fixture();
  const read = rows.find(r => r.stage === 'read'); read.stage = 'handshake_read'; read.thread = 1; read.related = '0';
  rows.push({ ...read, ns: '1001', stage: 'thread_end', id: '0', related: '0', detail: '0' });
  assert.equal(analyze(encode(rows, { threads: 2 })).distributions.readToApply.count, 1);
});
test('direct exit Close completion is distinct from a control acknowledgement', () => {
  const rows = fixture();
  rows.splice(-1, 0, { ...rows[0], stage: 'effect', id: '90', related: '0', detail: '2' },
    { ...rows[0], stage: 'host_close', id: '90', related: '0', detail: '0' });
  rows.forEach((r,i) => r.ns = String(i * 1000));
  assert.equal(analyze(encode(rows)).complete, true);
  rows.find(r => r.id === '90' && r.stage === 'effect').detail = '1';
  assert.throws(() => analyze(encode(rows)), /effect completion/);
});

test('retained input intent can contribute after a connection generation changes', () => {
  const rows = fixture();
  for (const r of rows) if (['effect', 'decision', 'accepted', 'ack_effect', 'flushed', 'backpressure'].includes(r.stage)) r.generation = 2;
  assert.equal(analyze(encode(rows)).distributions.admissionToFlushed.count, 1);
});

test('retry and retirement cannot hide open submit, sync or present spans', () => {
  for (const end of ['submit_end', 'sync_end', 'present_end']) {
    for (const boundary of ['skipped', 'retried', 'presented']) {
      const rows = fixture();
      rows.splice(rows.findLastIndex(r => r.stage === end), 1);
      rows.find(r => r.stage === 'presented').stage = boundary;
      assert.throws(() => analyze(encode(rows), { cpu: true }), /unfinished picture span/);
    }
  }
});
test('final completeness also rejects an open span without retirement', () => {
  for (const begin of ['submit_begin', 'sync_begin', 'present_begin']) {
    const rows = fixture(), end = rows.at(-1);
    const prefix = rows.slice(0, rows.findIndex(r => r.stage === begin) + 1);
    assert.throws(() => analyze(encode([...prefix, end]), { cpu: true }), /unfinished picture span/);
  }
});
function tail(stage, id, related = '0', detail = '0') {
  const rows = fixture();
  rows.splice(-1, 0, { ...rows[0], stage, id, related, detail });
  rows.forEach((r, i) => r.ns = String(i * 1000));
  return rows;
}

function terminalFixture() {
  const rows = fixture();
  const loss = rows.find(r => r.stage === 'flushed');
  rows.splice(rows.indexOf(loss), 0, { ...loss, stage: 'normal_close', id: '90', related: '0' });
  loss.stage = 'closed_unflushed'; loss.related = '90';
  rows.forEach((r, i) => r.ns = String(i * 1000));
  return rows;
}
test('terminal accepted-but-unflushed sends are counted without inventing delivery', () => {
  const rows = terminalFixture();
  const result = analyze(encode(rows));
  assert.equal(result.complete, true);
  assert.equal(result.closedUnflushed, 1);
  assert.equal(result.distributions.acceptedToFlushed, undefined);
  assert.equal(result.distributions.admissionToFlushed, undefined);
  assert.equal(result.distributions.decisionToAccepted.count, 1);
  const duplicate = structuredClone(rows);
  duplicate.splice(-1, 0, { ...rows.find(r => r.stage === 'closed_unflushed'), stage: 'flushed' });
  duplicate.forEach((r, i) => r.ns = String(i * 1000));
  assert.throws(() => analyze(encode(duplicate)), /duplicate send completion/);
  const orphan = structuredClone(rows);
  orphan.find(r => r.stage === 'closed_unflushed').id = '9999';
  assert.throws(() => analyze(encode(orphan)), /completion|orphan/);
});

test('terminal losses require a normal-close witness', () => {
  const rows = terminalFixture();
  assert.throws(() => analyze(encode(rows.filter(r => r.stage !== 'normal_close'))), /orphan.*normal_close/);
  rows.find(r => r.stage === 'closed_unflushed').related = '0';
  assert.throws(() => analyze(encode(rows)), /missing normal-close witness/);
});

test('send after terminal close is rejected even with a complete lifecycle and equal timestamps', () => {
  for (const tied of [false, true]) {
    const rows = terminalFixture();
    const later = ['effect', 'accepted', 'ack_effect', 'flushed'].map(stage => ({ ...rows[0], stage, id: '99', related: '0', detail: stage === 'effect' ? '1' : '0' }));
    rows.splice(-1, 0, ...later);
    const close = rows.findIndex(r => r.stage === 'normal_close');
    rows.forEach((r, i) => r.ns = String((tied && i >= close ? close : i) * 1000));
    assert.throws(() => analyze(encode(rows)), /send after terminal close/);
    // The same complete send is legal on the next connection generation.
    later.forEach(r => r.generation = 2);
    assert.equal(analyze(encode(rows)).complete, true);
  }
});

test('a send created before close cannot be accepted or flushed afterwards', () => {
  const rows = terminalFixture();
  rows.splice(rows.findIndex(r => r.stage === 'normal_close'), 0,
    { ...rows[0], stage: 'effect', id: '99', related: '0', detail: '1' });
  rows.splice(-1, 0, ...['accepted','ack_effect','flushed'].map(stage => ({ ...rows[0], stage, id: '99', related: '0', detail: '0' })));
  rows.forEach((r, i) => r.ns = String(i * 1000));
  assert.throws(() => analyze(encode(rows)), /send after terminal close/);
});

test('fatal shutdown I/O invalidates analysis even without pending send loss', () => {
  for (const operation of ['1', '2', '3']) {
    assert.throws(() => analyze(encode(tail('transport_failed', '0', '0', operation))), /failed stage transport_failed/);
  }
});
test('every nonzero wire effect and poll reference must resolve', () => {
  for (const stage of ['write', 'write_blocked', 'flush_blocked']) {
    for (const [id, poll] of [['9999', '0'], ['0', '8888'], ['9999', '8888']]) {
      assert.throws(() => analyze(encode(tail(stage, id, poll))), /orphan/);
    }
  }
  assert.equal(analyze(encode(tail('write', '5', '1', '42'))).complete, true);
  assert.equal(analyze(encode(tail('write', '0', '0', '42'))).complete, true);
});
test('ingress acquisition and packet references cannot be orphaned', () => {
  for (const [id, packet, generation] of [['9999', '0', '1'], ['0', '9999', '0'], ['12', '3', '9999']]) {
    assert.throws(() => analyze(encode(tail('last_ingress', id, packet, generation))), /orphan/);
  }
  const rows = tail('last_ingress', '12', '3', '0');
  for (const r of rows) {
    if (['acquire_begin', 'acquire_end'].includes(r.stage) && r.id === '12') r.generation = 0;
    if (r.stage === 'issued') r.detail = '0';
  }
  assert.equal(analyze(encode(rows)).complete, true);
});
