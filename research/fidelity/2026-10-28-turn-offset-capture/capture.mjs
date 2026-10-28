// Research-only steering around the existing collector; importing never connects.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { Bot, Recorder, headingDanger, parseArgs, captureSourceIdentity,
  createCaptureBatch, finalizeCaptureBatch, runCollectorCli } from '../../../tools/fidelity/slither-headless.mjs';
import { ArcSelector, durations, key } from './selection.mjs';

const SOURCE = captureSourceIdentity(import.meta.url);
const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '../../..');
const hash = p => createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const write = (p, v) => fs.writeFileSync(p, JSON.stringify(v, null, 2) + '\n', { flag: 'wx' });

export function pose(bot, now) {
  const s = bot.world.snakes.get(bot.world.self);
  if (!s?.pts.length || bot.world.dead || bot.ws?.readyState !== 1) return null;
  return { id: s.id, life: bot.lives, ...s.pts.at(-1), count: s.sct, age: now - s.t };
}
export function pairStatus(bots, now) {
  const [a, b] = bots.map(bot => pose(bot, now));
  if (!a || !b) return { ready: false, reason: 'pair-unavailable' };
  const cached = bots[1].world.snakes.get(a.id);
  const positive = !!cached?.pts.length && cached.t >= bots[0].connectedAt;
  const separation = Math.hypot(a.x - b.x, a.y - b.y);
  const cacheAge = positive ? now - cached.t : null;
  const ready = positive && [a.age, b.age, cacheAge].every(t => t >= 0 && t <= 300) && separation <= 800;
  return { ready, reason: ready ? null : 'pair-stale-or-distant', actor: a, observer: b,
    positive, cacheAge, separation };
}

export class Acquisition {
  constructor(ordinal, emit) {
    this.order = ordinal % 2 ? [1, -1] : [-1, 1]; this.emit = emit;
    this.done = new Set(); this.attained = false; this.paired = false;
    this.life = null; this.countEvents = []; this.maxCounts = [0, 0];
    this.failures = {}; this.pending = null; this.selector = null;
    this.control = null;
  }
  interrupt(reason) {
    if (this.selector?.anchors.length) {
      this.failures[reason] = (this.failures[reason] ?? 0) + 1;
      this.emit({ event: 'turn-hold-interrupted', reason, count: this.target, sign: this.sign });
    }
    this.selector?.reset(); this.pending = null;
  }
  update(bots, now, remaining) {
    const pair = pairStatus(bots, now); this.paired ||= pair.ready;
    bots.forEach((b, i) => this.maxCounts[i] = Math.max(this.maxCounts[i], pose(b, now)?.count ?? 0));
    const actor = pair.actor ?? pose(bots[0], now);
    if (actor && actor.life !== this.life) {
      this.interrupt('new-life'); this.life = actor.life; this.passed = new Set();
      this.lastCount = null; this.lastCountAt = now;
    }
    if (actor && actor.count !== this.lastCount) {
      this.interrupt('count-change'); this.lastCount = actor.count; this.lastCountAt = now;
      this.emit({ event: 'turn-count', life: actor.life, id: actor.id, count: actor.count });
    }
    if (actor) for (const n of durations.keys()) if (actor.count > n) this.passed.add(n);
    this.attained ||= actor?.count === 37 || actor?.count === 100;
    const next = [...durations.keys()].flatMap(n => this.order.map(sign => [n, sign]))
      .find(([n, sign]) => !this.done.has(key(n, sign)) && !this.passed?.has(n) &&
        (n !== 174 || remaining >= (durations.get(n) + 4) * 1000));
    const [target, sign] = next ?? [null, null];
    if (target !== this.target || sign !== this.sign) {
      this.interrupt('cell-change'); this.target = target; this.sign = sign;
      this.selector = target ? new ArcSelector(target, sign) : null;
    }
    this.pair = pair;
    this.ready = !!actor && actor.count === target && pair.ready && now - this.lastCountAt >= 1000;
    if (!this.ready) this.interrupt('eligibility');
    if (this.pending && now - this.pending.at >= 1000 && this.ready) {
      this.done.add(key(target, sign));
      this.emit({ event: 'turn-cell-control-complete', count: target, sign, life: this.life,
        selection: this.pending.result });
      this.pending = null;
    }
    return pair;
  }
  anchor(p, now) {
    if (!this.ready || !this.selector) return;
    if (p.count !== this.target) { this.interrupt('count-change-on-receipt'); return; }
    const control = this.control, elapsed = now - (control?.t ?? -Infinity), pair = control?.pair;
    if (!control?.qualified || control.life !== this.life || control.target !== this.target ||
        elapsed < 0 || elapsed > 75 || !pair?.ready ||
        [pair.actor.age, pair.observer.age, pair.cacheAge].some(age => age + elapsed > 300)) {
      this.interrupt('stale-control-on-receipt'); return;
    }
    const result = this.selector.push({ ...p, qualified: true });
    if (result && !this.pending) this.pending = { at: now, result };
  }
}

