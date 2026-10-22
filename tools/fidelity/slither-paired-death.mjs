// Bounded two-client compact-death experiment. Raw frames use the ordinary
// headless archive schema; coordination is diagnostic only, never a policy input.
import fs from 'node:fs';
import path from 'node:path';
import { Bot, Recorder, headingDanger, captureSourceIdentity, createCaptureBatch,
  finalizeCaptureBatch, runCollectorCli, validateOrigin } from './slither-headless.mjs';

const COLLECTOR_SOURCE = captureSourceIdentity(import.meta.url);

const PREPARE_TIMEOUT_MS = 30000;
const RENDEZVOUS_TIMEOUT_MS = 90000;
const CONTACT_TIMEOUT_MS = 5000;
const CONTACT_RADIUS = 240;

export function bodyGeometry(points, head) {
  if (points.length < 2) return null;
  const body = [...points, head];
  const spans = body.slice(1).map((point, index) =>
    Math.hypot(point.x - body[index].x, point.y - body[index].y));
  const sorted = spans.slice().sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return { medianSpacing: sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2,
    extentFromHead: Math.max(...body.map(point => Math.hypot(point.x - head.x, point.y - head.y))) };
}

export function nearbyCollisionBody(world, me, head, radius = CONTACT_RADIUS) {
  let best = null;
  for (const snake of world.snakes.values()) {
    if (snake.id === me.id) continue;
    for (const point of snake.pts) {
      const distance = Math.hypot(point.x - head.x, point.y - head.y);
      if (distance <= radius && (!best || distance < best.distance)) {
        best = { x: point.x, y: point.y, snakeId: snake.id, distance };
      }
    }
  }
  return best;
}

export class PairState {
  constructor(minSegments = 30) {
    this.minSegments = minSegments;
    this.preparedLives = 0;
    this.reset();
  }
  reset() {
    this.victimIndex = null;
    this.victimIdentity = null;
    this.observerIdentity = null;
    this.trial = null;
    this.phase = 'forage';
    this.started = 0;
    this.lastGrowth = 0;
    this.segments = 0;
    this.fam = null;
    this.shape = null;
    this.reason = null;
    this.meetingTargets = null;
    this.headDistance = null;
  }
  role(index) {
    return this.victimIndex === null ? 'forager' : this.victimIndex === index ? 'victim' : 'observer';
  }
  update(now, first, second) {
    const snapshots = [first, second];
    const identity = index => snapshots[index] &&
      `${index}/${snapshots[index].life}/${snapshots[index].id}`;
    if (this.victimIndex !== null &&
        (identity(this.victimIndex) !== this.victimIdentity ||
         identity(1 - this.victimIndex) !== this.observerIdentity)) {
      this.reset();
    }
    if (this.victimIndex === null) {
      if (!first || !second) return;
      // First threshold crossing seen by the shared timer wins. Simultaneous
      // qualifying samples tie deterministically in favor of bot0.
      const index = snapshots.findIndex(snake => snake.segments >= this.minSegments);
      if (index < 0) return;
      this.victimIndex = index;
      this.victimIdentity = identity(index);
      this.observerIdentity = identity(1 - index);
      this.trial = ++this.preparedLives;
      this.shape = this.trial % 2 ? 'compact' : 'stretched';
      const dx = second.x - first.x, dy = second.y - first.y;
      const distance = Math.hypot(dx, dy);
      const perpendicular = distance > 0 ? { x: -dy / distance, y: dx / distance } : { x: 0, y: 1 };
      const midpoint = { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 };
      this.meetingTargets = [-1, 1].map(sign => ({
        x: midpoint.x + sign * 150 * perpendicular.x,
        y: midpoint.y + sign * 150 * perpendicular.y }));
      this.phase = 'rendezvous'; this.started = now;
    }
    const victim = snapshots[this.victimIndex];
    this.headDistance = Math.hypot(first.x - second.x, first.y - second.y);
    if (this.phase === 'rendezvous') {
      if (this.headDistance <= 400 && first.peerVisible && second.peerVisible) {
        this.phase = 'prepare'; this.started = now;
        this.lastGrowth = now;
      } else if (now - this.started >= RENDEZVOUS_TIMEOUT_MS) {
        this.phase = 'abandon'; this.reason = 'rendezvous_timeout';
      }
    }
    if (victim.segments !== this.segments || victim.fam !== this.fam) {
      this.lastGrowth = now; this.segments = victim.segments; this.fam = victim.fam;
    }
    const shapeReady = this.shape === 'compact'
      ? victim.geometry?.medianSpacing < 10
      : victim.geometry?.medianSpacing > 20;
    if (this.phase === 'prepare' && now - this.started >= 12000 &&
        now - this.lastGrowth >= 5000 && shapeReady && victim.collisionBody) {
      this.phase = 'contact'; this.started = now;
    }
    if (this.phase === 'prepare' && now - this.started >= PREPARE_TIMEOUT_MS) {
      this.phase = 'abandon'; this.reason = 'shape_or_body_unavailable';
    }
    if (this.phase === 'contact' && now - this.started >= CONTACT_TIMEOUT_MS) {
      this.phase = 'abandon'; this.reason = 'contact_timeout';
    }
  }
}

