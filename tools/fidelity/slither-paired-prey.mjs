// Bounded two-client prey observation. Archives raw protocol frames independently;
// coordination never substitutes intended visibility for a received prey packet.
import fs from 'node:fs';
import path from 'node:path';
import { Bot, Recorder, headingDanger, captureSourceIdentity, createCaptureBatch,
  finalizeCaptureBatch, runCollectorCli, validateOrigin } from './slither-headless.mjs';

const COLLECTOR_SOURCE = captureSourceIdentity(import.meta.url);

export const PAIR_PREY = Object.freeze({
  rendezvousDistance: 650, separationLimit: 1300,
  preyOffset: 450, targetHoldRadius: 80,
  staleMs: 2500, observerSampleMs: 100,
  minArenaGap: 650, maxSeconds: 480,
});
export const CROSSING = Object.freeze({
  nearOffset: 800, farOffset: 1850, followerOffset: 450,
  maxPairSeparation: 2400, minArenaGap: 2200,
  flightBoxRadius: 320,
  endpointTolerance: 120, crossTrackTolerance: 180,
  legTimeoutMs: 30000, primaryFreshMs: 600,
});

export function snapshot(bot, peer, now) {
  const me = bot.world.snakes.get(bot.world.self);
  if (!me?.pts.length || bot.world.dead || bot.ws?.readyState !== 1) return null;
  const rawHead = me.pts.at(-1);
  const peerMe = peer.world.snakes.get(peer.world.self);
  return { id: me.id, life: bot.lives, x: rawHead.x, y: rawHead.y,
    anchorAgeMs: now - me.t,
    peerVisible: !!peerMe?.pts.length && bot.world.snakes.has(peerMe.id),
    prey: [...bot.world.prey.values()].filter(p => now - p.t <= PAIR_PREY.staleMs)
      .map(p => ({ id: p.id, generation: p.generation, x: p.x, y: p.y,
        sp: p.sp, t: p.t })) };
}

export function preyOffset(prey, center, sign) {
  const horizontal = Math.abs(prey.x - center) < Math.abs(prey.y - center);
  return { x: prey.x + (horizontal ? sign * PAIR_PREY.preyOffset : 0),
    y: prey.y + (horizontal ? 0 : sign * PAIR_PREY.preyOffset) };
}

export function crossingOffset(prey, crossing, observer) {
  const offset = observer === 0 ? CROSSING.followerOffset :
    crossing.leg === 'near' ? CROSSING.nearOffset : CROSSING.farOffset;
  return { x: prey.x + (crossing.axis === 'x' ? crossing.sign * offset : 0),
    y: prey.y + (crossing.axis === 'y' ? crossing.sign * offset : 0) };
}

export function safePreyWaypoint(own, prey, target) {
  const margin = PAIR_PREY.preyOffset;
  if (target.x !== prey.x) {
    if (Math.sign(own.x - prey.x) !== Math.sign(target.x - prey.x) &&
        Math.abs(own.y - prey.y) < margin)
      return { x: own.x, y: prey.y + (own.y >= prey.y ? margin : -margin) };
    if (Math.abs(own.y - prey.y) >= margin && Math.abs(own.x - target.x) > PAIR_PREY.targetHoldRadius)
      return { x: target.x, y: own.y };
  }
  if (target.y !== prey.y) {
    if (Math.sign(own.y - prey.y) !== Math.sign(target.y - prey.y) &&
        Math.abs(own.x - prey.x) < margin)
      return { x: prey.x + (own.x >= prey.x ? margin : -margin), y: own.y };
    if (Math.abs(own.x - prey.x) >= margin && Math.abs(own.y - target.y) > PAIR_PREY.targetHoldRadius)
      return { x: own.x, y: target.y };
  }
  return target;
}

