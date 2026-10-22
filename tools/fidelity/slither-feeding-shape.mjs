// Bounded two-client geometry pilot. Raw frames are ordinary headless archives.
import fs from 'node:fs';
import path from 'node:path';
import { Bot, Recorder, headingDanger, captureSourceIdentity, createCaptureBatch,
  finalizeCaptureBatch, runCollectorCli, validateOrigin } from './slither-headless.mjs';

const COLLECTOR_SOURCE = captureSourceIdentity(import.meta.url);

const RENDEZVOUS_MS = 90000;
const CONTACT_MS = 5000;
const HOLD_MS = 20000;
const COLLECTION_MS = 30000;
const STABLE_MS = 5000;
const TARGET_RADIUS = 400;
const ORBIT_RADIUS = 300;

export function rawGeometry(points) {
  if (points.length < 2) return null;
  const spans = points.slice(1).map((point, i) =>
    Math.hypot(point.x - points[i].x, point.y - points[i].y));
  const sorted = spans.slice().sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  const pathLength = spans.reduce((sum, span) => sum + span, 0);
  let distantPairs = 0;
  for (let i = 0; i < points.length; i++) {
    for (let j = i + 8; j < points.length; j++) {
      if (Math.hypot(points[i].x - points[j].x, points[i].y - points[j].y) < 30) distantPairs++;
    }
  }
  return { medianSpacing: sorted.length % 2 ? sorted[middle] :
    (sorted[middle - 1] + sorted[middle]) / 2,
  distantPairs, pathRatio: pathLength ? Math.hypot(
    points.at(-1).x - points[0].x, points.at(-1).y - points[0].y) / pathLength : null,
  pathLength, anchorCount: points.length };
}

export function rearBodyTarget(world, receiverId, donorHead, receiverHead) {
  const receiver = world.snakes.get(receiverId);
  if (!receiver?.pts?.length) return null;
  let best = null;
  for (const point of receiver.pts.slice(0, -3)) {
    const fromHead = Math.hypot(point.x - receiverHead.x, point.y - receiverHead.y);
    const distance = Math.hypot(point.x - donorHead.x, point.y - donorHead.y);
    if (fromHead < 250 || (best && distance >= best.distance)) continue;
    best = { x: point.x, y: point.y, distance, fromHead, snakeId: receiverId };
  }
  return best;
}

