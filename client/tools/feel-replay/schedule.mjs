import { orderEvents, validateSchedule } from './schema.mjs';

// Historical schema-1 envelopes stay intact. Inspection below is metadata-only.
export function packetTypes(body) {
  const bytes = Uint8Array.from(atob(body), c => c.charCodeAt(0));
  if (!bytes.length) throw new Error('Empty packet');
  if (bytes[0] >= 32) return [bytes[0]];
  const types = []; let i = 0;
  while (i < bytes.length) {
    const prefix = bytes[i++];
    const length = prefix < 32 ? prefix * 256 + bytes[i++] : prefix - 32;
    if (!length || i + length > bytes.length || bytes[i] < 32) throw new Error('Invalid envelope');
    types.push(bytes[i]); i += length;
  }
  return types;
}
export function validateRecord(record) {
  if (record?.schema !== 1 || record.framing !== 'header-free' || !Array.isArray(record.packets)
    || !Number.isFinite(record.durationMs) || record.durationMs < 0) throw new Error('Expected schema-1 header-free record');
  for (const rows of [record.packets, record.inputs ?? [], record.controls ?? []]) {
    let previous = 0;
    for (const row of rows) {
      if (!Number.isFinite(row.at) || row.at < previous || row.at > record.durationMs) throw new Error('Invalid record time');
      previous = row.at;
    }
  }
  for (const packet of record.packets) {
    if (typeof packet.body !== 'string' || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(packet.body)) throw new Error('Invalid packet base64');
    packetTypes(packet.body);
  }
  for (const input of record.inputs ?? []) normalizeInput(input);
  for (const control of record.controls ?? []) {
    if (!['close', 'play'].includes(control.action)) throw new Error('Unsupported session control');
  }
  let previousGap = -1;
  for (const gap of record.betweenLives ?? []) {
    if (![gap.fromMs, gap.untilMs].every(Number.isFinite) || gap.fromMs <= previousGap || gap.untilMs <= gap.fromMs || gap.untilMs > record.durationMs
      || !(record.controls ?? []).some(c => c.action === 'close' && c.at === gap.fromMs)
      || !(record.controls ?? []).some(c => c.action === 'play' && c.at > gap.fromMs && c.at < gap.untilMs)
      || !record.packets.some(p => p.at === gap.untilMs && packetTypes(p.body).includes(115))) throw new Error('Declared between-lives close/Play/appearance interval required');
    previousGap = gap.untilMs;
  }
  return record;
}
export function normalizeInput(input, { width = 960, height = 540 } = {}) {
  if (!['mousemove', 'mousedown', 'mouseup', 'pointermove', 'pointerdown', 'pointerup', 'keydown', 'keyup', 'blur', 'resize'].includes(input.type)) throw new Error('Unsupported DOM event');
  const pointer = /^(mouse|pointer)/.test(input.type);
  if (pointer && (![input.x, input.y].every(Number.isFinite) || input.x < 0 || input.x > 1 || input.y < 0 || input.y > 1)) throw new Error('Expected normalized pointer');
  if (input.type.startsWith('key') && !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'Space', 'Enter', 'Escape'].includes(input.code)) throw new Error('Unsupported key');
  const target = input.target ?? 'surface';
  if (!['surface', 'overlay', 'input', 'textarea', 'editable', 'button'].includes(target)) throw new Error('Unsupported target');
  return { type: input.type, target, x: pointer ? input.x * width : null, y: pointer ? input.y * height : null,
    code: input.code ?? null, repeat: input.repeat ?? false, buttons: input.buttons ?? (input.type.endsWith('down') && pointer ? 1 : 0), button: input.button ?? 0 };
}
export function targetFor(peer, target) {
  if (!['canvas-wasm', 'webgpu-wasm'].includes(peer)) throw new Error('Unknown browser peer');
  return target === 'surface' ? (peer === 'canvas-wasm' ? '#slither-client-root > canvas' : '#game') : `#replay-${target}`;
}
export function buildSchedule(record, { expectedCommands, appearanceAtMs, stepMs = 1000 / 60, label = 'offline browser replay' }) {
  validateRecord(record);
  if (!Number.isFinite(stepMs) || stepMs <= 0 || !Number.isFinite(appearanceAtMs) || appearanceAtMs < 0) throw new Error('Explicit frame interval and appearance expectation required');
  if (record.packets.some(p => p.at === 0)) throw new Error('Packet at origin precedes the scheduled startup/Connect acknowledgment');
  if ((record.betweenLives ?? []).some(g => g.fromMs <= appearanceAtMs)) throw new Error('Between-lives interval must follow initial own');
  const rows = [...record.packets.map((p, index) => ({ sourceId: `packet-${index}`, kind: 'packet', atMs: p.at, data: p })),
    ...(record.inputs ?? []).map((p, index) => ({ sourceId: `input-${index}`, kind: 'input', atMs: p.at, data: p })),
    ...(record.controls ?? []).map((p, index) => ({ sourceId: `control-${index}`, kind: p.action === 'close' ? 'packet' : 'input', atMs: p.at, data: { control: p.action } }))];
  // One initial reset/Connect RAF on BOTH peers, at the declared clock origin.
  rows.push({ sourceId: 'startup-frame', kind: 'frame', atMs: 0, data: null });
  for (let f = 0; f * stepMs <= record.durationMs; f++) rows.push({ sourceId: `frame-${f}`, kind: 'frame', atMs: f * stepMs, data: null });
  if (rows.filter(e => e.kind === 'frame').at(-1).atMs !== record.durationMs) rows.push({ sourceId: 'final-frame', kind: 'frame', atMs: record.durationMs, data: null });
  const ordered = orderEvents(rows);
  let draw = 0, frame = 0;
  const boundaries = [];
  ordered.forEach((event, eventSeq) => {
    const phases = event.kind === 'frame' ? ['update', 'draw']
      : event.data?.control ? [] : event.kind === 'packet' ? [...packetTypes(event.data.body).filter(t => t === 97).map(() => 'draw'),
        ...(packetTypes(event.data.body).includes(54) ? [] : ['update'])]
        : event.data.type === 'resize' ? ['draw'] : [];
    for (const phase of phases) boundaries.push({ id: `${event.sourceId}-${phase}-${boundaries.length}`, eventSeq, phase,
      frameId: frame, drawId: phase === 'draw' ? draw++ : null, atMs: event.atMs,
      eligibility: event.atMs < appearanceAtMs ? 'pre-appearance'
        : (record.betweenLives ?? []).some(g => event.atMs >= g.fromMs && event.atMs < g.untilMs) ? 'between-lives' : 'own' });
    if (event.kind === 'frame') frame++;
  });
  const schedule = validateSchedule({ schema: 2, label, durationMs: record.durationMs,
    events: ordered.map(({ sourceId, kind, atMs }) => ({ sourceId, kind, atMs })), boundaries, expectedCommands });
  return { schedule, deliveries: ordered.map(e => e.data) };
}
export async function coordinate(schedule, deliveries, peers, acknowledge) {
  for (let seq = 0; seq < schedule.events.length; seq++) {
    for (const peer of peers) {
      const ack = await peer.step(schedule.events[seq], deliveries[seq], seq);
      if (ack.seq !== seq || ack.sourceId !== schedule.events[seq].sourceId || !Object.is(ack.atMs, schedule.events[seq].atMs)) throw new Error('Peer acknowledgment boundary mismatch');
      await acknowledge(peer, ack);
    }
  }
}