export class PairPreyState {
  constructor(mode = 'overlap') { this.mode = mode; this.crossingTrial = 0; this.reset(); }
  reset() {
    this.identities = null;
    this.phase = 'waiting';
    this.target = null;
    this.pairDistance = null;
    this.meetingTargets = null;
    this.searchCenter = null;
    this.navigationPrey = null;
    this.lastSeenAt = null;
    this.reason = null;
    this.crossing = null;
    this.crossingSnapshots = null;
  }
  update(now, snapshots, worlds) {
    const [first, second] = snapshots;
    if (!first || !second) { if (this.identities) this.reset(); return; }
    const identities = snapshots.map((s, i) => `${i}/${s.life}/${s.id}`);
    if (this.identities && identities.some((id, i) => id !== this.identities[i])) this.reset();
    if (this.mode === 'crossing') this.crossingSnapshots = snapshots;
    if (!this.identities) {
      this.identities = identities;
      const midpoint = { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 };
      const dx = second.x - first.x, dy = second.y - first.y;
      const length = Math.hypot(dx, dy) || 1;
      this.meetingTargets = [{ x: midpoint.x - dy / length * 220,
        y: midpoint.y + dx / length * 220 },
      { x: midpoint.x + dy / length * 220,
        y: midpoint.y - dx / length * 220 }];
      this.phase = 'rendezvous';
    }
    this.pairDistance = Math.hypot(first.x - second.x, first.y - second.y);
    const separationLimit = this.mode === 'crossing' ? CROSSING.maxPairSeparation : PAIR_PREY.separationLimit;
    if (this.pairDistance > separationLimit && this.phase !== 'rendezvous') {
      this.target = null; this.navigationPrey = null;
      this.crossing = null;
      this.phase = 'rendezvous'; this.reason = 'pair_separated';
    }
    if (this.phase === 'rendezvous' && this.pairDistance <= PAIR_PREY.rendezvousDistance &&
        first.peerVisible && second.peerVisible) {
      this.phase = 'search';
      this.searchCenter = { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 };
    }
    if (this.target) {
      const observations = snapshots.map(s => s.prey.find(p => p.id === this.target.id));
      if (this.mode === 'crossing' && (!observations[0] ||
          now - observations[0].t > CROSSING.primaryFreshMs)) {
        this.target = null; this.navigationPrey = null;
        this.crossing = null; this.phase = 'search'; this.reason = 'primary_visibility_lost';
      } else if (observations.some((p, i) => p &&
          (this.mode !== 'crossing' || i === 0) && this.target.generations[i] !== null &&
          p.generation !== this.target.generations[i])) {
        this.target = null; this.navigationPrey = null;
        this.crossing = null;
        this.phase = 'search'; this.reason = 'prey_generation_changed';
      } else if (observations.every(p => !p) && now - this.lastSeenAt > PAIR_PREY.staleMs) {
        this.target = null; this.navigationPrey = null;
        this.crossing = null;
        this.phase = 'search'; this.reason = 'prey_lost';
      } else {
        if (this.mode === 'crossing' && observations[1] &&
            this.target.generations[1] !== null &&
            observations[1].generation !== this.target.generations[1]) {
          this.crossing.reentries++;
          this.target.generations[1] = observations[1].generation;
        }
        observations.forEach((p, i) => { if (p && this.target.generations[i] === null)
          this.target.generations[i] = p.generation; });
        if (observations.some(Boolean)) this.lastSeenAt = Math.max(...observations.filter(Boolean).map(p => p.t));
        const newest = this.mode === 'crossing' ? { ...observations[0], observer: 0 } :
          observations.map((p, observer) => p && ({ ...p, observer }))
            .filter(Boolean).sort((a, b) => b.t - a.t)[0];
        if (newest) this.navigationPrey = newest;
        if (this.mode === 'crossing') {
          const c = this.crossing, p = observations[0];
          const arenaGap = Math.min(worlds[0].radius, worlds[1].radius) -
            Math.hypot(p.x - worlds[0].grd, p.y - worlds[0].grd);
          if (arenaGap < CROSSING.minArenaGap) {
            this.target = null; this.navigationPrey = null;
            this.crossing = null; this.phase = 'search'; this.reason = 'rim_margin_lost';
          } else {
          const axis = c.axis, crossAxis = axis === 'x' ? 'y' : 'x';
          c.actualAxisDistance = c.sign * (snapshots[1][axis] - p[axis]);
          c.actualCrossTrack = snapshots[1][crossAxis] - p[crossAxis];
          c.actualBoxDistance = Math.max(Math.abs(snapshots[1].x - p.x),
            Math.abs(snapshots[1].y - p.y));
          c.actualEuclideanDistance = Math.hypot(snapshots[1].x - p.x,
            snapshots[1].y - p.y);
          const goal = c.leg === 'near' ? CROSSING.nearOffset : CROSSING.farOffset;
          if (Math.abs(c.actualAxisDistance - goal) <= CROSSING.endpointTolerance &&
              Math.abs(c.actualCrossTrack) <= CROSSING.crossTrackTolerance) {
            c.leg = c.leg === 'near' ? 'far' : 'near';
            c.legStarted = now; c.completedLegs++;
          } else if (now - c.legStarted > CROSSING.legTimeoutMs) {
            this.target = null; this.navigationPrey = null;
            this.crossing = null; this.phase = 'search'; this.reason = 'crossing_leg_timeout';
          }
          }
        }
      }
    }
    if (this.phase !== 'search' || this.target) return;
    const candidates = new Map();
    for (const [i, s] of snapshots.entries()) for (const p of s.prey) {
      if (this.mode === 'crossing' && i !== 0) continue;
      if (this.mode === 'crossing' && now - p.t > CROSSING.primaryFreshMs) continue;
      const row = candidates.get(p.id) ?? { prey: p, observer: i,
        observers: 0, generations: [null, null] };
      row.observers++;
      row.generations[i] = p.generation;
      if (p.t > row.prey.t) { row.prey = p; row.observer = i; }
      candidates.set(p.id, row);
    }
    const midpoint = { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 };
    const center = worlds[0].grd, radius = Math.min(worlds[0].radius, worlds[1].radius);
    const eligible = [...candidates.entries()].filter(([, row]) => {
      const p = row.prey;
      const gap = radius - Math.hypot(p.x - center, p.y - center);
      if (gap < (this.mode === 'crossing' ? CROSSING.minArenaGap : PAIR_PREY.minArenaGap) ||
          Math.hypot(p.x - midpoint.x, p.y - midpoint.y) > 1300) return false;
      if (this.mode !== 'crossing') return true;
      const axis = Math.abs(p.x - center) >= Math.abs(p.y - center) ? 'y' : 'x';
      const sign = Math.sign(second[axis] - p[axis] || first[axis] - p[axis]) || 1;
      return sign * (first[axis] - p[axis]) >= CROSSING.followerOffset &&
        sign * (second[axis] - p[axis]) >= CROSSING.followerOffset;
    });
    eligible.sort((a, b) => b[1].observers - a[1].observers ||
      Math.hypot(a[1].prey.x - midpoint.x, a[1].prey.y - midpoint.y) -
      Math.hypot(b[1].prey.x - midpoint.x, b[1].prey.y - midpoint.y));
    if (eligible.length) {
      const [id, row] = eligible[0];
      if (this.mode === 'crossing') row.generations[1] = snapshots[1].prey.find(p => p.id === id)?.generation ?? null;
      this.target = { id, generations: row.generations };
      this.navigationPrey = { ...row.prey, observer: row.observer };
      this.lastSeenAt = row.prey.t;
      this.phase = 'follow'; this.reason = null;
      if (this.mode === 'crossing') {
        const axis = Math.abs(row.prey.x - center) >= Math.abs(row.prey.y - center) ? 'y' : 'x';
        const fromB = snapshots[1][axis] - row.prey[axis];
        const fromA = snapshots[0][axis] - row.prey[axis];
        this.crossing = { trial: ++this.crossingTrial, selectedAt: now,
          axis, sign: Math.sign(fromB || fromA) || 1,
          leg: 'near', legStarted: now, completedLegs: 0, reentries: 0,
          actualAxisDistance: null, actualCrossTrack: null,
          actualBoxDistance: null, actualEuclideanDistance: null };
      }
    }
  }
}

