// Acquisition controls, not official-server laws. Frozen method and limitations:
// research/fidelity/2026-10-26-collision-pilot/plan.json and REPORT.md.
import fs from 'node:fs';
import path from 'node:path';
import { isIP } from 'node:net';
import { Bot, Recorder, headingDanger, splitFrame, splitPayload, captureSourceIdentity, createCaptureBatch,
  finalizeCaptureBatch, runCollectorCli, validateOrigin } from './slither-headless.mjs';

const SOURCE = captureSourceIdentity(import.meta.url);
export const C = Object.freeze({ tickMs: 25, freshMs: 350, gapMs: 100,
  agreement: 25, guard: 600, rim: 900, matchSpeed: .05, heading: .16,
  stageMs: 45000, runMs: 12000, holdMs: 1500, observeMs: 1000,
  maxPopulation: 100, factors: Object.freeze([1.3, 1.1, .9, .7]) });
const angleError = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
const distance = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
export const radius = n => 14.5 * Math.min(6, Math.max(1, 1 + (n - 2) / 106));

export function trialSpec(ordinal) {
  const slot = ordinal % 12;
  if (slot < 8) return { kind: 'ladder', probe: Math.floor(slot / 4),
    first: Math.floor(slot / 4), factor: C.factors[slot % 4], sign: ordinal % 2 ? -1 : 1 };
  return { kind: 'head-on', probe: (slot - 8) % 2, first: Math.floor((slot - 8) / 2),
    factor: 0, sign: 1 };
}

export function segmentDistance(p, a, b) {
  const dx = b.x - a.x, dy = b.y - a.y, d = dx * dx + dy * dy;
  const t = d ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / d)) : 0;
  return Math.hypot(p.x - a.x - t * dx, p.y - a.y - t * dy);
}

export function snapshot(bot, peer, now) {
  const me = bot.world.snakes.get(bot.world.self);
  if (!me?.pts.length || bot.world.dead || bot.ws?.readyState !== 1) return null;
  const cached = bot.world.snakes.get(peer.world.self);
  return { id: me.id, life: bot.lives, ...bot.world.head(me, now),
    anchor: { ...me.pts.at(-1) }, age: now - me.t, heading: me.ang,
    speed: me.sp / .032, segments: me.sct, fam: me.fam, boost: bot.boost,
    body: me.pts.map(p => ({ ...p })), peerAge: cached ? now - cached.t : null,
    peerHead: cached?.pts.length ? bot.world.head(cached, now) : null,
    population: bot.population ?? null, populationAt: bot.populationAt ?? null };
}

// Check complete body edges, not just their endpoints. Coverage is still only
// what these two clients receive; an unseen third party cannot be ruled out.
export function thirdParty(bots, poses) {
  const ids = new Set(bots.map(b => b.world.self));
  for (const bot of bots) for (const s of bot.world.snakes.values()) {
    if (ids.has(s.id)) continue;
    const points = [...s.pts, bot.world.head(s, performance.now())];
    for (let i = 1; i < points.length; i++) for (const p of poses.filter(Boolean)) {
      if (segmentDistance(p, points[i - 1], points[i]) < C.guard) return s.id;
    }
  }
  return null;
}

export function quality(poses, initial, gap = C.tickMs) {
  if (gap > C.gapMs) return 'control_gap';
  if (poses.some(p => !p)) return 'pair_unavailable';
  for (const [i, p] of poses.entries()) {
    if (![p.x, p.y, p.heading, p.speed, p.age, p.peerAge, p.fam].every(Number.isFinite) ||
        !Number.isInteger(p.segments) || p.segments < 2 || p.segments > 80 ||
        p.age < 0 || p.age > C.freshMs || p.peerAge < 0 || p.peerAge > C.freshMs || !p.peerHead)
      return 'stale_or_unknown_pose';
    if (distance(p.peerHead, poses[1 - i]) > C.agreement) return 'stream_disagreement';
    if (p.boost || p.speed <= 0) return 'boost_or_speed';
    if (initial && (p.id !== initial[i].id || p.life !== initial[i].life ||
        p.segments !== initial[i].segments || Math.abs(p.fam - initial[i].fam) > 1e-6))
      return 'life_or_size_changed';
  }
  if (Math.abs(poses[0].speed / poses[1].speed - 1) > C.matchSpeed ||
      Math.abs(radius(poses[0].segments) / radius(poses[1].segments) - 1) > .05)
    return 'unmatched_speed_or_size';
  return null;
}

