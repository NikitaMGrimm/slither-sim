// Controlled acquisition waypoints, not measured server visibility ranges.
import fs from 'node:fs';
import path from 'node:path';
import { isIP } from 'node:net';
import { Bot, Recorder, headingDanger, captureSourceIdentity, createCaptureBatch,
  finalizeCaptureBatch, runCollectorCli, validateOrigin } from './slither-headless.mjs';

const SOURCE = captureSourceIdentity(import.meta.url);
const TAU = Math.PI * 2;
export const PAIR_SNAKE = Object.freeze({
  defaultSeconds: 480, maxSeconds: 3600, rendezvousDistance: 650,
  orbitRadius: 80, referenceTolerance: 200,
  nearOffset: 800, farOffset: 2300, endpointHoldMs: 2000,
  endpointAnchorFreshMs: 150, endpointTolerance: 120, crossTrackTolerance: 180,
  // Engineering pause bounds; neither extends endpoint or scientific freshness.
  // R7 failures: research/fidelity/2026-10-23-visibility-recheck/REPORT.md#exploratory-route-pilots-r7.
  controlAnchorMaxAgeMs: 500, pairedReacquireMs: 1000,
  minArenaGap: 700, maxPairSeparation: 3000, legTimeoutMs: 30000,
  sampleMs: 100, targetTickMs: 25,
});
export const DIRECTIONS = Object.freeze({
  x: Object.freeze({ x: 1, y: 0 }), y: Object.freeze({ x: 0, y: 1 }),
  'diagonal-positive': Object.freeze({ x: Math.SQRT1_2, y: Math.SQRT1_2 }),
  'diagonal-negative': Object.freeze({ x: Math.SQRT1_2, y: -Math.SQRT1_2 }),
});

function validateDirection(direction) {
  if (typeof direction !== 'string' || !Object.hasOwn(DIRECTIONS, direction))
    throw Error('use --direction x|y|diagonal-positive|diagonal-negative');
  return direction;
}

export function railPoint(center, direction, sign, offset) {
  const d = DIRECTIONS[validateDirection(direction)];
  return { x: center.x + sign * offset * d.x, y: center.y + sign * offset * d.y };
}

export function railProjection(center, direction, sign, point) {
  const d = DIRECTIONS[validateDirection(direction)];
  const dx = point.x - center.x, dy = point.y - center.y;
  return { along: sign * (dx * d.x + dy * d.y), crossTrack: -dx * d.y + dy * d.x };
}

export function snapshot(bot, peer, now) {
  const me = bot.world.snakes.get(bot.world.self);
  if (!me?.pts.length || bot.world.dead || bot.ws?.readyState !== 1) return null;
  const peerId = peer.world.self;
  const cached = bot.world.snakes.get(peerId);
  const bodyPositive = !!cached?.pts.length;
  return { id: me.id, life: bot.lives, ...me.pts.at(-1), anchorAgeMs: now - me.t,
    peerVisible: bodyPositive,
    localPeerSnakeId: peerId, localPeerSnakeCachePresent: !!cached,
    localPeerSnakeBodyPositive: bodyPositive,
    localPeerSnakeBodyPositiveAgeMs: bodyPositive ? now - cached.t : null,
    localPeerSnakeBodyPoints: cached?.pts.length ?? 0,
    localPeerSnakeHead: bodyPositive ? { ...cached.pts.at(-1) } : null };
}

function arenaGap(world, point) {
  return world.radius - Math.hypot(point.x - world.grd, point.y - world.grd);
}

export function siteInside(center, direction, sign, worlds) {
  const far = railPoint(center, direction, sign, PAIR_SNAKE.farOffset);
  return worlds.every(w => Number.isFinite(w.radius) && Number.isFinite(w.grd) &&
    arenaGap(w, far) >= PAIR_SNAKE.minArenaGap &&
    arenaGap(w, center) >= PAIR_SNAKE.minArenaGap + PAIR_SNAKE.referenceTolerance);
}

