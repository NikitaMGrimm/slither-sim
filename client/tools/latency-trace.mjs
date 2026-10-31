import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const stages = new Set(`thread_end ready poll poll_ready read handshake_read message deferred apply_begin apply_end apply_failed
build_begin build_end build_failed issued superseded retried skipped picture_failed
sync_begin sync_end submit_begin submit_end pre_present_notify present_begin present_end presented
callback admitted suppressed coalesced effect decision decision_input accepted write write_blocked backpressure
flush_blocked flushed ack_effect failed_effect send_abandoned game_failed host_close acquire_begin acquire_end last_ingress`.split(/\s+/));
const integer = x => Number.isSafeInteger(x) && x >= 0;
const decimal = x => typeof x === 'string' && /^(0|[1-9][0-9]*)$/.test(x) && BigInt(x) <= 0xffffffffffffffffn;
const check = (ok, message) => { if (!ok) throw Error(message); };
const key = r => `${r.thread}/${r.generation}/${r.id}`;
const ref = (r, id) => `${r.thread}/${r.generation}/${id}`;

export function analyze(text, { cpu = false } = {}) {
  const lines = text.trim().split('\n').map(line => JSON.parse(line));
  const header = lines.shift(), end = lines.pop();
  check(header?.format === 'latency-trace-v1' && header.complete === true && header.dropped === 0,
    'incomplete/overflow or unsupported trace');
  check(typeof header.run === 'string' && header.run.length > 0 && typeof header.clockDomain === 'string' && /^process-instant-[0-9]+-[0-9]+$/.test(header.clockDomain)
    && /^[a-f0-9]{8,64}$/i.test(header.sourceRevision) && integer(header.threads) && header.threads > 0, 'invalid header');
  check(end?.end === true && end.records === lines.length, 'missing completion or truncated tail');
  const threads = new Map(), maps = new Map(), intervals = new Map(), dependencies = new Set(), submissions = new Set();
  const add = (name, begin, finish) => {
    check(finish >= begin, `reversed ${name}`);
    if (!intervals.has(name)) intervals.set(name, []);
    intervals.get(name).push(Number(finish - begin) / 1e6);
  };
  const table = stage => { if (!maps.has(stage)) maps.set(stage, new Map()); return maps.get(stage); };
  const one = (stage, r, id = r.id) => {
    const row = table(stage).get(ref(r, id));
    check(row, `orphan ${r.stage}: missing ${stage} ${ref(r, id)}`);
    return row;
  };
  const unique = (stage, r) => {
    check(!table(stage).has(key(r)), `duplicate ${stage} ${key(r)}`);
    table(stage).set(key(r), r);
  };
  for (const r of lines) {
    check(integer(r.thread) && r.thread < header.threads && integer(r.generation) && integer(r.event)
      && decimal(r.ns) && decimal(r.id) && decimal(r.related) && decimal(r.detail) && stages.has(r.stage), 'invalid record');
    const previous = threads.get(r.thread);
    check(r.event === (previous ? previous.event + 1 : 0), 'duplicate or missing event');
    r.t = BigInt(r.ns);
    check(!previous || r.t >= previous.t, 'thread clock reversed');
    threads.set(r.thread, r);
    // Repeated issue/retry/submit/write records have their own ordered event identity.
    if (!['issued', 'retried', 'submit_begin', 'submit_end', 'sync_begin', 'sync_end', 'write',
      'backpressure', 'write_blocked', 'flush_blocked', 'last_ingress', 'decision_input', 'coalesced'].includes(r.stage)) unique(r.stage, r);
    if (r.stage === 'submit_begin') {
      const k = ref(r, r.related); check(r.related !== '0' && !submissions.has(k), 'duplicate/invalid submission ID'); submissions.add(k);
    }
    if (['decision_input', 'coalesced'].includes(r.stage)) {
      const k = `${r.stage}/${key(r)}/${r.related}/${r.detail}`; check(!dependencies.has(k), 'duplicate input dependency'); dependencies.add(k);
    }
    if (r.stage.endsWith('_failed') || r.stage === 'failed_effect' || r.stage === 'send_abandoned') throw Error(`failed stage ${r.stage}`);
  }
  check(threads.size === header.threads && [...threads.values()].every(r => r.stage === 'thread_end'), 'missing thread completion');
  const each = stage => [...table(stage).values()];
  // Ready IDs are the first-pending epoch offsets, so they can cross thread boundaries.
  const readiness = new Map();
  for (const r of each('ready')) {
    const k = `${r.generation}/${r.id}`;
    check(!readiness.has(k), 'duplicate readiness identity'); readiness.set(k, r);
  }
  for (const r of each('poll_ready')) {
    one('poll', r);
    check(readiness.has(`${r.generation}/${r.related}`), 'orphan readiness');
    check(r.t >= BigInt(r.related), 'reversed readiness');
  }
  const reads = new Map();
  for (const r of [...each('read'), ...each('handshake_read')]) {
    const k = `${r.generation}/${r.id}`; check(!reads.has(k), 'duplicate raw read ID'); reads.set(k, r);
  }
  const read = (r, id) => { const value = reads.get(`${r.generation}/${id}`); check(value, `orphan raw read ${id}`); return value; };
  for (const r of each('read')) {
    const p = one('poll', r, r.related);
    check(r.t >= p.t && BigInt(r.detail) > 0n, 'invalid read');
    const ready = table('poll_ready').get(key(p));
    const observation = ready && readiness.get(`${ready.generation}/${ready.related}`);
    if (observation && (BigInt(observation.detail) & 1n) !== 0n) add('readyToRead', BigInt(ready.related), r.t);
  }
  for (const r of each('message')) {
    if (r.related !== '0' || r.detail !== '0') {
      const first = read(r, r.related), last = read(r, r.detail);
      check(first.t <= last.t && last.t <= r.t, 'invalid read span');
    } else check(cpu, 'native message has no read span');
    const begin = one('apply_begin', r), finish = one('apply_end', r);
    check(begin.t >= r.t && finish.t >= begin.t, 'invalid packet apply');
    if (r.detail !== '0') add('readToApply', read(r, r.detail).t, begin.t);
  }
  for (const stage of ['apply_begin', 'apply_end', 'deferred']) {
    for (const r of each(stage)) one('message', r);
  }
  const inputs = new Map(each('admitted').map(r => [r.id, r]));
  check(inputs.size === each('admitted').length, 'duplicate process input ID');
  const input = id => { const r = inputs.get(id); check(r, `orphan input ${id}`); return r; };
  for (const r of each('callback')) {
    const admitted = table('admitted').get(key(r)), suppressed = table('suppressed').get(key(r));
    check(Boolean(admitted) !== Boolean(suppressed), 'input missing or duplicate admission outcome');
    check((admitted ?? suppressed).t >= r.t, 'input outcome precedes callback');
  }
  for (const stage of ['admitted', 'suppressed']) for (const r of each(stage)) one('callback', r);
  for (const r of lines.filter(r => r.stage === 'coalesced')) {
    input(r.id); input(r.related);
  }
  for (const r of each('effect')) {
    check(['0', '1', '2'].includes(r.detail), 'unknown effect kind');
    const acknowledged = table('ack_effect').get(key(r)), closed = table('host_close').get(key(r));
    check(Boolean(acknowledged) !== Boolean(closed) && (!closed || r.detail === '2'), 'missing/duplicate effect completion');
    const ack = acknowledged ?? closed;
    check(ack.t >= r.t, 'effect acknowledgement precedes decision');
    if (r.detail === '1') {
      const accepted = one('accepted', r), flushed = one('flushed', r);
      check(accepted.t >= r.t && flushed.t >= accepted.t, 'invalid effect send order');
      add('acceptedToFlushed', accepted.t, flushed.t);
    }
  }
  for (const stage of ['accepted', 'flushed', 'ack_effect', 'host_close', 'decision']) for (const r of each(stage)) one('effect', r);
  for (const r of [...each('accepted'), ...each('flushed'), ...each('decision')]) check(one('effect', r).detail === '1', 'send stage on non-send effect');
  for (const r of [...each('decision'), ...lines.filter(r => r.stage === 'decision_input')]) {
    const decision = one('decision', r), accepted = one('accepted', r), flushed = one('flushed', r);
    if (r.stage === 'decision') add('decisionToAccepted', decision.t, accepted.t);
    if (r.related !== '0') {
      const admitted = input(r.related);
      add('admissionToDecision', admitted.t, decision.t);
      add('admissionToFlushed', admitted.t, flushed.t);
    }
  }
  const pictures = new Map();
  for (const r of each('build_begin')) pictures.set(key(r), { begin: r, state: 'building', submits: 0 });
  for (const r of lines) {
    if (['build_begin', 'build_end', 'issued', 'retried', 'superseded', 'skipped', 'sync_begin', 'sync_end',
      'submit_begin', 'submit_end', 'pre_present_notify', 'present_begin', 'present_end', 'presented'].includes(r.stage)) {
      const p = pictures.get(key(r)); check(p, `orphan picture ${key(r)}`);
      switch (r.stage) {
        case 'build_begin': break;
        case 'build_end': {
          check(p.state === 'building' && p.begin.related === r.related, 'changed picture watermark');
          p.end = r; p.state = 'prepared';
          if (r.related !== '0') {
            const apply = one('apply_begin', r, r.related), applied = one('apply_end', r, r.related);
            add('applyStartToBuildEnd', apply.t, r.t);
            // Setup prepares inside application; report that overlap rather than invent a negative delay.
            if (r.t >= applied.t) add('applyEndToBuildEnd', applied.t, r.t);
          }
          break;
        }
        case 'issued': {
          check(p.state === 'prepared', 'issue violates picture ownership');
          if (r.related !== '0') {
            const acquire = table('acquire_end').get(`${r.thread}/${r.detail}/${r.related}`);
            check(acquire && acquire.t <= r.t, 'orphan/reversed picture acquisition');
          } else check(cpu, 'native picture has no acquisition');
          p.state = 'issued'; break;
        }
        case 'retried': check(p.state === 'issued', 'retry without issue'); p.state = 'prepared'; break;
        case 'superseded': check(p.state === 'prepared', 'superseded issued picture'); p.state = 'done'; break;
        case 'skipped': check(p.state === 'issued', 'skip without issue'); p.state = 'done'; break;
        case 'submit_begin': check(p.state === 'issued' && !p.submit, 'invalid submit'); p.submit = r; break;
        case 'submit_end': check(p.submit && p.submit.related === r.related && r.t >= p.submit.t, 'submit without matching begin'); p.submit = null; p.lastSubmit = r; p.submits++; break;
        case 'sync_begin': check(p.state === 'issued' && !p.sync, 'invalid resource sync'); p.sync = r; break;
        case 'sync_end': check(p.sync && r.t >= p.sync.t, 'sync without begin'); p.sync = null; break;
        case 'pre_present_notify': check(p.state === 'issued', 'notify without issue'); p.notify = r; break;
        case 'present_begin': check(p.state === 'issued' && p.notify && p.submits > 0 && !p.present, 'invalid present'); p.present = r; break;
        case 'present_end': check(p.present, 'present without begin'); add('presentCall', p.present.t, r.t); p.returned = r; break;
        case 'presented':
          check(p.state === 'issued' && (cpu || p.returned) && !p.submit && !p.sync, 'ack without completed present');
          p.state = 'done';
          if (p.lastSubmit) add('buildToLastSubmit', p.end.t, p.lastSubmit.t);
          break;
      }
    }
  }
  for (const p of pictures.values()) check(p.state === 'done', 'incomplete picture tail');
  for (const r of each('acquire_begin')) add('acquireWait', r.t, one('acquire_end', r).t);
  for (const r of each('acquire_end')) one('acquire_begin', r);
  const distributions = {};
  for (const [name, values] of intervals) {
    values.sort((a, b) => a - b);
    const q = p => values[Math.ceil(p * values.length) - 1];
    distributions[name] = { count: values.length, p50Ms: q(.5), p95Ms: q(.95), maxMs: values.at(-1) };
  }
  return { format: header.format, run: header.run, sourceRevision: header.sourceRevision,
    complete: true, endpoint: cpu ? 'CPU replay (no GPU/present proof)' : 'native CPU stages', distributions };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const args = process.argv.slice(2), cpu = args.includes('--cpu');
    const paths = args.filter(a => a !== '--cpu');
    check(paths.length === 1, 'usage: node tools/latency-trace.mjs [--cpu] trace.jsonl');
    console.log(JSON.stringify(analyze(readFileSync(paths[0], 'utf8'), { cpu }), null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