export function followSteering(bot, state, now, me, choice) {
  const own = bot.world.head(me, now);
  let intended = null, target = null, navigationSource = null;
  if (state.phase === 'rendezvous') target = state.meetingTargets?.[bot.index];
  else if (state.phase === 'search' && state.searchCenter) {
    const angle = now * .00025 + bot.index * Math.PI;
    target = { x: state.searchCenter.x + 240 * Math.cos(angle),
      y: state.searchCenter.y + 240 * Math.sin(angle) };
    navigationSource = 'shared_search';
  }
  else if (state.phase === 'follow' && state.target) {
    const local = bot.world.prey.get(state.target.id);
    const expected = state.target.generations[bot.index];
    const localVisible = local && (expected === null || local.generation === expected) &&
      now - local.t <= PAIR_PREY.staleMs;
    const prey = state.mode === 'crossing' ? state.navigationPrey :
      localVisible ? local : state.navigationPrey;
    if (prey && now - prey.t <= PAIR_PREY.staleMs) {
      const offset = state.mode === 'crossing' && state.crossing
        ? crossingOffset(prey, state.crossing, bot.index)
        : preyOffset(prey, bot.world.grd, bot.index ? 1 : -1);
      target = safePreyWaypoint(own, prey, offset);
      navigationSource = state.mode === 'crossing' && bot.index === 1 ? 'primary' :
        localVisible ? 'local' :
        state.navigationPrey?.observer === bot.index ? 'cached_local' : 'peer';
    }
  }
  if (target) {
    const distance = Math.hypot(target.x - own.x, target.y - own.y);
    if (distance > PAIR_PREY.targetHoldRadius)
      intended = Math.atan2(target.y - own.y, target.x - own.x);
    else if (state.phase === 'follow') {
      // Orbit within the assigned offset instead of falling back to food seeking.
      const bearing = distance > 1 ? Math.atan2(own.y - target.y, own.x - target.x) :
        Math.atan2(target.y - (state.navigationPrey?.y ?? target.y),
          target.x - (state.navigationPrey?.x ?? target.x));
      intended = bearing + Math.PI / 2;
    }
  }
  const danger = intended === null ? null : headingDanger(bot.world, now, me, own,
    intended, Math.min(6, 1 + (me.sct - 2) / 106));
  const interruption = danger !== null && danger > .2 ? 'avoidance' : null;
  if (intended !== null && !interruption) choice.a = intended;
  return { intendedHeading: intended, intendedDanger: danger, interruption,
    desiredBoost: false, targetPoint: target, navigationSource };
}