export class FeedingState {
  constructor() { this.reset(); }
  reset() {
    this.phase = 'forage'; this.reason = null; this.started = null;
    this.receiverIdentity = null; this.donorIdentity = null;
    this.heading = null; this.orbitCenter = null; this.lastMass = null; this.lastMassAt = null;
    this.interruptions = 0; this.feedTarget = null; this.feedMassChanged = false;
    this.lastDonor = null; this.deathSite = null; this.deathAt = null; this.deathMass = null;
    this.rapidIds = new Set(); this.collectedRapidIds = new Set();
  }
  finish(reason) { this.phase = 'terminal'; this.reason = reason; }
  update(now, receiver, donor, donorDead = false) {
    const receiverIdentity = receiver && `0/${receiver.life}/${receiver.id}`;
    const donorIdentity = donor && `1/${donor.life}/${donor.id}`;
    if (this.phase === 'terminal') return;
    if (this.phase !== 'forage' &&
        (receiverIdentity !== this.receiverIdentity ||
         (!['collect', 'hold'].includes(this.phase) && !donorDead && donorIdentity !== this.donorIdentity))) {
      this.finish(receiver ? 'donor_or_peer_lost' : 'receiver_lost'); return;
    }
    if (this.phase === 'forage') {
      if (!receiver || !donor) return;
      if (receiver.segments > 42) { this.finish('pre_feed_overshoot'); return; }
      if (receiver.segments < 38) return;
      if (donor.segments > 8) { this.finish('donor_oversized'); return; }
      this.receiverIdentity = receiverIdentity; this.donorIdentity = donorIdentity;
      this.heading = null;
      this.orbitCenter = { x: receiver.x - Math.sin(receiver.heading) * ORBIT_RADIUS,
        y: receiver.y + Math.cos(receiver.heading) * ORBIT_RADIUS };
      this.lastMass = `${receiver.segments}/${receiver.fam}`;
      this.lastMassAt = now; this.started = now; this.phase = 'rendezvous';
    }
    if (receiver.segments > 45) { this.finish('receiver_overshoot'); return; }
    if (donor && !donorDead) this.lastDonor = donor;
    const mass = `${receiver.segments}/${receiver.fam}`;
    if (mass !== this.lastMass) {
      this.lastMass = mass; this.lastMassAt = now;
      if (this.phase === 'collect' || this.phase === 'hold') this.feedMassChanged = true;
    }
    if (this.phase === 'rendezvous') {
      if (donorDead) { this.finish('premature_donor_death'); return; }
      this.feedTarget = donor?.target ?? null;
      if (this.feedTarget && donor.peerVisible && receiver.peerVisible &&
          donor.receivedAgeMs <= 500 && donor.anchorAgeMs <= 500 &&
          receiver.receivedAgeMs <= 500 && receiver.anchorAgeMs <= 500 &&
          donor.segments <= 8 && donor.targetSource === 'local' &&
          this.feedTarget.distance <= TARGET_RADIUS &&
          this.feedTarget.fromHead >= 250 && donor.targetDanger <= .2) {
        this.phase = 'contact'; this.started = now;
      } else if (now - this.started >= RENDEZVOUS_MS) {
        this.finish('rendezvous_timeout');
      }
    }
    if (this.phase === 'contact') {
      this.feedTarget = donor?.target ?? null;
      if (donorDead) {
        if (!this.lastDonor) { this.finish('donor_death_location_unknown'); return; }
        this.deathSite = { x: this.lastDonor.x, y: this.lastDonor.y };
        this.deathAt = now; this.deathMass = receiver.segments + receiver.fam;
        this.phase = 'collect'; this.started = now; return;
      }
      if (!this.feedTarget || !donor.peerVisible || !receiver.peerVisible ||
          donor.receivedAgeMs > 500 || donor.anchorAgeMs > 500 ||
          receiver.receivedAgeMs > 500 || receiver.anchorAgeMs > 500 ||
          donor.targetSource !== 'local' ||
          donor.segments > 8 || this.feedTarget.distance > TARGET_RADIUS ||
          this.feedTarget.fromHead < 250 || donor.targetDanger > .2) {
        this.finish('contact_gate_lost'); return;
      }
      if (now - this.started >= CONTACT_MS) this.finish('contact_timeout');
    }
    if (this.phase === 'collect') {
      for (const id of receiver.rapidNearDeath ?? []) this.rapidIds.add(id);
      for (const id of receiver.collectedRapidIds ?? []) if (this.rapidIds.has(id))
        this.collectedRapidIds.add(id);
      // A mass increase alone can be ambient food. Require a newly reported rapid
      // pellet near the corpse to disappear while the receiver is nearby as well.
      if (receiver.segments + receiver.fam > this.deathMass + 1e-5 &&
          receiver.receivedAgeMs <= 500 && receiver.anchorAgeMs <= 500 &&
          this.collectedRapidIds.size &&
          Math.hypot(receiver.x - this.deathSite.x, receiver.y - this.deathSite.y) < 300) {
        this.heading = receiver.heading; this.phase = 'hold'; this.started = now;
        this.lastMassAt = now; this.interruptions = 0; return;
      }
      if (now - this.started >= COLLECTION_MS) this.finish('collection_unconfirmed');
    }
    if (this.phase === 'hold') {
      if (receiver.interruption) this.interruptions++;
      const g = receiver.geometry;
      if (this.feedMassChanged && now - this.lastMassAt >= STABLE_MS &&
          receiver.receivedAgeMs <= 500 && receiver.anchorAgeMs <= 500 &&
          Number.isFinite(receiver.fam) && receiver.fam >= 0 && receiver.fam < 1 &&
          receiver.segments >= 40 &&
          receiver.segments <= 45 && g && g.medianSpacing >= 10 &&
          g.medianSpacing <= 12 && g.distantPairs <= 30 && g.pathRatio >= .9 &&
          this.interruptions === 0) this.finish('straight_compressed_observed');
      else if (now - this.started >= HOLD_MS) this.finish('stable_shape_not_observed');
    }
  }
}