export class TurnBot extends Bot {
  constructor(opts, index, recorder, state, bots, sockets, halt) {
    super(opts, index, recorder); Object.assign(this, { state, bots, sockets, halt });
    this.anchorIndex = 0;
  }
  connect(server) {
    if (this.stopped) return;
    if ([...this.sockets].some(ws => ws.readyState !== 3 && ws === this.ws) ||
        [...this.sockets].filter(ws => ws.readyState !== 3).length >= 2)
      throw Error('two-socket bound');
    super.connect(server); this.sockets.add(this.ws); this.anchorIndex = 0;
    // Tap after each unchanged World.packet application, preserving commands
    // within a batched frame instead of substituting the frame's final state.
    const apply = this.world.packet.bind(this.world);
    this.world.packet = (p, now) => {
      const result = apply(p, now);
      if (p[0] === 97 && p[23] !== 19) this.halt('non-19-setup');
      const me = this.world.snakes.get(this.world.self);
      if (!me?.pts.length) return result;
      const c = String.fromCharCode(p[0]);
      if (c === 's' || 'gnGN+=r'.includes(c)) this.rec.meta({ event: 'turn-receipt', life: this.lives,
        id: me.id, count: me.sct, serverClock: this.serverClock, dead: this.world.dead });
      const own = 'GN'.includes(c) || (c === '=' && p.length === 7) || (c === '+' && p.length === 10) ||
        ('gn+='.includes(c) && (p[1] << 8 | p[2]) === me.id);
      if (own && this.index === 0) {
        const h = me.pts.at(-1);
        this.state.anchor({ ...h, index: this.anchorIndex++, server: this.serverClock,
          count: me.sct, speed: me.sp, wanted: me.wang }, now);
      }
      return result;
    };
  }
  movement(now, me, choice) {
    let intendedHeading = null, intendedDanger = null, interruption = null;
    const a = this.state;
    if (this.index === 0 && a.ready) intendedHeading = me.ang + a.sign * Math.PI / 2;
    if (this.index === 1) {
      const actor = pose(this.bots[0], now);
      if (actor && actor.age <= 300) {
        const h = this.world.head(me, now), dx = actor.x - h.x, dy = actor.y - h.y;
        const distance = Math.hypot(dx, dy);
        intendedHeading = Math.atan2(dy, dx) + (distance < 450 ? Math.PI : distance <= 700 ? Math.PI / 2 : 0);
      }
    }
    if (intendedHeading !== null) {
      intendedDanger = headingDanger(this.world, now, me, this.world.head(me, now), intendedHeading,
        Math.min(6, 1 + (me.sct - 2) / 106));
      if (intendedDanger > .2) {
        interruption = 'avoidance';
        if (this.index === 0) a.interrupt(interruption);
      } else choice.a = intendedHeading;
    }
    this.movementIntent = { intendedHeading, intendedDanger, interruption, desiredBoost: false };
    const control = { event: 'turn-control', t: +now.toFixed(3), life: this.lives, id: me.id, count: me.sct,
      target: a.target, sign: a.sign, qualified: this.index === 0 && a.ready && !interruption,
      pair: a.pair, intendedHeading, intendedDanger, interruption };
    if (this.index === 0) a.control = control;
    this.rec.meta(control);
  }
}

export function verifySources() {
  const p = JSON.parse(fs.readFileSync(path.join(HERE, 'CAPTURE-PROTOCOL.json')));
  if (hash(path.join(REPO, 'tools/fidelity/slither-headless.mjs')) !== p.collectorSha256 ||
      hash(path.join(REPO, 'tools/fidelity/collision-delay-scout.mjs')) !== p.discoveryDecoderSha256 ||
      hash(path.join(HERE, 'PLAN.md')) !== p.planSha256) throw Error('frozen source drift');
  const frozen = JSON.parse(fs.readFileSync(path.join(HERE, 'SOURCE-FREEZE.json')));
  for (const [relative, digest] of Object.entries(frozen.sha256))
    if (hash(path.join(REPO, relative)) !== digest) throw Error('acquisition source drift');
}