export class Trial {
  constructor(ordinal, emit = () => {}) {
    this.ordinal = ordinal; this.spec = trialSpec(ordinal); this.emit = emit;
    this.phase = 'rendezvous'; this.started = null; this.samples = 0;
    this.reasons = new Set(); this.minOffset = Infinity;
  }
  event(event, fields = {}) {
    this.emit({ event: `collision-${event}`, trial: this.ordinal, spec: this.spec,
      phase: this.phase, ...fields });
  }
  end(now, reason, deaths = [false, false]) {
    if (this.phase === 'done') return;
    if (reason) this.reasons.add(reason);
    this.phase = 'done';
    this.result = { trial: this.ordinal, spec: this.spec, qualified: !!this.initial &&
      this.reasons.size === 0 && this.samples >= 4, reasons: [...this.reasons],
      deaths, outcome: deaths.some(Boolean) ? 'contact' : reason ? 'censored' : 'survival',
      samples: this.samples, targetOffset: this.offset ?? null,
      achievedMinOffset: Number.isFinite(this.minOffset) ? this.minOffset : null,
      initial: this.initial ?? null, endedAt: now };
    this.event('end', this.result);
  }
  update(now, poses, { gap = C.tickMs, intruder = null, deaths = [false, false] } = {}) {
    if (this.phase === 'done') return;
    this.started ??= now;
    if (intruder !== null) { this.end(now, 'third_party', deaths); return; }
    if (now - this.started > C.stageMs && !this.initial) {
      this.end(now, 'staging_timeout', deaths); return;
    }
    // Death packets often remove a body before game-over reaches its own
    // client. Do not retrospectively require a live pose after the event.
    if (deaths.some(Boolean)) {
      if (!this.initial || now - this.lastGood > C.gapMs) this.reasons.add('unqualified_death');
      this.end(now, null, deaths); return;
    }
    const bad = quality(poses, this.initial, gap);
    if (bad) {
      // Removal can precede the own game-over in the same event. Wait only
      // within the frozen event bound; missing state never yields survival.
      const pendingDeath = poses.some(p => !p || !p.peerHead);
      if (this.initial && !(pendingDeath && now - this.lastGood <= C.gapMs)) this.end(now, bad);
      return;
    }
    const [a, b] = poses;
    this.site ??= this.spawnSite ?? { x: poses[this.spec.first].x, y: poses[this.spec.first].y };
    const reference = poses[1 - this.spec.probe], probe = poses[this.spec.probe];
    if (this.phase === 'rendezvous' && distance(a, b) < 800) {
      this.phase = 'stage'; this.stageAt = now;
      this.offset = this.spec.sign * this.spec.factor * .975 * radius(reference.segments);
      this.heading = Math.round(reference.heading / (2 * Math.PI) * 251) / 251 * 2 * Math.PI;
      if (this.spec.kind === 'ladder') {
        // Use the already occupied side. Crossing the reference body merely
        // to alternate signs would contaminate the staging phase with deaths.
        this.spec.sign = Math.sign(this.project(probe).cross - this.project(reference).cross) || 1;
        this.offset = this.spec.sign * this.spec.factor * .975 * radius(reference.segments);
      }
      this.event('stage', { site: this.site, heading: this.heading, targetOffset: this.offset });
    }
    if (this.phase === 'stage') {
      const g = this.geometry(poses);
      const ready = this.spec.kind === 'ladder'
        ? g.lag > 28 && g.lag < Math.min(100, g.bodyExtent - 8) &&
          Math.abs(g.cross - this.spec.sign * (Math.abs(this.offset) + 120)) < 10 &&
          poses.every(p => Math.abs(angleError(p.heading, this.heading)) < .12)
        : poses.every((p, i) => distance(p, this.startPoint(i)) < 160);
      if (ready) { this.phase = 'align'; this.event('align'); }
    }
    if (this.phase === 'align') {
      const g = this.geometry(poses);
      const headings = poses.every((p, i) => Math.abs(angleError(p.heading,
        this.desiredHeading(i))) < (this.spec.kind === 'ladder' ? C.heading : .06));
      const ready = this.spec.kind === 'ladder' ? headings && g.lag > 28 &&
        g.lag < g.bodyExtent - 8 : headings && this.headSeparation(poses) > 180 &&
        poses.every((p, i) => Math.abs(this.project(p).cross) < 2);
      if (ready) {
        this.phase = 'run'; this.initial = structuredClone(poses); this.runAt = now;
        this.event('launch', { initial: this.initial, targetOffset: this.offset });
      } else if (this.spec.kind === 'head-on' && this.headSeparation(poses) < 180) {
        this.end(now, 'alignment_failed'); return;
      }
    }
    if (this.phase === 'run') {
      const g = this.geometry(poses);
      const headingOK = poses.every((p, i) => Math.abs(angleError(p.heading,
        this.desiredHeading(i))) <= C.heading);
      if (!headingOK) { this.end(now, 'heading_changed'); return; }
      if (this.spec.kind === 'ladder' && (g.lag < 28 || g.lag > g.bodyExtent - 8)) {
        this.end(now, 'body_overlap_lost'); return;
      }
      this.samples++; this.lastGood = now;
      this.minOffset = Math.min(this.minOffset, Math.abs(g.cross));
      this.event('sample', { poses, geometry: g, targetOffset: this.offset, gapMs: gap });
      const atTarget = this.spec.kind === 'ladder' ? Math.abs(g.cross - this.offset) < .6 :
        this.headSeparation(poses) < -140;
      if (atTarget) this.targetAt ??= now;
      else this.targetAt = null;
      if (this.targetAt !== null && this.targetAt !== undefined && now - this.targetAt >= C.holdMs)
        this.end(now, null);
      else if (now - this.runAt > C.runMs) this.end(now, 'run_timeout');
    }
  }
  project(p) {
    const dx = p.x - this.site.x, dy = p.y - this.site.y;
    return { along: dx * Math.cos(this.heading) + dy * Math.sin(this.heading),
      cross: -dx * Math.sin(this.heading) + dy * Math.cos(this.heading) };
  }
  point(along, cross) {
    return { x: this.site.x + along * Math.cos(this.heading) - cross * Math.sin(this.heading),
      y: this.site.y + along * Math.sin(this.heading) + cross * Math.cos(this.heading) };
  }
  geometry(poses) {
    const r = poses[1 - this.spec.probe], p = poses[this.spec.probe];
    const rp = this.project(r), pp = this.project(p);
    return { lag: rp.along - pp.along, cross: pp.cross - rp.cross,
      bodyExtent: Math.max(...r.body.map(p => distance(p, r))),
      referenceRadius: radius(r.segments) };
  }
  desiredHeading(i) { return this.heading + (this.spec.kind === 'head-on' && i !== this.spec.probe ? Math.PI : 0); }
  startPoint(i) { return this.point(i === this.spec.probe ? -500 : 500, 0); }
  headSeparation(poses) { return this.project(poses[1 - this.spec.probe]).along - this.project(poses[this.spec.probe]).along; }
  control(index, poses) {
    const p = poses[index];
    if (!p) return null;
    const other = poses[1 - index];
    let target, boost = false;
    if (!this.site || this.phase === 'rendezvous') {
      if (!other) return { heading: p.heading + .6, boost: false };
      const first = this.spec.first === index;
      target = first ? { x: p.x + Math.cos(p.heading + 1.2) * 100,
        y: p.y + Math.sin(p.heading + 1.2) * 100 } : other;
      boost = !first && distance(p, other) > 900 && p.segments >= 5;
    } else if (this.spec.kind === 'ladder') {
      if (index !== this.spec.probe) {
        const g = this.geometry(poses);
        const catchingUp = this.phase === 'stage' && g.lag > 75;
        return { heading: this.heading + (catchingUp ?
          (Math.floor((this.lastControlAt ?? 0) / 800) % 2 ? .65 : -.65) : 0), boost: false };
      }
      const ref = poses[1 - this.spec.probe];
      if (!ref) return null;
      const g = this.geometry(poses), rp = this.project(ref), pp = this.project(p);
      const lag = Math.min(55, Math.max(35, g.bodyExtent * .55));
      const cross = this.phase === 'stage' ? this.spec.sign * (Math.abs(this.offset) + 120) : this.offset;
      if (this.phase === 'stage') {
        target = this.point(rp.along - lag, rp.cross + cross);
        // Catch up outside the contact band, then release boost before launch.
        boost = g.lag > lag + 20 && Math.abs(g.cross) > radius(ref.segments) * 2 && p.segments >= 5;
      } else {
        // A shallow lateral approach keeps speed and heading matched; continue
        // parallel once the requested clearance is achieved.
        const error = cross - g.cross;
        return { heading: this.heading + (Math.abs(error) < .4 ? 0 :
          Math.sign(error) * Math.max(.025, Math.min(.12, Math.abs(error) / 80))), boost: false };
      }
      if (distance(p, target) < 25) target = this.point(pp.along + 100, rp.cross + cross);
    } else if (this.phase === 'stage') {
      target = this.startPoint(index);
      if (distance(p, target) < 100) return { heading: p.heading + 1.1, boost: false };
    } else {
      const cross = this.project(p).cross, direction = index === this.spec.probe ? 1 : -1;
      return { heading: this.desiredHeading(index) + Math.max(-.7, Math.min(.7, -direction * cross / 50)), boost: false };
    }
    return { heading: Math.atan2(target.y - p.y, target.x - p.x), boost };
  }
}