export class FeedingBot extends Bot {
  receive(bytes) {
    const world = this.world, addFood = world.addFood, packet = world.packet;
    if (this.foodEaterWorld !== world) {
      this.foodEaterWorld = world; this.lastFoodEater = null;
      this.recentRapid = new Map(); this.receiverEatenRapid = new Set();
    }
    world.addFood = function (...args) {
      addFood.apply(this, args);
      if (bot.index !== 0) return;
      const [sx, sy, rx, ry] = args;
      const id = sx << 24 | sy << 16 | rx << 8 | ry;
      bot.recentRapid.delete(id); bot.receiverEatenRapid.delete(id);
      if (args[5]) bot.recentRapid.set(id,
        { ...this.foods.get(id), at: performance.now() });
    };
    const bot = this;
    world.packet = (p, now) => {
      const code = String.fromCharCode(p[0]);
      if (code === '<' && p.length >= 5)
        this.lastFoodEater = p.at(-2) << 8 | p.at(-1);
      let eatenId = null;
      if (code === '<' || code === 'C') {
        const reuse = code === '<' ? p.length === 5 : p.length === 3;
        const sx = reuse ? world.lfe[0] : p[1], sy = reuse ? world.lfe[1] : p[2];
        const m = reuse ? 1 : 3;
        eatenId = sx << 24 | sy << 16 | p[m] << 8 | p[m + 1];
      }
      const existed = eatenId !== null && world.foods.has(eatenId);
      const result = packet.call(world, p, now);
      if (this.index === 0 && existed && !world.foods.has(eatenId) &&
          this.lastFoodEater === world.self && this.recentRapid.has(eatenId))
        this.receiverEatenRapid.add(eatenId);
      return result;
    };
    try { super.receive(bytes); }
    finally { world.addFood = addFood; world.packet = packet; }
  }
  movement(now, me, choice) {
    const state = this.feeding;
    const head = this.world.head(me, now);
    const receiver = this.peers[0];
    const receiverMe = receiver.world.snakes.get(receiver.world.self);
    let desired = null;
    if (this.index === 0 && ['rendezvous', 'contact'].includes(state.phase)) {
      const dx = head.x - state.orbitCenter.x, dy = head.y - state.orbitCenter.y;
      const radius = Math.hypot(dx, dy);
      desired = Math.atan2(dy, dx) + Math.PI / 2 +
        Math.atan2(radius - ORBIT_RADIUS, ORBIT_RADIUS);
    }
    if (this.index === 0 && state.phase === 'collect') {
      const food = [...this.world.foods.entries()]
        .filter(([id, f]) => this.recentRapid?.has(id) &&
          Math.hypot(f.x - state.deathSite.x, f.y - state.deathSite.y) < 250)
        .map(([, f]) => f).sort((a, b) =>
          Math.hypot(a.x - head.x, a.y - head.y) - Math.hypot(b.x - head.x, b.y - head.y))[0];
      const point = food ?? state.deathSite;
      desired = Math.atan2(point.y - head.y, point.x - head.x);
    }
    if (this.index === 0 && state.phase === 'hold') desired = state.heading;
    if (this.index === 1) {
      if (state.phase === 'forage') desired = this.cruiseHeading ??= me.ang;
      else if (['rendezvous', 'contact'].includes(state.phase)) {
        const target = this.feedSnapshot?.target;
        if (target) {
          desired = Math.atan2(target.y - head.y, target.x - head.x);
        }
      }
    }
    let danger = null, interruption = null;
    if (desired !== null && state.phase !== 'terminal') {
      const scale = Math.min(6, 1 + (me.sct - 2) / 106);
      // Only the donor may disregard the named receiver body during contact.
      // All other bodies and the arena rim remain in the danger calculation.
      let dangerWorld = this.world;
      if (this.index === 1 && state.phase === 'contact' && this.feedSnapshot?.peerVisible &&
          this.feedSnapshot?.target?.snakeId === receiverMe?.id &&
          this.feedSnapshot.target.distance <= TARGET_RADIUS &&
          this.feedSnapshot.target.fromHead >= 250) {
        dangerWorld = Object.create(this.world);
        dangerWorld.snakes = new Map([...this.world.snakes].filter(([id]) => id !== receiverMe.id));
      }
      danger = headingDanger(dangerWorld, now, me, head, desired, scale);
      if (danger > .2) interruption = 'avoidance';
      else choice.a = desired;
    }
    this.movementIntent = { intendedHeading: desired, intendedDanger: danger,
      interruption, desiredBoost: false };
    const sampled = `${this.lives}/${state.phase}/${state.reason}`;
    if (sampled !== this.lastFeedingPhase || now - (this.lastFeedingSample ?? -Infinity) >= 200) {
      this.lastFeedingPhase = sampled; this.lastFeedingSample = now;
      this.rec.meta({ event: 'feeding-shape-state', botIndex: this.index, life: this.lives,
        serverClock: this.serverClock, selfId: me.id, phase: state.phase,
        reason: state.reason, receiverIdentity: state.receiverIdentity,
        donorIdentity: state.donorIdentity, frozenHeading: state.heading,
        segments: me.sct, fam: me.fam, speed: me.sp,
        geometry: this.feedSnapshot?.geometry ?? null,
        target: this.feedSnapshot?.target ?? null,
        targetSource: this.feedSnapshot?.targetSource ?? null,
        peerVisible: this.feedSnapshot?.peerVisible ?? false,
        deathSite: state.deathSite, rapidNearDeath: state.rapidIds.size,
        collectedRapidNearDeath: state.collectedRapidIds.size,
        lastMassAgeMs: state.lastMassAt === null ? null : now - state.lastMassAt,
        interruption, intendedHeading: desired, danger,
        requestedHeading: choice.a });
    }
  }
}