export async function capture(root, signal) {
  verifySources();
  if (!fs.existsSync(path.join(root, 'guard-active.json'))) throw Error('exclusive guard required');
  const ledgerPath = path.join(root, 'invocations.jsonl');
  const previous = fs.existsSync(ledgerPath) ? fs.readFileSync(ledgerPath, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse) : [];
  const ordinal = previous.at(-1)?.ordinal;
  if (!Number.isInteger(ordinal) || ordinal < 1 || ordinal > 4 ||
      previous.some((r, i) => r.ordinal !== i + 1)) throw Error('guard reservation required');
  if (ordinal >= 3) {
    const gate = JSON.parse(fs.readFileSync(path.join(root, 'heldout-gate.json')));
    if (!gate.coveredAllPrimary || !gate.freezeCommit) throw Error('held-out gate closed');
  }
  const out = path.join(root, `invocation-${ordinal}`); fs.mkdirSync(out);
  let failure = null, reason = null, completed = false;
  const bots = [], recorders = [], sockets = new Set();
  const halt = why => { reason ??= why; };
  const state = new Acquisition(ordinal, m => bots.forEach(b => b.rec.meta(m)));
  const started = performance.now();
  try {
    const server = JSON.parse(fs.readFileSync(path.join(root, 'endpoint-private.json')));
    const privatePlan = path.join(root, 'plan-private.json');
    const opts = parseArgs(['--out', out, '--seconds', '1200', '--bots', '2', '--scenario', 'movement',
      '--nick', 'Turn study', '--origin', 'http://slither.io', '--plan', privatePlan]);
    const batch = createCaptureBatch(opts, 2, SOURCE);
    for (let i = 0; i < 2; i++) {
      const rec = new Recorder(path.join(out, `bot${i}.jsonl.gz`), { schema: 1, server,
        nick: opts.nick, scenario: 'turn-offset', dataOrigin: 'official-server',
        wall: Date.now(), t: performance.now(), captureProvenance: batch.provenance(i) });
      recorders.push(rec);
      bots.push(new TurnBot(opts, i, rec, state, bots, sockets, halt));
      rec.failure.then(error => { failure ??= error; halt('recorder-failure'); });
    }
    for (const b of bots) {
      if (reason || signal?.aborted) break;
      b.connect(server);
    }
    let lastStatus = 0, lastTick = started, maxTickGapMs = 0, ticks = 0;
    while (!reason && !signal?.aborted) {
      const now = performance.now(), elapsed = now - started;
      // Leave ten seconds for closing; the outer guard kills at 1200 seconds.
      if (elapsed >= 1190000) { halt('deadline-with-close-reserve'); break; }
      if (fs.existsSync(path.join(root, 'STOP'))) { halt('external-integrity-stop'); break; }
      maxTickGapMs = Math.max(maxTickGapMs, now - lastTick); lastTick = now; ticks++;
      state.update(bots, now, 1200000 - elapsed);
      if (!state.paired && elapsed >= 300000) halt('no-pair-by-300s');
      if (!state.attained && elapsed >= 600000) halt('no-primary-count-by-600s');
      if (bots.every(b => b.stopped)) halt('both-default-clients-failed');
      if (bots.some(b => b.rec.error)) halt('recorder-error');
      if (state.done.size >= 4 && [37, 100].every(n => [1, -1].every(s => state.done.has(key(n, s)))) &&
          1200000 - elapsed < 344000) halt('primary-complete-optional-cannot-fit');
      if (reason) break;
      for (const bot of bots) bot.tick(now);
      if (now - lastStatus >= 10000) {
        verifySources();
        lastStatus = now;
        const status = { ordinal, elapsedSeconds: +(elapsed / 1000).toFixed(1),
          lives: bots.map(b => b.lives), joins: bots.map(b => b.joins), frames: bots.map(b => b.frames),
          maxCounts: state.maxCounts, paired: state.pair.ready, done: [...state.done], ticks, maxTickGapMs };
        console.log(JSON.stringify(status)); bots.forEach(b => b.rec.meta({ event: 'turn-status', ...status }));
      }
      await pause(Math.max(0, 25 - (performance.now() - now)));
    }
    completed = !failure && !signal?.aborted && !['non-19-setup', 'external-integrity-stop'].includes(reason);
  } catch (e) { failure = e; reason ??= 'exception'; }
  finally {
    const elapsedSeconds = (performance.now() - started) / 1000;
    try {
      await finalizeCaptureBatch({ bots, recorders, source: SOURCE, completed, signal, error: failure,
        metadata: { ordinal, stopReason: reason, elapsedSeconds, controlsDone: [...state.done] } });
    } catch (e) { failure ??= e; }
    const closeStart = performance.now();
    while ([...sockets].some(ws => ws.readyState !== 3) && performance.now() - closeStart < 9000) await pause(50);
    const closed = [...sockets].every(ws => ws.readyState === 3);
    const result = { ordinal, completed: completed && !failure, reason, elapsedSeconds,
      failure: failure ? String(failure) : null, socketsClosed: closed, maxCounts: state.maxCounts,
      controlsDone: [...state.done], interruptionCounts: state.failures,
      archives: recorders.map((r, i) => ({ member: i, sha256: hash(path.join(out, `bot${i}.jsonl.gz`)) })) };
    write(path.join(out, 'result-private.json'), result);
    if (!closed || failure || ['non-19-setup', 'external-integrity-stop'].includes(reason))
      write(path.join(root, 'STUDY-STOP.json'), { ordinal, reason, failure: String(failure), closed });
    console.log(JSON.stringify({ ordinal, reason, elapsedSeconds, socketsClosed: closed, maxCounts: state.maxCounts }));
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  runCollectorCli(signal => capture(process.argv[2], signal));