export function chooseSite(first, second, worlds, direction) {
  const w = worlds[0], d = DIRECTIONS[validateDirection(direction)];
  const margin = PAIR_SNAKE.farOffset + PAIR_SNAKE.minArenaGap +
    Math.hypot(PAIR_SNAKE.endpointTolerance, PAIR_SNAKE.crossTrackTolerance);
  const inner = Math.min(...worlds.map(world => world.radius)) - margin;
  if (!Number.isFinite(inner) || inner <= 0) return null;
  // Prefer A's current location; clamp inward when the whole rail needs room.
  const dx = first.x - w.grd, dy = first.y - w.grd, radius = Math.hypot(dx, dy);
  const scale = radius > inner ? inner / radius : 1;
  const base = { x: w.grd + dx * scale, y: w.grd + dy * scale };
  const awayX = first.x - second.x, awayY = first.y - second.y;
  const length = Math.hypot(awayX, awayY) || 1;
  for (const shift of [0, 300, 600]) {
    const center = { x: base.x + shift * awayX / length, y: base.y + shift * awayY / length };
    const sign = Math.sign((second.x - center.x) * d.x + (second.y - center.y) * d.y) || 1;
    if (!siteInside(center, direction, sign, worlds)) continue;
    // This is a cached-body clearance screen, not a guarantee about future traffic.
    const obstructed = [...w.snakes.values()].some(s => s.id !== w.self && s.pts.some(p =>
      Math.hypot(p.x - center.x, p.y - center.y) < PAIR_SNAKE.referenceTolerance + 100));
    if (!obstructed) return { center, sign };
  }
  return null;
}

function rawPose(s) {
  return s ? { id: s.id, life: s.life, x: s.x, y: s.y, anchorAgeMs: s.anchorAgeMs } : null;
}

function freshAnchor(age) {
  return Number.isFinite(age) && age >= 0 && age <= PAIR_SNAKE.endpointAnchorFreshMs;
}

