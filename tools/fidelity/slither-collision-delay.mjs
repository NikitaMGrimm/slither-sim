// Experimental collector only. Neither the simulator nor host rewinds state.
import fs from 'node:fs';
import path from 'node:path';
import { World, Recorder, splitFrame, splitPayload, captureSourceIdentity,
  createCaptureBatch, finalizeCaptureBatch, parseServer } from './slither-headless.mjs';
import { CollisionBot, segmentDistance, stageSteering } from './slither-paired-collision.mjs';

const SOURCE = captureSourceIdentity(import.meta.url);
const TAU = 2 * Math.PI;
const error = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
const distance = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

export class DelayBot extends CollisionBot {
  constructor(opts, index, recorder) {
    super(opts, index, recorder);
    this.delayMs = opts.delayMs ?? 0;
    this.incoming = []; this.outgoing = [];
    this.tap = new World({ serverBody: true });
    this.tapFrames = 0; this.tapRaw = false; this.wireFrames = 0;
  }
  connect(server) {
    this.incoming = []; this.outgoing = [];
    this.tap = new World({ serverBody: true }); this.tapFrames = 0; this.tapRaw = false;
    super.connect(server);
  }
  receive(bytes) {
    const now = performance.now();
    // Standard raw frame timestamps remain actual socket ingress/egress.
    this.rec.frame('i', bytes, now);
    this.frames = ++this.wireFrames;
    if (++this.tapFrames === 1 && bytes[0] === 54) this.tapRaw = true;
    for (const packet of this.tapRaw ? splitPayload(bytes) : splitFrame(bytes)) this.tap.packet(packet, now);
    this.incoming.push({ bytes: bytes.slice(), due: now + this.delayMs, at: now, ordinal: this.wireFrames });
  }
  send(bytes) {
    if (this.ws?.readyState !== 1 || this.deathObservation && !(bytes.length === 1 && bytes[0] === 251)) return;
    const now = performance.now();
    this.outgoing.push({ bytes: Uint8Array.from(bytes), due: now + this.delayMs, at: now });
    return now;
  }
  flush(now) {
    while (this.incoming[0]?.due <= now) {
      const item = this.incoming.shift(), frame = this.rec.frame;
      // Bot.receive performs ordinary decoding and records metadata; its raw
      // input was already recorded at ingress, before the artificial delay.
      this.rec.frame = (direction, ...args) => direction === 'i' ? undefined : frame.call(this.rec, direction, ...args);
      this.frames = item.ordinal - 1;
      try { super.receive(item.bytes); } finally { this.rec.frame = frame; this.frames = this.wireFrames; }
      this.rec.meta({ event: 'delay-inbound-applied', ingressAt: item.at, requestedMs: this.delayMs,
        appliedMs: now - item.at });
    }
    while (this.outgoing[0]?.due <= now) {
      const item = this.outgoing.shift();
      if (this.ws?.readyState !== 1) continue;
      const sentAt = performance.now();
      this.ws.send(item.bytes); this.rec.frame('o', item.bytes, sentAt);
      this.rec.meta({ event: 'delay-outbound-sent', requestedAt: item.at, requestedMs: this.delayMs,
        appliedMs: sentAt - item.at, bytes: Buffer.from(item.bytes).toString('base64') });
    }
  }
}

function pose(bot, now, tap = false) {
  const world = tap ? bot.tap : bot.world, snake = world.snakes.get(world.self);
  if (!snake?.pts.length || world.dead || bot.ws?.readyState !== 1) return null;
  return { ...world.head(snake, now), heading: snake.ang, speed: snake.sp / .032,
    body: snake.pts.map(p => ({ ...p })), age: now - snake.t, segments: snake.sct,
    fam: snake.fam, id: snake.id, life: bot.lives };
}

export function intruder(bots, poses) {
  const ids = new Set(bots.map(bot => bot.tap.self));
  for (const bot of bots) for (const snake of bot.tap.snakes.values()) {
    if (ids.has(snake.id)) continue;
    const points = [...snake.pts, bot.tap.head(snake, performance.now())];
    for (const p of poses.filter(Boolean)) {
      if (points.some(q => distance(p, q) < 600)) return snake.id;
      for (let k = 1; k < points.length; k++) if (segmentDistance(p, points[k - 1], points[k]) < 600) return snake.id;
    }
  }
  return null;
}

