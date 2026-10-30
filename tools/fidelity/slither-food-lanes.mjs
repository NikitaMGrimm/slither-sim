// One bounded food-delivery contrast. The 75-unit lane offset is an acquisition
// control, not an assertion about the server's subscription implementation.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Bot, Recorder, captureSourceIdentity, createCaptureBatch,
  finalizeCaptureBatch, runCollectorCli, validateOrigin, headingDanger, parseLocalServer, validateLocalServer } from './slither-headless.mjs';

const blocks = ['A', 'B', 'B', 'A'];
export const FOOD_LANES = Object.freeze({
  scenario: 'food-lanes', seconds: 480, maxSeconds: 480, bots: 1,
  sectorSize: 300, laneOffset: 37.5, laneSeparation: 75, halfSpan: 650,
  endpointRadius: 55, endpointAnchorMaxAgeMs: 150,
  anchorMaxAgeMs: 500, frameGapMs: 600, crossTrackLimit: 180,
  rimMargin: 900, otherClearance: 800, acquireTimeoutMs: 90000,
  legMaxMs: 30000, transferMaxMs: 16000,
  slots: Object.freeze(blocks.flatMap((lane, block) => [0, 1].map(pass => Object.freeze({
    scheduledSlot: block * 2 + pass + 1, block, lane,
    direction: pass === 0 ? 'positive' : 'negative',
    split: block < 2 ? 'calibration' : 'holdout',
  })))),
});
export const FOOD_LANES_SOURCE = Object.freeze(captureSourceIdentity(import.meta.url));

export function foodLaneGeometry(head) {
  const cfg = FOOD_LANES;
  const cx = cfg.sectorSize * Math.floor(head.x / cfg.sectorSize) + cfg.sectorSize / 2;
  const yA = cfg.sectorSize * Math.floor(head.y / cfg.sectorSize) + cfg.laneOffset;
  const yB = yA + cfg.laneSeparation;
  return { cx, yA, yB,
    A: { west: { x: cx - cfg.halfSpan, y: yA }, east: { x: cx + cfg.halfSpan, y: yA } },
    B: { west: { x: cx - cfg.halfSpan, y: yB }, east: { x: cx + cfg.halfSpan, y: yB } } };
}

function pointSegmentDistance(p, a, b) {
  const dx = b.x - a.x, dy = b.y - a.y;
  const lengthSquared = dx * dx + dy * dy;
  const t = lengthSquared === 0 ? 0 : Math.max(0, Math.min(1,
    ((p.x - a.x) * dx + (p.y - a.y) * dy) / lengthSquared));
  return Math.hypot(p.x - a.x - t * dx, p.y - a.y - t * dy);
}

function segmentDistance(a, b, c, d) {
  const ux = b.x - a.x, uy = b.y - a.y, vx = d.x - c.x, vy = d.y - c.y;
  const denominator = ux * vy - uy * vx;
  if (denominator !== 0) {
    const dx = c.x - a.x, dy = c.y - a.y;
    const t = (dx * vy - dy * vx) / denominator;
    const s = (dx * uy - dy * ux) / denominator;
    if (t >= 0 && t <= 1 && s >= 0 && s <= 1) return 0;
  }
  return Math.min(pointSegmentDistance(a, c, d), pointSegmentDistance(b, c, d),
    pointSegmentDistance(c, a, b), pointSegmentDistance(d, a, b));
}

export function foodLaneSiteQuiet(world, now, geometry, head) {
  const { A, B } = geometry;
  const corridors = [[A.west, A.east], [B.west, B.east], [A.west, B.west], [head, A.west]];
  // Received bodies plus extrapolated tips must clear whole planned corridors;
  // this engineering margin cannot guarantee future or unreceived safety.
  // R7 witnesses: research/fidelity/2026-10-23-visibility-recheck/REPORT.md#exploratory-route-pilots-r7.
  return [...world.snakes.values()].filter(other => other.id !== world.self).every(other => {
    const points = [...other.pts, world.head(other, now)];
    for (let i = 1; i < points.length; i++) {
      if (!corridors.every(([a, b]) =>
        segmentDistance(points[i - 1], points[i], a, b) > FOOD_LANES.otherClearance)) return false;
    }
    return true;
  });
}