export class PairSnakeState {
  constructor(direction, emit = () => {}) {
    this.direction = validateDirection(direction); this.emit = emit;
    this.trialCount = 0; this.reset();
  }
  reset(now, reason) {
    const c = this.crossing;
    if (c) this.emit({ event: 'paired-snake-trial-end', endedAt: now, reason,
      status: reason.startsWith('capture_') ? 'right-censored' : 'abandoned',
      pairIdentities: [...this.identities], fixedCenter: { ...c.center },
      crossingDirection: this.direction, crossingSign: c.sign, crossingTrial: c.trial,
      crossingSelectedAt: c.selectedAt, crossingLeg: c.leg,
      crossingLegStartedAt: c.legStarted, crossingHoldUntil: c.holdUntil,
      crossingHeldOffset: c.holdOffset, endpointVisits: c.endpointVisits,
      completedLegs: c.completedLegs, feasibilityPassed: c.feasibilityPassed,
      legInterruptionCount: c.interruptions, pauseInterruptionCount: c.pauseInterruptions,
      avoidanceInterruptionCount: c.avoidanceInterruptions,
      poseStatus: this.poseStatus, pauseStartedAt: this.pauseStartedAt });
    this.identities = null; this.crossing = null; this.snapshots = [null, null];
    this.poseStatus = 'paused'; this.pauseStartedAt = null;
    this.pauseIdentities = null;
    this.phase = 'waiting'; this.reason = reason ?? null;
    this.meetingTargets = null; this.pairDistance = null;
  }
  noteControl(interruption) {
    if (this.crossing && interruption) {
      this.crossing.interruptions++;
      this.crossing.avoidanceInterruptions++;
    }
  }
  poseEvent(event, now) {
    const c = this.crossing;
    this.emit({ event, controlAt: now, poseStatus: this.poseStatus,
      pauseStartedAt: this.pauseStartedAt, pairIdentities: this.identities ?? this.pauseIdentities,
      pairAnchorAgesMs: this.snapshots.map(s => s?.anchorAgeMs ?? null),
      crossingTrial: c?.trial ?? null, fixedCenter: c?.center ?? null,
      crossingLeg: c?.leg ?? null, crossingHoldUntil: c?.holdUntil ?? null,
      crossingHeldOffset: c?.holdOffset ?? null });
  }
  update(now, snapshots, worlds) {
    const [a, b] = snapshots;
    if (!a || !b) {
      if (this.identities || this.pauseStartedAt !== null) this.reset(now, 'pair_unavailable');
      this.snapshots = snapshots; return;
    }
    const identities = snapshots.map((s, i) => `${i}/${s.life}/${s.id}`);
    const fresh = snapshots.every(s => freshAnchor(s.anchorAgeMs));
    const previousIdentities = this.identities ?? this.pauseIdentities;
    if (previousIdentities && identities.some((id, i) => id !== previousIdentities[i])) {
      this.reset(now, 'pair_identity_changed');
      if (!fresh) { this.snapshots = snapshots; return; }
    }
    this.snapshots = snapshots;
    const c = this.crossing;
    if (c && !siteInside(c.center, this.direction, c.sign, worlds)) {
      this.reset(now, 'rim_margin_lost'); return;
    }
    if (c && now - c.legStarted > PAIR_SNAKE.legTimeoutMs) {
      this.reset(now, 'crossing_leg_timeout'); return;
    }
    this.pairDistance = fresh ? Math.hypot(a.x - b.x, a.y - b.y) : null;
    if (c && fresh && this.pairDistance > PAIR_SNAKE.maxPairSeparation) {
      this.reset(now, 'pair_separated'); return;
    }
    if (snapshots.some(s => !Number.isFinite(s.anchorAgeMs) || s.anchorAgeMs < 0 ||
        s.anchorAgeMs > PAIR_SNAKE.controlAnchorMaxAgeMs)) {
      this.reset(now, 'own_anchor_stale'); this.snapshots = snapshots; return;
    }
    if (this.pauseStartedAt !== null && now - this.pauseStartedAt > PAIR_SNAKE.pairedReacquireMs) {
      this.reset(now, 'paired_reacquire_timeout'); this.snapshots = snapshots; return;
    }
    if (!fresh && this.pauseStartedAt === null) {
      this.pauseStartedAt = now; this.poseStatus = 'paused'; this.reason = 'own_anchor_stale';
      this.pauseIdentities = identities;
      // Target persistence cannot restore an uninterrupted leg or certify its hold.
      if (c) { c.interruptions++; c.pauseInterruptions++; c.cleanLegStreak = 0; }
      this.poseEvent('paired-snake-pause', now);
    }
    if (this.pauseStartedAt !== null) {
      // Age is receipt-based. Both anchors must be strictly newer than pause entry.
      if (!fresh || snapshots.some(s => now - s.anchorAgeMs <= this.pauseStartedAt)) return;
      this.poseStatus = 'ready'; this.reason = null;
      this.poseEvent('paired-snake-resume', now);
      this.pauseStartedAt = null; this.pauseIdentities = null;
    }
    this.poseStatus = 'ready';
    if (this.reason === 'own_anchor_stale' || this.reason === 'paired_reacquire_timeout') this.reason = null;
    if (!this.identities) {
      this.identities = identities;
      const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
      const dx = b.x - a.x, dy = b.y - a.y, length = Math.hypot(dx, dy) || 1;
      this.meetingTargets = [
        { x: mx - dy / length * 220, y: my + dx / length * 220 },
        { x: mx + dy / length * 220, y: my - dx / length * 220 }];
      this.phase = 'rendezvous';
    }
    if (this.phase === 'rendezvous' && this.pairDistance <= PAIR_SNAKE.rendezvousDistance &&
        a.peerVisible && b.peerVisible) this.phase = 'site';
    if (this.phase === 'site') {
      const site = chooseSite(a, b, worlds, this.direction);
      if (!site) { this.reason = 'no_safe_site'; return; }
      this.crossing = { trial: ++this.trialCount, selectedAt: now,
        ...site, leg: 'near', legStarted: now, holdUntil: null, holdOffset: null,
        endpointVisits: 0, completedLegs: 0, legStart: null, interruptions: 0,
        pauseInterruptions: 0, avoidanceInterruptions: 0,
        cleanLegStreak: 0, feasibilityPassed: false };
      this.phase = 'crossing'; this.reason = null;
      return; // The first endpoint requires a later observed/control update.
    }
    if (!c) return;
    const released = c.holdUntil !== null && now >= c.holdUntil;
    if (released) { c.holdUntil = null; c.holdOffset = null; }
    if (released || c.holdUntil !== null) return;
    const p = railProjection(c.center, this.direction, c.sign, b);
    const offset = c.leg === 'near' ? PAIR_SNAKE.nearOffset : PAIR_SNAKE.farOffset;
    if (now > c.legStarted && Math.abs(p.along - offset) <= PAIR_SNAKE.endpointTolerance &&
        Math.abs(p.crossTrack) <= PAIR_SNAKE.crossTrackTolerance &&
        Math.hypot(a.x - c.center.x, a.y - c.center.y) <= PAIR_SNAKE.referenceTolerance) {
      const end = { t: now, offset, pairIdentities: [...identities],
        primary: rawPose(a), secondary: rawPose(b), fixedCenter: { ...c.center },
        direction: this.direction, sign: c.sign, along: p.along, crossTrack: p.crossTrack };
      const witness = { start: c.legStart, end, completed: c.legStart !== null,
        interrupted: c.interruptions > 0, interruptionCount: c.interruptions,
        pauseInterruptionCount: c.pauseInterruptions, avoidanceInterruptionCount: c.avoidanceInterruptions };
      if (witness.completed) c.completedLegs++;
      c.cleanLegStreak = witness.completed && !witness.interrupted ? c.cleanLegStreak + 1 : 0;
      c.feasibilityPassed ||= c.cleanLegStreak >= 2 && offset === PAIR_SNAKE.nearOffset;
      c.endpointVisits++; c.legStart = end; c.interruptions = 0;
      c.pauseInterruptions = 0; c.avoidanceInterruptions = 0;
      c.leg = c.leg === 'near' ? 'far' : 'near'; c.legStarted = now;
      c.holdOffset = offset; c.holdUntil = now + PAIR_SNAKE.endpointHoldMs;
      this.emit({ event: 'paired-snake-endpoint', crossingTrial: c.trial,
        crossingEndpoint: witness, endpointVisits: c.endpointVisits,
        completedLegs: c.completedLegs, feasibilityPassed: c.feasibilityPassed,
        crossingHoldUntil: c.holdUntil, crossingHeldOffset: c.holdOffset }, 1);
    }
  }
}

