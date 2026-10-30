import { differences } from './differences.mjs';

// Authored synchronous-present windows, frozen before executing LocalGame.
// Missed redraws coalesce; transport and input wait for the host to return.
// This models CPU call order, not GPU completion or measured scanout latency.
export function desktopSteps(steps, delayMs, oneFrame = false) {
  if (!Number.isFinite(delayMs) || delayMs < 0) throw new Error('Invalid acknowledgment delay');
  let busyUntil = 0;
  const planned = [];
  const displays = [...new Set(steps.filter(s => s.event.kind === 'frame').map(s => s.event.actualMs))];
  const nextDisplay = new Map(displays.map((t, i) => [t, displays[i + 1] ?? t + delayMs]));
  for (const original of steps) {
    const step = structuredClone(original), now = step.event.actualMs;
    if (step.event.kind === 'frame') {
      step.skippedRedraw = now < busyUntil;
      if (!step.skippedRedraw) {
        // Use the authored next display time, avoiding addition-vs-multiply
        // rounding that could invent a missed redraw at high refresh rates.
        busyUntil = oneFrame ? nextDisplay.get(now) : now + delayMs;
        step.ackAtMs = busyUntil;
        if (delayMs) planned.push({ event: { ...step.event, sourceId: `${step.event.sourceId}-ack`,
          kind: 'input', type: 'presentation-ack', scheduledMs: busyUntil, actualMs: busyUntil },
        actions: [], acknowledgment: true });
      }
    } else {
      step.event.actualMs = Math.max(now, busyUntil);
    }
    planned.push(step);
  }
  // A tape's next-session packets are available only once reset presentation
  // has allowed Connect. This is an authored causal barrier, not packet loss
  // or a candidate-derived timestamp. Preserve their original scheduled times.
  let connectReady = planned.find(s => s.event.sourceId === 'startup-frame')?.ackAtMs ?? 0;
  for (const original of steps) {
    const step = planned.find(s => s.event.sourceId === original.event.sourceId);
    if (step.control === 'play') {
      const reset = planned.find(s => s.event.kind === 'frame' && !s.skippedRedraw && s.event.actualMs >= step.event.actualMs);
      if (!reset) throw new Error('Play has no authored reset redraw');
      connectReady = reset.ackAtMs;
    } else if (step.packet) step.event.actualMs = Math.max(step.event.actualMs, connectReady);
  }
  // A synchronous return/ack precedes queued transport, then input, then redraw.
  const rank = s => s.acknowledgment ? 0 : s.event.kind === 'packet' ? 1 : s.event.kind === 'input' ? 2 : 3;
  return planned.sort((a, b) => a.event.actualMs - b.event.actualMs || rank(a) - rank(b))
    .map((s, seq) => ({ ...s, event: { ...s.event, seq } }));
}

export function classifyPath(path) {
  if (/^manifest\./.test(path)) return 'binding';
  if (/^commands/.test(path)) return 'command bytes or times';
  if (/^events/.test(path)) return 'event/service timing or effects';
  if (/^final/.test(path)) return 'completion/counts';
  if (/\.camera\.ring\[/.test(path)) return 'camera ring';
  if (/\.camera\./.test(path)) return 'camera samples';
  if (/^samples(\[\d+\])?$/.test(path) || /\.(atMs|eventSeq|frameId|drawId|id|phase|eligibility)$/.test(path)) return 'update/draw timing or missing boundary';
  if (/\.status\.|\.quality\./.test(path)) return 'presentation accounting';
  return 'world/input state at boundary';
}

// Retain ALL exact differences. This diagnostic never replaces A1's frozen
// completeness/provenance gate, nor learns expectations from a candidate trace.
export function desktopComparison(left, right) {
  const rows = differences(left, right).map(row => ({ ...row, classification: classifyPath(row.path) }));
  const classes = {};
  for (const row of rows) classes[row.classification] = (classes[row.classification] ?? 0) + 1;
  const frames = trace => trace.samples.filter(s => s.phase === 'update' && trace.events[s.eventSeq].kind === 'frame');
  const firstOwn = trace => trace.samples.find(s => s.own !== null)?.atMs ?? null;
  return { classes, rows, counts: [left, right].map(trace => ({ events: trace.events.length,
    commands: trace.commands.length, samples: trace.samples.length, draws: trace.final.draws,
    preparedDraws: trace.samples.filter(s => s.phase === 'draw').length,
    frameUpdates: frames(trace).length, firstOwnMs: firstOwn(trace), pending: [trace.final.pendingEffects, trace.final.pendingFrames] })) };
}

// Compare the actual sample/command histories independently of host event IDs
// and presentation receipts. Kept alongside, never instead of, the full report.
export function coreHistoryComparison(left, right) {
  const history = trace => ({ manifest: {}, events: [], final: {},
    commands: trace.commands.map(({ bytes, decisionMs, sendMs, ackMs }) => ({ bytes, decisionMs, sendMs, ackMs })),
    samples: trace.samples.map(({ phase, atMs, delta, lag, own, camera, geometry }) => ({ phase, atMs, delta, lag, own, camera, geometry })) });
  const rows = differences(history(left), history(right)).map(row => ({ ...row, classification: classifyPath(row.path) }));
  const classes = {};
  for (const row of rows) classes[row.classification] = (classes[row.classification] ?? 0) + 1;
  return { exact: rows.length === 0, classes, rows };
}
