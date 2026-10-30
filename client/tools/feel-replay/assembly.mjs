import { observation } from './observation.mjs';
import { safeCommand } from './schema.mjs';
export function traceAssembly(schedule, manifest) {
  const events = [], commands = [], samples = [];
  let frame = 0, draw = 0, successfulDraws = 0;
  let complete = true;
  let appeared = false;
  return {
    append(ack) {
      const eventSeq = events.length;
      if (ack.observationErrors?.length) complete = false;
      if (ack.seq !== eventSeq) throw new Error('Duplicate/out-of-order acknowledgment');
      const event = { ...ack.event, effectIds: [...(ack.effects ?? [])] };
      events.push(event);
      for (const sent of ack.commands) {
        if (!safeCommand(sent.bytes)) throw new Error('Unsafe command in peer output');
        const effectId = sent.effectId ?? `send-1-${commands.length}`;
        event.effectIds.push(effectId);
        commands.push({ seq: commands.length, eventSeq, frameId: frame, effectId, ...sent });
      }
      for (const row of ack.observations) {
        const o = observation(row, ack.geometry);
        const eligibility = o.own !== null ? 'own' : appeared ? 'between-lives' : 'pre-appearance';
        if (o.own !== null) appeared = true;
        const drawId = o.phase === 'draw' ? draw++ : null;
        if (o.phase === 'draw' && !ack.failed) successfulDraws++;
        samples.push({ id: `${ack.sourceId}-${o.phase}-${samples.length}`, eventSeq, phase: o.phase,
          frameId: frame, drawId, atMs: ack.atMs, eligibility,
          delta: o.delta, lag: { active: ack.lag.active, multiplier: o.multiplier,
            pingPending: ack.lag.pingPending, pongAgeMs: ack.lag.pongAgeMs },
          own: o.own, camera: o.camera, quality: { requested: 'high', active: ack.highQuality ? 'high' : 'low', successfulDraws },
          geometry: ack.geometry, status: { prepared: o.phase === 'draw', submitted: o.phase === 'draw', committed: o.phase === 'draw' && !ack.failed,
            retries: 0, sendFailed: ack.sendFailed, presentFailed: ack.failed } });
      }
      if (event.kind === 'frame') frame++;
    },
    finish(state) {
      return { schema: 2, manifest, events, commands, samples, final: {
        atMs: state.atMs, events: events.length, commands: commands.length, samples: samples.length,
        draws: successfulDraws, lastBoundaryId: samples.at(-1)?.id ?? 'missing', ...state, completed: state.completed && complete,
      } };
    },
  };
}