function orbitHeading(own, center, heading) {
  const dx = own.x - center.x, dy = own.y - center.y, radius = Math.hypot(dx, dy);
  const bearing = radius > 1 ? Math.atan2(dy, dx) : heading - Math.PI / 2;
  return bearing + Math.PI / 2 + Math.atan((radius - PAIR_SNAKE.orbitRadius) / PAIR_SNAKE.orbitRadius);
}

export function safeSteering(bot, now, me, choice, intendedHeading) {
  if (intendedHeading === null) return { intendedHeading, intendedDanger: null,
    selectedDanger: null, interruption: null, controlStatus: 'waiting', desiredBoost: false };
  const own = bot.world.head(me, now), scale = Math.min(6, 1 + (me.sct - 2) / 106);
  const danger = a => headingDanger(bot.world, now, me, own, a, scale);
  const intendedDanger = danger(intendedHeading);
  let selected = { a: intendedHeading, danger: intendedDanger };
  let interruption = null, controlStatus = 'direct';
  if (intendedDanger > .2) {
    let safe = null, escape = null;
    for (let i = 0; i < 32; i++) {
      const a = i * TAU / 32, risk = danger(a);
      const turn = Math.abs(Math.atan2(Math.sin(a - me.ang), Math.cos(a - me.ang)));
      const score = Math.cos(a - intendedHeading) - .01 * turn;
      if (risk <= .2 && (!safe || score > safe.score)) safe = { a, danger: risk, score };
      if (!escape || risk < escape.danger || risk === escape.danger && score > escape.score)
        escape = { a, danger: risk, score };
    }
    selected = safe ?? escape;
    controlStatus = safe ? 'deflected' : 'blocked';
    interruption = safe ? 'avoidance_deflection' : 'avoidance_blocked';
  }
  choice.a = selected.a;
  return { intendedHeading, intendedDanger, selectedDanger: selected.danger,
    interruption, controlStatus, desiredBoost: false };
}

