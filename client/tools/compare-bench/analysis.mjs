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
  const windowRows = displayed.filter(r => r.at >= startMs && r.at <= endMs);
  const times = windowRows.map(r => r.at);
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
    presentModes: [...new Set(windowRows.map(r => r.PresentMode))], syncIntervals: [...new Set(windowRows.map(r => r.SyncInterval))] };
}
export function hostStats(text) {
  return [...text.matchAll(/stats: ticks\/s=([\d.]+) deferred_ticks=(\d+) lost_seconds=([\d.]+) simulation_load=([\d.]+)% players=(\d+) bot_slots=(\d+)/g)]
    .map(m => ({ ticksPerSecond: +m[1], deferredTicks: +m[2], lostSeconds: +m[3], simulationLoad: +m[4], players: +m[5], bots: +m[6] }));
}
export function presentMonFailure(text) {
  if (/[1-9]\d*\s+ETW events were lost/i.test(text)) return 'PresentMon lost ETW events; presentation coverage is incomplete';
  if (/[1-9]\d*\s+ETW buffers were lost/i.test(text)||/\b(?:EventsLost|BuffersLost|OverflowedPresents)=[1-9]\d*/.test(text)) return 'PresentMon ETW loss or present overflow; presentation coverage is incomplete';
  if (/access.*denied|error.*(?:trace|session)|failed to start/i.test(text)) return 'PresentMon ETW capture failed; inspect retained console output';
  return null;
}
export function presentMonArguments({session,file,pid,seconds}) {
  if(!Number.isInteger(pid)||pid<=0)throw Error('PresentMon requires an owned render process ID');
  // GPU Engine counters identify the adapter separately. Duration/input ETW
  // providers are unnecessary for the displayed timestamps measured here.
  return ['--session_name',session,'--output_file',file,'--process_id',String(pid),
    '--v2_metrics','--qpc_time_ms','--timed',String(seconds+12),'--terminate_after_timed',
    '--no_track_gpu','--no_track_input','--no_console_stats','--track_etw_status'];
}
export function transientFailure(run) {
  return (run.windowSamples??[]).some(s=>s.buildRunning||s.ownerInput||!s.foreground||s.occluded||s.minimized||!s.visible)
    || /ETW|PresentMon collection|presentation CSV|Foreground acquisition|Owner input|Window obscured/.test(run.error??'');
}
export function checkedPresentation(collector,{pids,seconds,startMs,endMs}) {
  if(collector.error||collector.exitCode!==0)throw Error(`PresentMon collection failed: ${collector.error??`exit ${collector.exitCode}`}`);
  const failure=presentMonFailure(collector.log??'');if(failure)throw Error(failure);
  const surfaces=new Map();
  for(const row of csv(collector.text)){
    if(!pids.includes(+row.ProcessID)||!(+row.DisplayedTime>0)||row.DisplayedTime==='NA')continue;
    if(!(+row.CPUStartQPCTime>=startMs&&+row.CPUStartQPCTime<=endMs))continue;
    const key=`${row.ProcessID}/${row.SwapChainAddress}`;surfaces.set(key,(surfaces.get(key)??0)+1);
  }
  if(surfaces.size!==1)throw Error(`Ambiguous/missing presentation surface (${surfaces.size}); no fastest-surface selection`);
  const [pid,swapchain]=[...surfaces.keys()][0].split('/');
  const metrics=presentation(collector.text,{pid:+pid,swapchain,seconds,startMs,endMs});
  if(metrics.coveredMs<seconds*1000*.9||metrics.boundaryGapMs.some(g=>g>250))throw Error('Incomplete display coverage at window boundaries');
  return {surfaces:[...surfaces],metrics};
}
export function wholeMap({ camera, center, radius, zoom, viewport }) {
  if (![...camera, center, radius, zoom, ...viewport].every(Number.isFinite) || radius <= 0 || zoom <= 0) return false;
  return (radius + Math.abs(camera[0] - center)) * zoom <= viewport[0] / 2 + .01
    && (radius + Math.abs(camera[1] - center)) * zoom <= viewport[1] / 2 + .01;
}
export function streamCoverage(rows,startMs,endMs,maxGapMs) {
  if(!Number.isFinite(startMs)||!Number.isFinite(endMs)||endMs<=startMs||!rows?.length)return false;
  if(rows.some(r=>!Number.isFinite(r.atMs)))return false;
  const at=rows.map(r=>r.atMs).sort((a,b)=>a-b).filter(t=>t>=startMs-maxGapMs&&t<=endMs+maxGapMs);
  if(!at.length||at[0]>startMs+maxGapMs||at.at(-1)<endMs-maxGapMs)return false;
  return at.every((t,i)=>i===0||t-at[i-1]<=maxGapMs);
}
export function contained(rect,bounds) {
  if(!rect||!bounds||![...rect,...bounds].every(Number.isFinite))return false;
  const [x,y,w,h]=rect,[left,top,width,height]=bounds;
  return w>0&&h>0&&x>=left&&y>=top&&x+w<=left+width&&y+h<=top+height;
}
export function sceneChecks(run) {
  if(!run.views?.length||!run.census?.length)return false;
  return run.views.every(v=>{
    const near=run.census.filter(c=>Math.abs(c.atMs-v.atMs)<=1200);
    if(!near.length)return false;
    const expected=near.map(c=>c.players?.[0]?.decoratedWireFood).filter(Number.isFinite);
    if(!expected.length)return false;
    if(v.sceneEvidence==='image')return v.imageSha256?.length===64&&v.pelletCandidates>0&&v.gameCanvasVisible===true&&v.sceneConfidence==='presence-only; exact count unverified';
    const live=v.receivedUneatenPellets??v.retainedPellets;
    // Wire cells collapse physical duplicates; decorations are included in this
    // baseline. Only retained eaten flights are excluded from the client count.
    const slack=32+Math.ceil(Math.max(...expected)*.05);
    return Number.isInteger(live)&&live>=Math.min(...expected)-slack&&live<=Math.max(...expected)+slack
      && Number.isInteger(v.visiblePellets)&&v.visiblePellets>0;
  });
}
export function drawnSnakeEvidence(run) {
  const reasons=[];let missing=!run.views?.length;
  for(const view of run.views??[]){
    if(!Object.hasOwn(view,'drawnLiveSnakes')){missing=true;continue;}
    const drawn=view.drawnLiveSnakes;
    if(!Number.isInteger(drawn)||drawn<0){reasons.push('Invalid drawn-live-snake count');continue;}
    const near=(run.census??[]).filter(c=>Number.isFinite(c.atMs)&&Number.isFinite(view.atMs)&&Math.abs(c.atMs-view.atMs)<=1200);
    const delivered=near.map(c=>c.players?.[0]?.wireLivingSnakes).filter(Number.isFinite);
    if(!delivered.length){missing=true;continue;}
    // Bracket with census receipts within 1.2 s, then allow two additional
    // snakes for in-flight appearance/death and presentation timing. A living
    // local player must still be drawn: the allowance never excuses zero.
    const ownAlive=near.some(c=>c.players?.[0]?.alive&&c.players[0].setup);
    const minimum=run.scenario==='battledome'&&view.wholeMap===true?Math.max(ownAlive?1:0,Math.min(...delivered)-2):ownAlive?1:0;
    if(drawn<minimum||drawn>Math.max(...delivered)+2)reasons.push('Drawn living snakes contradict nearby authoritative delivery');
  }
  return {status:reasons.length?'contradicted':missing?'unestablished':'consistent',reasons:[...new Set(reasons)],
    censusDistanceMs:1200,deliveryTimingAllowanceSnakes:2,
    scope:'whole-map count range; normal/limited views check the delivered upper bound and living local player'};
}
export function nativeProduction(journal,{startMs,endMs,qpcUnixOffset}){
  if(journal.overflow)throw Error('Native present-identity journal overflow');
  const rows=journal.rows.map(([ns,identity])=>({at:journal.unixStartMs+ns/1e6+qpcUnixOffset,identity})).filter(r=>r.at>=startMs&&r.at<=endMs);
  if(!rows.length)throw Error('No native present identities in measured window');
  const fresh=rows.filter((r,i)=>i===0||r.identity!==rows[i-1].identity);
  return {successfulPresentReceipts:rows.length,freshPreparedPictures:fresh.length,duplicatePicturePresents:rows.length-fresh.length,
    scope:'prepared-picture identity, not pixel uniqueness; no per-frame scene scan',
    contentDisplayCorrelation:rows.length===fresh.length?'all successful receipts used a new prepared picture; ETW identity mapping unestablished':'duplicate native pictures; displayed-content mapping unestablished'};
}
export function windowReasons(run) {
  const reasons = [];
  if (!run.windowSamples?.length) reasons.push('No window/load evidence');
  for (const s of run.windowSamples ?? []) {
    if (!s.foreground || s.occluded || s.minimized || !s.visible) reasons.push('Window not foreground and fully unoccluded');
    if (s.buildRunning) reasons.push('Build/test process running');
    if (s.width !== run.width || s.height !== run.height || s.display !== run.display || s.dpi !== run.dpi) reasons.push('Window size/display/DPI changed');
    if (!Number.isFinite(s.refreshHz) || s.refreshHz <= 0 || s.refreshHz !== run.refreshHz) reasons.push('Monitor refresh changed or unavailable');
    if(!Number.isFinite(s.nominalRefreshHz)||s.nominalRefreshHz<=0||Math.abs(s.nominalRefreshHz-s.refreshHz)>.5)reasons.push('Current DisplayConfig and mode refresh disagree or unavailable');
    if(!contained(s.clientRect,s.displayBounds))reasons.push('Client area is clipped or spans displays');
    if(s.ownerInput)reasons.push('Owner input during acquisition');
    if(s.gpuCounterError||!s.gpuEngines?.length)reasons.push('Missing current GPU counter evidence');
    const clock=run.clock??run.machine?.clock;
    if(clock&&(!Number.isFinite(s.unixMs)||Math.abs((s.qpcMs-s.unixMs)-(clock.qpcMs-clock.unixMs))>10))reasons.push('QPC/UNIX clock mapping changed or unavailable');
  }
  if(!streamCoverage(run.windowSamples,run.startMs,run.endMs,750))reasons.push('window telemetry does not cover the measured window');
  return [...new Set(reasons)];
}
export function validity(run) {
  const reasons = windowReasons(run);
  if (run.error) reasons.push(run.error);
  if (run.cleanupError) reasons.push('Acquisition cleanup failed');
  if (!run.metrics) reasons.push('No comparable displayed-frame measurement');
  if(run.client==='ntl'){
    if(!run.networkVerified)reasons.push(run.networkProof?.failure??'Loopback game traffic and named probe allowance not verified');
  }else{
    if(!run.nativeNetwork||run.nativeNetwork.status==='unavailable')reasons.push('Native endpoint sampling unavailable');
    if(run.nativeNetwork?.rejectedEndpoints?.length)reasons.push('Non-loopback native endpoint observed');
  }
  for(const [name,rows,gap] of [['window',run.windowSamples,750],['host stats',run.stats,1200],['host census',run.census,1200],['view',run.views,3000]]) {
    if(!streamCoverage(rows,run.startMs,run.endMs,gap))reasons.push(`${name} telemetry does not cover the measured window`);
  }
  if ((run.windowSamples ?? []).some((s, i, a) => i > 0 && s.qpcMs - a[i-1].qpcMs > 750)) reasons.push('Window/load sampler gap exceeded 750 ms');
  if (run.windowSamples?.length && Number.isFinite(run.endMs) && run.endMs - run.windowSamples.at(-1).qpcMs > 750) reasons.push('Window monitor ended before measurement');
  if (run.views?.some((v, i, a) => i > 0 && Number.isFinite(v.atMs) && v.atMs - a[i-1].atMs > 3000)) reasons.push('View telemetry stalled');
  if (!run.stats?.length) reasons.push('No host tick stats');
  for (const s of run.stats ?? []) {
    if (s.ticksPerSecond < .98 / run.dt || s.deferredTicks > 0 || s.lostSeconds > 0) reasons.push('Host failed fixed tick cadence');
    if (s.bots !== run.bots || s.players !== 1) reasons.push('Wrong bot/player population');
  }
  if (!run.census?.length || run.census.some(s => !s.players?.[0]?.setup || !s.players[0].alive)) reasons.push('Player admission/alive state not verified throughout');
  if (run.census?.some(s => s.admittedBotSlots !== run.bots)) reasons.push('Not all internal bot seats admitted');
  if (run.census?.some(s => s.players?.[0]?.protocol !== (run.client === 'ntl' ? 13 : 19))) reasons.push('Unexpected negotiated protocol');
  if (run.scenario === 'battledome' && run.census?.some(s => s.players?.[0]?.wireLivingSnakes !== s.totalAliveSnakes)) reasons.push('Host did not deliver every living snake');
  if (!run.views?.length) reasons.push('Camera/drawn-pellet evidence unavailable');
  if(run.views?.some(v=>v.sceneEvidence==='image'&&(v.viewport?.[0]!==run.width||v.viewport?.[1]!==run.height)))reasons.push('Browser content viewport does not match requested client area');
  if(!sceneChecks(run))reasons.push('Client scene does not match authoritative delivered food / image evidence');
  reasons.push(...drawnSnakeEvidence(run).reasons);
  if (run.views?.some(v => v.connected === false || v.alive === false)) reasons.push('Client not alive/connected');
  if (run.client === 'vlither' && run.views?.some(v => v.protocol !== 19)) reasons.push('Vlither protocol changed');
  if (run.scenario === 'normal' && ['original','native'].includes(run.client)
      && run.views?.some(v => v.userZoom !== 1)) reasons.push('Normal scenario left stock zoom');
  if (run.scenario === 'battledome' && run.viewCondition!=='ntl-farthest' && (!run.views?.length || run.views.some(v => !v.wholeMap))) reasons.push('Whole-map camera not verified (limited view)');
  if(run.scenario==='battledome'&&run.views?.some(v=>v.sceneEvidence==='image'&&!v.boundaryAnchor))reasons.push('Image boundary is unanchored to outer map extent; whole-map acceptance unavailable');
  if(run.viewCondition==='ntl-farthest'){
    if(run.views?.some(v=>v.wholeMap===true))reasons.push('Limited-view row label conflicts with whole-map state');
    const target=run.ntlViewCalibration?.physicalPixelsPerWorldUnit;
    if(!Number.isFinite(target)||target<=0||!run.views?.length||run.views.some(v=>{
      const scale=v.boundaryAnchor?.physicalPixelsPerWorldUnit??v.zoom*run.width/v.viewport?.[0];
      return !Number.isFinite(scale)||Math.abs(scale/target-1)>.05;
    }))reasons.push('Farthest NTL view scale not matched within 5%');
  }
  if (run.scenario === 'battledome' && (!run.census?.length || run.census.some(v => !v.wholeMapFood))) reasons.push('Whole-map food delivery not enabled');
  if (run.metrics && (run.metrics.coveredMs < run.seconds * 1000 * .9 || run.metrics.boundaryGapMs.some(g => g > 250))) reasons.push('Incomplete display coverage at window boundaries');
  if (!run.vsync) reasons.push('Vsync state unknown');
  if (!run.adapter || !run.refreshHz) reasons.push('Adapter/display refresh unavailable');
  if (!run.renderAdapters?.length || run.renderAdapters.some(a => !a.name || a.name === 'unmapped')) reasons.push('Actual render GPU not established by per-process GPU engines');
  if (run.expectedRenderLuid && (run.renderAdapters?.length !== 1 || run.renderAdapters[0].luid !== run.expectedRenderLuid)) reasons.push('Requested adapter not established as the actual render adapter');
  if (run.targetDisplay && (run.display !== run.targetDisplay || run.refreshHz !== run.targetRefreshHz)) reasons.push('Window is not on the requested display/refresh experiment');
  if(run.client==='ntl'&&(!run.contentMetrics||!Number.isInteger(run.contentMetrics.frames)||run.contentMetrics.frames<3
    ||!Number.isFinite(run.contentMetrics.achievedFps)||run.contentMetrics.achievedFps<=0))reasons.push('No game-canvas production measure; compositor FPS is not game FPS');
  if(run.client==='ntl'&&(run.provenance?.ntl?.error||!run.provenance?.ntl?.tree?.sha256))reasons.push('NTL input provenance unavailable');
  if(run.telemetry==='off')reasons.push('Instrumentation-off diagnostic; scene/content acceptance unavailable');
  return { valid: reasons.length === 0, reasons: [...new Set(reasons)] };
}
export function comparison(a, b) {
  const reasons = [];
  const gpu = r => (r.renderAdapters ?? []).map(a => a.luid).sort().join(',');
  if((a.viewCondition??'whole-map')!==(b.viewCondition??'whole-map'))reasons.push('different view conditions');
  if (!a.validity?.valid || !b.validity?.valid) reasons.push('invalid run');
  if (!gpu(a) || gpu(a) !== gpu(b)) reasons.push('different or unknown render adapters');
  for (const field of ['display', 'refreshHz', 'width', 'height', 'dpi', 'scenario', 'experiment', 'vsyncCondition']) {
    if (a[field] === undefined || a[field] !== b[field]) reasons.push(`different or unknown ${field}`);
  }
  if (a.vsyncCondition === 'recorded-modes-require-qualified-comparison' || b.vsyncCondition === 'recorded-modes-require-qualified-comparison') reasons.push('vsync/presentation conditions require qualification');
  const modes=r=>r.metrics?.presentModes?.length===1&&!/^(unknown|na|other|)$/i.test(r.metrics.presentModes[0])?r.metrics.presentModes[0]:null;
  if(!modes(a)||modes(a)!==modes(b))reasons.push('different or unknown presentation modes');
  if(a.contentDisplayCorrelation!=='established'||b.contentDisplayCorrelation!=='established')reasons.push('new game content is not correlated to displayed frames');
  if(a.views?.some(v=>v.sceneEvidence==='image')||b.views?.some(v=>v.sceneEvidence==='image'))reasons.push('picture presence evidence does not establish exact scene counts');
  if(drawnSnakeEvidence(a).status!=='consistent'||drawnSnakeEvidence(b).status!=='consistent')reasons.push('drawn-snake scene acceptance is unestablished or contradicted');
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
  const attempts = log.events?.filter(e => /TCP_CONNECT_ATTEMPT/.test(names[e.type])) ?? [];
  const starts = attempts.filter(e => e.params?.address || e.params?.remote_address);
  const addresses = starts.map(e => e.params.address ?? e.params.remote_address);
  const successful = starts.filter(e => attempts.some(end => end.source?.id === e.source?.id && end.phase === 2 && !end.params?.net_error));
  const loopback = a => /^127\.\d+\.\d+\.\d+:\d+$/.test(a) || /^\[::1\]:\d+$/.test(a);
  const probeEndpoint='[2001:4860:4860::8888]:443',allowance='no-packet reachability probe';
  // Chromium's probe calls UDP ConnectAsync then GetLocalAddress, without I/O:
  // https://github.com/chromium/chromium/blob/f842f52ef39fd97b449eb27cff414bf05a98ddce/net/dns/host_resolver_manager.cc#L1644
  // The allowance is socket-scoped, not an endpoint whitelist. Both linked UDP
  // lifetimes must close, contain only the verified non-I/O event shape, and
  // resolve through source dependencies to an uncached reachability check.
  const sourceTypes=Object.fromEntries(Object.entries(log.constants?.logSourceType??{}).map(([k,v])=>[v,k]));
  const key=s=>Number.isInteger(s?.id)&&Number.isInteger(s?.type)?`${s.type}/${s.id}`:null;
  const sources=new Map();
  for(const event of log.events??[]){const k=key(event.source);if(k){if(!sources.has(k))sources.set(k,[]);sources.get(k).push(event);}}
  const rows=s=>sources.get(key(s))??[];
  const time=e=>e?.time!==null&&e?.time!==''&&Number.isFinite(Number(e?.time))?Number(e.time):NaN;
  const ioParams=value=>value&&typeof value==='object'&&Object.entries(value).some(([k,v])=>/bytes?|payload|buffer|read|write|send|recv|sent|receiv/i.test(k)||ioParams(v));
  const lifetime=events=>{
    const begin=events.filter(e=>names[e.type]==='SOCKET_ALIVE'&&e.phase===1),end=events.filter(e=>names[e.type]==='SOCKET_ALIVE'&&e.phase===2);
    if(begin.length!==1||end.length!==1||!Number.isFinite(time(begin[0]))||!Number.isFinite(time(end[0]))||time(end[0])<time(begin[0]))return null;
    if(events.some(e=>!Number.isFinite(time(e))||time(e)<time(begin[0])||time(e)>time(end[0])||ioParams(e.params)))return null;
    return {begin:begin[0],end:end[0]};
  };
  const allowedEvents=new Set(),probes=[];
  for(const connect of log.events??[]){
    if(names[connect.type]!=='UDP_CONNECT'||connect.phase!==1||connect.params?.address!==probeEndpoint||sourceTypes[connect.source?.type]!=='UDP_SOCKET')continue;
    const udp=rows(connect.source),u=lifetime(udp);if(!u||udp.some(e=>!['SOCKET_ALIVE','UDP_CONNECT'].includes(names[e.type])))continue;
    const connects=udp.filter(e=>names[e.type]==='UDP_CONNECT');
    if(connects.length!==2||connects.filter(e=>e.phase===1).length!==1||connects.filter(e=>e.phase===2).length!==1)continue;
    const end=connects.find(e=>e.phase===2);if(time(end)<time(connect))continue;
    if(connects.some(e=>e.params?.address!==undefined&&e.params.address!==probeEndpoint))continue;
    const wrapperSource=u.begin.params?.source_dependency;
    if(sourceTypes[wrapperSource?.type]!=='UDP_CLIENT_SOCKET')continue;
    const wrapper=rows(wrapperSource),w=lifetime(wrapper);if(!w||wrapper.some(e=>!['SOCKET_ALIVE','SOCKET_OPEN','SOCKET_CONNECT'].includes(names[e.type])))continue;
    const opens=wrapper.filter(e=>names[e.type]==='SOCKET_OPEN'),connections=wrapper.filter(e=>names[e.type]==='SOCKET_CONNECT');
    if(opens.length!==1||opens[0].phase!==0||connections.length!==1||connections[0].phase!==0||connections[0].params?.address!==probeEndpoint)continue;
    const parent=w.begin.params?.source_dependency;
    const checks=rows(parent).filter(e=>names[e.type]==='HOST_RESOLVER_MANAGER_IPV6_REACHABILITY_CHECK'&&e.phase===0&&e.params?.cached===false&&typeof e.params.ipv6_available==='boolean'
      &&time(e)>=time(connect)&&time(e)<=Math.max(time(u.end),time(w.end))+1);
    if(checks.length!==1)continue;
    for(const e of [...udp,...wrapper])allowedEvents.add(e);
    probes.push({name:allowance,endpoint:probeEndpoint,udpSocket:connect.source,wrapperSocket:wrapperSource,
      reachabilitySource:parent,closed:true,bytesSent:0,bytesReceived:0,socketIoEvents:0});
  }
  const transportPeers=[];
  for(const event of log.events??[]){if(!/TCP|UDP|QUIC|SOCKET|WEBTRANSPORT/i.test(names[event.type]??''))continue;
    for(const key of ['address','remote_address','peer_address','destination','server_address']) {
      const peer=event.params?.[key];if(typeof peer==='string')transportPeers.push({protocol:names[event.type],peer,allowedProbe:allowedEvents.has(event)&&peer===probeEndpoint});
    }
  }
  const rejectedPeers=transportPeers.filter(p=>!loopback(p.peer)&&!p.allowedProbe);
  const gameSocketVerified=successful.some(e => (e.params.address ?? e.params.remote_address) === endpoint);
  const failure=rejectedPeers.length?`Non-loopback transport fails the "${allowance}" allowance (exact closed, attributed, zero-I/O UDP ${probeEndpoint} required): ${[...new Set(rejectedPeers.map(p=>p.peer))].slice(0,5).join(', ')}`:null;
  return { verified: addresses.length > 0 && addresses.every(loopback)&&rejectedPeers.length===0&&gameSocketVerified,
    gameSocketVerified,addresses:[...new Set(addresses)],rejectedPeers,failure,
    externalSocketAttemptsObserved:transportPeers.some(p=>!loopback(p.peer)),
    noPacketReachabilityProbeCount:probes.length,
    allowances:[{name:allowance,endpoint:probeEndpoint,count:probes.length,probes}] };
}