export class CollisionBot extends Bot {
  connect(server) {
    this.stopped = false; this.population = null; this.populationAt = null;
    super.connect(server);
    // The coordinator owns joins. Never reconnect one life independently or
    // replace a socket until BOTH close handshakes have completed.
    this.ws.onclose = e => {
      this.finishDeathObservation('socket-close');
      this.rec.meta({ event: 'close', code: e.code, life: this.lives });
      this.ws = null;
    };
  }
  receive(a) {
    super.receive(a);
    for (const p of this.rawFraming ? splitPayload(a) : splitFrame(a)) {
      if (p[0] === 108 && p.length >= 6) {
        this.population = p[4] * 256 + p[5]; this.populationAt = performance.now();
        this.rec.meta({ event: 'collision-population', population: this.population, life: this.lives });
      }
    }
  }
  drive(control, now) {
    if (this.world.dead || this.ws?.readyState !== 1) return;
    this.ping(now);
    if (!control) return;
    const encoded = Math.round(251 * ((control.heading % (2 * Math.PI) + 2 * Math.PI) % (2 * Math.PI)) / (2 * Math.PI)) % 251;
    if (encoded !== this.lastHeading) {
      const sentAt = this.send([encoded]);
      if (sentAt !== undefined) this.lastHeading = encoded;
    }
    this.setBoost(control.boost, now);
    this.rec.meta({ event: 'collision-control', life: this.lives, encodedHeading: this.lastHeading,
      heldBoost: this.boost, ...control });
  }
}