// A tandem approach stages the probe ahead and to the side, then turns it
// perpendicular behind the moving reference tail. Staging survives intruders.
export class TailTrial {
  constructor(spec, emit) {
    this.spec = spec; this.emit = emit; this.phase = 'rendezvous'; this.samples = [];
    this.interruptions = []; this.started = performance.now(); this.lastState = 0;
  }
  project(p) {
    return { x: p.x * Math.cos(this.heading) + p.y * Math.sin(this.heading),
      y: -p.x * Math.sin(this.heading) + p.y * Math.cos(this.heading) };
  }
  update(now, poses, wire, bots, gap) {
    const [ref, probe] = poses, guarded = intruder(bots, wire);
    if (now - this.started > 90000 && this.phase !== 'run') return this.end('staging_timeout');
    if (bots.some(b => b.tap.dead)) {
      if (this.phase !== 'run') return this.end('staging_death');
      return this.end(null, bots.map(b => b.tap.dead));
    }
    if (!ref || !probe) return null;
    const rim = wire.some((p, i) => p && bots[i].tap.radius - Math.hypot(p.x - bots[i].tap.grd, p.y - bots[i].tap.grd) < 1200);
    if (guarded !== null || rim) {
      if (this.phase === 'run') return this.end(guarded !== null ? 'third_party' : 'arena_margin');
      if (now - (this.interruptedAt ?? -Infinity) > 1000) {
        this.interruptedAt = now;
        this.interruptions.push({ at: now, reason: guarded !== null ? 'third_party' : 'arena_margin' });
        this.emit({ event: 'tail-interruption', ...this.spec, ...this.interruptions.at(-1) });
      }
      this.phase = 'rendezvous'; this.heading = null;
      return null;
    }
    if (this.phase === 'rendezvous' && distance(ref, probe) < 900) {
      const w = bots[0].world;
      this.heading = Math.round(Math.atan2(w.grd - ref.y, w.grd - ref.x) / TAU * 251) / 251 * TAU;
      this.phase = 'stage';
    }
    if (this.heading == null) return null;
    const r = this.project(ref), p = this.project(probe);
    const tail = this.project(ref.body[0]);
    const extent = r.x - tail.x;
    if (Math.abs(error(ref.heading, this.heading)) < .08) this.straightSince ??= now;
    else this.straightSince = null;
    if (now - (this.straightSince ?? now) >= 2000) this.stageExtent ??= extent;
    const desiredLead = this.stageExtent == null ? 0 : 350 - this.stageExtent + this.spec.offset;
    if (this.phase === 'stage' && this.stageExtent != null && Math.abs(p.x - r.x - desiredLead) < 35 &&
        Math.abs(p.y - r.y + 350) < 12 && poses.every(s => Math.abs(error(s.heading, this.heading)) < .08)) {
      this.phase = 'run'; this.runAt = now;
      this.initial = structuredClone(wire);
      this.emit({ event: 'tail-launch', ...this.spec, poses: wire, controllerPoses: poses, heading: this.heading });
    }
    if (this.phase === 'run') {
      if (gap > 100) return this.end('control_gap');
      if (wire.some((s, i) => !s || s.age > 350 || s.segments !== this.initial[i].segments ||
          Math.abs(s.fam - this.initial[i].fam) > 1e-6)) return this.end('stale_or_size_changed');
      this.samples.push({ at: now, poses: wire });
      if (p.y - r.y > 220) return this.end(null, [false, false]);
      if (now - this.runAt > 8000) return this.end('run_timeout');
    }
    if (now - this.lastState >= 100) {
      this.lastState = now;
      this.emit({ event: 'tail-state', ...this.spec, phase: this.phase, poses: wire,
        controllerPoses: poses, heading: this.heading, desiredLead, extent, gap });
    }
    return null;
  }
  end(reason, deaths = [false, false]) {
    const result = { ...this.spec, reason, phase: this.phase, deaths,
      qualified: !reason && this.phase === 'run' && this.samples.length >= 4,
      outcome: reason ? 'excluded' : deaths[1] ? 'contact' : deaths[0] ? 'reference_death' : 'survival',
      samples: this.samples.length, interruptions: this.interruptions };
    this.phase = 'done'; this.emit({ event: 'tail-result', ...result }); return result;
  }
  control(index, poses, bots) {
    const me = poses[index], other = poses[1 - index];
    if (!me) return null;
    const world = bots[index].world;
    if (this.heading == null || this.phase === 'rendezvous') {
      const target = index === 0 || !other ? { x: world.grd, y: world.grd } : other;
      const heading = distance(me, target) < 250 ? me.heading + .8 : Math.atan2(target.y - me.y, target.x - me.x);
      return { heading, boost: index === 1 && me.segments >= 5 && distance(me, target) > 1000 };
    }
    if (this.phase === 'run') return { heading: this.heading + (index === 1 ? Math.PI / 2 : 0), boost: false };
    if (!other) return null;
    const ref = poses[0], probe = poses[1], r = this.project(ref), p = this.project(probe);
    const lead = this.stageExtent == null ? 0 : 350 - this.stageExtent + this.spec.offset;
    if (index === 0) {
      const behind = this.stageExtent != null && p.x - r.x < lead - 20;
      return { heading: this.heading + (behind ? (Math.floor(performance.now() / 700) % 2 ? .7 : -.7) : 0), boost: false };
    }
    const ey = r.y - 350 - p.y, ex = r.x + lead - p.x;
    const correction = Math.max(-.5, Math.min(.5, ey / 120));
    return { heading: this.heading + correction + (ex < -25 ?
      (Math.floor(performance.now() / 700) % 2 ? .7 : -.7) : 0), boost: ex > 25 && me.segments >= 5 };
  }
}