export class FoodLaneBot extends Bot {
  constructor(opts, index, rec, { onFinish = () => {}, now = performance.now() } = {}) {
    // Reuse the standard movement dispatch, quantized sends and avoidance path.
    // Only external options/header provenance identify the acquisition scenario.
    super({ ...opts, scenario: 'movement' }, index, rec);
    this.attemptStartedAt = now;
    this.onRouteFinish = onFinish;
    this.geometry = null;
    this.lanePhase = null;
    this.slotIndex = 0;
    this.routeStatus = 'active';
    this.routeReason = null;
    this.slotResults = FOOD_LANES.slots.map(slot => ({ ...slot, status: 'missing' }));
    this.controlSends = [];
    this.explicitBoostOff = false;
    this.connectedOnce = false;
    this.movementIntent = { intendedHeading: null, desiredBoost: false, interruption: null };
  }

  connect(server) {
    if (this.connectedOnce || this.routeStatus !== 'active') return;
    this.connectedOnce = true;
    super.connect(server);
    // Do not call the base close handler: it schedules another connection.
    this.ws.onclose = event => {
      if (this.stopped) return;
      this.rec.meta({ event: 'close', code: event.code, reason: event.reason, life: this.lives });
      this.finishRoute('socket_close', performance.now());
    };
    const reportSocketError = this.ws.onerror;
    this.ws.onerror = event => {
      if (this.stopped) return;
      reportSocketError(event);
      this.finishRoute('socket_error', performance.now());
    };
  }

  receive(bytes) {
    super.receive(bytes);
    if (this.world.dead || this.deathObservation) this.finishRoute('death', performance.now());
    else if (this.joins > 1) this.finishRoute('unexpected_setup', performance.now());
  }

  send(bytes) {
    const sentAt = super.send(bytes);
    if (sentAt !== undefined) this.controlSends.push({ sentAt, bytes: Array.from(bytes) });
    return sentAt;
  }

  snapshot(now, me = this.world.snakes.get(this.world.self)) {
    if (!me?.pts?.length) return null;
    return { anchor: { ...me.pts.at(-1) }, anchorAgeMs: now - me.t,
      head: this.world.head(me, now), headSource: 'last_anchor_extrapolation',
      ang: me.ang, physicalHeading: me.ang, wang: me.wang, sp: me.sp, sct: me.sct, fam: me.fam,
      radius: this.world.radius, grd: this.world.grd, serverClock: this.serverClock };
  }

  summary() {
    return { status: this.routeStatus, reason: this.routeReason, geometry: this.geometry,
      slots: this.slotResults.map(slot => ({ ...slot })),
      completedPasses: this.slotResults.filter(slot => slot.status === 'completed').length };
  }

  startPhase(kind, now, me) {
    const slot = FOOD_LANES.slots[this.slotIndex];
    const lane = this.geometry[slot.lane];
    const target = kind === 'pass' && slot.direction === 'positive' ? lane.east : lane.west;
    this.phase = kind === 'pass' ? `food_lane_pass_${slot.scheduledSlot}` : `food_lane_${kind}`;
    this.lanePhase = { kind, scheduledSlot: kind === 'pass' ? slot.scheduledSlot : null,
      nextScheduledSlot: slot.scheduledSlot, lane: slot.lane,
      split: kind === 'pass' ? slot.split : 'excluded',
      direction: kind === 'pass' ? slot.direction : null, target: { ...target },
      expectedStart: kind === 'pass' ? { ...(slot.direction === 'positive' ? lane.west : lane.east) } : null,
      startedAt: now, achievedStart: this.snapshot(now, me), maxCrossTrack: 0 };
    if (kind === 'pass') this.slotResults[this.slotIndex].status = 'active';
    this.rec.meta({ event: 'food-lane-phase', life: this.lives, ...this.lanePhase });
  }