export class PairBot extends Bot {
  movement(now, me, choice) {
    const state = this.pair;
    const role = state.role(this.index);
    const peer = this.peers[1 - this.index];
    const peerMe = peer.world.snakes.get(peer.world.self);
    const head = this.world.head(me, now);
    let desired = null;
    if (state.phase === 'abandon' && role === 'victim') {
      if (this.abandonRequestedTrial !== state.trial) {
        this.abandonRequestedTrial = state.trial;
        this.rec.meta({ event: 'paired-death-abandon', victimIdentity: state.victimIdentity,
          trial: state.trial, victimIndex: state.victimIndex, selfId: me.id,
          phase: state.phase, meetingTargets: state.meetingTargets, headDistance: state.headDistance,
          shape: state.shape, reason: state.reason, geometry: this.pairSnapshot?.geometry ?? null });
        this.ws?.close();
      }
      return;
    }
    if (state.phase === 'rendezvous') {
      const target = state.meetingTargets[this.index];
      desired = Math.hypot(target.x - head.x, target.y - head.y) <= 100
        ? me.ang + 1.35 : Math.atan2(target.y - head.y, target.x - head.x);
    } else if (state.phase !== 'forage' && (role === 'victim' || peerMe)) {
      if (role === 'victim') {
        desired = state.shape === 'compact' ? me.ang + 1.35 : me.ang;
        if (state.phase === 'contact') {
          // Aim at the closest locally observed body after the shape interval.
          const target = this.pairSnapshot?.collisionBody;
          if (target) {
            desired = Math.atan2(target.y - head.y, target.x - head.x);
          }
        }
      } else {
        const peerHead = peer.world.head(peerMe, now);
        const target = { x: peerHead.x + 190, y: peerHead.y };
        desired = Math.atan2(target.y - head.y, target.x - head.x);
      }
    }
    let danger = null, interruption = null;
    if (desired !== null) {
      const world = Object.create(this.world);
      // A distant designated observer cannot intersect the victim's tight
      // circle. The generic straight-ray avoidance would interrupt each lap
      // unnecessarily. Close peers retain avoidance until intentional contact.
      const knownPeer = peerMe && this.world.snakes.get(peerMe.id);
      const peerDistance = knownPeer?.pts.length ? Math.min(...knownPeer.pts.map(p =>
        Math.hypot(p.x - head.x, p.y - head.y))) : Infinity;
      if (role === 'victim' && (state.phase === 'contact' ||
          (state.phase === 'prepare' && peerDistance > 180))) {
        world.snakes = new Map([...this.world.snakes].filter(([id]) => id !== peerMe?.id));
      }
      danger = headingDanger(world, now, me, head, desired, Math.min(6, 1 + (me.sct - 2) / 106));
      if (danger > .2 && !(role === 'victim' && state.phase === 'contact')) interruption = 'avoidance';
      else choice.a = desired;
    }
    this.movementIntent = { intendedHeading: desired, intendedDanger: danger,
      interruption, desiredBoost: false };
    const sampleState = `${state.trial}/${role}/${state.phase}`;
    // State updates and both movement calls share one synchronous timer turn.
    // Record a reset/role transition immediately, before another wire event
    // could make a death inherit a recent control from the previous trial.
    if (sampleState !== this.lastPairSampleState || now - (this.lastPairSample ?? -Infinity) >= 200) {
      this.lastPairSampleState = sampleState;
      this.lastPairSample = now;
      this.rec.meta({ event: 'paired-death-state', role, phase: state.phase,
        roleMode: 'adaptive', trial: state.trial, botIndex: this.index, life: this.lives,
        victimIndex: state.victimIndex, observerIdentity: state.observerIdentity,
        victimIdentity: state.victimIdentity, shape: state.shape,
        pairedIdentities: `${state.victimIdentity}:${state.observerIdentity}`, minSegments: state.minSegments,
        millisecondsSinceGrowth: now - state.lastGrowth,
        phaseAgeMs: now - state.started, peerId: peerMe?.id ?? null,
        meetingTargets: state.meetingTargets, headDistance: state.headDistance,
        peerVisible: this.pairSnapshot?.peerVisible ?? false,
        selfId: me.id, segments: me.sct, fam: me.fam, ...head,
        medianServerAnchorSpacing: this.pairSnapshot?.geometry?.medianSpacing ?? null,
        bodyExtentFromHead: this.pairSnapshot?.geometry?.extentFromHead ?? null,
        nearbyCollisionBody: this.pairSnapshot?.collisionBody ?? null,
        targetHeading: desired, interruption, danger });
    }
  }
}

