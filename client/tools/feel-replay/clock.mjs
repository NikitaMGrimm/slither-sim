// Adapted from the retained offline fence; no native timer or RAF survives bootstrap.
export function virtualClock(realm) {
  let now = 0, serial = 0;
  const timers = new Map(), frames = new Map(), NativeDate = realm.Date;
  function ClockDate(...args) {
    if (!new.target) return new NativeDate(1700000000000 + now).toString();
    return new NativeDate(...(args.length ? args : [1700000000000 + now]));
  }
  Object.setPrototypeOf(ClockDate, NativeDate); ClockDate.prototype = NativeDate.prototype;
  ClockDate.now = () => 1700000000000 + now; realm.Date = ClockDate;
  Object.defineProperty(realm.performance, 'now', { value: () => now });
  realm.setTimeout = (callback, delay = 0, ...args) => {
    if (typeof callback !== 'function') throw new Error('Function timer required');
    const id = ++serial;
    timers.set(id, { callback, args, at: now + Math.max(1, Number(delay) || 0), period: 0 }); return id;
  };
  realm.setInterval = (callback, delay = 0, ...args) => {
    const id = realm.setTimeout(callback, delay, ...args); timers.get(id).period = Math.max(1, Number(delay) || 0); return id;
  };
  realm.clearTimeout = realm.clearInterval = id => timers.delete(id);
  realm.requestAnimationFrame = callback => { const id = ++serial; frames.set(id, callback); return id; };
  realm.cancelAnimationFrame = id => frames.delete(id);
  return {
    now: () => now,
    advance(at) {
      if (at < now) throw new Error('Clock cannot move backwards');
      let budget = 100000;
      for (;;) {
        const next = [...timers].filter(([, t]) => t.at <= at).sort((a, b) => a[1].at - b[1].at || a[0] - b[0])[0];
        if (!next) break;
        if (!--budget) throw new Error('Virtual timer budget');
        const [id, t] = next; now = t.at;
        if (t.period) t.at += t.period; else timers.delete(id);
        t.callback(...t.args);
      }
      now = at;
    },
    frame() { const current = [...frames.values()]; frames.clear(); for (const callback of current) callback(now); },
    pendingFrames: () => frames.size,
  };
}