  endPhase(reason, now, me) {
    if (!this.lanePhase) return;
    const phase = this.lanePhase;
    this.lanePhase = null;
    this.phase = null;
    const achievedEnd = this.snapshot(now, me);
    const completed = reason === 'endpoint_reached';
    const result = { ...phase, reason, interruption: completed ? null : reason, completed,
      endedAt: now, elapsedMs: now - phase.startedAt, achievedEnd,
      endpointDistance: achievedEnd ? Math.hypot(achievedEnd.anchor.x - phase.target.x,
        achievedEnd.anchor.y - phase.target.y) : null };
    if (phase.kind === 'pass') Object.assign(this.slotResults[this.slotIndex],
      { status: completed ? 'completed' : 'aborted', reason,
        achievedStart: phase.achievedStart, achievedEnd });
    this.rec.meta({ event: 'food-lane-phase-end', life: this.lives, ...result });
  }

  finishRoute(reason, now = performance.now()) {
    if (this.routeStatus !== 'active') return;
    this.endPhase(reason, now);
    this.routeStatus = reason === 'completed' ? 'completed' : 'aborted';
    this.routeReason = reason;
    this.rec.meta({ event: 'food-lane-route-end', life: this.lives, ...this.summary() });
    super.stop();
    this.onRouteFinish(this.summary());
  }

  stop() {
    if (this.routeStatus === 'active') this.finishRoute('capture_stop');
    else super.stop();
  }

  tick(now) {
    if (this.routeStatus !== 'active' || this.stopped) return;
    this.controlSends = [];
    if (this.world.dead || this.deathObservation) return this.finishRoute('death', now);
    if (!this.geometry && now - this.attemptStartedAt >= FOOD_LANES.acquireTimeoutMs)
      return this.finishRoute('acquire_timeout', now);
    if (this.lanePhase && now - this.lanePhase.startedAt >=
        (this.lanePhase.kind === 'transfer' ? FOOD_LANES.transferMaxMs : FOOD_LANES.legMaxMs))
      return this.finishRoute('phase_timeout', now);
    if (this.geometry && !this.world.snakes.get(this.world.self))
      return this.finishRoute('own_snake_unavailable', now);
    super.tick(now);
    if (!this.controlSnapshot) return;
    this.rec.meta({ event: 'food-lane-control', life: this.lives, controlAt: now,
      ...this.controlSnapshot, sentControls: this.controlSends,
      headingSent: this.controlSends.some(packet => packet.bytes.length === 1 && packet.bytes[0] <= 250),
      sentHeading: this.lastHeading, heldBoost: this.boost,
      explicitBoostOff: this.explicitBoostOff, routeStatus: this.routeStatus });
    this.controlSnapshot = null;
  }