export class PairPreyBot extends Bot {
  movement(now, me, choice) {
    this.movementIntent = followSteering(this, this.pair, now, me, choice);
    const prey = this.pair.target && this.world.prey.get(this.pair.target.id);
    const expected = this.pair.target?.generations[this.index];
    const visible = !!prey && (expected === null || prey.generation === expected) &&
      now - prey.t <= PAIR_PREY.staleMs;
    const raw = me.pts.at(-1);
    const navigationPrey = this.pair.navigationPrey;
    const rawDx = navigationPrey ? raw.x - navigationPrey.x : null;
    const rawDy = navigationPrey ? raw.y - navigationPrey.y : null;
    const crossing = this.pair.crossing;
    // These are raw snake anchors and the primary's cached prey packet. The geometry
    // flag below describes the observed positions, not server-side visibility timing.
    const primary = this.pair.crossingSnapshots?.[0];
    const secondary = this.pair.crossingSnapshots?.[1];
    const primaryDx = crossing && primary && navigationPrey ? primary.x - navigationPrey.x : null;
    const primaryDy = crossing && primary && navigationPrey ? primary.y - navigationPrey.y : null;
    const secondaryDx = crossing && secondary && navigationPrey ? secondary.x - navigationPrey.x : null;
    const secondaryDy = crossing && secondary && navigationPrey ? secondary.y - navigationPrey.y : null;
    const primaryAxisDistance = primaryDx === null ? null : crossing.sign *
      (crossing.axis === 'x' ? primaryDx : primaryDy);
    const primaryCrossTrack = primaryDx === null ? null :
      (crossing.axis === 'x' ? primaryDy : primaryDx);
    const secondaryAxisDistance = secondaryDx === null ? null : crossing.sign *
      (crossing.axis === 'x' ? secondaryDx : secondaryDy);
    const secondaryCrossTrack = secondaryDx === null ? null :
      (crossing.axis === 'x' ? secondaryDy : secondaryDx);
    const primaryBoxDistance = primaryDx === null ? null :
      Math.max(Math.abs(primaryDx), Math.abs(primaryDy));
    const secondaryBoxDistance = secondaryDx === null ? null :
      Math.max(Math.abs(secondaryDx), Math.abs(secondaryDy));
    const primaryGeometryReady = primaryAxisDistance !== null &&
      Math.abs(primaryAxisDistance - CROSSING.followerOffset) <= CROSSING.endpointTolerance &&
      Math.abs(primaryCrossTrack) <= CROSSING.crossTrackTolerance &&
      primaryBoxDistance >= CROSSING.flightBoxRadius;
    if (now - (this.lastPairSample ?? -Infinity) >= PAIR_PREY.observerSampleMs ||
        this.lastPairPhase !== this.pair.phase || this.lastPairTarget !== this.pair.target?.id ||
        this.lastPairLeg !== crossing?.leg || this.lastPairTrial !== crossing?.trial) {
      this.lastPairSample = now; this.lastPairPhase = this.pair.phase;
      this.lastPairTarget = this.pair.target?.id; this.lastPairLeg = crossing?.leg;
      this.lastPairTrial = crossing?.trial;
      this.pendingPairSample = { event: 'paired-prey-state', botIndex: this.index,
        mode: this.pair.mode, life: this.lives, selfId: me.id, serverClock: this.serverClock,
        phase: this.pair.phase, pairDistance: this.pair.pairDistance,
        pairIdentities: this.pair.identities, targetId: this.pair.target?.id ?? null,
        targetVisible: visible, targetGeneration: visible ? prey.generation : null,
        expectedGeneration: expected ?? null,
        targetAgeMs: visible ? now - prey.t : null,
        targetX: visible ? prey.x : null, targetY: visible ? prey.y : null,
        targetSpeed: visible ? prey.sp : null,
        navigationSource: this.movementIntent.navigationSource,
        navigationPreyObserver: ['peer', 'primary'].includes(this.movementIntent.navigationSource)
          ? this.pair.navigationPrey?.observer ?? null : null,
        navigationPreyAgeMs: crossing || ['peer', 'primary'].includes(this.movementIntent.navigationSource)
          ? now - this.pair.navigationPrey.t : null,
        crossingLeg: crossing?.leg ?? null, crossingAxis: crossing?.axis ?? null,
        crossingSign: crossing?.sign ?? null,
        crossingTrial: crossing?.trial ?? null, crossingSelectedAt: crossing?.selectedAt ?? null,
        crossingLegStartedAt: crossing?.legStarted ?? null,
        intendedOffset: crossing ? this.index === 0 ? CROSSING.followerOffset :
          crossing.leg === 'near' ? CROSSING.nearOffset : CROSSING.farOffset : null,
        completedLegs: crossing?.completedLegs ?? null,
        observerReentries: crossing?.reentries ?? null,
        primaryAnchorAgeMs: crossing ? primary?.anchorAgeMs ?? null : null,
        secondaryAnchorAgeMs: crossing ? secondary?.anchorAgeMs ?? null : null,
        primaryPreyBoxDistance: primaryBoxDistance,
        primaryPreyAxisDistance: primaryAxisDistance,
        primaryPreyCrossTrack: primaryCrossTrack,
        secondaryPreyBoxDistance: secondaryBoxDistance,
        secondaryPreyAxisDistance: secondaryAxisDistance,
        secondaryPreyCrossTrack: secondaryCrossTrack,
        primaryGeometryReady: crossing ? primaryGeometryReady : null,
        crossingGeometryReady: crossing ? primaryGeometryReady &&
          secondaryAxisDistance >= CROSSING.flightBoxRadius &&
          Math.abs(secondaryCrossTrack) <= CROSSING.crossTrackTolerance : null,
        actualPreyBoxDistance: rawDx === null ? null : Math.max(Math.abs(rawDx), Math.abs(rawDy)),
        actualPreyEuclideanDistance: rawDx === null ? null : Math.hypot(rawDx, rawDy),
        actualPreyAxisDistance: crossing && navigationPrey
          ? crossing.sign * (crossing.axis === 'x' ? rawDx : rawDy) : null,
        actualPreyCrossTrack: crossing && navigationPrey
          ? crossing.axis === 'x' ? rawDy : rawDx : null,
        ownX: raw.x, ownY: raw.y, ownAnchorAgeMs: now - me.t,
        intendedHeading: this.movementIntent.intendedHeading,
        interruption: this.movementIntent.interruption,
        requestedHeading: choice.a,
        encodedHeading: Math.floor(251 * (((choice.a % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2)) / (Math.PI * 2)),
      };
    }
  }
  tick(now) {
    this.pendingPairSample = null;
    super.tick(now);
    if (this.pendingPairSample) this.rec.meta({ ...this.pendingPairSample,
      sentHeading: this.lastHeading, boost: this.boost });
  }
}

export function parseArgs(argv) {
  const args = {}, allowed = new Set(['--out', '--server', '--seconds', '--mode', '--plan', '--origin']);
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i], value = argv[i + 1];
    if (!allowed.has(key) || !value || value.startsWith('--') || key in args)
      throw Error(`invalid or repeated option: ${key}`);
    args[key] = value;
  }
  return { out: args['--out'], server: args['--server'],
    ...(args['--origin'] ? { origin: validateOrigin(args['--origin']) } : {}),
    seconds: Number(args['--seconds'] ?? PAIR_PREY.maxSeconds),
    mode: args['--mode'] ?? 'overlap', ...(args['--plan'] ? { plan: args['--plan'] } : {}) };
}

