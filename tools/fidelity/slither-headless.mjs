// Headless protocol-19 client for bounded calibration captures on a loopback host.
// Records every raw frame (including the server's elapsed-ms header, when present) plus outgoing controls, and plays
// with a simple forager so the capture contains natural motion, eating, boosting and deaths.
//
//   node tools/fidelity/slither-headless.mjs --out recordings/headless/run1 --seconds 60 --server 127.0.0.1:444
//       [--origin http://localhost:8080]
//       [--bots 1] [--scenario forage|boost|ownpellets|spawn|movement|preyprobe|boundaryprobe|visibilityprobe|speedprobe] [--nick "(Bot) Bottingson"]
//       [--speed-min-segments 6] (speedprobe acquisition threshold)
//       [--movement-min-segments 6] (first movement tier acquisition threshold)
import { isIP } from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { createHash, randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const COLLECTOR_PATH = fileURLToPath(import.meta.url);
const COLLECTOR_SHA256 = createHash('sha256').update(fs.readFileSync(COLLECTOR_PATH)).digest('hex');

export function captureSourceIdentity(driverUrl) {
  const driverPath = fileURLToPath(driverUrl);
  return { driverPath, collectorSha256: driverPath === COLLECTOR_PATH ? COLLECTOR_SHA256 :
    createHash('sha256').update(fs.readFileSync(driverPath)).digest('hex'),
    headlessSha256: COLLECTOR_SHA256 };
}

const COLLECTOR_SOURCE = captureSourceIdentity(import.meta.url);

export function createCaptureBatch(options, botCount, source = COLLECTOR_SOURCE) {
  if (!Number.isInteger(botCount) || botCount < 1 || botCount > 3)
    throw new Error('use 1-3 bots');
  const { out, plan, ...acquisitionOptions } = options;
  let planSha256 = null;
  let dataOrigin;
  if (plan) {
    const planBytes = fs.readFileSync(plan);
    if (planBytes.length > 262144) throw new Error('capture plan exceeds 256 KiB');
    dataOrigin = JSON.parse(planBytes.toString('utf8')).dataOrigin;
    if (dataOrigin !== undefined && dataOrigin !== 'local-simulator')
      throw new Error('capture plans require local-simulator dataOrigin');
    planSha256 = createHash('sha256').update(planBytes).digest('hex');
  }
  const captureBatchId = randomUUID();
  return { dataOrigin, provenance(botIndex) {
    if (!Number.isInteger(botIndex) || botIndex < 0 || botIndex >= botCount)
      throw new Error('invalid bot index');
    return { version: 1, captureBatchId, captureId: randomUUID(), botIndex, botCount,
      collectorSha256: source.collectorSha256, headlessSha256: source.headlessSha256,
      planSha256, nodeVersion: process.version, platform: process.platform, arch: process.arch,
      timeOrigin: performance.timeOrigin, options: { ...acquisitionOptions, bots: botCount } };
  } };
}

export async function finalizeCaptureBatch({ bots, recorders = bots.map(bot => bot.rec),
  source = COLLECTOR_SOURCE, completed, signal, error = null, metadata = {}, drainDelayMs = 0 }) {
  let failure = error ?? recorders.find(rec => rec.error)?.error ?? null;
  for (const bot of bots) {
    try { bot.stop(); } catch (stopError) { failure ??= stopError; }
  }
  if (drainDelayMs) await new Promise(resolve => setTimeout(resolve, drainDelayMs));
  failure ??= recorders.find(rec => rec.error)?.error ?? null;
  const digest = file => {
    try { return createHash('sha256').update(fs.readFileSync(file)).digest('hex'); }
    catch { return null; }
  };
  const collectorEndSha256 = digest(source.driverPath);
  const headlessEndSha256 = digest(COLLECTOR_PATH);
  for (const [index, rec] of recorders.entries()) {
    const bot = bots[index];
    try {
      rec.finish({ ...metadata,
        event: completed && !signal?.aborted && !failure ? 'capture-end' : 'capture-interrupted',
        ...(!completed || signal?.aborted || failure ?
          { reason: String(failure ?? signal?.reason ?? 'capture incomplete') } : {}),
        frames: bot?.frames ?? 0, joins: bot?.joins ?? 0,
        maxSegments: bot?.maxSegments ?? 0, failures: bot?.failures ?? 0,
        collectorEndSha256, headlessEndSha256,
        collectorSourcesUnchanged: collectorEndSha256 === source.collectorSha256 &&
          headlessEndSha256 === source.headlessSha256 });
    } catch (finishError) { failure ??= finishError; }
  }
  const closed = await Promise.allSettled(recorders.map(rec => Promise.resolve().then(() => rec.close())));
  for (const result of closed) if (result.status === 'rejected') failure ??= result.reason;
  if (failure) throw failure;
}

export function runCollectorCli(run) {
  const controller = new AbortController();
  const interrupt = signal => {
    if (controller.signal.aborted) return;
    process.exitCode = signal === 'SIGINT' ? 130 : 143;
    controller.abort(signal);
  };
  const onInterrupt = () => interrupt('SIGINT');
  const onTerminate = () => interrupt('SIGTERM');
  process.on('SIGINT', onInterrupt);
  process.on('SIGTERM', onTerminate);
  return Promise.resolve().then(() => run(controller.signal)).then(result => {
    console.log(controller.signal.aborted ? 'interrupted' : 'done', result);
  }).catch(error => {
    console.error(error);
    process.exitCode = 1;
  }).finally(() => {
    process.off('SIGINT', onInterrupt);
    process.off('SIGTERM', onTerminate);
  });
}

const TAU = Math.PI * 2;
export const DEATH_OBSERVATION_MS = 1000;
const MOVEMENT_TIERS = [{ name: 'small', minSegments: 6 }, { name: 'medium', minSegments: 15 }, { name: 'large', minSegments: 35 }];
const MOVEMENT_STEPS = [
  { name: 'straight_cruise', ms: 1200, steering: 'straight' },
  { name: 'left_cruise', ms: 2200, steering: 'left' },
  { name: 'right_cruise', ms: 2200, steering: 'right' },
  { name: 'reverse_left_cruise', ms: 1800, steering: 'reverse_left' },
  { name: 'reverse_right_cruise', ms: 1800, steering: 'reverse_right' },
  { name: 'straight_boost', ms: 850, steering: 'straight', boost: true },
  { name: 'left_boost', ms: 1100, steering: 'left', boost: true },
  { name: 'right_boost', ms: 1100, steering: 'right', boost: true },
  { name: 'recover_forage', ms: 7000, steering: 'forage' },
];
// Fixed before the controlled capture: cross the suspected 300-450-unit flight
// threshold, retreat without intentionally eating the prey, then repeat while
// boosting if mass permits. Other snakes and prey inputs are observational only.
export const PREY_PROBE = Object.freeze({
  acquireMin: 350, acquireMax: 1200, boostApproachStart: 700,
  boostApproachStop: 560, approachStop: 280,
  retreatDone: 680, hardStop: 250,
  acquireOtherClearance: 600, abandonOtherClearance: 350,
  targetMaxAgeMs: 2500, phaseMaxMs: 14000, boostRetreatMs: 1000,
  hypothesis: 'prey flight begins near a head at roughly 300-450 units, speed follows that snake speed, and decisions may be sampled on a roughly 338-ms prey clock',
  geometryHypothesis: 'compare max(abs(dx),abs(dy)) around 320 with Euclidean thresholds 300 and 415; natural approaches should span axis and diagonal bearings',
});
export const BOUNDARY_PROBE = Object.freeze({
  stageGap: 350, entryGap: 40, lossGap: 100, acquireAngle: 0.18, acquireTimeoutMs: 180000,
  approachFoodRange: 600, approachFoodCone: 0.7,
  shallowMs: 15000, outwardStepMs: 10000,
  boostShallowMs: 1000, boostOutwardStepMs: 1000,
  outwardAngles: Object.freeze([0.05, 0.10, 0.18, 0.35, 0.50, 0.60, 0.65, 0.70, 0.75, 0.85, 1.10]),
  boostMinSegments: 5, boostStartAngle: 0.35, otherClearance: 450,
  hypothesis: 'test raw server radius crossing at shallow and progressively outward headings; even lives boost from angle 0.35 when eligible; reverse tangent direction every two lives',
});
export const VISIBILITY_PROBE = Object.freeze({
  halfSpan: 650, endpointRadius: 55, crossTrackLimit: 180,
  rimMargin: 900, acquireOtherClearance: 800,
  anchorMaxAgeMs: 500, frameGapMs: 600, acquireTimeoutMs: 90000,
  legMaxMs: 30000, transferMaxMs: 16000, boostMaxMs: 8000,
  boostPrepareMinSegments: 5, boostPrepareMaxMs: 45000,
  reacquireCooldownMs: 1500,
  hypothesis: 'compare sector introduction and exit geometry at matched travel headings and notification phases on repeated axis and diagonal crossings',
});
const VISIBILITY_BLOCKS = Object.freeze([
  { orientation: 'axis', mode: 'cruise' },
  { orientation: 'diagonal', mode: 'cruise' },
  { orientation: 'axis', mode: 'boost' },
  { orientation: 'diagonal', mode: 'boost' },
]);
export const SPEED_PROBE = Object.freeze({
  minSegments: 6, settleMs: 4500, boostMs: 850, coastMs: 4500,
  betweenMs: 1500, corridorLength: 1500, rimMargin: 700,
  anchorMaxAgeMs: 500, frameGapMs: 600,
  hypothesis: 'compare fixed-heading anchor displacement before and after a brief boost; record growth for fixed-size and retained-cruise analyses',
});

// The '6' packet carries obfuscated JS that fills a 27-byte reply; the stock bundle then
// applies a hidden letter rotation (its `qff9x` helper) before sending it.
export function solveChallenge(raw) {
  if (typeof raw !== 'string' || !raw.length || raw.length > 512 || raw.length % 2)
    throw new Error('Invalid challenge size');
  let code = '', d = 0, k = 23, f = 0;
  for (let i = 0; i < raw.length; i++) {
    let b = raw.charCodeAt(i);
    if (!(/[a-z]/i).test(raw[i])) throw new Error('Invalid challenge encoding');
    if (b <= 96) b += 32;
    b = (b - 97 - k) % 26;
    if (b < 0) b += 26;
    if (b > 15) throw new Error('Invalid challenge nibble');
    d = d * 16 + b;
    k += 17;
    if (f === 1) { code += String.fromCharCode(d); f = d = 0; } else f++;
  }
  // Parse the recorded string-copy template; never evaluate server-provided JS.
  const prefix = 'var a = "', between = '";a+="';
  const suffix = '";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}';
  if (!code.startsWith(prefix) || code.slice(prefix.length + 5, prefix.length + 5 + between.length) !== between)
    throw new Error('Unsupported challenge syntax');
  const start = prefix.length + 5 + between.length;
  if (code.slice(start + 22) !== suffix) throw new Error('Unsupported challenge syntax');
  const literal = code.slice(prefix.length, prefix.length + 5) + code.slice(start, start + 22);
  if (literal.length !== 27 || [...literal].some(c => c < ' ' || c > '~' || c === '"' || c === '\\'))
    throw new Error('Unsupported challenge literal');
  const idba = Uint8Array.from(literal, c => c.charCodeAt(0));
  let r = 0;
  for (let i = 0; i < idba.length; i++) {
    let av = 0x41, v = idba[i];
    if (v >= 0x61) { av += 0x20; v -= 0x20; }
    v -= 0x41;
    if (i === 0) r = 3 + v;
    const v3 = (v + r) % 26;
    r += 4 + v;
    idba[i] = v3 + av;
  }
  return idba;
}

export function loginPacket(nick, skin = 0) {
  const name = Buffer.from(nick.slice(0, 24), 'latin1');
  // The local host's short login selects protocol 19 without the stock password.
  return Uint8Array.from([115, 31, 0, 0, skin, name.length, ...name, 0, 255]);
}

export function splitFrame(a) {
  if (a.length < 3) throw new Error('short timing frame');
  return splitPayload(a.subarray(2));
}

export function splitPayload(a) {
  if (!a.length) throw new Error('empty packet payload');
  let m = 0;
  if (a[m] >= 32) return [a.subarray(m)];
  const out = [];
  while (m < a.length) {
    let len;
    if (a[m] < 32) {
      if (m + 1 >= a.length) throw new Error('truncated packet length');
      len = a[m] << 8 | a[m + 1]; m += 2;
    } else { len = a[m] - 32; m++; }
    if (!len || m + len > a.length) throw new Error('invalid packet length');
    out.push(a.subarray(m, m + len));
    m += len;
  }
  return out;
}

const u16 = (p, m) => p[m] << 8 | p[m + 1];
const u24 = (p, m) => p[m] << 16 | p[m + 1] << 8 | p[m + 2];

/** Minimal client-side world state: enough to steer, not a renderer. */
export class World {
  constructor({ serverBody = false } = {}) {
    this.serverBody = serverBody;
    this.cst = .43;
    this.snakes = new Map();
    this.foods = new Map();
    this.prey = new Map();
    this.preyGenerations = new Map();
    this.self = null;
    this.msl = 42;
    this.sectorSize = 300;
    this.grd = 21600;
    this.radius = 21000;
    this.lfs = [0, 0];
    this.lfe = [0, 0];
    this.dead = false;
    this.rapid = []; // ids of boost/death pellets ('b') since the last read
  }
  addFood(sx, sy, rx, ry, rad, rapid = false) {
    const k = this.sectorSize / 256;
    const id = sx << 24 | sy << 16 | rx << 8 | ry;
    this.foods.set(id, { x: sx * this.sectorSize + rx * k, y: sy * this.sectorSize + ry * k, size: rad, sx, sy });
    if (rapid) this.rapid.push(id);
  }
  packet(p, now) {
    const c = String.fromCharCode(p[0]);
    const d = p.length - 1;
    if (c === 'a') {
      this.grd = u24(p, 1);
      this.sectorSize = u16(p, 6);
      if (this.serverBody && p.length > 22) this.cst = u16(p, 21) / 1000;
      if (p.length > 24) this.msl = p[24];
      this.radius = p.length > 29 ? u24(p, 27) : this.grd * 0.98;
    } else if (c === 'z') this.radius = u24(p, 1);
    else if (c === 's') this.snakePacket(p, now);
    else if (c === 'R' && p.length === 2) {
      const me = this.snakes.get(this.self);
      if (me) { me.rsc = p[1]; return true; }
    }
    else if ('gnGN+='.includes(c)) this.anchor(c, p, now);
    else if ('eE345d7'.includes(c)) this.rotation(c, p);
    else if (c === 'h') { const o = this.snakes.get(u16(p, 1)); if (o) o.fam = u24(p, 3) / 16777215; }
    else if (c === 'r') { const o = this.snakes.get(u16(p, 1)); if (o) { o.pts.shift(); o.sct--; if (d >= 4) o.fam = u24(p, 3) / 16777215; } }
    else if (c === 'F') { for (let m = 3; m + 4 <= p.length; m += 4) this.addFood(p[1], p[2], p[m + 1], p[m + 2], p[m + 3] / 5); }
    else if (c === 'b' || c === 'f') {
      let m = 1;
      if (d >= 5) { this.lfs = [p[1], p[2]]; m = 3; }
      const rx = p[m], ry = p[m + 1];
      m += 2;
      if (d === 4 || d === 6) m++;
      this.addFood(this.lfs[0], this.lfs[1], rx, ry, p[m] / 5, c === 'b');
    } else if (c === 'c' || c === 'C' || c === '<') {
      let m = 1;
      if ((c === 'c' && d === 2) || (c === '<' && d === 4) || (c === 'C' && d === 2)) { /* reuse last sector */ }
      else { this.lfe = [p[1], p[2]]; m = 3; }
      this.foods.delete(this.lfe[0] << 24 | this.lfe[1] << 16 | p[m] << 8 | p[m + 1]);
    } else if (c === 'w') {
      for (const [k, f] of this.foods) if (f.sx === p[1] && f.sy === p[2]) this.foods.delete(k);
    } else if (c === 'y') {
      const id = u16(p, 1);
      if (d === 2 || d === 4) this.prey.delete(id);
      else {
        const generation = (this.preyGenerations.get(id) ?? 0) + 1;
        this.preyGenerations.set(id, generation);
        this.prey.set(id, { id, generation, x: u24(p, 4) / 5, y: u24(p, 7) / 5,
          size: p[10] / 5, wang: u24(p, 12) * TAU / 16777215,
          ang: u24(p, 15) * TAU / 16777215, sp: u16(p, 18) / 1000, t: now });
      }
    } else if (c === 'j') {
      const prey = this.prey.get(u16(p, 1));
      if (prey) {
        prey.x = 1 + u16(p, 3) * 3;
        prey.y = 1 + u16(p, 5) * 3;
        prey.t = now;
        if (d === 15) { prey.ang = u24(p, 8) * TAU / 16777215; prey.wang = u24(p, 11) * TAU / 16777215; prey.sp = u16(p, 14) / 1000; }
        else if (d === 11) { prey.ang = u24(p, 7) * TAU / 16777215; prey.sp = u16(p, 10) / 1000; }
        else if (d === 12) { prey.wang = u24(p, 8) * TAU / 16777215; prey.sp = u16(p, 11) / 1000; }
        else if (d === 13) { prey.ang = u24(p, 8) * TAU / 16777215; prey.wang = u24(p, 11) * TAU / 16777215; }
        else if (d === 9) prey.ang = u24(p, 7) * TAU / 16777215;
        else if (d === 10) prey.wang = u24(p, 8) * TAU / 16777215;
        else if (d === 8) prey.sp = u16(p, 7) / 1000;
      }
    } else if (c === 'v') this.dead = true;
  }
  snakePacket(p, now) {
    const id = u16(p, 1);
    if (p.length - 1 <= 6) { this.snakes.delete(id); return; }
    let m = 3;
    const ang = u24(p, m) * TAU / 16777215; m += 4;
    const wang = u24(p, m) * TAU / 16777215; m += 3;
    const sp = u16(p, m) / 1000; m += 2;
    const fam = u24(p, m) / 16777215; m += 4;
    m += 6;
    m += 1 + p[m];
    m += 1 + p[m];
    m += 1;
    const pts = [];
    let x = 0, y = 0, iang = 0;
    while (m < p.length) {
      if (!pts.length) { x = u24(p, m) / 5; y = u24(p, m + 3) / 5; m += 6; }
      else if (m === p.length - 2) { iang = u16(p, m); m += 2; x += Math.cos(iang * TAU / 65536) * this.msl; y += Math.sin(iang * TAU / 65536) * this.msl; }
      else { x += (p[m] - 127) / 2; y += (p[m + 1] - 127) / 2; m += 2; }
      pts.push({ x, y });
    }
    const o = { id, ang, wang, sp, fam, pts, sct: pts.length, rsc: 0, iang, t: now };
    this.snakes.set(id, o);
    if (this.self === null) this.self = id;
  }
  anchor(c, p, now) {
    let m = 1, o;
    if ('GN'.includes(c) || (c === '=' && p.length === 7) || (c === '+' && p.length === 10)) o = this.snakes.get(this.self);
    else { o = this.snakes.get(u16(p, 1)); m = 3; }
    if (!o || !o.pts.length) return;
    if (this.serverBody) {
      // Snapshot-validated server order: contract before removal/growth/append.
      const k = o.pts.length - 3;
      for (let j = k - 1; j >= 0; j--) {
        const weight = this.cst * Math.min(k - j, 4) / 4;
        const point = o.pts[j], neighbor = o.pts[j + 1];
        point.x += (neighbor.x - point.x) * weight;
        point.y += (neighbor.y - point.y) * weight;
      }
    }
    const last = o.pts[o.pts.length - 1];
    let x, y;
    if (c === '+' || c === '=') {
      o.iang = u16(p, m); x = u16(p, m + 2); y = u16(p, m + 4); m += 6;
    } else {
      const d = p.length - 1;
      if ((c === 'G' && d === 2) || (c === 'N' && d === 5) || (c === 'g' && d === 4) || (c === 'n' && d === 7)) { o.iang = u16(p, m); m += 2; }
      x = last.x + Math.cos(o.iang * TAU / 65536) * this.msl;
      y = last.y + Math.sin(o.iang * TAU / 65536) * this.msl;
    }
    if ('nN+'.includes(c)) { o.sct++; o.fam = u24(p, m) / 16777215; } else o.pts.shift();
    o.pts.push({ x, y });
    o.t = now;
  }
  rotation(c, p) {
    const plen = p.length;
    let m = 1, o;
    if ('d7'.includes(c) || (plen - 1 <= 2 && 'eE345'.includes(c))) o = this.snakes.get(this.self);
    else { o = this.snakes.get(u16(p, 1)); m = 3; }
    if (!o) return;
    const a = v => v * TAU / 256;
    if (plen === 6) { o.ang = a(p[m]); o.wang = a(p[m + 1]); o.sp = p[m + 2] / 18; }
    else if (plen === 5 || plen === 3) {
      if (c === 'e') { o.ang = a(p[m]); o.sp = p[m + 1] / 18; }
      else if (c === 'E' || c === '4') { o.wang = a(p[m]); o.sp = p[m + 1] / 18; }
      else { o.ang = a(p[m]); o.wang = a(p[m + 1]); }
    } else if (plen === 4 || plen === 2) {
      if (c === 'e') o.ang = a(p[m]);
      else if (c === 'E' || c === '4') o.wang = a(p[m]);
      else if (c === '3') o.sp = p[m] / 18;
      else { o.ang = a(p[m]); o.wang = a(p[m + 1]); o.sp = p[m + 2] / 18; }
    }
  }
  /** Predicted head: last anchor advanced along the current angle for the elapsed time. */
  head(o, now) {
    const last = o.pts[o.pts.length - 1];
    const d = Math.min(o.sp / 32 * (now - o.t), this.msl * 1.5);
    return { x: last.x + Math.cos(o.ang) * d, y: last.y + Math.sin(o.ang) * d };
  }
}

export function headingDanger(world, now, me, h, a, sc) {
  const dx = Math.cos(a), dy = Math.sin(a);
  let danger = 0;
  for (const o of world.snakes.values()) {
    if (o.id === world.self) continue;
    const pts = o.pts.concat([world.head(o, now)]);
    for (const q of pts) {
      const qx = q.x - h.x, qy = q.y - h.y;
      const dist = Math.hypot(qx, qy);
      if (dist > 420) continue;
      const along = qx * dx + qy * dy;
      const side = Math.abs(qx * dy - qy * dx);
      const width = 30 * sc + 25 * Math.min(6, 1 + (o.sct - 2) / 106) + 30;
      if (along > -20 && side < width) danger = Math.max(danger, 1 - Math.max(0, along) / 420);
    }
  }
  const cx = h.x - world.grd, cy = h.y - world.grd;
  const edge = world.radius - Math.hypot(cx + dx * 400, cy + dy * 400);
  if (edge < 600) danger = Math.max(danger, 1 - Math.max(0, edge) / 600);
  return danger;
}

/** Candidate-heading forager: food attraction minus body/boundary danger along short rays. */
export function chooseHeading(world, now) {
  const me = world.snakes.get(world.self);
  if (!me) return null;
  const h = world.head(me, now);
  const sc = Math.min(6, 1 + (me.sct - 2) / 106);
  let best = null;
  for (let i = 0; i < 32; i++) {
    const a = i * TAU / 32;
    const dx = Math.cos(a), dy = Math.sin(a);
    let turn = Math.abs(Math.atan2(Math.sin(a - me.ang), Math.cos(a - me.ang)));
    // Food utility is size^2/(40+distance), usually well below one per pellet.
    // The former 20/radian penalty ignored even nearby death food in empty space.
    let score = -turn * 0.01;
    for (const f of world.foods.values()) {
      const fx = f.x - h.x, fy = f.y - h.y;
      const dist = Math.hypot(fx, fy);
      if (dist > 900) continue;
      const along = (fx * dx + fy * dy) / (dist || 1);
      if (along > 0.7) score += f.size * f.size * along / (40 + dist);
    }
    const danger = headingDanger(world, now, me, h, a, sc);
    score -= danger * 1e4;
    if (!best || score > best.score) best = { a, score, danger };
  }
  return best;
}

export function visibilityEndpoints(center, orientation, halfSpan = VISIBILITY_PROBE.halfSpan) {
  const direction = orientation === 'axis' ? { x: 1, y: 0 } :
    orientation === 'diagonal' ? { x: Math.SQRT1_2, y: Math.SQRT1_2 } : null;
  if (!direction) throw new Error(`unknown visibility orientation ${orientation}`);
  return {
    positive: { x: center.x + direction.x * halfSpan, y: center.y + direction.y * halfSpan },
    negative: { x: center.x - direction.x * halfSpan, y: center.y - direction.y * halfSpan },
  };
}

export class Bot {
  constructor(opts, index, recorder) {
    this.opts = opts;
    this.index = index;
    this.rec = recorder;
    this.world = new World({ serverBody: opts.serverBody });
    this.lastHeading = -1;
    this.lastHeadingAt = 0;
    this.boost = false;
    this.lastBoostAt = -1e9;
    this.lastPing = 0;
    this.awaitingPong = false;
    this.pendingPing = null;
    this.phaseStart = 0;
    this.phase = null;
    this.phaseIndex = 0;
    this.movementTier = 0;
    this.movementStep = 0;
    this.movementIntent = null;
    this.preyTarget = null;
    this.preyIntent = null;
    this.preyCycles = 0;
    this.preyCooldownUntil = 0;
    this.boundaryEntryAt = null;
    this.boundaryIntent = null;
    this.boundaryPhase = null;
    this.boundaryCensored = false;
    this.boundaryBoostInsufficientSeen = false;
    this.boundaryContact = false;
    this.boundaryStep = -1;
    this.boundaryDwellAt = null;
    this.visibility = null;
    this.visibilityIntent = null;
    this.visibilityAcquireSince = null;
    this.visibilityCooldownUntil = 0;
    this.visibilityRouteId = 0;
    this.visibilityNextBlock = 0;
    this.visibilityBoostPrepareSince = null;
    this.speedTrial = null;
    this.speedIntent = null;
    this.speedTrials = 0;
    this.speedCooldownUntil = 0;
    this.speedGrowthTarget = null;
    this.serverClock = null;
    this.rawFraming = false;
    this.deathObservation = null;
    this.own = new Map();
    this.lives = 0;
    this.failures = 0;
    this.frames = 0;
    this.connectionFrames = 0;
    this.joins = 0;
    this.maxSegments = 0;
  }
  connect(server) {
    if (this.stopped) return;
    validateLocalServer(server);
    this.world = new World({ serverBody: this.opts.serverBody });
    this.phase = null;
    this.movementTier = 0;
    this.movementStep = 0;
    this.movementIntent = null;
    this.preyTarget = null;
    this.preyIntent = null;
    this.preyCycles = 0;
    this.preyCooldownUntil = 0;
    this.boundaryEntryAt = null;
    this.boundaryIntent = null;
    this.boundaryPhase = null;
    this.boundaryCensored = false;
    this.boundaryBoostInsufficientSeen = false;
    this.boundaryContact = false;
    this.boundaryStep = -1;
    this.boundaryDwellAt = null;
    this.visibility = null;
    this.visibilityIntent = null;
    this.visibilityAcquireSince = null;
    this.visibilityCooldownUntil = 0;
    this.visibilityRouteId = 0;
    this.visibilityNextBlock = 0;
    this.visibilityBoostPrepareSince = null;
    this.speedTrial = null;
    this.speedIntent = null;
    this.speedTrials = 0;
    this.speedCooldownUntil = 0;
    this.speedGrowthTarget = null;
    this.serverClock = null;
    this.rawFraming = false;
    this.connectionFrames = 0;
    this.deathObservation = null;
    this.own.clear();
    this.lastHeading = -1;
    this.lastHeadingAt = 0;
    this.boost = false;
    this.lastBoostAt = -1e9;
    this.lastPing = 0;
    this.awaitingPong = false;
    this.pendingPing = null;
    this.connectedAt = this.lastReceivedAt = performance.now();
    this.lives++;
    const origin = validateOrigin(this.opts.origin);
    const ws = new WebSocket(`ws://${server.ip}:${server.port}/slither`,
      origin ? { headers: { Origin: origin } } : undefined);
    ws.binaryType = 'arraybuffer';
    this.ws = ws;
    this.rec.meta({ event: 'connect', server, life: this.lives });
    ws.onopen = () => {
      if (this.stopped) return;
      this.rec.meta({ event: 'open', server, life: this.lives,
        connectDurationMs: performance.now() - this.connectedAt });
      this.send([99, 0]);
    };
    ws.onmessage = e => {
      if (this.stopped) return;
      try { this.receive(new Uint8Array(e.data)); }
      catch (error) {
        this.stopped = true;
        this.rec.meta({ event: 'receive-error', life: this.lives, message: String(error) });
        // Preserve the offending raw frame and drain siblings with interrupted receipts.
        this.rec.reportFailure(error instanceof Error ? error : new Error(String(error)));
      }
    };
    ws.onclose = e => {
      if (this.stopped) return;
      if (this.opts.scenario === 'visibilityprobe') this.endVisibilityLeg(performance.now(), 'socket_close');
      if (this.opts.scenario === 'speedprobe') this.endSpeedTrial(performance.now(), 'socket_close');
      this.finishDeathObservation(this.deathObservation?.closeRequestedAt !== undefined ? 'deadline' : 'socket-close');
      if (this.pendingPing) this.rec.meta({ event: 'ping-censored', life: this.lives,
        sentAt: this.pendingPing.t, waitedMs: performance.now() - this.pendingPing.t });
      this.pendingPing = null;
      this.rec.meta({ event: 'close', code: e.code, reason: e.reason, life: this.lives });
      this.ws = null;
      if (performance.now() - this.connectedAt < 10000) this.failures++;
      else this.failures = 0;
      if (!this.stopped && this.failures < 3) {
        this.reconnectTimer = setTimeout(() => this.connect(server), 3000 * 2 ** this.failures);
      } else if (!this.stopped) {
        this.rec.meta({ event: 'connection-failed', failures: this.failures });
        this.stopped = true;
      }
    };
    ws.onerror = event => {
      if (this.stopped) return;
      let message = event.message;
      if (typeof message !== 'string' || !message) message = event.error?.message;
      this.rec.meta({ event: 'socket-error', life: this.lives,
        message: typeof message === 'string' ? message.slice(0, 512) : null });
    };
  }
  send(bytes) {
    if (!this.ws || this.ws.readyState !== 1 ||
        this.deathObservation && !(bytes.length === 1 && bytes[0] === 251)) return;
    const a = Uint8Array.from(bytes);
    const sentAt = performance.now();
    this.ws.send(a);
    this.rec.frame('o', a, sentAt);
    return sentAt;
  }
  receive(a) {
    const now = performance.now();
    this.lastReceivedAt = now;
    this.frames++;
    this.connectionFrames++;
    this.rec.frame('i', a, now);
    // Some community servers send the initial challenge without a timing header.
    if (this.connectionFrames === 1 && a[0] === 54) {
      this.rawFraming = true;
      this.rec.meta({ event: 'unframed-challenge' });
    }
    const packets = this.rawFraming ? splitPayload(a) : splitFrame(a);
    if (!this.rawFraming) this.serverClock = packets.some(p => p[0] === 97) ? 0 : (this.serverClock ?? 0) + (a[0] << 8 | a[1]);
    for (const p of packets) {
      const c = String.fromCharCode(p[0]);
      if (c === '6') {
        this.send(solveChallenge(Buffer.from(p.subarray(1)).toString('latin1')));
        this.send(loginPacket(this.opts.nick));
      } else if (c === 'p') {
        if (this.pendingPing) this.rec.meta({ event: 'ping-rtt', life: this.lives,
          sentAt: this.pendingPing.t, receivedAt: now, rttMs: now - this.pendingPing.t });
        else this.rec.meta({ event: 'ping-unmatched', life: this.lives });
        this.pendingPing = null;
        this.awaitingPong = false;
      }
      const applied = this.world.packet(p, now);
      if (c === 'R' && applied) this.rec.meta({ event: 'rsc', serverClock: this.serverClock,
        value: p[1], snakeId: this.world.self, life: this.lives });
      if (c === 'a') this.joins++;
      if (this.opts.scenario === 'preyprobe' && (c === 'y' || c === 'j') && this.preyTarget?.id === u16(p, 1)) {
        const prey = this.world.prey.get(this.preyTarget.id);
        this.rec.meta({ event: 'prey-target-packet', serverClock: this.serverClock,
          packet: Buffer.from(p).toString('hex'), targetId: this.preyTarget.id,
          generation: this.preyTarget.generation, phase: this.phase,
          prey: prey ? { x: prey.x, y: prey.y, sp: prey.sp, wang: prey.wang } : null });
      }
    }
    const w = this.world, me = w.snakes.get(w.self);
    if (me) this.maxSegments = Math.max(this.maxSegments, me.sct);
    if (this.phase === 'boostcircle' && me && me.pts.length) {
      const tail = me.pts[0];
      for (const id of w.rapid) {
        const f = w.foods.get(id);
        if (f && Math.hypot(f.x - tail.x, f.y - tail.y) < 90) this.own.set(id, f);
      }
    }
    w.rapid.length = 0;
    if (this.world.dead && this.ws && !this.deathObservation) {
      if (this.opts.scenario === 'visibilityprobe') this.endVisibilityLeg(now, 'death');
      if (this.opts.scenario === 'speedprobe') this.endSpeedTrial(now, 'death');
      this.deathObservation = { startedAt: now, serverClock: this.serverClock,
        frame: this.frames, ended: false };
      this.rec.meta({ event: 'death-observation-start', life: this.lives,
        serverClock: this.serverClock, durationMs: DEATH_OBSERVATION_MS });
    }
  }
  finishDeathObservation(reason, now = performance.now()) {
    const observation = this.deathObservation;
    if (!observation || observation.ended) return;
    observation.ended = true;
    this.rec.meta({ event: 'death-observation-end', life: this.lives, reason,
      elapsedMs: now - observation.startedAt,
      serverElapsedMs: this.serverClock === null || observation.serverClock === null ? null :
        this.serverClock - observation.serverClock,
      framesAfterDeath: this.frames - observation.frame });
  }
  ping(now) {
    if (!this.awaitingPong && now - this.lastPing > 250) {
      const sentAt = this.send([251]);
      if (sentAt !== undefined) {
        this.lastPing = sentAt;
        this.awaitingPong = true;
        this.pendingPing = { t: sentAt };
        this.rec.meta({ event: 'ping-sent', life: this.lives, sentAt });
      }
    }
  }
  tick(now) {
    if (this.stopped) return;
    // Keep recording after game-over, without controls or artificial physics delay.
    // A server close or the capture deadline may censor this observation window.
    if (this.deathObservation) {
      if (!this.deathObservation.ended && this.deathObservation.closeRequestedAt === undefined &&
          now >= this.deathObservation.startedAt + DEATH_OBSERVATION_MS) {
        this.deathObservation.closeRequestedAt = now;
        this.rec.meta({ event: 'death-observation-close-requested', life: this.lives });
        this.ws?.close();
      } else if (!this.deathObservation.ended && this.deathObservation.closeRequestedAt === undefined) {
        this.ping(now);
      }
      return;
    }
    if (this.opts.scenario === 'spawn' && this.ws?.readyState === 1 && this.world.self !== null && now - this.connectedAt > 15000) {
      this.rec.meta({ event: 'spawn-sample-complete' });
      this.ws.close();
      return;
    }
    if (this.ws && now - this.lastReceivedAt > 15000) {
      this.rec.meta({ event: 'receive-timeout' });
      this.lastReceivedAt = now;
      this.ws.close();
      return;
    }
    const w = this.world;
    const me = w.snakes.get(w.self);
    if (!this.ws || !me) return;
    if (this.opts.scenario === 'boundaryprobe' && !this.boundaryCensored &&
        this.boundaryEntryAt === null && now - this.connectedAt >= BOUNDARY_PROBE.acquireTimeoutMs) {
      this.boundaryCensored = true;
      this.rec.meta({ event: 'boundary-censored', reason: 'acquire_timeout', life: this.lives,
        serverClock: this.serverClock, gap: w.radius - Math.hypot(me.pts.at(-1).x - w.grd, me.pts.at(-1).y - w.grd),
        sct: me.sct, fam: me.fam });
      this.ws.close();
      return;
    }
    if (this.boundaryCensored) return;
    this.ping(now);
    const choice = chooseHeading(w, now);
    if (!choice) return;
    if (this.opts.scenario === 'ownpellets') this.ownPellets(now, me, choice);
    if (this.opts.scenario === 'movement') this.movement(now, me, choice);
    if (this.opts.scenario === 'preyprobe') this.probePrey(now, me, choice);
    if (this.opts.scenario === 'boundaryprobe') this.probeBoundary(now, me, choice);
    if (this.opts.scenario === 'visibilityprobe') this.probeVisibility(now, me, choice);
    if (this.opts.scenario === 'speedprobe') this.probeSpeed(now, me, choice);
    const sang = Math.floor(251 * (((choice.a % TAU) + TAU) % TAU) / TAU);
    const headingSent = sang !== this.lastHeading && now - this.lastHeadingAt > 33 &&
      (this.opts.scenario !== 'visibilityprobe' || this.ws?.readyState === 1);
    if (headingSent) {
      this.lastHeading = sang;
      this.lastHeadingAt = now;
      this.send([sang]);
    }
    const boostBefore = this.boost;
    this.setBoost(this.wantBoost(now, me, choice), now);
    if (this.opts.scenario === 'movement') this.rec.meta({
      event: 'movement-control', tier: MOVEMENT_TIERS[this.movementTier]?.name ?? 'complete',
      phase: this.phase ?? 'forage_wait', intendedHeading: this.movementIntent.intendedHeading,
      requestedHeading: sang, sentHeading: this.lastHeading,
      desiredBoost: this.movementIntent.desiredBoost, heldBoost: this.boost,
      interruption: this.movementIntent.interruption, intendedDanger: this.movementIntent.intendedDanger,
      sct: me.sct, fam: me.fam, ang: me.ang, wang: me.wang, sp: me.sp,
    });
    if (this.opts.scenario === 'preyprobe') this.rec.meta({ event: 'prey-control',
      phase: this.phase ?? 'forage_wait', targetId: this.preyTarget?.id ?? null,
      generation: this.preyTarget?.generation ?? null,
      targetDistance: this.preyIntent?.distance ?? null,
      targetBoxDistance: this.preyIntent?.boxDistance ?? null,
      targetBearing: this.preyIntent?.bearing ?? null,
      nearestOther: this.preyIntent?.nearestOther ?? null,
      otherHeadsWithin450: this.preyIntent?.otherHeadsWithin450 ?? null,
      otherHeadsWithin800: this.preyIntent?.otherHeadsWithin800 ?? null,
      targetSpeed: this.preyIntent?.targetSpeed ?? null,
      targetAgeMs: this.preyIntent?.targetAgeMs ?? null,
      intendedHeading: this.preyIntent?.intendedHeading ?? null,
      requestedHeading: sang, sentHeading: this.lastHeading,
      desiredBoost: this.preyIntent?.desiredBoost ?? false, heldBoost: this.boost,
      interruption: this.preyIntent?.interruption ?? null,
      sct: me.sct, fam: me.fam, ang: me.ang, wang: me.wang, sp: me.sp });
    if (this.opts.scenario === 'boundaryprobe') this.rec.meta({ event: 'boundary-control',
      life: this.lives, serverClock: this.serverClock,
      ...this.boundaryIntent, requestedHeading: choice.a, encodedHeading: sang,
      sentHeading: this.lastHeading, desiredBoost: this.boundaryIntent.desiredBoost,
      heldBoost: this.boost, x: me.pts.at(-1).x, y: me.pts.at(-1).y,
      anchorAgeMs: now - me.t,
      ang: me.ang, wang: me.wang, sp: me.sp, sct: me.sct, fam: me.fam,
      radius: w.radius, grd: w.grd });
    if (this.opts.scenario === 'visibilityprobe') this.rec.meta({ event: 'visibility-control',
      life: this.lives, routeId: this.visibility?.id ?? null, serverClock: this.serverClock,
      ...this.visibilityIntent, encodedHeading: sang, sentHeading: this.lastHeading,
      headingSent, heldBoost: this.boost, boostSent: this.boost !== boostBefore && this.ws?.readyState === 1,
      x: w.head(me, now).x, y: w.head(me, now).y,
      anchor: me.pts.at(-1), anchorAgeMs: now - me.t,
      headSource: 'last_anchor_extrapolation',
      ang: me.ang, wang: me.wang, sp: me.sp, sct: me.sct, fam: me.fam,
      radius: w.radius, grd: w.grd });
    if (this.opts.scenario === 'speedprobe') this.rec.meta({ event: 'speed-control',
      life: this.lives, serverClock: this.serverClock, ...this.speedIntent,
      encodedHeading: sang, sentHeading: this.lastHeading, headingSent,
      heldBoost: this.boost,
      boostSent: this.boost !== boostBefore && this.ws?.readyState === 1,
      anchor: me.pts.at(-1), anchorAgeMs: now - me.t,
      head: w.head(me, now), headSource: 'last_anchor_extrapolation',
      ang: me.ang, wang: me.wang,
      sct: me.sct, fam: me.fam, sp: me.sp, radius: w.radius, grd: w.grd });
  }
  /** Probe the server's rim with raw received anchors, not client head extrapolation. */
  probeBoundary(now, me, choice) {
    const w = this.world, head = me.pts.at(-1);
    const radial = Math.atan2(head.y - w.grd, head.x - w.grd);
    const tangentSign = Math.floor(Math.max(0, this.lives - 1) / 2) % 2 ? -1 : 1;
    const gap = w.radius - Math.hypot(head.x - w.grd, head.y - w.grd);
    const firstContact = this.boundaryEntryAt === null;
    let pendingContact = false;
    if (this.boundaryContact && gap > BOUNDARY_PROBE.lossGap &&
        this.boundaryStep < BOUNDARY_PROBE.outwardAngles.length) {
      this.boundaryContact = false;
      this.boundaryDwellAt = null;
      this.rec.meta({ event: 'boundary-contact-lost', life: this.lives,
        serverClock: this.serverClock, reason: 'gap', step: this.boundaryStep, gap });
    }
    if (!this.boundaryContact && gap <= BOUNDARY_PROBE.entryGap) {
      this.boundaryContact = true;
      this.boundaryDwellAt = now;
      pendingContact = true;
    }
    // Steering hysteresis may keep a shallow heading through a small radius
    // expansion, but that time is not near-rim experimental exposure.
    if (this.boundaryContact && gap > BOUNDARY_PROBE.entryGap) this.boundaryDwellAt = null;
    else if (this.boundaryContact && this.boundaryDwellAt === null) this.boundaryDwellAt = now;
    if (this.boundaryContact && this.boundaryDwellAt !== null &&
        this.boundaryStep < BOUNDARY_PROBE.outwardAngles.length &&
        now - this.boundaryDwellAt >= (this.lives % 2 === 0
          ? (this.boundaryStep < 0 ? BOUNDARY_PROBE.boostShallowMs : BOUNDARY_PROBE.boostOutwardStepMs)
          : (this.boundaryStep < 0 ? BOUNDARY_PROBE.shallowMs : BOUNDARY_PROBE.outwardStepMs))) {
      this.boundaryStep++;
      this.boundaryDwellAt = now;
    }
    let phase = 'approach', angle = null, intendedHeading = radial;
    if (!this.boundaryContact && (this.boundaryEntryAt !== null || gap <= BOUNDARY_PROBE.stageGap)) {
      phase = this.boundaryEntryAt === null ? 'acquire' : 'reacquire';
      angle = BOUNDARY_PROBE.acquireAngle;
      intendedHeading = radial + tangentSign * (Math.PI / 2 - angle);
    } else if (!this.boundaryContact) {
      let nearestFood = Infinity;
      for (const food of w.foods.values()) {
        const distance = Math.hypot(food.x - head.x, food.y - head.y);
        const bearing = Math.atan2(food.y - head.y, food.x - head.x);
        const delta = Math.atan2(Math.sin(bearing - radial), Math.cos(bearing - radial));
        if (distance < nearestFood && distance < BOUNDARY_PROBE.approachFoodRange &&
            Math.abs(delta) < BOUNDARY_PROBE.approachFoodCone) {
          nearestFood = distance;
          intendedHeading = bearing;
        }
      }
    } else {
      const step = this.boundaryStep;
      phase = step < 0 ? 'shallow' : step < BOUNDARY_PROBE.outwardAngles.length ?
        `outward_${step + 1}` : 'complete';
      angle = step < 0 ? 0.02 : BOUNDARY_PROBE.outwardAngles[step] ?? null;
      intendedHeading = phase === 'complete' ? radial + Math.PI : radial + tangentSign * (Math.PI / 2 - angle);
    }
    let nearestOtherClearance = Infinity, nearestPredictedHeadClearance = Infinity, otherDanger = false;
    const dx = Math.cos(intendedHeading), dy = Math.sin(intendedHeading);
    const ownScale = Math.min(6, 1 + (me.sct - 2) / 106);
    for (const snake of w.snakes.values()) {
      if (snake.id === w.self) continue;
      const otherScale = Math.min(6, 1 + (snake.sct - 2) / 106);
      for (const point of snake.pts) {
        const qx = point.x - head.x, qy = point.y - head.y;
        const distance = Math.hypot(qx, qy);
        nearestOtherClearance = Math.min(nearestOtherClearance, distance - 17 * otherScale);
        const along = qx * dx + qy * dy;
        const side = Math.abs(qx * dy - qy * dx);
        if (along > -20 && along < BOUNDARY_PROBE.otherClearance &&
            side < 30 * ownScale + 25 * otherScale + 30) otherDanger = true;
      }
      const predicted = w.head(snake, now);
      const qx = predicted.x - head.x, qy = predicted.y - head.y;
      nearestPredictedHeadClearance = Math.min(nearestPredictedHeadClearance,
        Math.hypot(qx, qy) - 17 * otherScale);
      const along = qx * dx + qy * dy;
      const side = Math.abs(qx * dy - qy * dx);
      if (along > -20 && along < BOUNDARY_PROBE.otherClearance &&
          side < 30 * ownScale + 25 * otherScale + 30) otherDanger = true;
    }
    // Do not use headingDanger here: its rim term would prevent the trial.
    const interruption = otherDanger ? 'other_snake' : null;
    if (interruption && this.boundaryContact && phase !== 'complete') {
      this.boundaryContact = false;
      this.boundaryDwellAt = null;
      if (!pendingContact) this.rec.meta({ event: 'boundary-contact-lost', life: this.lives,
        serverClock: this.serverClock, reason: interruption, step: this.boundaryStep, gap });
      phase = 'reacquire';
      angle = BOUNDARY_PROBE.acquireAngle;
    }
    if (pendingContact && !interruption) {
      if (firstContact) this.boundaryEntryAt = now;
      this.rec.meta({ event: firstContact ? 'boundary-contact' : 'boundary-reacquired',
        life: this.lives, serverClock: this.serverClock, step: this.boundaryStep, gap });
    }
    if (phase !== this.boundaryPhase) {
      this.boundaryPhase = phase;
      this.rec.meta({ event: 'phase', scenario: 'boundaryprobe', life: this.lives,
        phase, angle, gap, serverClock: this.serverClock });
    }
    if (!interruption) choice.a = intendedHeading;
    const boostWindow = angle !== null && angle >= BOUNDARY_PROBE.boostStartAngle;
    const desiredBoost = this.lives % 2 === 0 && this.boundaryContact && boostWindow &&
      !interruption && me.sct >= BOUNDARY_PROBE.boostMinSegments;
    if (this.lives % 2 === 0 && this.boundaryContact && boostWindow &&
        me.sct < BOUNDARY_PROBE.boostMinSegments && !this.boundaryBoostInsufficientSeen) {
      this.boundaryBoostInsufficientSeen = true;
      this.rec.meta({ event: 'boundary-boost-ineligible', life: this.lives,
        serverClock: this.serverClock, phase, sct: me.sct, fam: me.fam });
    }
    this.boundaryIntent = { phase, angle, gap, intendedHeading, interruption,
      tangentSign,
      nearRimDwellMs: this.boundaryDwellAt === null ? 0 : now - this.boundaryDwellAt,
      nearestOtherClearance: Number.isFinite(nearestOtherClearance) ? nearestOtherClearance : null,
      nearestPredictedHeadClearance: Number.isFinite(nearestPredictedHeadClearance) ? nearestPredictedHeadClearance : null,
      boostEligible: me.sct >= BOUNDARY_PROBE.boostMinSegments, boostWindow,
      desiredBoost, trial: this.lives % 2 === 0 ? 'boost' : 'cruise' };
  }
  speedCorridor(now, me, head, heading) {
    const cfg = SPEED_PROBE, w = this.world;
    const dx = Math.cos(heading), dy = Math.sin(heading);
    const end = { x: head.x + dx * cfg.corridorLength, y: head.y + dy * cfg.corridorLength };
    if (w.radius - Math.hypot(end.x - w.grd, end.y - w.grd) < cfg.rimMargin) return 'rim_corridor';
    const scale = Math.min(6, 1 + (me.sct - 2) / 106);
    if (headingDanger(w, now, me, head, heading, scale) > 0.2) return 'avoidance';
    for (const other of w.snakes.values()) {
      if (other.id === w.self) continue;
      for (const point of [...other.pts, w.head(other, now)]) {
        const rx = point.x - head.x, ry = point.y - head.y;
        const along = rx * dx + ry * dy, side = Math.abs(rx * dy - ry * dx);
        if (along > -30 && along < cfg.corridorLength && side < 120) return 'occupied_corridor';
      }
    }
    return null;
  }
  startSpeedPhase(now, me, phase) {
    const trial = this.speedTrial;
    trial.phase = phase;
    trial.phaseStartedAt = now;
    this.rec.meta({ event: 'speed-phase', scenario: 'speedprobe', life: this.lives,
      trial: trial.id, phase, heading: trial.heading, serverClock: this.serverClock,
      unboostedBaseline: trial.unboostedBaseline, growthTargetSegments: this.speedGrowthTarget,
      sct: me.sct, fam: me.fam, sp: me.sp, anchor: me.pts.at(-1) });
  }
  endSpeedTrial(now, reason, me = null) {
    const trial = this.speedTrial;
    if (!trial) return;
    this.rec.meta({ event: 'speed-trial-end', scenario: 'speedprobe', life: this.lives,
      trial: trial.id, phase: trial.phase, reason, completed: reason === 'coast_complete',
      unboostedBaseline: trial.unboostedBaseline, growthTargetSegments: this.speedGrowthTarget,
      elapsedMs: now - trial.startedAt, phaseElapsedMs: now - trial.phaseStartedAt,
      serverClock: this.serverClock, sct: me?.sct ?? null, fam: me?.fam ?? null,
      sp: me?.sp ?? null, anchor: me?.pts.at(-1) ?? null });
    this.speedTrial = null;
    this.speedCooldownUntil = now + SPEED_PROBE.betweenMs;
  }
  probeSpeed(now, me, choice) {
    const cfg = SPEED_PROBE, w = this.world, head = w.head(me, now);
    const growthSegments = this.opts.speedGrowthSegments ?? 0;
    const minSegments = Math.max(this.opts.speedMinSegments ?? cfg.minSegments,
      this.speedGrowthTarget ?? 0);
    const anchorAgeMs = now - me.t;
    let trial = this.speedTrial;
    if (!trial) {
      const inward = Math.atan2(w.grd - head.y, w.grd - head.x);
      const requestedByte = this.opts.speedHeadingBytes?.length ?
        this.opts.speedHeadingBytes[this.speedTrials % this.opts.speedHeadingBytes.length] : null;
      let heading = requestedByte === null ? me.ang : (requestedByte + 0.5) * TAU / 251;
      let corridor = this.speedCorridor(now, me, head, heading);
      if (requestedByte === null && corridor &&
          this.speedCorridor(now, me, head, inward) === null) heading = inward;
      corridor = this.speedCorridor(now, me, head, heading);
      const ready = me.sct >= minSegments && now >= this.speedCooldownUntil &&
        anchorAgeMs >= 0 && anchorAgeMs <= cfg.anchorMaxAgeMs &&
        now - this.lastReceivedAt <= cfg.frameGapMs && !corridor;
      if (!ready) {
        if (corridor === 'rim_corridor' &&
            this.speedCorridor(now, me, head, inward) === null) choice.a = inward;
        this.speedIntent = { trial: null, phase: 'forage_wait', intendedHeading: null,
          desiredBoost: false, interruption: me.sct < minSegments ? 'insufficient_mass' :
            now < this.speedCooldownUntil ? 'cooldown' : corridor ??
              (anchorAgeMs > cfg.anchorMaxAgeMs ? 'stale_anchor' : 'frame_gap') };
        return;
      }
      trial = this.speedTrial = { id: ++this.speedTrials, heading, requestedByte, startedAt: now,
        unboostedBaseline: growthSegments > 0 && this.speedGrowthTarget === null };
      this.startSpeedPhase(now, me, 'settle');
    }
    const corridor = this.speedCorridor(now, me, head, trial.heading);
    const interruption = now - this.lastReceivedAt > cfg.frameGapMs ? 'frame_gap' :
      anchorAgeMs < 0 || anchorAgeMs > cfg.anchorMaxAgeMs ? 'stale_anchor' :
        corridor || choice.danger > 0.2 ? corridor ?? 'avoidance' : null;
    if (interruption) {
      this.endSpeedTrial(now, interruption, me);
      this.speedIntent = { trial: trial.id, phase: trial.phase, intendedHeading: trial.heading,
        desiredBoost: false, interruption };
      return;
    }
    const elapsed = now - trial.phaseStartedAt;
    const duration = trial.phase === 'settle' ? cfg.settleMs :
      trial.phase === 'boost' ? cfg.boostMs : cfg.coastMs;
    if (elapsed >= duration) {
      this.rec.meta({ event: 'speed-phase-end', trial: trial.id, phase: trial.phase,
        elapsedMs: elapsed, serverClock: this.serverClock, sct: me.sct, fam: me.fam,
        sp: me.sp, anchor: me.pts.at(-1) });
      if (trial.phase === 'coast') {
        if (trial.unboostedBaseline) {
          this.speedGrowthTarget = me.sct + growthSegments;
          this.rec.meta({ event: 'speed-growth-target', life: this.lives, trial: trial.id,
            baselineSegments: me.sct, targetSegments: this.speedGrowthTarget,
            serverClock: this.serverClock });
        }
        this.endSpeedTrial(now, 'coast_complete', me);
        this.speedIntent = { trial: trial.id, phase: 'forage_wait', intendedHeading: null,
          desiredBoost: false, interruption: null };
        return;
      }
      this.startSpeedPhase(now, me,
        trial.phase === 'settle' && !trial.unboostedBaseline ? 'boost' : 'coast');
    }
    const headingError = Math.abs(Math.atan2(
      Math.sin(me.ang - trial.heading), Math.cos(me.ang - trial.heading)));
    choice.a = trial.heading;
    this.speedIntent = { trial: trial.id, phase: trial.phase,
      intendedHeading: trial.heading, desiredBoost: trial.phase === 'boost' &&
        (me.sct > 2 || me.fam > 0), interruption: null,
      headingError, headingSettled: headingError <= 0.1,
      phaseElapsedMs: now - trial.phaseStartedAt };
    this.speedIntent.unboostedBaseline = trial.unboostedBaseline;
    this.speedIntent.growthTargetSegments = this.speedGrowthTarget;
  }
  startVisibilityPhase(now, me, head, transfer = false) {
    const state = this.visibility;
    const block = VISIBILITY_BLOCKS[state.block];
    const endpoints = visibilityEndpoints(state.center, block.orientation);
    const direction = state.leg % 2 === 0 ? 'positive' : 'negative';
    state.transfer = transfer;
    state.phase = transfer ? 'transfer' : `${block.orientation}_${block.mode}_${direction}_${state.leg + 1}`;
    state.target = transfer ? state.center : endpoints[direction];
    state.phaseStartHead = { ...head };
    state.phaseStartedAt = now;
    state.previousMass = me.sct + me.fam;
    state.startMass = state.previousMass;
    state.maxCrossTrack = 0;
    state.fallbackReason = null;
    state.massChangeCount = 0;
    state.massDeltaTotal = 0;
    this.phase = state.phase;
    this.rec.meta({ event: 'phase', scenario: 'visibilityprobe', life: this.lives,
      routeId: state.id, phase: state.phase, block: state.block, leg: state.leg,
      mode: block.mode, orientation: block.orientation, transfer,
      boostEligibleAtStart: me.sct > 2 || me.fam > 0.2,
      effectiveModeAtStart: !transfer && block.mode === 'boost' && (me.sct > 2 || me.fam > 0.2) ? 'boost' : 'cruise',
      center: state.center, endpoints, startHead: head, target: state.target,
      serverClock: this.serverClock, sct: me.sct, fam: me.fam, sp: me.sp });
  }
  endVisibilityLeg(now, reason, head = null) {
    const state = this.visibility;
    if (!state?.phase) return;
    this.rec.meta({ event: 'visibility-phase-end', scenario: 'visibilityprobe',
      life: this.lives, routeId: state.id, phase: state.phase,
      transfer: state.transfer, reason, completed: reason === 'endpoint_reached',
      elapsedMs: now - state.phaseStartedAt, startHead: state.phaseStartHead,
      target: state.target, endHead: head, maxCrossTrack: state.maxCrossTrack,
      startMass: state.startMass,
      massChangeCount: state.massChangeCount, massDeltaTotal: state.massDeltaTotal,
      serverClock: this.serverClock });
    state.phase = null;
    if (reason !== 'endpoint_reached') {
      this.visibilityNextBlock = (state.block + 1) % VISIBILITY_BLOCKS.length;
      this.visibility = null;
      this.phase = null;
      this.visibilityCooldownUntil = now + VISIBILITY_PROBE.reacquireCooldownMs;
      this.visibilityAcquireSince = now;
    }
  }
  probeVisibility(now, me, choice) {
    const w = this.world, head = w.head(me, now), age = now - me.t;
    const cfg = VISIBILITY_PROBE;
    if (this.visibilityAcquireSince === null) this.visibilityAcquireSince = now;
    let state = this.visibility;
    if (!state) {
      const axis = visibilityEndpoints(head, 'axis');
      const diagonal = visibilityEndpoints(head, 'diagonal');
      const sites = [head, axis.positive, axis.negative, diagonal.positive, diagonal.negative];
      const rimGap = w.radius - Math.hypot(head.x - w.grd, head.y - w.grd);
      const quiet = [...w.snakes.values()].filter(o => o.id !== w.self).every(o =>
        sites.every(p => Math.hypot(w.head(o, now).x - p.x, w.head(o, now).y - p.y) > cfg.acquireOtherClearance));
      const ready = now >= this.visibilityCooldownUntil && age >= 0 && age <= cfg.anchorMaxAgeMs &&
        now - this.lastReceivedAt <= cfg.frameGapMs && rimGap >= cfg.halfSpan + cfg.rimMargin && quiet;
      const boostBlock = VISIBILITY_BLOCKS[this.visibilityNextBlock].mode === 'boost';
      const boostPrepared = me.sct >= cfg.boostPrepareMinSegments;
      if (!boostBlock || boostPrepared) this.visibilityBoostPrepareSince = null;
      else if (this.visibilityBoostPrepareSince === null) this.visibilityBoostPrepareSince = now;
      const boostPrepareElapsedMs = this.visibilityBoostPrepareSince === null ? 0 :
        now - this.visibilityBoostPrepareSince;
      const boostPrepareTimedOut = boostBlock && !boostPrepared &&
        boostPrepareElapsedMs >= cfg.boostPrepareMaxMs;
      if (ready && (!boostBlock || boostPrepared || boostPrepareTimedOut)) {
        if (boostPrepareTimedOut) this.rec.meta({ event: 'visibility-boost-prepare-timeout',
          life: this.lives, serverClock: this.serverClock, elapsedMs: boostPrepareElapsedMs,
          sct: me.sct, fam: me.fam, nextBlock: this.visibilityNextBlock });
        state = this.visibility = { id: ++this.visibilityRouteId, center: { ...head },
          block: this.visibilityNextBlock, leg: 0, phase: null };
        this.visibilityBoostPrepareSince = null;
        this.startVisibilityPhase(now, me, head);
      } else {
        if (now - this.visibilityAcquireSince >= cfg.acquireTimeoutMs) {
          this.rec.meta({ event: 'visibility-reacquire-timeout', life: this.lives,
            serverClock: this.serverClock, rimGap, quiet, anchorAgeMs: age });
          this.visibilityAcquireSince = now;
          this.ws?.close();
        } else if (rimGap < cfg.halfSpan + cfg.rimMargin) {
          const inward = Math.atan2(w.grd - head.y, w.grd - head.x);
          const sc = Math.min(6, 1 + (me.sct - 2) / 106);
          if (headingDanger(w, now, me, head, inward, sc) <= 0.2) choice.a = inward;
        }
        this.visibilityIntent = { routeId: null, phase: 'acquire', center: null, target: null,
          intendedHeading: null, desiredBoost: false,
          boostPrepareElapsedMs, boostPrepared, nextBlock: this.visibilityNextBlock,
          interruption: rimGap < cfg.halfSpan + cfg.rimMargin ? 'rim_margin' :
            !quiet ? 'nearby_snake' : age > cfg.anchorMaxAgeMs ? 'stale_anchor' :
              now < this.visibilityCooldownUntil ? 'cooldown' :
                boostBlock && !boostPrepared ? 'acquire_boost_mass' : null };
        return;
      }
    }
    const distance = Math.hypot(head.x - state.target.x, head.y - state.target.y);
    if (distance <= cfg.endpointRadius && age >= 0 && age <= cfg.anchorMaxAgeMs) {
      const transfer = state.transfer;
      this.endVisibilityLeg(now, 'endpoint_reached', head);
      if (transfer) this.startVisibilityPhase(now, me, head);
      else {
        state.leg++;
        if (state.leg === 4) {
          state.leg = 0;
          state.block = (state.block + 1) % VISIBILITY_BLOCKS.length;
          if (VISIBILITY_BLOCKS[state.block].mode === 'boost' &&
              me.sct < cfg.boostPrepareMinSegments) {
            // The completed cruise block remains valid. Release this route
            // while the ordinary forager acquires a useful boost reserve.
            this.visibilityNextBlock = state.block;
            this.visibility = null;
            this.phase = null;
            this.visibilityAcquireSince = now;
            this.visibilityBoostPrepareSince = now;
            this.rec.meta({ event: 'visibility-boost-prepare-start', life: this.lives,
              serverClock: this.serverClock, nextBlock: state.block,
              sct: me.sct, fam: me.fam });
            this.visibilityIntent = { routeId: null, phase: 'acquire_boost_mass',
              nextBlock: state.block, intendedHeading: null, desiredBoost: false,
              interruption: 'acquire_boost_mass' };
            return;
          }
          this.startVisibilityPhase(now, me, head, true);
        } else this.startVisibilityPhase(now, me, head);
      }
    }
    const target = state.target;
    const intendedHeading = Math.atan2(target.y - head.y, target.x - head.x);
    const realizedHeadingError = Math.abs(Math.atan2(
      Math.sin(me.ang - intendedHeading), Math.cos(me.ang - intendedHeading)));
    const sc = Math.min(6, 1 + (me.sct - 2) / 106);
    const intendedDanger = headingDanger(w, now, me, head, intendedHeading, sc);
    const dx = target.x - state.phaseStartHead.x, dy = target.y - state.phaseStartHead.y;
    const crossTrack = Math.abs((head.x - state.phaseStartHead.x) * dy -
      (head.y - state.phaseStartHead.y) * dx) / Math.max(1, Math.hypot(dx, dy));
    state.maxCrossTrack = Math.max(state.maxCrossTrack, crossTrack);
    const rimGap = w.radius - Math.hypot(head.x - w.grd, head.y - w.grd);
    const targetRimGap = w.radius - Math.hypot(target.x - w.grd, target.y - w.grd);
    const mass = me.sct + me.fam, deltaMass = mass - state.previousMass;
    state.massDeltaTotal += deltaMass;
    if (Math.abs(deltaMass) > 0.03) {
      state.massChangeCount++;
      this.rec.meta({ event: 'visibility-mass-change', life: this.lives,
        routeId: state.id, phase: state.phase, serverClock: this.serverClock,
        deltaMass, mass, sct: me.sct, fam: me.fam });
    }
    const block = VISIBILITY_BLOCKS[state.block];
    const boostEligible = me.sct > 2 || me.fam > 0.2;
    const desiredBoost = !state.transfer && block.mode === 'boost' && boostEligible &&
      now - state.phaseStartedAt < cfg.boostMaxMs;
    const boostFallback = !state.transfer && block.mode === 'boost' ?
      !boostEligible ? 'insufficient_mass' :
        now - state.phaseStartedAt >= cfg.boostMaxMs ? 'boost_window_complete' : null : null;
    if (boostFallback && boostFallback !== state.fallbackReason) {
      this.rec.meta({ event: 'visibility-boost-fallback', life: this.lives,
        routeId: state.id, phase: state.phase, reason: boostFallback,
        serverClock: this.serverClock, sct: me.sct, fam: me.fam });
    }
    state.fallbackReason = boostFallback;
    let interruption = null;
    if (now - this.lastReceivedAt > cfg.frameGapMs) interruption = 'frame_gap';
    else if (age < 0 || age > cfg.anchorMaxAgeMs) interruption = 'stale_anchor';
    else if (now - state.phaseStartedAt >= (state.transfer ? cfg.transferMaxMs : cfg.legMaxMs)) interruption = 'phase_timeout';
    else if (Math.min(rimGap, targetRimGap) < cfg.rimMargin) interruption = 'rim_margin';
    else if (crossTrack > cfg.crossTrackLimit && !state.transfer) interruption = 'off_path';
    else if (choice.danger > 0.2 || intendedDanger > 0.2) interruption = 'avoidance';
    const phase = state.phase;
    if (interruption) this.endVisibilityLeg(now, interruption, head);
    else choice.a = intendedHeading;
    if (this.visibility) state.previousMass = mass;
    this.visibilityIntent = { routeId: state.id, phase, center: state.center,
      endpoints: visibilityEndpoints(state.center, block.orientation), target,
      block: state.block, leg: state.leg, mode: block.mode,
      effectiveMode: desiredBoost ? 'boost' : 'cruise', boostFallback,
      intendedHeading, requestedHeading: choice.a, intendedDanger,
      realizedHeadingError, headingSettled: realizedHeadingError <= 0.1,
      desiredBoost: !interruption && desiredBoost, boostEligible,
      interruption, distance, crossTrack, rimGap, targetRimGap, deltaMass };
  }
  /** One pass through each size tier. A missed tier remains a safe forage wait until capture ends. */
  movement(now, me, choice) {
    let tier = MOVEMENT_TIERS[this.movementTier];
    if (this.phase && now - this.phaseStart >= MOVEMENT_STEPS[this.movementStep].ms) {
      this.movementStep++;
      this.phase = null;
      if (this.movementStep === MOVEMENT_STEPS.length) {
        this.rec.meta({ event: 'movement-tier-complete', tier: tier.name, sct: me.sct, fam: me.fam });
        this.movementTier++;
        this.movementStep = 0;
        tier = MOVEMENT_TIERS[this.movementTier];
      }
    }
    const minSegments = this.movementTier === 0 ? this.opts.movementMinSegments ?? tier.minSegments : tier?.minSegments;
    if (!this.phase && tier && me.sct >= minSegments) {
      const step = MOVEMENT_STEPS[this.movementStep];
      this.phase = step.name;
      this.phaseStart = now;
      this.phaseHeading = me.ang;
      this.rec.meta({ event: 'phase', scenario: 'movement', tier: tier.name, phase: step.name, minSegments, sct: me.sct, fam: me.fam });
    }
    const step = this.phase ? MOVEMENT_STEPS[this.movementStep] : null;
    let intendedHeading = null;
    if (step?.steering === 'straight') intendedHeading = this.phaseHeading;
    else if (step?.steering === 'left') intendedHeading = me.ang + 1.35;
    else if (step?.steering === 'right') intendedHeading = me.ang - 1.35;
    else if (step?.steering === 'reverse_left') intendedHeading = this.phaseHeading + Math.PI - 0.01;
    else if (step?.steering === 'reverse_right') intendedHeading = this.phaseHeading - Math.PI + 0.01;
    let intendedDanger = null, interruption = null;
    if (intendedHeading !== null) {
      const sc = Math.min(6, 1 + (me.sct - 2) / 106);
      intendedDanger = headingDanger(this.world, now, me, this.world.head(me, now), intendedHeading, sc);
      if (choice.danger > 0.2 || intendedDanger > 0.2) interruption = 'avoidance';
      else choice.a = intendedHeading;
    }
    const desiredBoost = !!step?.boost && !interruption && me.sct >= 5;
    if (step?.boost && me.sct < 5) interruption = interruption ?? 'insufficient_mass';
    this.movementIntent = { intendedHeading, intendedDanger, interruption, desiredBoost };
  }
  /** Approach a visible prey and retreat across the suspected flight radius.
   * An optional distant boosted approach ends at 560 units; nearby boost is
   * requested only while retreating, to avoid intentionally eating the target. */
  probePrey(now, me, choice) {
    const w = this.world, head = w.head(me, now);
    const otherHeads = prey => {
      let nearest = Infinity;
      let within450 = 0, within800 = 0;
      for (const snake of w.snakes.values()) {
        if (snake.id === w.self) continue;
        const other = w.head(snake, now);
        const d = Math.hypot(other.x - prey.x, other.y - prey.y);
        nearest = Math.min(nearest, d);
        if (d < 450) within450++;
        if (d < 800) within800++;
      }
      return { nearest, within450, within800 };
    };
    const release = reason => {
      if (this.preyTarget) this.rec.meta({ event: 'prey-target-end',
        targetId: this.preyTarget.id, generation: this.preyTarget.generation,
        phase: this.phase, cycles: this.preyCycles, reason });
      this.preyTarget = null;
      this.phase = null;
      this.preyCooldownUntil = now + 1500;
    };
    let prey = this.preyTarget && w.prey.get(this.preyTarget.id);
    if (this.preyTarget && (!prey || prey.generation !== this.preyTarget.generation)) {
      release('visibility_end_or_reentry');
      prey = null;
    }
    if (prey && now - prey.t > PREY_PROBE.targetMaxAgeMs) {
      release('stale_target'); prey = null;
    }
    if (prey && otherHeads(prey).nearest < PREY_PROBE.abandonOtherClearance) {
      release('other_head_near_target'); prey = null;
    }
    if (!prey && now >= this.preyCooldownUntil) {
      let best = null;
      for (const candidate of w.prey.values()) {
        const distance = Math.hypot(candidate.x - head.x, candidate.y - head.y);
        const other = otherHeads(candidate);
        const edge = w.radius - Math.hypot(candidate.x - w.grd, candidate.y - w.grd);
        if (now - candidate.t > PREY_PROBE.targetMaxAgeMs ||
            distance < PREY_PROBE.acquireMin || distance > PREY_PROBE.acquireMax ||
            other.nearest < PREY_PROBE.acquireOtherClearance || edge < 700) continue;
        if (!best || Math.abs(distance - 600) < Math.abs(best.distance - 600)) {
          best = { candidate, distance, other };
        }
      }
      if (best) {
        prey = best.candidate;
        this.preyTarget = { id: prey.id, generation: prey.generation };
        this.preyCycles = 0;
        this.phase = me.sct >= 5 && best.distance >= PREY_PROBE.boostApproachStart ?
          'approach_boost' : 'approach_cruise';
        this.phaseStart = now;
        this.rec.meta({ event: 'prey-target-start', targetId: prey.id,
          generation: prey.generation, distance: best.distance,
          nearestOther: best.other.nearest, otherHeadsWithin450: best.other.within450,
          otherHeadsWithin800: best.other.within800, sct: me.sct, fam: me.fam });
      }
    }
    if (!prey) {
      this.preyIntent = { distance: null, boxDistance: null, bearing: null,
        nearestOther: null, targetSpeed: null,
        targetAgeMs: null, intendedHeading: null, desiredBoost: false,
        interruption: this.preyCooldownUntil > now ? 'target_cooldown' : 'no_visible_prey' };
      return;
    }
    const distance = Math.hypot(prey.x - head.x, prey.y - head.y);
    const boxDistance = Math.max(Math.abs(prey.x - head.x), Math.abs(prey.y - head.y));
    const bearing = Math.atan2(prey.y - head.y, prey.x - head.x);
    const nearbyOtherHeads = otherHeads(prey);
    const nearestOther = nearbyOtherHeads.nearest;
    const age = now - prey.t;
    const phase = name => {
      this.phase = name;
      this.phaseStart = now;
      this.rec.meta({ event: 'phase', scenario: 'preyprobe', phase: name,
        targetId: prey.id, generation: prey.generation, distance, boxDistance, bearing,
        preySpeed: prey.sp, ownSpeed: me.sp, sct: me.sct, cycles: this.preyCycles });
    };
    if (this.phase === 'approach_boost' && distance <= PREY_PROBE.boostApproachStop) {
      phase('approach_cruise');
    } else if (this.phase === 'approach_cruise' && distance <= PREY_PROBE.approachStop) {
      phase(me.sct >= 5 ? 'retreat_boost' : 'retreat_cruise');
    } else if (this.phase === 'retreat_boost' &&
               (now - this.phaseStart >= PREY_PROBE.boostRetreatMs || distance >= PREY_PROBE.retreatDone)) {
      phase('retreat_cruise');
    } else if (this.phase === 'retreat_cruise' && distance >= PREY_PROBE.retreatDone) {
      this.preyCycles++;
      if (this.preyCycles >= 2) release('two_crossings_complete');
      else phase('approach_cruise');
    } else if (now - this.phaseStart >= PREY_PROBE.phaseMaxMs) release('phase_timeout');
    if (!this.preyTarget) {
      this.preyIntent = { distance, boxDistance, bearing, nearestOther, targetSpeed: prey.sp,
        targetAgeMs: age, intendedHeading: null, desiredBoost: false,
        interruption: 'target_released' };
      return;
    }
    const retreat = this.phase.startsWith('retreat') || distance <= PREY_PROBE.hardStop;
    const intendedHeading = bearing + (retreat ? Math.PI : 0);
    const sc = Math.min(6, 1 + (me.sct - 2) / 106);
    const intendedDanger = headingDanger(w, now, me, head, intendedHeading, sc);
    const interruption = choice.danger > 0.2 || intendedDanger > 0.2 ? 'avoidance' : null;
    if (!interruption) choice.a = intendedHeading;
    const desiredBoost = !interruption && me.sct >= 5 &&
      (this.phase === 'retreat_boost' || (this.phase === 'approach_boost' && !retreat));
    this.preyIntent = { distance, boxDistance, bearing, nearestOther,
      otherHeadsWithin450: nearbyOtherHeads.within450,
      otherHeadsWithin800: nearbyOtherHeads.within800, targetSpeed: prey.sp,
      targetAgeMs: age, intendedHeading, desiredBoost, interruption,
      intendedDanger };
  }
  /** Boost in a tight circle, then steer at cruise onto the pellets just dropped. */
  ownPellets(now, me, choice) {
    const cycle = { boostcircle: 2500, collect: 6000, forage: 4000 };
    const next = { boostcircle: 'collect', collect: 'forage', forage: 'boostcircle' };
    if (!this.phase) { if (me.sct < 4) return; this.phase = 'forage'; this.phaseStart = now; }
    const h = this.world.head(me, now);
    for (const id of this.own.keys()) if (!this.world.foods.has(id)) this.own.delete(id);
    const expired = now - this.phaseStart >= cycle[this.phase] || (this.phase === 'collect' && !this.own.size);
    if (expired && (this.phase !== 'forage' || (me.sct >= 4 && choice.danger < 0.05))) {
      if (this.phase === 'boostcircle') this.rec.meta({ event: 'own-pellets', ids: [...this.own.keys()] });
      this.phase = next[this.phase];
      this.phaseStart = now;
      if (this.phase === 'boostcircle') this.own.clear();
      this.rec.meta({ event: 'phase', phase: this.phase, sct: me.sct, fam: me.fam });
    }
    if (choice.danger > 0.2) return; // the forager's avoidance wins near danger
    if (this.phase === 'boostcircle') choice.a = me.ang + 1.2;
    else if (this.phase === 'collect') {
      let best = null;
      for (const f of this.own.values()) {
        const d = Math.hypot(f.x - h.x, f.y - h.y);
        if (!best || d < best.d) best = { d, f };
      }
      if (best) choice.a = Math.atan2(best.f.y - h.y, best.f.x - h.x);
    }
  }
  wantBoost(now, me, choice) {
    if (this.opts.scenario === 'boundaryprobe') return this.boundaryIntent.desiredBoost;
    if (this.opts.scenario === 'visibilityprobe') return this.visibilityIntent.desiredBoost;
    if (this.opts.scenario === 'speedprobe') return this.speedIntent.desiredBoost;
    if (this.opts.scenario === 'movement') return this.movementIntent.desiredBoost;
    if (this.opts.scenario === 'preyprobe') return this.preyIntent.desiredBoost;
    if (this.opts.scenario === 'ownpellets') return this.phase === 'boostcircle' && choice.danger <= 0.2;
    if (this.opts.scenario !== 'boost') return false;
    // Cycle hold, rest and pulse patterns once large enough; danger pauses boosting only.
    const cycle = [['hold', 3000, 1], ['rest', 2500, 0], ['pulse100', 2000, 100], ['rest', 2500, 0], ['pulse300', 2400, 300], ['rest', 4000, 0]];
    if (me.sct < 15 && !this.phase) return false;
    if (!this.phase || now - this.phaseStart >= cycle[this.phaseIndex][1]) {
      this.phaseIndex = this.phase ? (this.phaseIndex + 1) % cycle.length : 0;
      if (me.sct < 12) this.phaseIndex = 1;
      this.phase = cycle[this.phaseIndex][0];
      this.phaseStart = now;
      this.rec.meta({ event: 'phase', phase: this.phase, sct: me.sct, fam: me.fam });
    }
    const period = cycle[this.phaseIndex][2];
    if (period === 0 || choice.danger > 0.2) return false;
    return period === 1 || Math.floor((now - this.phaseStart) / period) % 2 === 0;
  }
  setBoost(on, now) {
    const emergencyRelease = !on && (this.opts.scenario === 'visibilityprobe' && this.visibilityIntent?.interruption ||
      this.opts.scenario === 'speedprobe' && this.speedIntent?.interruption);
    if (on !== this.boost && (now - this.lastBoostAt > 50 || emergencyRelease)) {
      this.boost = on;
      this.lastBoostAt = now;
      this.send([on ? 253 : 254]);
    }
  }
  stop() {
    this.stopped = true;
    if (this.opts.scenario === 'visibilityprobe') this.endVisibilityLeg(performance.now(), 'capture_stop');
    if (this.opts.scenario === 'speedprobe') this.endSpeedTrial(performance.now(), 'capture_stop');
    this.finishDeathObservation('capture-stop');
    if (this.pendingPing) this.rec.meta({ event: 'ping-censored', life: this.lives,
      sentAt: this.pendingPing.t, waitedMs: performance.now() - this.pendingPing.t });
    this.pendingPing = null;
    clearTimeout(this.reconnectTimer);
    if (this.ws) {
      // End the observation window before closing the archive. A delayed close or
      // message callback must not write to a finished gzip stream or reconnect.
      this.ws.onopen = this.ws.onmessage = this.ws.onclose = this.ws.onerror = null;
      this.ws.close();
      this.ws = null;
    }
  }
}

export class Recorder {
  constructor(file, header) {
    this.hash = createHash('sha256');
    this.lines = 0;
    this.payloadBytes = 0;
    this.counts = { i: { frames: 0, bytes: 0 }, o: { frames: 0, bytes: 0 } };
    this.backpressureWrites = 0;
    this.maxQueuedBytes = 0;
    this.sealed = false;
    this.error = null;
    this.queueLimitBytes = 32 * 1024 * 1024;
    this.failure = new Promise(resolve => { this.reportFailure = resolve; });
    this.gz = zlib.createGzip();
    this.output = fs.createWriteStream(file, { flags: 'wx' });
    this.finished = new Promise((resolve, reject) => {
      this.rejectFinished = reject;
      this.output.on('finish', resolve);
      this.output.on('error', reject);
      this.gz.on('error', reject);
    });
    this.finished.catch(() => {});
    this.output.on('error', error => this.fail(error));
    this.gz.on('error', error => this.fail(error));
    this.output.on('close', () => {
      if (!this.output.writableFinished) this.fail(new Error('recorder output closed before finish'));
    });
    this.gz.on('close', () => {
      if (!this.gz.writableFinished) this.fail(new Error('recorder gzip closed before finish'));
    });
    this.gz.pipe(this.output);
    this.line({ type: 'header', ...header });
  }
  writeLine(text) {
    if (this.sealed) throw new Error('recorder already finalized');
    if (this.error) return false;
    const queuedBytes = this.gz.writableLength + this.gz.readableLength + this.output.writableLength + Buffer.byteLength(text);
    if (queuedBytes > this.queueLimitBytes) {
      this.fail(new Error('recorder queue exceeds 32 MiB; capture aborted'));
      return false;
    }
    this.hash.update(text);
    this.lines++;
    this.payloadBytes += Buffer.byteLength(text);
    if (!this.gz.write(text)) this.backpressureWrites++;
    this.maxQueuedBytes = Math.max(this.maxQueuedBytes, queuedBytes);
    return true;
  }
  fail(error) {
    if (this.error) return;
    this.error = error;
    this.rejectFinished(error);
    this.reportFailure(error);
    this.gz.destroy(error);
    this.output.destroy(error);
  }
  line(obj) { return this.writeLine(JSON.stringify(obj) + '\n'); }
  frame(dir, bytes, t = performance.now()) {
    if (!['i', 'o'].includes(dir) || !Number.isFinite(t)) throw new Error('invalid frame');
    const saved = this.writeLine(`["${dir}",${t.toFixed(3)},"${Buffer.from(bytes).toString('base64')}"]\n`);
    if (saved) {
      this.counts[dir].frames++;
      this.counts[dir].bytes += bytes.length;
    }
    return saved;
  }
  meta(obj) { this.line({ type: 'meta', t: +performance.now().toFixed(3), wall: Date.now(), ...obj }); }
  finish(obj) {
    if (this.error) return false;
    this.meta({ ...obj, integrity: { format: 'sha256-jsonl-prefix-v1',
      sha256: this.hash.copy().digest('hex'), lines: this.lines, payloadBytes: this.payloadBytes,
      counts: structuredClone(this.counts), backpressureWrites: this.backpressureWrites,
      maxQueuedBytes: this.maxQueuedBytes, queueLimitBytes: this.queueLimitBytes } });
    this.sealed = true;
  }
  close() {
    if (!this.gz.writableEnded && !this.gz.destroyed) this.gz.end();
    return this.finished;
  }
}

export function validateOrigin(origin) {
  if (origin === undefined || origin === null) return null;
  let url;
  try { url = new URL(origin); } catch { throw new Error('--origin requires an HTTP(S) origin'); }
  if (!['http:', 'https:'].includes(url.protocol) || url.origin !== origin)
    throw new Error('--origin requires an HTTP(S) origin');
  return origin;
}

export function validateLocalServer({ ip, port }) {
  // Owner policy (2026-10-30): collectors only connect to numeric loopback hosts.
  // Never resolve hostnames, including localhost, at the socket boundary.
  if (isIP(ip) !== 4 || !ip.startsWith('127.') ||
      !Number.isInteger(port) || port < 1 || port > 65535)
    throw new Error('invalid server: --server requires loopback IPv4:port in 1..65535');
  return { ip, port };
}

export function parseLocalServer(server) {
  const [ip, portText, extra] = (server ?? '').split(':');
  if (extra !== undefined || !/^\d+$/.test(portText ?? ''))
    throw new Error('invalid server: --server requires loopback IPv4:port in 1..65535');
  return validateLocalServer({ ip, port: Number(portText) });
}

export function parseArgs(argv) {
  const o = { seconds: 300, bots: 1, scenario: 'forage', nick: '(Bot) Bottingson', server: null, origin: null, out: null, plan: null, speedMinSegments: SPEED_PROBE.minSegments, speedGrowthSegments: 0, speedHeadingBytes: '', movementMinSegments: MOVEMENT_TIERS[0].minSegments };
  for (let i = 0; i < argv.length; i += 2) {
    if (!argv[i].startsWith('--') || argv[i + 1] === undefined || argv[i + 1].startsWith('--') ||
        argv.slice(0, i).filter((value, index) => index % 2 === 0).includes(argv[i]))
      throw new Error(`invalid or repeated option: ${argv[i]}`);
    const k = argv[i].replace(/^--/, '').replace('speed-min-segments', 'speedMinSegments').replace('speed-growth-segments', 'speedGrowthSegments').replace('speed-heading-bytes', 'speedHeadingBytes').replace('movement-min-segments', 'movementMinSegments'), v = argv[i + 1];
    if (!(k in o)) throw new Error(`unknown option ${argv[i]}`);
    o[k] = ['seconds', 'bots', 'speedMinSegments', 'speedGrowthSegments', 'movementMinSegments'].includes(k) ? +v : v;
  }
  if (!o.out) throw new Error('--out is required');
  o.origin = validateOrigin(o.origin);
  if (typeof o.speedHeadingBytes !== 'string' ||
      (o.speedHeadingBytes !== '' && !/^\d+(,\d+)*$/.test(o.speedHeadingBytes)))
    throw new Error('speed-heading-bytes requires comma-separated integers in 0..250');
  o.speedHeadingBytes = o.speedHeadingBytes === '' ? [] : o.speedHeadingBytes.split(',').map(Number);
  if (o.speedHeadingBytes.some(value => !Number.isInteger(value) || value < 0 || value > 250))
    throw new Error('speed-heading-bytes requires comma-separated integers in 0..250');
  if (o.speedHeadingBytes.length && o.scenario !== 'speedprobe') throw new Error('speed-heading-bytes requires speedprobe');
  if (!Number.isInteger(o.speedMinSegments) || o.speedMinSegments < 2) throw new Error('speed-min-segments must be an integer >=2');
  if (!Number.isInteger(o.speedGrowthSegments) || o.speedGrowthSegments < 0) throw new Error('speed-growth-segments must be an integer >=0');
  if (o.speedGrowthSegments && o.scenario !== 'speedprobe') throw new Error('speed-growth-segments requires speedprobe');
  if (!Number.isInteger(o.movementMinSegments) || o.movementMinSegments < 2) throw new Error('movement-min-segments must be an integer >=2');
  if (!Number.isFinite(o.seconds) || o.seconds <= 0 || !Number.isInteger(o.bots) || o.bots < 1 || o.bots > 3) throw new Error('use positive seconds and 1-3 bots');
  if (!['forage', 'boost', 'ownpellets', 'spawn', 'movement', 'preyprobe', 'boundaryprobe', 'visibilityprobe', 'speedprobe'].includes(o.scenario)) throw new Error('unknown scenario');
  if (['preyprobe', 'boundaryprobe', 'visibilityprobe', 'speedprobe'].includes(o.scenario) && o.bots !== 1) throw new Error(`${o.scenario} requires one client`);
  return o;
}

/** Play `bots` clients on one server for `seconds`, writing `<prefix><i>.jsonl.gz`. */
export async function record(opts, server, prefix, bots = opts.bots, signal, captureBatch) {
  if (signal?.aborted) return [];
  validateLocalServer(server);
  const batch = captureBatch ?? createCaptureBatch(opts, bots);
  const list = [];
  let recordingError = null;
  try {
    for (let i = 0; i < bots; i++) {
      if (signal?.aborted) break;
      const rec = new Recorder(path.join(opts.out, `${prefix}${i}.jsonl.gz`), { schema: 1, server, nick: opts.nick, scenario: opts.scenario, wall: Date.now(), t: performance.now(), loginSelector: 31,
        ...(batch.dataOrigin ? { dataOrigin: batch.dataOrigin } : {}),
        captureProvenance: batch.provenance(i) });
      if (opts.scenario === 'preyprobe') rec.meta({ event: 'prey-probe-plan', ...PREY_PROBE,
        limits: 'one client, bounded capture; other snake and prey inputs not observed' });
      if (opts.scenario === 'boundaryprobe') rec.meta({ event: 'boundary-probe-plan', ...BOUNDARY_PROBE,
        limits: 'one client, bounded capture; raw received anchors, no interpolated head position' });
      if (opts.scenario === 'visibilityprobe') rec.meta({ event: 'visibility-probe-plan', ...VISIBILITY_PROBE,
        blocks: VISIBILITY_BLOCKS, limits: 'one client, bounded capture; commands and estimated head do not certify exact realized paths' });
      if (opts.scenario === 'speedprobe') rec.meta({ event: 'speed-probe-plan', ...SPEED_PROBE,
        minSegments: opts.speedMinSegments,
        growthSegments: opts.speedGrowthSegments,
        unboostedBaselineFirst: opts.speedGrowthSegments > 0,
        headingBytes: opts.speedHeadingBytes,
        limits: 'one client, bounded capture; growth and interruptions are recorded for analysis' });
      const bot = new Bot(opts, i, rec);
      list.push(bot);
      bot.connect(server);
    }
  } catch (error) {
    await finalizeCaptureBatch({ bots: list, completed: false, signal, error });
  }
  const collectorTiming = { targetTickMs: 25, ticks: 0, lateTicks: 0, maxTickGapMs: 0 };
  let lastTick = performance.now(), lastTiming = lastTick;
  let timer;
  await new Promise(resolve => {
    let settled = false;
    const finish = error => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      clearInterval(timer);
      signal?.removeEventListener('abort', finish);
      if (error instanceof Error) recordingError = error;
      resolve();
    };
    const timeout = setTimeout(finish, opts.seconds * 1000);
    timer = setInterval(() => {
      try {
        const now = performance.now(), gap = now - lastTick;
        collectorTiming.ticks++;
        collectorTiming.lateTicks += gap > 75 ? 1 : 0;
        collectorTiming.maxTickGapMs = Math.max(collectorTiming.maxTickGapMs, gap);
        lastTick = now;
        if (now - lastTiming >= 1000) {
          for (const bot of list) bot.rec.meta({ event: 'collector-timing', ...collectorTiming });
          lastTiming = now;
        }
        for (const bot of list) bot.tick(now);
      } catch (error) { finish(error); }
    }, 25);
    for (const bot of list) bot.rec.failure.then(finish);
    signal?.addEventListener('abort', finish, { once: true });
    if (signal?.aborted) finish();
  });
  clearInterval(timer);
  await finalizeCaptureBatch({ bots: list, completed: !recordingError, signal, error: recordingError,
    drainDelayMs: 300,
    metadata: { collectorTiming, ...(signal?.aborted || recordingError ? { requestedSeconds: opts.seconds } : {}) } });
  return list.map(b => b.lives);
}