export function feedingSnapshot(bot, peer, now) {
  const me = bot.world.snakes.get(bot.world.self);
  if (!me || bot.world.dead || bot.ws?.readyState !== 1) return null;
  const head = bot.world.head(me, now);
  const peerMe = peer.world.snakes.get(peer.world.self);
  const localPeer = peerMe ? bot.world.snakes.get(peerMe.id) : null;
  const peerVisible = peer.ws?.readyState === 1 && !peer.world.dead && !!peerMe &&
    !!localPeer?.pts.length && now - localPeer.t <= 500;
  let target = null, targetDanger = null, targetSource = null;
  if (bot.index === 1 && peerMe && now - peer.lastReceivedAt <= 500 &&
      now - peerMe.t <= 500) {
    const receiverHead = peerVisible ? bot.world.head(localPeer, now) :
      peer.world.head(peerMe, now);
    target = rearBodyTarget(peerVisible ? bot.world : peer.world,
      peerMe.id, head, receiverHead);
    targetSource = target ? peerVisible ? 'local' : 'shared' : null;
    if (target) {
      const a = Math.atan2(target.y - head.y, target.x - head.x);
      const safeWorld = Object.create(bot.world);
      safeWorld.snakes = new Map([...bot.world.snakes].filter(([id]) => id !== peerMe.id));
      targetDanger = headingDanger(safeWorld, now, me, head, a,
        Math.min(6, 1 + (me.sct - 2) / 106));
    }
  }
  const geometry = rawGeometry(me.pts);
  const death = bot.feeding?.deathSite;
  const rapidNearDeath = [], collectedRapidIds = [];
  if (bot.index === 0 && death) {
    for (const [id, food] of bot.recentRapid ?? []) {
      if (now - food.at > COLLECTION_MS ||
          Math.abs(food.at - bot.feeding.deathAt) > 500 || food.size < 10 ||
          Math.hypot(food.x - death.x, food.y - death.y) >= 100) continue;
      rapidNearDeath.push(id);
      if (bot.receiverEatenRapid?.has(id))
        collectedRapidIds.push(id);
    }
  }
  bot.feedSnapshot = { geometry, target, targetSource, peerVisible };
  return { life: bot.lives, id: me.id, segments: me.sct, fam: me.fam,
    heading: me.ang, speed: me.sp, geometry, target, targetDanger,
    targetSource, peerVisible, rapidNearDeath, collectedRapidIds,
    receivedAgeMs: now - bot.lastReceivedAt, anchorAgeMs: now - me.t,
    interruption: bot.movementIntent?.interruption ?? null,
    x: head.x, y: head.y };
}