  movement(now, me, choice) {
    const cfg = FOOD_LANES;
    const pose = this.snapshot(now, me), head = pose.head;
    this.movementIntent = { intendedHeading: choice.a, desiredBoost: false,
      interruption: null, intendedDanger: choice.danger };
    if (!this.explicitBoostOff && this.send([254]) !== undefined) this.explicitBoostOff = true;
    if (!this.geometry) {
      const candidate = foodLaneGeometry(pose.anchor);
      const sites = [candidate.A.west, candidate.A.east, candidate.B.west, candidate.B.east, head];
      const quiet = foodLaneSiteQuiet(this.world, now, candidate, head);
      const rimSafe = sites.every(site => this.world.radius -
        Math.hypot(site.x - this.world.grd, site.y - this.world.grd) >= cfg.rimMargin);
      const ready = pose.anchorAgeMs >= 0 && pose.anchorAgeMs <= cfg.endpointAnchorMaxAgeMs &&
        now - this.lastReceivedAt <= cfg.frameGapMs && quiet && rimSafe && choice.danger <= 0.2 &&
        this.world.sectorSize === cfg.sectorSize;
      this.controlSnapshot = { phase: 'acquire', scheduledSlot: null, split: 'excluded',
        ...pose, quiet, rimSafe, intendedHeading: choice.a, desiredBoost: false, interruption: null };
      if (!ready) return;
      this.geometry = candidate;
      this.rec.meta({ event: 'food-lane-site', life: this.lives, geometry: candidate,
        selectedAt: now, receivedPose: pose });
      this.startPhase('approach', now, me);
    }
    let phase = this.lanePhase;
    const rimGap = this.world.radius - Math.hypot(head.x - this.world.grd, head.y - this.world.grd);
    let targetRimGap = this.world.radius - Math.hypot(phase.target.x - this.world.grd, phase.target.y - this.world.grd);
    let reason = pose.anchorAgeMs < 0 || pose.anchorAgeMs > cfg.anchorMaxAgeMs ? 'stale_anchor' :
      now - this.lastReceivedAt > cfg.frameGapMs ? 'frame_gap' :
        now - phase.startedAt >= (phase.kind === 'transfer' ? cfg.transferMaxMs : cfg.legMaxMs) ? 'phase_timeout' :
          Math.min(rimGap, targetRimGap) < cfg.rimMargin ? 'rim_margin' : null;
    const initialHeading = Math.atan2(phase.target.y - head.y, phase.target.x - head.x);
    const scale = Math.min(6, 1 + (me.sct - 2) / 106);
    const initialDanger = headingDanger(this.world, now, me, head, initialHeading, scale);
    const initialCrossTrack = phase.kind === 'pass' ? Math.abs(head.y - phase.target.y) : null;
    if (initialCrossTrack !== null) phase.maxCrossTrack = Math.max(phase.maxCrossTrack, initialCrossTrack);
    reason ??= choice.danger > 0.2 || initialDanger > 0.2 ? 'avoidance' :
      initialCrossTrack !== null && initialCrossTrack > cfg.crossTrackLimit ? 'off_path' : null;
    const endpointDistance = Math.hypot(pose.anchor.x - phase.target.x, pose.anchor.y - phase.target.y);
    // Fresh received anchors certify arrivals; extrapolation never advances slots.
    if (!reason && endpointDistance <= cfg.endpointRadius && pose.anchorAgeMs <= cfg.endpointAnchorMaxAgeMs) {
      const previous = phase;
      this.endPhase('endpoint_reached', now, me);
      if (previous.kind === 'pass') {
        this.slotIndex++;
        if (this.slotIndex === cfg.slots.length) return this.finishRoute('completed', now);
      }
      const next = cfg.slots[this.slotIndex];
      this.startPhase(previous.kind === 'pass' && next.lane !== previous.lane ? 'transfer' : 'pass', now, me);
      phase = this.lanePhase;
    }
    const intendedHeading = Math.atan2(phase.target.y - head.y, phase.target.x - head.x);
    targetRimGap = this.world.radius - Math.hypot(phase.target.x - this.world.grd, phase.target.y - this.world.grd);
    const intendedDanger = headingDanger(this.world, now, me, head, intendedHeading, scale);
    const crossTrack = phase.kind === 'pass' ? Math.abs(head.y - phase.target.y) : null;
    if (crossTrack !== null) phase.maxCrossTrack = Math.max(phase.maxCrossTrack, crossTrack);
    reason ??= Math.min(rimGap, targetRimGap) < cfg.rimMargin ? 'rim_margin' :
      choice.danger > 0.2 || intendedDanger > 0.2 ? 'avoidance' :
      crossTrack !== null && crossTrack > cfg.crossTrackLimit ? 'off_path' : null;
    this.movementIntent = { intendedHeading, intendedDanger, desiredBoost: false, interruption: reason };
    this.controlSnapshot = { phase: phase.kind, scheduledSlot: phase.scheduledSlot,
      nextScheduledSlot: phase.nextScheduledSlot, lane: phase.lane, split: phase.split,
      direction: phase.direction, target: phase.target, expectedStart: phase.expectedStart,
      ...pose, intendedHeading, intendedDanger, crossTrack, rimGap, targetRimGap,
      endpointDistance: Math.hypot(pose.anchor.x - phase.target.x, pose.anchor.y - phase.target.y),
      headingError: Math.atan2(Math.sin(me.ang - intendedHeading), Math.cos(me.ang - intendedHeading)),
      interruption: reason, desiredBoost: false };
    if (reason) return this.finishRoute(reason, now);
    choice.a = intendedHeading;
  }
}

