// Standard browser APIs only. This measures canvas production, not scanout or
// pixel uniqueness. It cannot correlate a particular draw to an ETW display.
export function installContentProbe() {
  const records = [], canvases = new Map(), ids = new WeakMap(), outsideRaf=new Map();
  let active = false, overflow = false, next = 1, draws = null, callbackTime = null,rafCallbacks=0,lastCanvas=null;
  function id(canvas) {
    if (!ids.has(canvas)) { ids.set(canvas, next++); canvases.set(ids.get(canvas), canvas); }
    return ids.get(canvas);
  }
  function append(at, canvas) {
    if (!active) return;
    const last = records.at(-1);
    if (last?.at === at && last.canvas === canvas) return;
    if (records.length >= 262144) { overflow = true; return; }
    records.push({ at, canvas });
  }
  function activity(canvas) {
    if (!active || !canvas) return;
    // The common path draws many objects to one canvas in one callback. Observe
    // that canvas once, rather than adding a map/set operation per pellet.
    if(draws&&lastCanvas===canvas)return;
    const key = id(canvas);
    if (draws) {draws.add(key);lastCanvas=canvas;}
    else outsideRaf.set(key,(outsideRaf.get(key)??0)+1);
  }
  function wrap(proto, names) {
    if (!proto) return;
    for (const name of names) {
      const original = proto[name]; if (typeof original !== 'function') continue;
      proto[name] = function (...args) {
        const result = Reflect.apply(original, this, args);
        activity(this.canvas); return result;
      };
    }
  }
  wrap(globalThis.CanvasRenderingContext2D?.prototype, ['drawImage','fill','stroke','fillRect','strokeRect','putImageData','fillText','strokeText']);
  wrap(globalThis.WebGLRenderingContext?.prototype, ['drawArrays','drawElements','clear']);
  wrap(globalThis.WebGL2RenderingContext?.prototype, ['drawArrays','drawElements','drawArraysInstanced','drawElementsInstanced','blitFramebuffer','clear']);
  const raf = globalThis.requestAnimationFrame;
  globalThis.requestAnimationFrame = function (callback) {
    return Reflect.apply(raf, this, [function (at) {
      if(active)rafCallbacks++;
      const previous = draws, previousTime = callbackTime,previousCanvas=lastCanvas;
      draws = new Set(); callbackTime = at;lastCanvas=null;
      try { return Reflect.apply(callback, this, [at]); }
      finally { for (const canvas of draws) append(callbackTime, canvas); draws = previous; callbackTime = previousTime;lastCanvas=previousCanvas; }
    }]);
  };
  // Non-enumerable harness-owned observation API, separate from client state.
  Object.defineProperty(globalThis, '__frameBenchProduction', { value: Object.freeze({
    start() { records.length = 0;outsideRaf.clear(); overflow = false;rafCallbacks=0; active = true; return {timeOrigin:performance.timeOrigin,now:performance.now()}; },
    stop() { active = false; return { records: records.slice(), outsideRaf:[...outsideRaf],overflow,rafCallbacks, now: performance.now(), timeOrigin: performance.timeOrigin,
      canvases: [...canvases].map(([canvas, element]) => { const r = element.getBoundingClientRect(); return { canvas, width:element.width,height:element.height,rect:[r.x,r.y,r.width,r.height] }; }) }; },
  }), configurable:false });
}

export function contentIntervals(observation, { canvas, seconds, startMs, endMs }) {
  if (observation.overflow) throw Error('Canvas-production journal overflow');
  if(observation.outsideRaf?.some(([id,count])=>id===canvas&&count>0))throw Error('Game-canvas draws outside rAF are unsupported; no content FPS inferred');
  const times = [...new Set(observation.records.filter(r => r.canvas === canvas && Number.isFinite(r.at)
    && r.at >= startMs && r.at <= endMs).map(r => r.at))].sort((a,b)=>a-b);
  if (times.length < 3) throw Error('Missing canvas production');
  const gaps = times.slice(1).map((t,i)=>t-times[i]), sorted=[...gaps].sort((a,b)=>a-b);
  const q = fraction => {const x=(sorted.length-1)*fraction,i=Math.floor(x);return sorted[i]+(sorted[Math.ceil(x)]-sorted[i])*(x-i);};
  return { measure:'canvas draw activity grouped by rAF timestamp; not displayed-frame identity',
    frames:times.length, achievedFps:times.length/seconds, medianMs:q(.5),p95Ms:q(.95),p99Ms:q(.99),
    longestStallMs:sorted.at(-1),stallsOver50Ms:gaps.filter(x=>x>50).length,
    stallsOver100Ms:gaps.filter(x=>x>100).length,stallsOver250Ms:gaps.filter(x=>x>250).length,
    duplicateCallbacksRemoved:observation.records.filter(r=>r.canvas===canvas).length-times.length,
    displayCorrelation:'unestablished: do not equate compositor presents with game frames' };
}