export async function recordPair({ out, server, seconds = PAIR_PREY.maxSeconds, mode = 'overlap', plan, origin }, signal) {
  if (signal?.aborted) return [];
  if (!out || !server || !Number.isFinite(seconds) || seconds < 1 || seconds > PAIR_PREY.maxSeconds)
    throw Error('use --out, --server ip:port and 1..480 seconds');
  if (!['overlap', 'crossing'].includes(mode)) throw Error('use --mode overlap|crossing');
  origin = validateOrigin(origin);
  if (fs.existsSync(out)) throw Error(`refusing to overwrite ${out}`);
  const [ip, portText] = server.split(':');
  const port = Number(portText);
  if (!ip || !Number.isInteger(port) || port < 1 || port > 65535) throw Error('invalid server');
  const batch = createCaptureBatch({ seconds, mode, server, scenario: 'paired-prey', plan, origin }, 2, COLLECTOR_SOURCE);
  const state = new PairPreyState(mode), bots = [], recorders = [];
  let timer, deadline, onAbort, completed = false, failure = null;
  const collectorTiming = { targetTickMs: 25, ticks: 0, lateTicks: 0, maxTickGapMs: 0 };
  let lastTick = performance.now();
  try {
    fs.mkdirSync(out, { recursive: true });
    for (const index of [0, 1]) {
      if (signal?.aborted) break;
      const nick = `Probe prey ${index}`;
      const rec = new Recorder(path.join(out, `bot${index}.jsonl.gz`), {
        schema: 1, server: { ip, port }, nick, scenario: 'paired-prey',
        mode, botIndex: index, plan: mode === 'crossing' ? { ...PAIR_PREY, ...CROSSING } : PAIR_PREY,
        wall: Date.now(), t: performance.now(), captureProvenance: batch.provenance(index) });
      recorders.push(rec);
      bots.push(new PairPreyBot({ scenario: 'movement', nick, origin }, index, rec));
    }
    for (const bot of bots) {
      if (signal?.aborted) break;
      bot.pair = state; bot.peers = bots; bot.connect({ ip, port });
    }
    await new Promise((resolve, reject) => {
      onAbort = () => resolve();
      signal?.addEventListener('abort', onAbort, { once: true });
      for (const rec of recorders) rec.failure.then(reject);
      if (signal?.aborted) { resolve(); return; }
      deadline = setTimeout(resolve, seconds * 1000);
      timer = setInterval(() => {
        try {
          const now = performance.now();
          const gap = now - lastTick;
          collectorTiming.ticks++;
          collectorTiming.lateTicks += gap > 75 ? 1 : 0;
          collectorTiming.maxTickGapMs = Math.max(collectorTiming.maxTickGapMs, gap);
          lastTick = now;
          state.update(now, [snapshot(bots[0], bots[1], now), snapshot(bots[1], bots[0], now)],
            [bots[0].world, bots[1].world]);
          for (const bot of bots) bot.tick(now);
          if (bots.some(bot => bot.stopped)) reject(Error('paired capture failed: a client exhausted reconnect attempts'));
        } catch (error) { reject(error); }
      }, 25);
    });
    if (signal?.aborted) return bots.map(bot => ({ joins: bot.joins, maxSegments: bot.maxSegments }));
    if (bots.some(bot => bot.joins === 0)) throw Error('paired capture failed: a client never joined');
    completed = true;
  } catch (error) { failure = error; }
  finally {
    clearInterval(timer); clearTimeout(deadline);
    signal?.removeEventListener('abort', onAbort);
    await finalizeCaptureBatch({ bots, recorders, source: COLLECTOR_SOURCE, completed,
      signal, error: failure, metadata: { collectorTiming, requestedSeconds: seconds } });
  }
  return bots.map(bot => ({ joins: bot.joins, maxSegments: bot.maxSegments }));
}

if (process.argv[1]?.endsWith('slither-paired-prey.mjs')) {
  runCollectorCli(signal => recordPair(parseArgs(process.argv.slice(2)), signal));
}
