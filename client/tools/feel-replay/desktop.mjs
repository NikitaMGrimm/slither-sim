import { scheduleHash } from './io.mjs';

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

// Authored zero-delay desktop receipt policy: packet Setup candidates are
// superseded before the next fresh redraw (#323). Freeze before candidate I/O;
// all preparation rows and their numeric observations remain untouched.
export function desktopExpectation(baseline, frozen) {
  const schedule = structuredClone(frozen);
  schedule.pictures = frozen.pictures.filter(id => {
    const b = frozen.boundaries.find(b => b.phase === 'draw' && b.drawId === id);
    return frozen.events[b.eventSeq].kind !== 'packet';
  });
  const trace = structuredClone(baseline), pictures = new Set(schedule.pictures);
  let count = 0;
  for (const s of trace.samples) {
    if (s.phase === 'draw' && !pictures.has(s.drawId)) {
      s.status.submitted = s.status.committed = false;
      s.acknowledgedMs = null;
    }
    if (s.phase === 'draw' && s.status.committed) count++;
    s.quality.successfulDraws = count;
  }
  trace.final.draws = count;
  trace.manifest.scheduleSha256 = scheduleHash(schedule);
  trace.manifest.input.label += ' + authored desktop receipt expectation';
  return { schedule, trace };
}