export function stageSteering(bot, control, now) {
  if (!control) return null;
  const me = bot.world.snakes.get(bot.world.self);
  if (!me) return control;
  const head = bot.world.head(me, now), sc = radius(me.sct) / 14.5;
  const risk = a => headingDanger(bot.world, now, me, head, a, sc);
  if (risk(control.heading) <= .2) return control;
  const candidates = Array.from({ length: 64 }, (_, i) => {
    const heading = i * Math.PI * 2 / 64;
    return { heading, risk: risk(heading), score: Math.cos(heading - control.heading) -
      .05 * Math.abs(angleError(heading, me.ang)) };
  });
  candidates.sort((a, b) => (a.risk > .2) - (b.risk > .2) ||
    (a.risk > .2 ? a.risk - b.risk : b.score - a.score));
  return { heading: candidates[0].heading, boost: false, interruption: 'staging_avoidance' };
}

export function validateOptions(opts) {
  const { out, server, seconds = 600, allowOutside = false, plan } = opts;
  const [ip, portText, extra] = (server ?? '').split(':'), port = Number(portText);
  if (!out || isIP(ip) !== 4 || extra !== undefined || !Number.isInteger(port) || port < 1 || port > 65535 ||
      !Number.isFinite(seconds) || seconds < 1 || seconds > 600) throw Error('use --out, IPv4:port, and 1..600 seconds');
  const local = ip.startsWith('127.');
  if (opts.localCycle && !local) throw Error('localCycle is loopback-only');
  const dataOrigin = plan ? JSON.parse(fs.readFileSync(plan)).dataOrigin : null;
  if (local && dataOrigin && dataOrigin !== 'local-simulator') throw Error('loopback requires local-simulator provenance');
  if (!local && !(allowOutside && dataOrigin === 'official-server'))
    throw Error('outside connections require --allow-outside yes and an official-server plan');
  return { ...opts, seconds, ip, port, local, origin: validateOrigin(opts.origin) };
}