async function main(signal) {
  const opts = parseArgs(process.argv.slice(2));
  if (!opts.server) throw new Error('--server ip:port is required');
  const server = parseLocalServer(opts.server);
  if (fs.existsSync(opts.out)) throw new Error(`refusing to overwrite ${opts.out}`);
  const batch = createCaptureBatch(opts, opts.bots);
  fs.mkdirSync(opts.out, { recursive: true });
  if (signal?.aborted) return;
  console.log(`recording ${opts.bots} bot(s) on ${server.ip}:${server.port} for ${opts.seconds}s -> ${opts.out}`);
  const result = await record(opts, server, 'bot', opts.bots, signal, batch);
  console.log(signal?.aborted ? 'interrupted' : 'done', result);
}

if (process.argv[1]?.endsWith('slither-headless.mjs')) {
  const controller = new AbortController();
  const interrupt = signal => {
    if (controller.signal.aborted) return;
    process.exitCode = signal === 'SIGINT' ? 130 : 143;
    controller.abort(signal);
  };
  const onInterrupt = () => interrupt('SIGINT');
  const onTerminate = () => interrupt('SIGTERM');
  process.on('SIGINT', onInterrupt);
  process.on('SIGTERM', onTerminate);
  main(controller.signal).catch(error => {
    if (controller.signal.aborted && error.name === 'AbortError') console.log('interrupted', controller.signal.reason);
    else { console.error(error); process.exitCode = 1; }
  }).finally(() => {
    process.off('SIGINT', onInterrupt);
    process.off('SIGTERM', onTerminate);
  });
}