export async function recordTail({ out, server, plan, specs, delayMs = 150, seconds = 600,
  connectGapMs = 8000 }) {
  const { ip, port } = parseServer(server);
  const batch = createCaptureBatch({ scenario: 'collision-delay', server, plan, seconds, delayMs }, 2, SOURCE);
  if (fs.existsSync(out)) throw Error('output exists');
  fs.mkdirSync(out, { recursive: true });
  const recorders = [0, 1].map(i => new Recorder(path.join(out, `bot${i}.jsonl.gz`), {
    schema: 1, scenario: 'collision-delay', server: { ip, port }, botIndex: i, t: performance.now(), wall: Date.now(),
    dataOrigin: batch.dataOrigin, captureProvenance: batch.provenance(i) }));
  const emit = row => recorders.forEach(r => r.meta(row));
  const bots = recorders.map((rec, i) => new DelayBot({ nick: `Delay study ${i}`, serverBody: true,
    delayMs: 0 }, i, rec));
  const results = [], started = performance.now();
  let lastConnect = -Infinity, failure, trial;
  const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
  try {
    for (const spec of specs) {
      bots[1].delayMs = spec.arm === 'delay' ? delayMs : 0;
      for (const bot of bots) {
        while (performance.now() - lastConnect < connectGapMs) await wait(25);
        lastConnect = performance.now(); bot.connect({ ip, port });
        while (!pose(bot, performance.now())) {
          const now = performance.now(); bots.forEach(b => { b.flush(now); b.drive(null, now); });
          if (!bot.ws || now - lastConnect > 15000) throw Error('early_close_or_join_timeout');
          await wait(10);
        }
      }
      trial = new TailTrial(spec, emit);
      let lastTick = performance.now(), result;
      while (performance.now() - started < seconds * 1000) {
        const now = performance.now(), gap = now - lastTick; lastTick = now;
        bots.forEach(b => b.flush(now));
        const poses = bots.map(b => pose(b, now)), wire = bots.map(b => pose(b, now, true));
        result = trial.update(now, poses, wire, bots, gap);
        if (result) break;
        bots.forEach((b, i) => b.drive(trial.phase === 'run' ? trial.control(i, poses, bots) :
          stageSteering(b, trial.control(i, poses, bots), now), now));
        if (bots.some(b => !b.ws)) throw Error('socket_closed');
        await wait(10);
      }
      result ??= trial.end('capture_deadline'); results.push(result);
      // Continue recording after both event and survival. Await close before
      // admitting any replacement; failed joins never trigger automatic retry.
      const observeUntil = performance.now() + 1000 + delayMs;
      while (performance.now() < observeUntil) { bots.forEach(b => b.flush(performance.now())); await wait(10); }
      bots.forEach(b => b.ws?.close());
      const closeAt = performance.now();
      while (bots.some(b => b.ws)) {
        if (performance.now() - closeAt > 5000) throw Error('close_timeout');
        await wait(10);
      }
      if (performance.now() - started >= seconds * 1000) break;
    }
  } catch (e) { failure = e; }
  finally {
    fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify({ results, failure: failure ? String(failure) : null }, null, 2) + '\n');
    await finalizeCaptureBatch({ bots, recorders, source: SOURCE, completed: !failure, error: failure,
      metadata: { results, requestedSeconds: seconds } });
  }
  return results;
}
