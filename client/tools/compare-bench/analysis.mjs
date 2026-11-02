// Authored offline analysis. Never use submission intervals or browser rAF as scanout.
export function csv(text) {
  const rows = []; let row = [], field = '', quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      if (quoted && text[i + 1] === '"') { field += '"'; i++; }
      else quoted = !quoted;
    } else if (c === ',' && !quoted) { row.push(field); field = ''; }
    else if (c === '\n' && !quoted) { row.push(field.replace(/\r$/, '')); rows.push(row); row = []; field = ''; }
    else field += c;
  }
  if (quoted) throw Error('Unclosed CSV quote');
  if (field || row.length) { row.push(field.replace(/\r$/, '')); rows.push(row); }
  const headers = rows.shift()?.map(h => h.replace(/^\uFEFF/, ''));
  if (!headers?.length || new Set(headers).size !== headers.length) throw Error('Invalid CSV headers');
  return rows.filter(r => r.some(Boolean)).map(r => {
    if (r.length !== headers.length) throw Error('CSV column count changed');
    return Object.fromEntries(headers.map((h, i) => [h, r[i]]));
  });
}
const number = v => v !== '' && v !== undefined && v !== 'NA' && Number.isFinite(Number(v)) ? Number(v) : null;
export function quantile(sorted, q) {
  const x = (sorted.length - 1) * q, i = Math.floor(x);
  return sorted[i] + (sorted[Math.ceil(x)] - sorted[i]) * (x - i);
}
export function presentation(text, { pid, swapchain, seconds, startMs, endMs }) {
  const rows = csv(text);
  const required = ['ProcessID', 'SwapChainAddress', 'CPUStartQPCTime', 'DisplayLatency', 'DisplayedTime', 'PresentMode', 'SyncInterval'];
  if (!rows.length || required.some(k => !(k in rows[0]))) throw Error('Need PresentMon 2.x --qpc_time_ms display metrics; no timing fallback');
  const selected = rows.filter(r => Number(r.ProcessID) === pid && r.SwapChainAddress === swapchain);
  const displayed = selected.filter(r => number(r.DisplayedTime) > 0 && number(r.DisplayLatency) !== null)
    .map(r => ({ ...r, at: number(r.CPUStartQPCTime) + number(r.DisplayLatency) }))
    .filter(r => Number.isFinite(r.at)).sort((a, b) => a.at - b.at);
  // Timestamp differences, rather than duration fields, include a long frame
  // with no intervening presentation. Do not merge unrelated browser surfaces.
  const times = displayed.filter(r => r.at >= startMs && r.at <= endMs).map(r => r.at);
  if (times.length < 3) throw Error('Fewer than three displayed frames on selected PID/swapchain');
  const gaps = times.slice(1).map((t, i) => t - times[i]);
  if (gaps.some(v => v <= 0)) throw Error('Duplicate display timestamps / multiple display layers: select an unambiguous surface');
  const sorted = [...gaps].sort((a, b) => a - b);
  return { measure: 'ETW display timestamp deltas (ms)', displayedFrames: times.length,
    medianMs: quantile(sorted, .5), p95Ms: quantile(sorted, .95), p99Ms: quantile(sorted, .99),
    longestStallMs: sorted.at(-1), stallsOver50Ms: gaps.filter(x => x > 50).length,
    stallsOver100Ms: gaps.filter(x => x > 100).length, stallsOver250Ms: gaps.filter(x => x > 250).length,
    achievedFps: times.length / seconds, coveredMs: times.at(-1) - times[0],
    boundaryGapMs: [times[0] - startMs, endMs - times.at(-1)],
    presentModes: [...new Set(selected.map(r => r.PresentMode))], syncIntervals: [...new Set(selected.map(r => r.SyncInterval))] };
}
export function hostStats(text) {
  return [...text.matchAll(/stats: ticks\/s=([\d.]+) deferred_ticks=(\d+) lost_seconds=([\d.]+) simulation_load=([\d.]+)% players=(\d+) bot_slots=(\d+)/g)]
    .map(m => ({ ticksPerSecond: +m[1], deferredTicks: +m[2], lostSeconds: +m[3], simulationLoad: +m[4], players: +m[5], bots: +m[6] }));
}
export function wholeMap({ camera, center, radius, zoom, viewport }) {
  if (![...camera, center, radius, zoom, ...viewport].every(Number.isFinite) || radius <= 0 || zoom <= 0) return false;
  return (radius + Math.abs(camera[0] - center)) * zoom <= viewport[0] / 2 + .01
    && (radius + Math.abs(camera[1] - center)) * zoom <= viewport[1] / 2 + .01;
}
export function validity(run) {
  const reasons = [];
  if (run.error) reasons.push(run.error);
  if (run.cleanupError) reasons.push('Acquisition cleanup failed');
  if (!run.metrics) reasons.push('No comparable displayed-frame measurement');
  if (!run.networkVerified) reasons.push('Loopback-only game traffic not verified');
  if (!run.windowSamples?.length) reasons.push('No window/load evidence');
  for (const s of run.windowSamples ?? []) {
    if (!s.foreground || s.occluded || s.minimized || !s.visible) reasons.push('Window not foreground and fully unoccluded');
    if (s.buildRunning) reasons.push('Build/test process running');
    if (s.width !== run.width || s.height !== run.height || s.display !== run.display || s.dpi !== run.dpi) reasons.push('Window size/display/DPI changed');
    if (s.refreshHz !== undefined && s.refreshHz !== run.refreshHz) reasons.push('Monitor refresh changed');
  }
  if (!run.stats?.length) reasons.push('No host tick stats');
  for (const s of run.stats ?? []) {
    if (s.ticksPerSecond < .98 / run.dt || s.deferredTicks > 0 || s.lostSeconds > 0) reasons.push('Host failed fixed tick cadence');
    if (s.bots !== run.bots || s.players !== 1) reasons.push('Wrong bot/player population');
  }
  if (!run.census?.length || run.census.some(s => !s.players?.[0]?.setup || !s.players[0].alive)) reasons.push('Player admission/alive state not verified throughout');
  if (!run.views?.length) reasons.push('Camera/drawn-pellet evidence unavailable');
  if (run.views?.some(v => !Number.isInteger(v.visiblePellets) || !Number.isInteger(v.retainedPellets))) reasons.push('Pellet census unavailable');
  if (run.views?.some(v => v.connected === false || v.alive === false)) reasons.push('Client not alive/connected');
  if (run.client === 'vlither' && run.views?.some(v => v.protocol !== 19)) reasons.push('Vlither protocol changed');
  if (run.scenario === 'battledome' && (!run.views?.length || run.views.some(v => !v.wholeMap))) reasons.push('Whole-map camera not verified (limited view)');
  if (run.scenario === 'battledome' && (!run.census?.length || run.census.some(v => !v.wholeMapFood))) reasons.push('Whole-map food delivery not enabled');
  if (run.metrics && (run.metrics.coveredMs < run.seconds * 1000 * .9 || run.metrics.boundaryGapMs.some(g => g > 250))) reasons.push('Incomplete display coverage at window boundaries');
  if (!run.vsync) reasons.push('Vsync state unknown');
  if (!run.adapter || !run.refreshHz) reasons.push('Adapter/display refresh unavailable');
  if (!run.renderAdapters?.length || run.renderAdapters.some(a => !a.name || a.name === 'unmapped')) reasons.push('Actual render GPU not established by per-process GPU engines');
  if (run.expectedRenderLuid && (run.renderAdapters?.length !== 1 || run.renderAdapters[0].luid !== run.expectedRenderLuid)) reasons.push('Requested adapter not established as the actual render adapter');
  if (run.targetDisplay && (run.display !== run.targetDisplay || run.refreshHz !== run.targetRefreshHz)) reasons.push('Window is not on the requested display/refresh experiment');
  return { valid: reasons.length === 0, reasons: [...new Set(reasons)] };
}
export function comparison(a, b) {
  const reasons = [];
  const gpu = r => (r.renderAdapters ?? []).map(a => a.luid).sort().join(',');
  if (!a.validity?.valid || !b.validity?.valid) reasons.push('invalid run');
  if (!gpu(a) || gpu(a) !== gpu(b)) reasons.push('different or unknown render adapters');
  for (const field of ['display', 'refreshHz', 'width', 'height', 'dpi', 'scenario', 'experiment', 'vsyncCondition']) {
    if (a[field] === undefined || a[field] !== b[field]) reasons.push(`different or unknown ${field}`);
  }
  if (a.vsyncCondition === 'recorded-modes-require-qualified-comparison' || b.vsyncCondition === 'recorded-modes-require-qualified-comparison') reasons.push('vsync/presentation conditions require qualification');
  return { classification: reasons.length ? 'labelled comparison only' : 'direct comparison', reasons };
}
export function schedule(rounds = 3, clients = ['original', 'native', 'ntl', 'vlither']) {
  if (!Number.isInteger(rounds) || rounds < 1) throw Error('rounds must be a positive integer');
  return Array.from({ length: rounds }, (_, round) =>
    (round % 2 ? [...clients].reverse() : clients).map(client => ({ round: round + 1, client }))).flat();
}
export function netlogProof(log, endpoint) {
  // TCP connect completion is network evidence; JS WebSocket hooks alone are not.
  const names = Object.fromEntries(Object.entries(log.constants?.logEventTypes ?? {}).map(([k, v]) => [v, k]));
  const connected = log.events?.filter(e => /TCP_CONNECT_ATTEMPT/.test(names[e.type]) && e.params?.remote_address && e.params?.net_error === undefined) ?? [];
  const addresses = connected.map(e => e.params.remote_address);
  const loopback = a => /^127\.\d+\.\d+\.\d+:\d+$/.test(a) || /^\[::1\]:\d+$/.test(a);
  return { verified: addresses.length > 0 && addresses.every(loopback) && addresses.includes(endpoint), addresses };
}