export function pairSnapshot(bot, peer, now) {
  const me = bot.world.snakes.get(bot.world.self);
  if (!me || bot.world.dead || bot.ws?.readyState !== 1) return null;
  const head = bot.world.head(me, now);
  const geometry = bodyGeometry(me.pts, head);
  const collisionBody = nearbyCollisionBody(bot.world, me, head);
  const peerMe = peer.world.snakes.get(peer.world.self);
  const peerVisible = peer.ws?.readyState === 1 && !peer.world.dead && !!peerMe &&
    !!bot.world.snakes.get(peerMe.id)?.pts.length;
  bot.pairSnapshot = { geometry, collisionBody, peerVisible };
  return { id: me.id, life: bot.lives,
    segments: me.sct, fam: me.fam, heading: me.ang, speed: me.sp,
    geometry, collisionBody, peerVisible, ...head };
}

export async function recordPair({ out, server, seconds = 480, minSegments = 30, plan, origin }, signal) {
  if (signal?.aborted) return [];
  if (!out || !server || !Number.isFinite(seconds) || seconds < 1 || seconds > 480 ||
      !Number.isInteger(minSegments) || minSegments < 6 || minSegments > 80) {
    throw Error('use --out, --server ip:port, 1..480 seconds and 6..80 min-segments');
  }
  origin = validateOrigin(origin);
  if (fs.existsSync(out)) throw Error(`refusing to overwrite ${out}`);
  const [ip, portText] = server.split(':');
  const port = Number(portText);
  if (!ip || !Number.isInteger(port) || port < 1 || port > 65535) throw Error('invalid server');
  const batch = createCaptureBatch({ seconds, minSegments, server, scenario: 'paired-death',
    serverBody: true, plan, origin }, 2, COLLECTOR_SOURCE);
  const state = new PairState(minSegments);
  const bots = [], recorders = [];
  let timer, deadline, onAbort, completed = false, failure = null;
  const collectorTiming = { targetTickMs: 25, ticks: 0, lateTicks: 0, maxTickGapMs: 0 };
  let lastTick = performance.now();
  try {
    fs.mkdirSync(out, { recursive: true });
    for (const index of [0, 1]) {
      if (signal?.aborted) break;
      const nick = `Probe pair ${index}`;
      const options = { scenario: 'movement', nick, serverBody: true, origin };
      const rec = new Recorder(path.join(out, `bot${index}.jsonl.gz`), {
        schema: 1, server: { ip, port }, nick, scenario: 'paired-death',
        roleMode: 'adaptive', botIndex: index, minSegments,
        wall: Date.now(), t: performance.now(), captureProvenance: batch.provenance(index) });
      recorders.push(rec);
      bots.push(new PairBot(options, index, rec));
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
          state.update(now, pairSnapshot(bots[0], bots[1], now), pairSnapshot(bots[1], bots[0], now));
          for (const bot of bots) bot.tick(now);
          if (bots.some(bot => bot.stopped)) {
            reject(Error('paired capture failed: a client exhausted reconnect attempts'));
          }
        } catch (error) { reject(error); }
      }, 25);
    });
    if (signal?.aborted) return bots.map(bot => ({ joins: bot.joins, maxSegments: bot.maxSegments }));
    if (bots.some(bot => bot.joins === 0)) {
      throw Error('paired capture failed: a client never joined');
    }
    completed = true;
  } catch (error) {
    failure = error;
  } finally {
    clearInterval(timer);
    clearTimeout(deadline);
    signal?.removeEventListener('abort', onAbort);
    await finalizeCaptureBatch({ bots, recorders, source: COLLECTOR_SOURCE, completed,
      signal, error: failure, metadata: { collectorTiming, requestedSeconds: seconds } });
  }
  return bots.map(bot => ({ joins: bot.joins, maxSegments: bot.maxSegments }));
}

export function parseArgs(argv) {
  const args = {};
  const allowed = new Set(['--out', '--server', '--seconds', '--min-segments', '--plan', '--origin']);
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i], value = argv[i + 1];
    if (!allowed.has(key) || !value || value.startsWith('--') || key in args) {
      throw Error(`invalid or repeated option: ${key}`);
    }
    args[key] = value;
  }
  return { out: args['--out'], server: args['--server'],
    ...(args['--origin'] ? { origin: validateOrigin(args['--origin']) } : {}),
    seconds: Number(args['--seconds'] ?? 480),
    minSegments: Number(args['--min-segments'] ?? 30),
    ...(args['--plan'] ? { plan: args['--plan'] } : {}) };
}

if (process.argv[1]?.endsWith('slither-paired-death.mjs')) {
  runCollectorCli(signal => recordPair(parseArgs(process.argv.slice(2)), signal));
}