export function parseArgs(argv) {
  const opts = { out: null, server: null, seconds: 480, origin: null, plan: null,
    scenario: 'food-lanes', bots: 1, nick: '(Bot) Food lanes' };
  const seen = new Set();
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i]?.slice(2), value = argv[i + 1];
    if (!argv[i].startsWith('--') || !['out', 'server', 'seconds', 'origin', 'plan'].includes(key) ||
        value === undefined || value.startsWith('--') || seen.has(key)) throw new Error(`invalid option ${argv[i]}`);
    seen.add(key);
    opts[key] = key === 'seconds' ? Number(value) : value;
  }
  if (!opts.out || !opts.server) throw new Error('--out and --server ip:port are required');
  if (!Number.isFinite(opts.seconds) || opts.seconds <= 0 || opts.seconds > 480)
    throw new Error('--seconds must be positive and at most 480');
  parseLocalServer(opts.server);
  opts.origin = validateOrigin(opts.origin);
  return opts;
}

export async function recordFoodLanes(opts, server, signal, batch = createCaptureBatch(opts, 1, FOOD_LANES_SOURCE)) {
  if (signal?.aborted) return;
  validateLocalServer(server);
  if (!Number.isFinite(opts.seconds) || opts.seconds <= 0 || opts.seconds > 480 ||
      opts.bots !== 1 || opts.scenario !== 'food-lanes') throw new Error('invalid food-lanes options');
  const rec = new Recorder(path.join(opts.out, 'bot0.jsonl.gz'), {
    schema: 1, server, nick: opts.nick, scenario: 'food-lanes',
    wall: Date.now(), t: performance.now(), loginSelector: 31,
    ...(batch.dataOrigin ? { dataOrigin: batch.dataOrigin } : {}), captureProvenance: batch.provenance(0) });
  const bots = [];
  let finish, failure = null, timer, deadline, finalized = false;
  const done = new Promise(resolve => { finish = resolve; });
  const bot = new FoodLaneBot(opts, 0, rec, { onFinish: finish });
  bots.push(bot);
  const fail = error => {
    if (finalized) return;
    failure ??= error instanceof Error ? error : new Error(String(error));
    bot.finishRoute('capture_error');
    finish();
  };
  const interrupt = () => { bot.finishRoute('capture_cancelled'); finish(); };
  rec.failure.then(fail);
  try {
    rec.meta({ event: 'food-lane-plan', config: FOOD_LANES, internalControlScenario: 'movement',
      limits: 'one route, no reconnect/replacement; controls and receipts do not establish subscription geometry' });
    signal?.addEventListener('abort', interrupt, { once: true });
    if (signal?.aborted) interrupt();
    else {
      bot.connect(server);
      timer = setInterval(() => { try { bot.tick(performance.now()); } catch (error) { fail(error); } }, 25);
      deadline = setTimeout(() => bot.finishRoute('capture_deadline'), opts.seconds * 1000);
    }
    await done;
  } catch (error) { fail(error); }
  finally {
    clearInterval(timer);
    clearTimeout(deadline);
    signal?.removeEventListener('abort', interrupt);
  }
  // An aborted route is an observed scientific outcome, not broken gzip integrity.
  // Control/decoder/recorder exceptions and cancellation interrupt the capture.
  finalized = true;
  await finalizeCaptureBatch({ bots, source: FOOD_LANES_SOURCE, completed: !failure,
    signal, error: failure, metadata: { foodLaneRoute: bot.summary() }, drainDelayMs: 300 });
  return bot.summary();
}

export async function main(signal) {
  const opts = parseArgs(process.argv.slice(2));
  if (signal?.aborted) return;
  if (fs.existsSync(opts.out)) throw new Error(`refusing to overwrite ${opts.out}`);
  const batch = createCaptureBatch(opts, 1, FOOD_LANES_SOURCE);
  fs.mkdirSync(opts.out, { recursive: true });
  const [ip, port] = opts.server.split(':');
  return recordFoodLanes(opts, { ip, port: Number(port) }, signal, batch);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) runCollectorCli(main);