export class PairSnakeBot extends Bot {
  send(bytes) {
    const sentAt = super.send(bytes);
    if (sentAt !== undefined && bytes.length === 1 && bytes[0] <= 250) {
      this.lastDeliveredHeading = bytes[0]; this.lastDeliveredLife = this.lives;
    }
    return sentAt;
  }
  targetPoint() {
    const state = this.pair, c = state.crossing;
    return c ? this.index === 0 ? c.center : railPoint(c.center, state.direction, c.sign,
      c.holdOffset ?? (c.leg === 'near' ? PAIR_SNAKE.nearOffset : PAIR_SNAKE.farOffset)) :
      state.meetingTargets?.[this.index] ?? null;
  }
  movement(now, me, choice) {
    const state = this.pair, own = this.world.head(me, now), c = state.crossing;
    const targetPoint = this.targetPoint(), orbit = !!c && this.index === 0;
    let intended = null;
    if (targetPoint) intended = orbit || Math.hypot(own.x - targetPoint.x, own.y - targetPoint.y) <= PAIR_SNAKE.orbitRadius
      ? orbitHeading(own, targetPoint, me.ang) : Math.atan2(targetPoint.y - own.y, targetPoint.x - own.x);
    this.movementIntent = { ...safeSteering(this, now, me, choice, intended), targetPoint };
    state.noteControl(this.movementIntent.interruption);
    this.sampleState(now, me, choice);
  }
  sampleState(now, me, choice = null) {
    const state = this.pair, c = state.crossing;
    const key = `${this.lives}/${state.phase}/${c?.trial}/${c?.leg}/${c?.holdUntil}/${state.poseStatus}`;
    if (key === this.lastSampleKey && now - (this.lastSampleAt ?? -Infinity) < PAIR_SNAKE.sampleMs) return;
    this.lastSampleKey = key; this.lastSampleAt = now;
    const local = snapshot(this, this.peers[1 - this.index], now);
    const raw = me.pts.at(-1);
    const p = c ? railProjection(c.center, state.direction, c.sign, raw) : null;
    this.pendingSample = { event: 'paired-snake-state', scenario: 'paired-snake', mode: 'crossing',
      direction: state.direction, botIndex: this.index, life: this.lives, selfId: me.id,
      serverClock: this.serverClock, phase: state.phase, reason: state.reason,
      controlAt: now, poseStatus: state.poseStatus, pauseStartedAt: state.pauseStartedAt,
      pairAnchorAgesMs: state.snapshots.map(s => s?.anchorAgeMs ?? null),
      pairIdentities: state.identities, pairDistance: state.pairDistance,
      primary: rawPose(state.snapshots[0]), secondary: rawPose(state.snapshots[1]),
      ownX: raw.x, ownY: raw.y, ownAnchorAgeMs: now - me.t,
      localPeerSnakeId: local?.localPeerSnakeId ?? null,
      localPeerSnakeCachePresent: local?.localPeerSnakeCachePresent ?? false,
      localPeerSnakeBodyPositive: local?.localPeerSnakeBodyPositive ?? false,
      localPeerSnakeBodyPositiveAgeMs: local?.localPeerSnakeBodyPositiveAgeMs ?? null,
      localPeerSnakeBodyPoints: local?.localPeerSnakeBodyPoints ?? 0,
      localPeerSnakeHead: local?.localPeerSnakeHead ?? null,
      fixedCenter: c?.center ?? null, railUnit: DIRECTIONS[state.direction],
      crossingSign: c?.sign ?? null, crossingTrial: c?.trial ?? null,
      crossingSelectedAt: c?.selectedAt ?? null, crossingLeg: c?.leg ?? null,
      crossingLegStartedAt: c?.legStarted ?? null, crossingHoldUntil: c?.holdUntil ?? null,
      crossingHeldOffset: c?.holdOffset ?? null, endpointVisits: c?.endpointVisits ?? null,
      completedLegs: c?.completedLegs ?? null, feasibilityPassed: c?.feasibilityPassed ?? false,
      actualAlong: p?.along ?? null, actualCrossTrack: p?.crossTrack ?? null,
      legInterruptionCount: c?.interruptions ?? null,
      pauseInterruptionCount: c?.pauseInterruptions ?? null,
      avoidanceInterruptionCount: c?.avoidanceInterruptions ?? null,
      ...this.movementIntent, requestedHeading: choice?.a ?? null,
      encodedHeading: choice ? Math.floor(251 * ((choice.a % TAU + TAU) % TAU) / TAU) : null };
  }
  tick(now) {
    this.pendingSample = null;
    const me = this.world.snakes.get(this.world.self);
    if (!this.stopped && this.ws?.readyState === 1 && me?.pts.length && !this.deathObservation &&
        now - this.lastReceivedAt <= 15000 &&
        (!freshAnchor(now - me.t) || this.pair.poseStatus !== 'ready')) {
      if (now - (this.lastStaleSampleAt ?? -Infinity) >= PAIR_SNAKE.sampleMs) {
        this.lastStaleSampleAt = now;
        this.rec.meta({ event: 'paired-snake-control-skipped', reason: 'own_anchor_stale',
          botIndex: this.index, life: this.lives, controlAt: now,
          ownAnchorAgeMs: now - me.t,
          pairAnchorAgesMs: this.pair.snapshots.map(s => s?.anchorAgeMs ?? null) });
      }
      this.ping(now);
      // Releasing boost must not wait for its ordinary 50 ms toggle throttle.
      if (this.boost) { this.send([254]); this.boost = false; this.lastBoostAt = now; }
      this.movementIntent = { intendedHeading: null, intendedDanger: null, selectedDanger: null,
        interruption: 'own_anchor_stale', controlStatus: 'paused', desiredBoost: false,
        targetPoint: this.targetPoint() };
      this.sampleState(now, me);
    } else super.tick(now);
    if (this.pendingSample) {
      const delivered = this.lastDeliveredLife === this.lives ? this.lastDeliveredHeading : null;
      this.rec.meta({ ...this.pendingSample, sentHeading: delivered,
        lastDeliveredHeading: delivered, boost: this.boost });
    }
  }
}