export async function recordFeedingShape({ out, server, seconds = 480, plan, origin }, signal) {
  if (signal?.aborted) return { outcome: null, bots: [] };
  if (!out || !server || !Number.isFinite(seconds) || seconds < 1 || seconds > 480)
    throw Error('use --out, --server ip:port and 1..480 seconds');
  origin = validateOrigin(origin);
  if (fs.existsSync(out)) throw Error(`refusing to overwrite ${out}`);
  const [ip, portText] = server.split(':');
  const port = Number(portText);
  if (!ip || !Number.isInteger(port) || port < 1 || port > 65535) throw Error('invalid server');
  const batch = createCaptureBatch({ seconds, server, scenario: 'feeding-shape', serverBody: true,
    plan, origin }, 2, COLLECTOR_SOURCE);
  const state = new FeedingState(), bots = [], recorders = [];
  let timer, deadline, onAbort, failure = null, completed = false, donorStopped = false;
  const collectorTiming = { targetTickMs: 25, ticks: 0, lateTicks: 0, maxTickGapMs: 0 };
  let lastTick = performance.now();
  try {
    fs.mkdirSync(out, { recursive: true });
    for (const index of [0, 1]) {
      if (signal?.aborted) break;
      const nick = `Feeding shape ${index}`;
      const rec = new Recorder(path.join(out, `bot${index}.jsonl.gz`), {
        schema: 1, server: { ip, port }, nick, scenario: 'feeding-shape',
        botIndex: index, wall: Date.now(), t: performance.now(), captureProvenance: batch.provenance(index) });
      recorders.push(rec);
      bots.push(new FeedingBot({ scenario: 'movement', nick, serverBody: true, origin }, index, rec));
    }
    for (const bot of bots) {
      if (signal?.aborted) break;
      bot.feeding = state; bot.peers = bots; bot.connect({ ip, port });
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
          const receiver = feedingSnapshot(bots[0], bots[1], now);
          const donor = feedingSnapshot(bots[1], bots[0], now);
          state.update(now, receiver, donor, bots[1].world.dead);
          if (state.phase === 'collect' && !donorStopped) {
            bots[1].stop(); donorStopped = true;
          }
          for (const bot of bots) if (!bot.stopped) bot.tick(now);
          if (state.phase === 'terminal') { resolve(); return; }
          if (bots[0].stopped || (!donorStopped && bots[1].stopped))
            reject(Error('client exhausted reconnect attempts'));
        } catch (error) { reject(error); }
      }, 25);
    });
    if (signal?.aborted) return { outcome: state.reason,
      bots: bots.map(bot => ({ joins: bot.joins, maxSegments: bot.maxSegments })) };
    if (bots.some(bot => bot.joins === 0)) throw Error('a client never joined');
    if (state.phase !== 'terminal') state.finish('capture_deadline_null');
    completed = true;
  } catch (error) { failure = error; }
  finally {
    clearInterval(timer); clearTimeout(deadline);
    signal?.removeEventListener('abort', onAbort);
    await finalizeCaptureBatch({ bots, recorders, source: COLLECTOR_SOURCE, completed,
      signal, error: failure, metadata: { outcome: state.reason, collectorTiming, requestedSeconds: seconds } });
  }
  return { outcome: state.reason, bots: bots.map(bot => ({ joins: bot.joins,
    maxSegments: bot.maxSegments })) };
}

export function parseArgs(argv) {
  const args = {}, allowed = new Set(['--out', '--server', '--seconds', '--plan', '--origin']);
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i], value = argv[i + 1];
    if (!allowed.has(key) || !value || value.startsWith('--') || key in args)
      throw Error(`invalid or repeated option: ${key}`);
    args[key] = value;
  }
  return { out: args['--out'], server: args['--server'],
    ...(args['--origin'] ? { origin: validateOrigin(args['--origin']) } : {}),
    seconds: Number(args['--seconds'] ?? 480), ...(args['--plan'] ? { plan: args['--plan'] } : {}) };
}

if (process.argv[1]?.endsWith('slither-feeding-shape.mjs')) {
  runCollectorCli(signal => recordFeedingShape(parseArgs(process.argv.slice(2)), signal));
}