export async function recordPair(opts, signal) {
  if (signal?.aborted) return [];
  const { out, server, seconds, ip, port, local, origin } = validateOptions(opts);
  if (fs.existsSync(out)) throw Error(`refusing to overwrite ${out}`);
  const batch = createCaptureBatch({ scenario: 'paired-collision', server, seconds,
    ...(opts.localCycle ? { localCycle: true } : {}), plan: opts.plan, origin }, 2, SOURCE);
  const bots = [], recorders = [], results = [];
  let completed = false, failure, trial, timer, deadline, abort;
  let ordinal = 0, state = 'join-first', observedUntil = null, lastTick = performance.now(), stagingFailures = 0;
  const emit = row => recorders.forEach(r => r.meta(row));
  const connect = i => bots[i].connect({ ip, port });
  try {
    fs.mkdirSync(out, { recursive: true });
    for (const index of [0, 1]) {
      const nick = `Collision ${index}`;
      const rec = new Recorder(path.join(out, `bot${index}.jsonl.gz`), {
        schema: 1, server: { ip, port }, nick, scenario: 'paired-collision', botIndex: index,
        dataOrigin: local ? 'local-simulator' : batch.dataOrigin,
        wall: Date.now(), t: performance.now(), captureProvenance: batch.provenance(index) });
      recorders.push(rec); bots.push(new CollisionBot({ nick, serverBody: true, origin }, index, rec));
    }
    trial = new Trial(ordinal, emit); connect(trial.spec.first);
    let joinAt = performance.now();
    await new Promise((resolve, reject) => {
      abort = resolve; signal?.addEventListener('abort', abort, { once: true });
      recorders.forEach(r => r.failure.then(reject));
      deadline = setTimeout(resolve, seconds * 1000);
      timer = setInterval(() => {
        try {
          const now = performance.now(), gap = now - lastTick; lastTick = now;
          const poses = bots.map((b, i) => snapshot(b, bots[1 - i], now));
          if (state === 'join-first') {
            const first = poses[trial.spec.first];
            if (first) { trial.spawnSite = { x: first.x, y: first.y };
              emit({ event: 'collision-join-order', trial: ordinal, first: trial.spec.first,
              firstId: first.id, firstLife: first.life }); connect(1 - trial.spec.first); state = 'join-second'; }
          } else if (state === 'join-second' && poses.every(Boolean)) state = 'trial';
          if (state.startsWith('join') && now - joinAt > C.stageMs) throw Error('join timeout');
          if (state === 'trial') {
            const populationOK = local || poses.every(p => p?.population !== null && p?.population !== undefined);
            if (!local && poses.some(p => p?.population > C.maxPopulation)) throw Error('population gate: choose a quieter server');
            const intruder = thirdParty(bots, poses);
            if (populationOK) trial.update(now, poses, { gap, intruder,
              deaths: bots.map(b => b.world.dead) });
            if (poses.some((p, i) => p && bots[i].world.radius -
                Math.hypot(p.x - bots[i].world.grd, p.y - bots[i].world.grd) < C.rim))
              trial.end(now, 'arena_margin');
            if (now - joinAt > C.stageMs + C.runMs) trial.end(now, 'pair_timeout');
            if (trial.phase === 'done') {
              results.push(trial.result); state = 'observe'; observedUntil = now + C.observeMs;
              stagingFailures = trial.result.reasons.includes('staging_timeout') ||
                trial.result.reasons.includes('pair_timeout') ? stagingFailures + 1 : 0;
            }
          }
          if (state === 'observe' && now >= observedUntil) {
            trial.result.observedDeaths = bots.map(b => b.world.dead);
            if (trial.result.outcome === 'contact') trial.result.deaths = trial.result.observedDeaths;
            trial.event('result', trial.result);
            for (const b of bots) b.ws?.close(); state = 'closing';
          }
          if (state === 'closing' && bots.every(b => !b.ws)) {
            if (!local && stagingFailures >= 3) throw Error('pilot stop: three consecutive staging timeouts');
            if (opts.localCycle && ordinal === 11) { resolve(); return; }
            trial = new Trial(++ordinal, emit); state = 'join-first'; joinAt = now;
            connect(trial.spec.first);
          }
          if (!['observe', 'closing'].includes(state)) {
            trial.lastControlAt = now;
            bots.forEach((b, i) => {
              const control = trial.control(i, poses);
              b.drive(trial.initial || trial.phase === 'align' ? control : stageSteering(b, control, now), now);
            });
            if (now - (trial.lastStateAt ?? -Infinity) >= 200) {
              trial.lastStateAt = now;
              trial.event('state', { poses, gapMs: gap,
                geometry: trial.heading !== undefined && poses.every(Boolean) ? trial.geometry(poses) : null });
            }
          }
        } catch (error) { reject(error); }
      }, C.tickMs);
      if (signal?.aborted) resolve();
    });
    if (trial.phase !== 'done') { trial.end(performance.now(), 'capture_deadline'); results.push(trial.result); }
    if (bots.some(b => b.joins === 0)) throw Error('a client never joined');
    completed = !signal?.aborted;
  } catch (error) { failure = error; }
  finally {
    clearInterval(timer); clearTimeout(deadline); signal?.removeEventListener('abort', abort);
    await finalizeCaptureBatch({ bots, recorders, source: SOURCE, completed, signal, error: failure,
      metadata: { results, requestedSeconds: seconds } });
  }
  return results;
}

export function parseArgs(argv) {
  const allowed = new Set(['--out', '--server', '--seconds', '--plan', '--origin', '--allow-outside']);
  const values = {};
  for (let i = 0; i < argv.length; i += 2) {
    if (!allowed.has(argv[i]) || !argv[i + 1] || argv[i + 1].startsWith('--') || argv[i] in values)
      throw Error(`invalid or repeated option: ${argv[i]}`);
    values[argv[i]] = argv[i + 1];
  }
  if (values['--allow-outside'] && values['--allow-outside'] !== 'yes') throw Error('use --allow-outside yes');
  return { out: values['--out'], server: values['--server'], seconds: Number(values['--seconds'] ?? 600),
    plan: values['--plan'], origin: values['--origin'], allowOutside: values['--allow-outside'] === 'yes' };
}
if (process.argv[1]?.endsWith('slither-paired-collision.mjs'))
  runCollectorCli(signal => recordPair(parseArgs(process.argv.slice(2)), signal));