function validateOptions({ out, server, seconds, mode, direction, origin }) {
  if (!out || typeof out !== 'string' || typeof server !== 'string' || !Number.isFinite(seconds) ||
      seconds < 1 || seconds > PAIR_SNAKE.maxSeconds) throw Error('use --out, --server ip:port and 1..3600 seconds');
  if (mode !== 'crossing') throw Error('use --mode crossing');
  validateDirection(direction); validateOrigin(origin);
  const parts = server.split(':');
  if (parts.length !== 2 || isIP(parts[0]) !== 4 || !/^\d+$/.test(parts[1]) ||
      Number(parts[1]) < 1 || Number(parts[1]) > 65535) throw Error('invalid server');
}

export function parseArgs(argv) {
  const args = {}, allowed = new Set(['--out', '--server', '--seconds', '--mode', '--direction', '--plan', '--origin']);
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i], value = argv[i + 1];
    if (!allowed.has(key) || !value || value.startsWith('--') || key in args) throw Error(`invalid or repeated option: ${key}`);
    args[key] = value;
  }
  const options = { out: args['--out'], server: args['--server'],
    seconds: Number(args['--seconds'] ?? PAIR_SNAKE.defaultSeconds), mode: args['--mode'] ?? 'crossing',
    direction: args['--direction'], ...(args['--plan'] ? { plan: args['--plan'] } : {}),
    ...(args['--origin'] ? { origin: validateOrigin(args['--origin']) } : {}) };
  validateOptions(options);
  return options;
}

