import { observation } from './observation.mjs';
import { safeCommand } from './schema.mjs';
export function traceAssembly(schedule, manifest) {
  const events = [], commands = [], samples = [];
  const observed = new Map(), pictures = new Map();
  let frame = 0, draw = 0;
  let complete = true;
  let appeared = false;
  const pictureDrawIds = () => [...pictures.keys()].map(sequence => {
    const sample = observed.get(sequence);
    if (sample?.phase !== 'draw') throw new Error('Picture acknowledgement has no matching draw observation');
    return sample.drawId;
  });
  return {
    // Optional validation witness; never serialized into an A1 v3 trace.
    pictureDrawIds,
    append(ack) {
      const eventSeq = events.length;
      if (ack.observationErrors?.length) complete = false;
      if (ack.seq !== eventSeq) throw new Error('Duplicate/out-of-order acknowledgment');
      if (!Array.isArray(ack.pictures)) throw new Error('Picture acknowledgements required');
      for (const sequence of ack.pictures) {
        if (typeof sequence !== 'string' || !/^(0|[1-9]\d*)$/.test(sequence) || pictures.has(sequence)) throw new Error('Invalid/duplicate picture acknowledgement');
        pictures.set(sequence, ack.atMs);
      }
      const event = { ...ack.event, effectIds: [...(ack.effects ?? [])] };
      events.push(event);
      for (const sent of ack.commands) {
        if (!safeCommand(sent.bytes)) throw new Error('Unsafe command in peer output');
        const effectId = sent.effectId ?? `send-1-${commands.length}`;
        event.effectIds.push(effectId);
        commands.push({ seq: commands.length, eventSeq, frameId: frame, effectId, ...sent });
      }
      for (const row of ack.observations) {
        const sequence = row[3];
        if (typeof sequence !== 'string' || !/^(0|[1-9]\d*)$/.test(sequence) || observed.has(sequence)) throw new Error('Invalid/duplicate observation identity');
        const o = observation(row, ack.geometry);
        const eligibility = o.own !== null ? 'own' : appeared ? 'between-lives' : 'pre-appearance';
        if (o.own !== null) appeared = true;
        const drawId = o.phase === 'draw' ? draw++ : null;
        const sample = { id: `${ack.sourceId}-${o.phase}-${samples.length}`, eventSeq, phase: o.phase,
          frameId: frame, drawId, atMs: ack.atMs, acknowledgedMs: null, eligibility,
          delta: o.delta, lag: { active: ack.lag.active, multiplier: o.multiplier,
            pingPending: ack.lag.pingPending, pongAgeMs: ack.lag.pongAgeMs },
          own: o.own, camera: o.camera, quality: { requested: 'high', active: ack.highQuality ? 'high' : 'low', successfulDraws: 0 },
          geometry: ack.geometry, status: { prepared: o.phase === 'draw', submitted: false, committed: false,
            retries: 0, sendFailed: ack.sendFailed, presentFailed: ack.failed } };
        observed.set(sequence, sample);
        samples.push(sample);
      }
      if (event.kind === 'frame') frame++;
    },
    finish(state) {
      // Match actual host receipts, even when a retry acknowledges a row after
      // its observation was drained. Equal numeric cells never identify a picture.
      pictureDrawIds(); // Validate orphan/non-draw receipts before dereferencing.
      const acknowledged = new Map([...pictures].map(([sequence, atMs]) => [observed.get(sequence).drawId, atMs]));
      let successfulDraws = 0;
      for (const sample of samples) {
        sample.status.submitted = sample.status.committed = sample.phase === 'draw' && acknowledged.has(sample.drawId);
        sample.acknowledgedMs = sample.status.committed ? acknowledged.get(sample.drawId) : null;
        if (sample.phase === 'draw' && sample.status.committed) successfulDraws++;
        sample.quality.successfulDraws = successfulDraws;
      }
      return { schema: 3, manifest, events, commands, samples, final: {
        atMs: state.atMs, events: events.length, commands: commands.length, samples: samples.length,
        draws: successfulDraws, lastBoundaryId: samples.at(-1)?.id ?? 'missing', ...state, completed: state.completed && complete,
      } };
    },
  };
}