export async function recordPair({ out, server, seconds = PAIR_SNAKE.defaultSeconds,
  mode = 'crossing', direction, plan, origin }, signal) {
  if (signal?.aborted) return [];
  validateOptions({ out, server, seconds, mode, direction, origin });
  origin = validateOrigin(origin);
  if (fs.existsSync(out)) throw Error(`refusing to overwrite ${out}`);
  const [ip, port] = server.split(':');
  const batch = createCaptureBatch({ server, seconds, mode, direction, origin, scenario: 'paired-snake', plan }, 2, SOURCE);
  const bots = [], recorders = [];
  const state = new PairSnakeState(direction, (row, recipient) => {
    recorders.forEach((rec, botIndex) => {
      if (recipient === undefined || recipient === botIndex) rec.meta({ ...row, botIndex });
    });
  });
  const collectorTiming = { targetTickMs: PAIR_SNAKE.targetTickMs, ticks: 0, lateTicks: 0, maxTickGapMs: 0 };
  let timer, deadline, onAbort, completed = false, failure = null, lastTick = performance.now();
  try {
    fs.mkdirSync(out, { recursive: true });
    for (const index of [0, 1]) {
      const nick = `(Bot) Probe snake ${index}`;
      const rec = new Recorder(path.join(out, `bot${index}.jsonl.gz`), {
        schema: 1, server: { ip, port: Number(port) }, nick, scenario: 'paired-snake', mode, direction,
        botIndex: index, plan: PAIR_SNAKE, ...(batch.dataOrigin ? { dataOrigin: batch.dataOrigin } : {}),
        wall: Date.now(), t: performance.now(), captureProvenance: batch.provenance(index) });
      recorders.push(rec); bots.push(new PairSnakeBot({ scenario: 'movement', nick, origin }, index, rec));
    }
    for (const bot of bots) { bot.pair = state; bot.peers = bots; }
    for (const bot of bots) { if (!signal?.aborted) bot.connect({ ip, port: Number(port) }); }
    await new Promise((resolve, reject) => {
      onAbort = () => resolve(); signal?.addEventListener('abort', onAbort, { once: true });
      for (const rec of recorders) rec.failure.then(reject);
      if (signal?.aborted) { resolve(); return; }
      deadline = setTimeout(resolve, seconds * 1000);
      timer = setInterval(() => {
        try {
          const now = performance.now(), gap = now - lastTick; lastTick = now;
          collectorTiming.ticks++; collectorTiming.lateTicks += gap > 75 ? 1 : 0;
          collectorTiming.maxTickGapMs = Math.max(collectorTiming.maxTickGapMs, gap);
          state.update(now, bots.map((bot, i) => snapshot(bot, bots[1 - i], now)), bots.map(bot => bot.world));
          for (const bot of bots) bot.tick(now);
          if (bots.some(bot => bot.stopped)) reject(Error('paired capture failed: a client exhausted reconnect attempts'));
        } catch (error) { reject(error); }
      }, PAIR_SNAKE.targetTickMs);
    });
    if (!signal?.aborted) {
      if (bots.some(bot => bot.joins === 0)) throw Error('paired capture failed: a client never joined');
      completed = true;
    }
  } catch (error) { failure = error; }
  finally {
    clearInterval(timer); clearTimeout(deadline); signal?.removeEventListener('abort', onAbort);
    try { state.reset(performance.now(), failure ? 'capture_failed' : signal?.aborted ? 'capture_interrupted' : 'capture_stop'); }
    catch (error) { failure ??= error; }
    await finalizeCaptureBatch({ bots, recorders, source: SOURCE, completed, signal, error: failure,
      metadata: { collectorTiming, requestedSeconds: seconds } });
  }
  return bots.map(bot => ({ joins: bot.joins, maxSegments: bot.maxSegments }));
}

if (process.argv[1]?.endsWith('slither-paired-snake.mjs'))
  runCollectorCli(signal => recordPair(parseArgs(process.argv.slice(2)), signal));
