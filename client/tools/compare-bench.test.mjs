import test from 'node:test';
import assert from 'node:assert/strict';
import { csv, presentation,checkedPresentation,presentMonArguments,transientFailure,hostStats, validity, wholeMap, schedule, netlogProof, comparison, presentMonFailure,streamCoverage,contained,nativeProduction,drawnSnakeEvidence } from './compare-bench/analysis.mjs';
import { endpointFrom, completeJsonLines,outsideRepository,lineJournal,nativeViewEvidence } from './compare-bench/run.mjs';
import { adapterFlags,profilePreferences,chromeSession,CDP,navigateLoaded } from './compare-bench/browser.mjs';
import {roundPlan,anchoredBoundary,zoomPlateau,checkNtlCalibratedView,settleNtlView,settleNativeView,collectRounds,requireNtlDisplayGate,sessionReport} from './compare-bench/matrix.mjs';
import { installContentProbe,contentIntervals } from './compare-bench/content-probe.mjs';
import { pictureEvidence } from './compare-bench/picture.mjs';
import {resourceScope,collectorCompletion,recoverCollectorLog,cleanupPresentMon} from './compare-bench/resources.mjs';
import {nativeNetworkEvidence,startNativeNetwork,nativeNetworkMaxSeconds} from './compare-bench/native-network.mjs';
import {spawn,execFileSync} from 'node:child_process';
import vm from 'node:vm';
import path from 'node:path';
import http from 'node:http';
import fs from 'node:fs/promises';
import os from 'node:os';
import {EventEmitter} from 'node:events';
import {PassThrough} from 'node:stream';

test('navigation waits for the matching document, including load before command acknowledgement',async()=>{
  for(const early of [true,false]){
    let resolved=false,seen=0;
    const load=(page,loaderId)=>page.onEvent({method:'Page.lifecycleEvent',params:{name:'load',frameId:'frame',loaderId}});
    const page={onEvent:()=>seen++,send:async(method)=>{
      if(method==='Page.setLifecycleEventsEnabled')return {};
      assert.equal(typeof page.onEvent,'function');load(page,'old-document');
      if(early)load(page,'new-document');else setTimeout(()=>{assert.equal(resolved,false);load(page,'new-document');},5);
      return {frameId:'frame',loaderId:'new-document'};
    }};
    const previous=page.onEvent;await navigateLoaded(page,'http://authored.invalid');resolved=true;
    assert.equal(page.onEvent,previous);assert.equal(seen,2);
  }
  const page={send:async()=>({frameId:'frame',loaderId:'missing'})};
  await assert.rejects(navigateLoaded(page,'http://authored.invalid',5),/load timed out/);
  assert.equal(page.onEvent,undefined);
});

test('first-round controls and limited-view pairs retain reversed client ordering',()=>{
  const plan=roundPlan(3,['original','native','ntl','vlither'],{limitedNtlView:true});
  assert.equal(plan.length,38);assert.equal(plan.filter(r=>r.telemetry==='off').length,8);
  assert.ok(plan.filter(r=>r.telemetry==='off').every(r=>r.round===1&&r.client!=='vlither'));
  assert.deepEqual([...new Set(plan.filter(r=>r.round===2).map(r=>r.client))],['vlither','ntl','native','original']);
  for(const round of [1,2,3])for(const client of ['original','native'])assert.ok(plan.some(r=>r.round===round&&r.client===client&&r.scenario==='battledome'&&r.viewCondition==='ntl-farthest'&&r.telemetry==='on'));
  for(const client of ['original','native']){
    const matched=plan.filter(r=>r.round===1&&r.client===client&&r.viewCondition==='ntl-farthest');
    assert.deepEqual(matched.map(r=>r.telemetry),client==='original'?['on','off']:['off','on']);
    assert.ok(matched.every(r=>r.scenario==='battledome'));
  }
  assert.equal(roundPlan(3,['original','native','ntl','vlither'],{telemetryControls:false}).length,24);
});
test('run/smoke require a passing NTL display gate before planning',()=>{
  for(const calibration of [undefined,{}, {displayGate:null},{displayGate:{}},{displayGate:{passed:false,reasons:[]}},{displayGate:{passed:'true'}}]){
    assert.throws(()=>requireNtlDisplayGate(calibration),/NTL display preflight rejected; no run plan or timed rounds: missing or non-passing displayGate evidence/);
  }
  assert.throws(()=>requireNtlDisplayGate({displayGate:{passed:false,reasons:['Window occluded','Wrong display']}}),/NTL display preflight rejected.*Window occluded; Wrong display/);
  assert.doesNotThrow(()=>requireNtlDisplayGate({displayGate:{passed:true,reasons:[]}}));
});

test('outer-circle anchoring rejects a pit circle and measures a zoom plateau',()=>{
  const census={radius:1000,players:[{alive:true,setup:true,x:750,y:0}]};
  const view={viewport:[1000,800],boundary:{x:350,y:400,radius:200}};
  const anchor=anchoredBoundary(view,census);assert.equal(anchor.physicalPixelsPerWorldUnit,.2);assert.equal(anchor.wholeMap,true);
  assert.equal(anchoredBoundary({...view,boundary:{x:350,y:400,radius:100}},census),null);
  assert.equal(zoomPlateau([anchor,{...anchor,physicalPixelsPerWorldUnit:.201},anchor]),true);
  assert.equal(zoomPlateau([anchor,{...anchor,physicalPixelsPerWorldUnit:.25},anchor]),false);
});
test('NTL rows reproduce calibration wheel/settings and settle before warmup',async()=>{
  const census={radius:1000,players:[{alive:true,setup:true,x:750,y:0}]},settings={zoomEnabled:true,slowReturnEnabled:false,controls:[]};
  const picture={viewport:[1000,800],boundary:{x:50,y:400,radius:600}};
  const calibration={physicalPixelsPerWorldUnit:.6,wholeMap:false,totalWheelNotches:1040,settings};
  const calls=[],browser={ntlSettings:settings,zoomOut:async n=>calls.push(['wheel',n])};
  const settled=await settleNtlView(browser,calibration,'ntl-farthest',async i=>{calls.push(['picture',i]);return {picture,census};},async ms=>calls.push(['settle',ms]));
  calls.push(['warmup']);assert.deepEqual(calls,[['wheel',1040],['picture',0],['settle',500],['picture',1],['warmup']]);
  assert.equal(settled.samples.length,2);assert.equal(settled.samples[1].anchor.wholeMap,false);
  const changed={...browser,ntlSettings:{...settings,slowReturnEnabled:true},zoomOut:()=>assert.fail('Wrong settings must fail before wheel input')};
  await assert.rejects(settleNtlView(changed,calibration,'ntl-farthest',()=>{},()=>{}),/zoom UI settings differ/);
  const drift=scale=>({viewport:[1000,800],boundary:{x:500-750*scale,y:400,radius:1000*scale}});
  await assert.rejects(settleNtlView(browser,calibration,'ntl-farthest',async i=>({picture:drift(i ? .624 : .6),census}),async()=>{}),/has not settled/);
  assert.throws(()=>checkNtlCalibratedView(drift(.8),census,calibration,'ntl-farthest',settings),/physical scale differs/);
  const whole={...calibration,physicalPixelsPerWorldUnit:.2,wholeMap:true},wholePicture=drift(.2);
  assert.equal(checkNtlCalibratedView(wholePicture,census,whole,'whole-map',settings).wholeMap,true);
  assert.throws(()=>checkNtlCalibratedView(wholePicture,census,whole,'ntl-farthest',settings),/row label differs/);
  assert.equal(roundPlan(1,['ntl'],{limitedNtlView:false}).find(r=>r.scenario==='battledome').viewCondition,'whole-map');
});
test('round interruption preserves current/remaining rounds and starts no later preflight',async()=>{
  for(const stopAt of ['before','preflight','acquire']){
    let interrupted=stopAt==='before';const preflights=[],acquired=[],writes=[];
    const result=await collectRounds({rounds:3,maxAttempts:3,plan:[1,2,3].map(round=>({round,client:'original'})),isInterrupted:()=>interrupted,
      preflight:async round=>{assert.equal(interrupted,false);preflights.push(round);if(stopAt==='preflight'){interrupted=true;throw Error('Interrupted preflight');}return {competingWork:[],idleMs:10000};},
      acquire:async item=>{acquired.push(item.round);if(item.round===2)interrupted=true;return {directory:`round-${item.round}`,run:{}};},
      persist:async rows=>writes.push(structuredClone(rows)),betweenAttempts:()=>assert.fail('No retry wait after interruption')});
    writes.push('summary');assert.ok(Array.isArray(writes.at(-2)),'attempts must be persisted before summary');
    assert.deepEqual(result.uncollectedRounds,stopAt==='acquire'?[2,3]:[1,2,3]);
    assert.deepEqual(preflights,stopAt==='before'?[]:stopAt==='preflight'?[1]:[1,2]);
    assert.deepEqual(acquired,stopAt==='acquire'?[1,2]:[]);
    assert.equal(result.roundAttempts.some(r=>r.round===3),false);
  }
});
test('a transient capture retries only its affected round and keeps prior failures',async()=>{
  const calls=[],result=await collectRounds({rounds:3,maxAttempts:3,plan:[1,2,3].map(round=>({round})),isInterrupted:()=>false,
    preflight:async(round,attempt)=>{calls.push([round,attempt]);return {competingWork:[],idleMs:10000};},
    acquire:async item=>({directory:`round-${item.round}-${item.attempt}`,run:item.round===1&&item.attempt===1?{error:'PresentMon lost ETW events'}:{}}),
    persist:async()=>{},betweenAttempts:async()=>{}});
  assert.deepEqual(calls,[[1,1],[2,1],[3,1],[1,2]]);assert.deepEqual(result.uncollectedRounds,[]);assert.equal(result.results.length,4);
});

test('retry index and summary select one complete attempt, retaining abandoned rows with reasons',async()=>{
  const plan=['original','ntl'].map(client=>({round:1,client,scenario:'battledome',viewCondition:'ntl-farthest',telemetry:'on'}));
  const collection=await collectRounds({rounds:1,maxAttempts:2,plan,isInterrupted:()=>false,
    preflight:async()=>({competingWork:[],idleMs:10000}),persist:async()=>{},betweenAttempts:async()=>{},
    acquire:async item=>({directory:`${item.client}-${item.attempt}`,valid:item.attempt===2||item.client==='original',
      run:{...item,error:item.client==='ntl'&&item.attempt===1?'PresentMon lost ETW events':undefined}})});
  const {summary,index}=sessionReport(collection,plan);
  assert.equal(index.results.length,4);assert.deepEqual(summary.selection,index.selection);
  assert.deepEqual(index.selection.selectedAttempts,[{round:1,attempt:2}]);
  assert.deepEqual(index.selection.selectedDirectories,['original-2','ntl-2']);
  assert.equal(index.selection.selectedRowCount,2);
  assert.deepEqual(index.comparisons.map(({a,b})=>[a,b]),[['original-2','ntl-2']]);
  assert.ok(index.results.filter(r=>r.attempt===1).every(r=>!r.comparisonSelection.included&&/transient/.test(r.comparisonSelection.reason)));
  assert.equal(summary.roundAttempts[0].comparisonSelection.included,false);
  for(const blocked of ['interrupted; current and remaining rounds uncollected',undefined]){
    const incomplete={...collection,results:collection.results.slice(0,1),roundAttempts:[{round:1,attempt:1,results:['original-1'],blocked}]};
    const report=sessionReport(incomplete,plan);assert.equal(report.index.comparisons.length,0);
    assert.equal(report.summary.selection.selectedRowCount,0);assert.equal(report.index.results[0].comparisonSelection.included,false);
  }
  const interrupted=sessionReport({...collection,roundAttempts:collection.roundAttempts.map(r=>({...r,blocked:'interrupted'}))},plan);
  assert.equal(interrupted.index.comparisons.length,0);
  const duplicate=sessionReport({...collection,roundAttempts:[{round:1,attempt:2,results:['original-2','original-2']}]},plan);
  assert.equal(duplicate.summary.selection.selectedRowCount,0);
  const extra=sessionReport({...collection,results:[...collection.results,{directory:'unrecorded',run:{...plan[0],attempt:2}}]},plan);
  assert.equal(extra.index.results.at(-1).comparisonSelection.included,false);
  assert.deepEqual(extra.summary.selection,index.selection);
});

test('matched native off control measures scale then acknowledges telemetry silence before warmup',async()=>{
  const calls=[];let zoom=.36,stopped=false;
  const options={target:.18,width:3840,readView:async()=>{calls.push('view');return nativeViewEvidence({setupOnly:true,zoom,viewport:[3840,2160]},1000);},
    wheel:async()=>{calls.push('wheel');zoom=.18;},stopTelemetry:async()=>{calls.push('stop');stopped=true;return 1234;}};
  const setup=await settleNativeView(options);assert.ok(stopped);calls.push('warmup');
  assert.deepEqual(calls,['view','wheel','view','stop','warmup']);
  assert.equal(setup.stoppedAtMs,1234);assert.match(setup.method,/measured setup-only.*stopped before warmup/);
  assert.equal(setup.physicalPixelsPerWorldUnit,.18);
  assert.equal(setup.view.wholeMap,null);assert.equal(setup.view.atMs,1000);
  await assert.rejects(settleNativeView({...options,stopTelemetry:async()=>{throw Error('Missing stop acknowledgement');}}),/Missing stop acknowledgement/);
  const on=await settleNativeView({...options,stopTelemetry:undefined});assert.match(on.method,/continuous/);assert.equal(on.stoppedAtMs,undefined);
  await assert.rejects(settleNativeView({...options,readView:async()=>({}),stopTelemetry:()=>assert.fail('Unmeasured scale must not stop setup')}),/physical zoom unavailable/);
});

test('collector exposes split live ETW loss before capture completion',async()=>{
  const child=new EventEmitter();child.stdout=new PassThrough();child.stderr=new PassThrough();
  let failure=null;const done=collectorCompletion(child,log=>{failure??=presentMonFailure(log);});
  child.stdout.write('[ETW Status] EventsLost=0 BuffersLost=0 OverflowedPresents=');assert.equal(failure,null);
  child.stdout.write('12\n');assert.match(failure,/overflow/i);
  child.emit('close',0);assert.match((await done).log,/OverflowedPresents=12/);
  assert.ok(transientFailure({error:failure}));
});

test('CSV quoting, BOM and malformed rows', () => {
  assert.deepEqual(csv('\uFEFFa,b\r\n"a,b","x""y"\r\n'), [{ a: 'a,b', b: 'x"y' }]);
  assert.throws(() => csv('a,b\n1\n'));
  assert.throws(() => csv('a\n"unterminated'));
});
test('display timestamps include latency and select exactly one PID/surface', () => {
  const header = 'ProcessID,SwapChainAddress,CPUStartQPCTime,DisplayLatency,DisplayedTime,PresentMode,SyncInterval\n';
  const text = header + ['7,A,100,10,10,Flip,1', '7,A,110,20,10,Flip,1', '7,A,120,100,NA,Flip,1', '7,A,200,10,10,Flip,1', '8,A,101,1,2,Flip,0', '7,B,102,1,2,Flip,0'].join('\n');
  const m = presentation(text, { pid: 7, swapchain: 'A', seconds: .2, startMs: 100, endMs: 300 });
  assert.equal(m.longestStallMs, 80);
  assert.equal(m.displayedFrames, 3);
  assert.equal(m.stallsOver50Ms, 1);
  assert.deepEqual(m.syncIntervals, ['1']);
  assert.throws(() => presentation('ProcessID,MsBetweenPresents\n7,10', {}), /no timing fallback/);
});
test('invalid/unknown validity cannot silently pass', () => {
  assert.equal(validity({}).valid, false);
  const run = { metrics: { coveredMs: 1000, boundaryGapMs: [0, 0] },startMs:0,endMs:1000, seconds: 1, dt: .007, bots: 100,
    scenario: 'battledome', nativeNetwork:{status:'sampled-loopback-only',complete:false,rejectedEndpoints:[]},width: 960, height: 540, display: 'D', dpi: 96,
    adapter: 'synthetic', renderAdapters: [{ name: 'Synthetic GPU', luid: 'luid1' }], refreshHz: 60, vsync: 'Fifo',
    windowSamples: [0,500,1000].map(atMs=>({atMs,qpcMs:atMs,foreground: true, occluded: false, minimized: false, visible: true, width: 960, height: 540, display: 'D', dpi: 96,refreshHz:60,nominalRefreshHz:60,clientRect:[0,0,960,540],displayBounds:[0,0,1920,1080],gpuEngines:[{instance:'gpu',utilization:2}]})),
    stats: [{atMs:0, ticksPerSecond: 142.85, deferredTicks: 0, lostSeconds: 0, bots: 100, players: 1 }],
    census: [{atMs:0, wholeMapFood: true, admittedBotSlots: 100, totalAliveSnakes: 100, players: [{ alive: true, setup: true, protocol: 19, wireLivingSnakes: 100,decoratedWireFood:40 }] }],
    views: [{atMs:0, wholeMap: true, visiblePellets: 40, retainedPellets: 40 }] };
  assert.equal(validity(run).valid, true);
  const refreshed=structuredClone(run);refreshed.machine={clock:{qpcMs:0,unixMs:0}};
  refreshed.windowSamples.forEach(s=>s.unixMs=s.qpcMs+1000000);
  assert.equal(validity(refreshed).valid,false);
  refreshed.clock={qpcMs:0,unixMs:1000000};assert.equal(validity(refreshed).valid,true);
  const ntl=structuredClone(run);ntl.client='ntl';ntl.census[0].players[0].protocol=13;
  ntl.networkVerified=true;
  ntl.contentMetrics={frames:60,achievedFps:60};
  assert.ok(validity(ntl).reasons.includes('NTL input provenance unavailable'));
  ntl.provenance={ntl:{tree:{sha256:'a'.repeat(64)}}};assert.equal(validity(ntl).valid,true);
  ntl.contentMetrics={};assert.equal(validity(ntl).valid,false);
  const clippedImage=structuredClone(run);clippedImage.scenario='normal';
  clippedImage.views[0]={...clippedImage.views[0],sceneEvidence:'image',viewport:[960,500]};
  assert.ok(validity(clippedImage).reasons.includes('Browser content viewport does not match requested client area'));
  for (const mutation of [r => r.windowSamples[0].foreground = false, r => r.windowSamples[0].occluded = true,
    r => r.windowSamples[0].buildRunning = true, r => r.stats[0].deferredTicks = 1,
    r => r.census[0].players[0].alive = false, r => r.views[0].wholeMap = false,
    r => r.census[0].wholeMapFood = false, r => r.nativeNetwork.rejectedEndpoints=[{remoteAddress:'203.0.113.1'}],
    r => r.metrics.boundaryGapMs = [1000, 0],r=>r.views[0].retainedPellets=0,
    r=>r.windowSamples[1].refreshHz=30,r=>r.windowSamples[0].nominalRefreshHz=30,
    r=>r.windowSamples[0].clientRect=[-20,0,960,540],r=>r.windowSamples[0].gpuCounterError='query failed',
    r=>delete r.windowSamples[0].refreshHz,r=>delete r.windowSamples[0].nominalRefreshHz,
    r=>{r.machine={clock:{qpcMs:1000,unixMs:100000}};r.windowSamples[0].unixMs=100000;}]) {
    const copy = structuredClone(run); mutation(copy); assert.equal(validity(copy).valid, false);
  }
});
test('adapter and refresh mismatches are labelled, never direct comparisons', () => {
  const run = { validity: { valid: true }, renderAdapters: [{ luid: 'A' }], display: 'D', refreshHz: 240, width: 1280, height: 720, dpi: 120, scenario: 'normal', experiment: 'primary', vsyncCondition: 'display-paced',metrics:{presentModes:['Composed: Flip']},contentDisplayCorrelation:'established' };
  run.views=[{atMs:0,drawnLiveSnakes:1}];run.census=[{atMs:0,players:[{wireLivingSnakes:1,alive:true,setup:true}]}];
  assert.equal(comparison(run, structuredClone(run)).classification, 'direct comparison');
  for (const change of [r => r.renderAdapters = [{ luid: 'B' }], r => r.refreshHz = 60, r => r.validity.valid = false,r=>r.metrics.presentModes=['Hardware: Independent Flip'],r=>r.metrics.presentModes=[],r=>r.metrics.presentModes=['Unknown'],r=>r.contentDisplayCorrelation='unestablished']) {
    const other = structuredClone(run); change(other); assert.equal(comparison(run, other).classification, 'labelled comparison only');
  }
});

function completeBattledome() {
  const times=Array.from({length:121},(_,i)=>i*500);
  const run={client:'original',scenario:'battledome',experiment:'primary',seconds:60,startMs:0,endMs:60000,dt:.007,bots:100,
    width:960,height:540,display:'D',dpi:96,refreshHz:240,adapter:'authored',renderAdapters:[{luid:'A',name:'Authored GPU'}],
    vsync:'Fifo',vsyncCondition:'sync-interval-1',contentDisplayCorrelation:'established',
    nativeNetwork:{status:'sampled-loopback-only',complete:false,rejectedEndpoints:[]},
    metrics:{coveredMs:60000,boundaryGapMs:[0,0],presentModes:['Composed: Flip']}};
  run.windowSamples=times.map(atMs=>({atMs,qpcMs:atMs,width:960,height:540,display:'D',dpi:96,refreshHz:240,nominalRefreshHz:240,
    foreground:true,visible:true,occluded:false,minimized:false,clientRect:[0,0,960,540],displayBounds:[0,0,1920,1080],gpuEngines:[{instance:'authored',utilization:1}]}));
  run.stats=times.map(atMs=>({atMs,ticksPerSecond:142.85,deferredTicks:0,lostSeconds:0,bots:100,players:1}));
  run.census=times.map(atMs=>({atMs,wholeMapFood:true,admittedBotSlots:100,totalAliveSnakes:101,players:[{alive:true,setup:true,protocol:19,wireLivingSnakes:101,decoratedWireFood:1000}]}));
  run.views=times.filter(t=>t%2000===0).map(atMs=>({atMs,wholeMap:true,retainedPellets:1000,visiblePellets:1000,drawnLiveSnakes:101}));
  return run;
}
test('complete 60-second delivery cannot pass a blank drawn-snake scene',()=>{
  const run=completeBattledome();assert.equal(validity(run).valid,true);
  run.views.forEach(v=>v.drawnLiveSnakes=0);
  assert.equal(validity(run).valid,false);assert.equal(drawnSnakeEvidence(run).status,'contradicted');
  run.views.forEach(v=>v.drawnLiveSnakes=99);assert.equal(validity(run).valid,true);
  run.views.forEach(v=>v.drawnLiveSnakes=98);assert.equal(validity(run).valid,false);
  run.views.forEach(v=>delete v.drawnLiveSnakes);assert.equal(drawnSnakeEvidence(run).status,'unestablished');
  const other=completeBattledome();run.validity=validity(run);other.validity=validity(other);
  assert.equal(comparison(run,other).classification,'labelled comparison only');
  assert.ok(comparison(run,other).reasons.some(reason=>reason.includes('drawn-snake')));
});
test('a labelled limited view must match its calibrated physical scale',()=>{
  const run=completeBattledome();run.viewCondition='ntl-farthest';
  run.ntlViewCalibration={physicalPixelsPerWorldUnit:.2};
  run.views.forEach(v=>Object.assign(v,{wholeMap:false,zoom:.2,viewport:[960,540]}));
  assert.equal(validity(run).valid,true);
  run.views[10].zoom=.25;
  assert.ok(validity(run).reasons.includes('Farthest NTL view scale not matched within 5%'));
  delete run.ntlViewCalibration;assert.equal(validity(run).valid,false);
});
test('native network observations retain early TCP and UDP violations without claiming complete proof',()=>{
  const sample={atUnixMs:0,pids:[7],tcpCollected:true,udpCollected:true,tcp:[{pid:7,localAddress:'127.0.0.1',remoteAddress:'127.0.0.1',state:'Established'}],udp:[{pid:7,localAddress:'::1',remoteAddress:null}]};
  const clean=nativeNetworkEvidence([sample]);assert.equal(clean.complete,false);assert.equal(Object.hasOwn(clean,'verified'),false);
  const run=completeBattledome();run.nativeNetwork=clean;assert.equal(validity(run).valid,true);
  const closed={...sample,atUnixMs:1000,tcp:[],udp:[]};
  const remote=structuredClone(sample);remote.tcp[0].remoteAddress='203.0.113.1';
  run.nativeNetwork=nativeNetworkEvidence([remote,closed]);assert.equal(validity(run).valid,false);
  const udp=structuredClone(sample);udp.udp[0].localAddress='0.0.0.0';
  run.nativeNetwork=nativeNetworkEvidence([udp,closed]);assert.equal(validity(run).valid,false);
  udp.udp[0]={pid:7,localAddress:'127.0.0.1',remoteAddress:'203.0.113.1'};
  assert.equal(nativeNetworkEvidence([udp]).rejectedEndpoints.length,1);
});
test('unpeered TCP Bound reservations are retained separately from traffic and listeners',()=>{
  const endpoint={pid:7,state:'Bound',localAddress:'0.0.0.0',localPort:12345,remoteAddress:'0.0.0.0',remotePort:0};
  const sample={atUnixMs:0,pids:[7],tcpCollected:true,udpCollected:true,tcp:[endpoint],udp:[]};
  const evidence=nativeNetworkEvidence([sample]);assert.equal(evidence.rejectedEndpoints.length,0);
  assert.equal(evidence.unconnectedReservations.length,1);assert.equal(evidence.complete,false);
  for(const change of [e=>e.state='Listen',e=>e.state='Established',e=>e.remoteAddress='203.0.113.1',e=>e.remotePort=80,e=>e.localAddress='192.0.2.1']){
    const changed=structuredClone(sample);change(changed.tcp[0]);assert.equal(nativeNetworkEvidence([changed]).rejectedEndpoints.length,1);
  }
  assert.equal(nativeNetworkEvidence([{...sample,tcp:[],udp:[endpoint]}]).rejectedEndpoints.length,1);
});
test('native endpoint sampling outlives a long warmup and the measured window',()=>{
  for(const warmupSeconds of [0,5,600]){
    const seconds=60,deadline=nativeNetworkMaxSeconds({seconds,warmupSeconds});
    // Authored elapsed-time model: launch/setup/admission, warmup, measurement,
    // then shutdown. The old launch-relative deadline expired during warmup.
    let elapsed=120;
    elapsed+=warmupSeconds;assert.ok(deadline>elapsed,'sampler must reach measurement start');
    elapsed+=seconds;assert.ok(deadline>elapsed,'sampler must cover measurement end');
    elapsed+=30;assert.ok(deadline>elapsed,'sampler must cover shutdown');
  }
  for(const warmupSeconds of [-1,NaN,Infinity,'600'])assert.throws(()=>nativeNetworkMaxSeconds({seconds:60,warmupSeconds}),/warmupSeconds/);
});
test('unprivileged native collector samples authored loopback TCP and UDP over process lifetime',{skip:process.platform!=='win32',timeout:30000},async()=>{
  const directory=await fs.mkdtemp(path.join(os.tmpdir(),'native-network-'));
  const script=`const net=require('net'),udp=require('dgram').createSocket('udp4');const tcp=net.createServer();tcp.listen(0,'127.0.0.1',()=>udp.bind(0,'127.0.0.1',()=>console.log(JSON.stringify({tcp:tcp.address().port,udp:udp.address().port}))));`;
  const child=spawn(process.execPath,['-e',script],{windowsHide:true});let monitor;
  try{
    const ports=await new Promise((resolve,reject)=>{child.once('error',reject);child.stdout.once('data',b=>resolve(JSON.parse(String(b))));});
    monitor=await startNativeNetwork({pid:child.pid,executable:process.execPath,directory,maxSeconds:15,startedUnixMs:Date.now()});
    await new Promise((resolve,reject)=>{let text='';const timer=setTimeout(()=>reject(Error('No network sample')),18000);monitor.child.stdout.on('data',b=>{text+=b;if(text.includes('\n')){clearTimeout(timer);resolve();}});monitor.child.once('error',reject);});
    child.kill();const evidence=await monitor.finish();monitor=null;
    assert.equal(evidence.complete,false);assert.notEqual(evidence.status,'unavailable',JSON.stringify(evidence.errors));
    assert.ok(evidence.samples.some(s=>s.tcp.some(p=>p.localPort===ports.tcp)));
    assert.ok(evidence.samples.some(s=>s.udp.some(p=>p.localPort===ports.udp)));
    assert.equal(evidence.rejectedEndpoints.length,0);
  }finally{if(child.exitCode===null)child.kill();if(monitor)await monitor.finish();await fs.rm(directory,{recursive:true,force:true});}
});
test('Chrome adapter selection is explicit, with no silent malformed fallback', () => {
  assert.deepEqual(adapterFlags('default'), []);
  assert.deepEqual(adapterFlags('luid_0x00000000_0x00000123'), ['--use-gl=angle', '--use-angle=d3d11', '--use-adapter-luid=0,291']);
  assert.throws(() => adapterFlags('NVIDIA'), /exact DXGI/);
});
test('host stats retain overload; camera accounts for offset', () => {
  assert.equal(hostStats('stats: ticks/s=100.00 deferred_ticks=3 lost_seconds=0.025000 simulation_load=41.2% players=1 bot_slots=500')[0].deferredTicks, 3);
  assert.equal(wholeMap({ camera: [0, 0], center: 0, radius: 100, zoom: 1, viewport: [200, 200] }), true);
  assert.equal(wholeMap({ camera: [1, 0], center: 0, radius: 100, zoom: 1, viewport: [200, 200] }), false);
});
test('round order reverses and keeps every variant', () => {
  assert.deepEqual(schedule(3, ['a', 'b']).map(x => x.client), ['a', 'b', 'b', 'a', 'a', 'b']);
});
test('network evidence rejects remote peers and missing game connection', () => {
  const make = addresses => ({ constants: { logEventTypes: { TCP_CONNECT_ATTEMPT: 5 } }, events: addresses.flatMap((address,id) => [{ type: 5, phase: 1, source:{id}, params: { address } },{type:5,phase:2,source:{id}}]) });
  assert.equal(netlogProof(make(['127.0.0.1:12']), '127.0.0.1:12').verified, true);
  assert.equal(netlogProof(make(['127.0.0.1:12', '203.0.113.1:80']), '127.0.0.1:12').verified, false);
  assert.equal(netlogProof(make(['127.0.0.1:13']), '127.0.0.1:12').verified, false);
});
function reachabilityFixture(endpoint='[2001:4860:4860::8888]:443') {
  const types={TCP_CONNECT_ATTEMPT:1,SOCKET_ALIVE:2,SOCKET_OPEN:3,UDP_CONNECT:4,SOCKET_CONNECT:5,HOST_RESOLVER_MANAGER_IPV6_REACHABILITY_CHECK:6,UDP_BYTES_SENT:7,UDP_BYTES_RECEIVED:8,UDP_READ_ERROR:9};
  const udp={id:20,type:24},wrapper={id:21,type:49},parent={id:22,type:3};
  const event=(name,source,phase,time,params)=>({type:types[name],source,phase,time:String(time),params});
  return {constants:{logEventTypes:types,logSourceType:{UDP_SOCKET:24,UDP_CLIENT_SOCKET:49,HOST_RESOLVER_MANAGER_REQUEST:3}},events:[
    event('TCP_CONNECT_ATTEMPT',{id:1,type:10},1,0,{address:'127.0.0.1:12345'}),event('TCP_CONNECT_ATTEMPT',{id:1,type:10},2,1),
    event('SOCKET_ALIVE',udp,1,10,{source_dependency:wrapper}),event('SOCKET_ALIVE',wrapper,1,10,{source_dependency:parent}),
    event('SOCKET_OPEN',wrapper,0,11),event('UDP_CONNECT',udp,1,11,{address:endpoint}),event('UDP_CONNECT',udp,2,12,{net_error:-109}),
    event('SOCKET_CONNECT',wrapper,0,12,{address:endpoint,net_error:-109}),event('SOCKET_ALIVE',wrapper,2,14),event('SOCKET_ALIVE',udp,2,14),
    event('HOST_RESOLVER_MANAGER_IPV6_REACHABILITY_CHECK',parent,0,14,{cached:false,ipv6_available:false})]};
}
test('the named no-packet probe allowance discloses exact closed attributed UDP sockets',()=>{
  const proof=netlogProof(reachabilityFixture(),'127.0.0.1:12345');
  assert.equal(proof.verified,true);assert.equal(proof.externalSocketAttemptsObserved,true);
  assert.equal(proof.noPacketReachabilityProbeCount,1);assert.equal(proof.allowances[0].name,'no-packet reachability probe');
  assert.equal(proof.allowances[0].probes[0].socketIoEvents,0);
  const noGame=reachabilityFixture();noGame.events=noGame.events.filter(e=>e.source.id!==1);
  assert.equal(netlogProof(noGame,'127.0.0.1:12345').verified,false);
});
test('probe bytes, I/O events, changed endpoints, TCP and incomplete attribution fail closed',()=>{
  const rejected=[];
  for(const name of ['UDP_BYTES_SENT','UDP_BYTES_RECEIVED','UDP_READ_ERROR']){
    const log=reachabilityFixture();log.events.push({type:log.constants.logEventTypes[name],phase:0,time:'13',source:{id:20,type:24},params:name==='UDP_READ_ERROR'?{net_error:-3}:{byte_count:name==='UDP_BYTES_RECEIVED'?0:1}});rejected.push(log);
  }
  const wrapperIo=reachabilityFixture();wrapperIo.events.push({type:wrapperIo.constants.logEventTypes.UDP_BYTES_SENT,phase:0,time:'13',source:{id:21,type:49},params:{byte_count:1}});rejected.push(wrapperIo);
  for(const endpoint of ['[2001:4860:4860::8889]:443','[2001:4860:4860::8888]:53'])rejected.push(reachabilityFixture(endpoint));
  const tcp=reachabilityFixture();tcp.events.push({type:1,source:{id:99,type:10},phase:1,time:'15',params:{address:'[2001:4860:4860::8888]:443'}});rejected.push(tcp);
  const unattributed=reachabilityFixture();delete unattributed.events.find(e=>e.source.id===21&&e.type===2&&e.phase===1).params.source_dependency;rejected.push(unattributed);
  const cached=reachabilityFixture();cached.events.at(-1).params.cached=true;rejected.push(cached);
  for(const id of [20,21]){const open=reachabilityFixture();open.events=open.events.filter(e=>!(e.source.id===id&&e.type===2&&e.phase===2));rejected.push(open);}
  const hiddenBytes=reachabilityFixture();hiddenBytes.events.find(e=>e.type===4&&e.phase===2).params.byte_count=1;rejected.push(hiddenBytes);
  for(const log of rejected){const proof=netlogProof(log,'127.0.0.1:12345');assert.equal(proof.verified,false);assert.match(proof.failure,/no-packet reachability probe/);}
  assert.match(netlogProof(reachabilityFixture('[2001:4860:4860::8888]:53'),'127.0.0.1:12345').failure,/:53/);
});
test('interrupted or failed collectors clean their owned session; a clean exit skips cleanup',async()=>{
  for(const exitCode of [null,1,7]){
    const child=new EventEmitter();child.stdout=new PassThrough();child.stderr=new PassThrough();child.exitCode=null;
    const completed=collectorCompletion(child);child.kill=()=>{child.exitCode=exitCode;child.emit('close',exitCode);};
    const interrupt=new EventEmitter();interrupt.once('SIGINT',()=>child.kill());interrupt.emit('SIGINT');await completed;
    const calls=[];assert.equal(cleanupPresentMon(child,'SlitherOwned-offline',name=>{calls.push(name);return 'stopped';}).status,'stopped');
    assert.deepEqual(calls,['SlitherOwned-offline']);
  }
  assert.equal(cleanupPresentMon({exitCode:0},'SlitherOwned-offline',()=>assert.fail('Clean collector must not terminate a session')),null);
});
test('only PresentMon exit 7 with the exact owned missing-session message is harmless',()=>{
  const session='SlitherOwned-offline',gone=Object.assign(Error('exit 7'),{status:7,stdout:'',stderr:`warning: option\nerror: no existing sessions found: ${session}\n`});
  assert.equal(cleanupPresentMon({exitCode:1},session,()=>{throw gone;}).status,'already-gone');
  for(const error of [Object.assign(Error('denied'),{status:7,stderr:'error: access denied'}),Object.assign(Error('wrong session'),{status:7,stderr:'error: no existing sessions found: another-session'}),Object.assign(Error('wrong status'),{status:1,stderr:gone.stderr})]){
    assert.throws(()=>cleanupPresentMon({exitCode:null},session,()=>{throw error;}),e=>e===error);
  }
});
test('readiness admits only complete allowed literal-loopback announcements', () => {
  assert.equal(endpointFrom('Listening on ws://127.0.0.1:49152/slither\n'), 'ws://127.0.0.1:49152/slither');
  assert.equal(endpointFrom('Listening on ws://127.0.0.1:49'), null);
  assert.equal(endpointFrom('Listening on ws://203.0.113.1:49152/slither\n'), null);
  for (const port of [0, 8766, 18884, 18885, 65536]) assert.throws(() => endpointFrom(`Listening on ws://127.0.0.1:${port}/slither\n`));
});
test('streamed telemetry ignores a partial final line without accepting defaults', () => {
  assert.deepEqual(completeJsonLines('other\nframe-bench: {"tick":1}\nframe-bench: {"ti', 'frame-bench: '), [{tick:1}]);
  assert.throws(() => completeJsonLines('frame-bench: malformed\n','frame-bench: '));
});
test('ETW loss is a collection failure even with successful process exit', () => {
  assert.match(presentMonFailure('warning: 12345 ETW events were lost.'), /lost ETW/);
  assert.equal(presentMonFailure('Started recording.\nStopped recording.'), null);
});

test('synthetic acceptance needs successful ETW and one complete owned display surface',()=>{
  const header='ProcessID,SwapChainAddress,CPUStartQPCTime,DisplayLatency,DisplayedTime,PresentMode,SyncInterval\n';
  const text=header+Array.from({length:240},(_,i)=>`7,A,${i*1000/240},0,4.1667,Composed: Flip,1`).join('\n');
  const options={pids:[7],seconds:1,startMs:0,endMs:1000};
  const collector={exitCode:0,log:'',text};
  assert.equal(checkedPresentation(collector,options).metrics.achievedFps,240);
  for(const failed of [{...collector,exitCode:1,log:'Access denied'},{...collector,error:'ENOENT'},
    {...collector,log:'123 ETW events were lost'},{...collector,text:header},
    {...collector,text:text.replaceAll('7,A,','8,A,')},
    {...collector,text:text+ '\n7,B,500,0,4,Composed: Flip,1'}])assert.throws(()=>checkedPresentation(failed,options));
});

test('a missing collector returns a handled launch failure through the cleanup path',async()=>{
  const scope=resourceScope();let closed=false;scope.add(()=>{closed=true;});
  let result;
  try{result=await collectorCompletion(spawn(path.join(os.tmpdir(),`missing-collector-${process.pid}-${Date.now()}`),[],{windowsHide:true}));}
  finally{await scope.close();}
  assert.match(result.error,/ENOENT/);assert.equal(result.exitCode,null);assert.equal(closed,true);
});
test('early acquisition failure retains collector diagnostics with a bounded cleanup wait',async()=>{
  const lost='warning: 123 ETW events were lost';
  assert.equal(await recoverCollectorLog(Promise.resolve({log:lost}),''),lost);
  assert.equal(await recoverCollectorLog(Promise.resolve({log:lost}),'existing evidence'),'existing evidence');
  assert.equal(await recoverCollectorLog(new Promise(()=>{}),'',5),'');
});
test('collector targets the owned process and keeps display tracking without escalation or trace takeover',()=>{
  const args=presentMonArguments({pid:7,seconds:5,session:'authored',file:'trace.csv'});
  assert.equal(args[args.indexOf('--process_id')+1],'7');assert.ok(args.includes('--v2_metrics'));
  for(const forbidden of ['--no_track_display','--restart_as_admin','--stop_existing_session','--terminate_existing_session'])assert.equal(args.includes(forbidden),false);
  assert.throws(()=>presentMonArguments({pid:0}),/owned render process/);
});
test('an occluding owner window stops the round and qualifies for bounded retry',()=>{
  const clean={foreground:true,visible:true,occluded:false,minimized:false,buildRunning:false};
  assert.equal(transientFailure({windowSamples:[clean]}),false);
  assert.equal(transientFailure({windowSamples:[{...clean,occluded:true}]}),true);
  assert.equal(transientFailure({error:'PresentMon lost ETW events'}),true);
});
test('Windows load gate excludes its own harness but retains another worker', {skip:process.platform!=='win32'},async()=>{
  const text=await fs.readFile(new URL('./compare-bench/windows.ps1',import.meta.url),'utf8');
  const classifier=text.match(/function Competing-Work \{[\s\S]*?\r?\n\}/)?.[0];assert.ok(classifier);
  const script=`$c=@{callerPid=11}; function Get-CimInstance { @(
    [pscustomobject]@{ProcessId=11;Name='node.exe';CommandLine='node synthetic-check.mjs'},
    [pscustomobject]@{ProcessId=22;Name='node.exe';CommandLine='node synthetic-check.mjs'},
    [pscustomobject]@{ProcessId=33;Name='rustc.exe';CommandLine='rustc authored.rs'}
  ) }; ${classifier}; @(Competing-Work) | ConvertTo-Json -Compress`;
  const rows=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-Command',script],{encoding:'utf8',windowsHide:true}));
  assert.deepEqual(rows.map(p=>p.ProcessId),[22,33]);
});
test('60 Hz canvas activity beside 240 Hz callbacks/compositor is counted as 60, not 240',()=>{
  class Context { constructor(canvas){this.canvas=canvas;} fillRect(){} }
  const canvas={width:960,height:540,getBoundingClientRect:()=>({x:0,y:0,width:960,height:540})};
  let scheduled;const sandbox={CanvasRenderingContext2D:Context,performance:{now:()=>0,timeOrigin:1},requestAnimationFrame:callback=>{scheduled=callback;return 1;}};
  vm.runInNewContext(`(${installContentProbe.toString()})()`,sandbox);
  sandbox.__frameBenchProduction.start();const ctx=new Context(canvas);
  for(let i=0;i<240;i++){sandbox.requestAnimationFrame(()=>{if(i%4===0){ctx.fillRect();ctx.fillRect();}});scheduled(i*1000/240);}
  const observed=sandbox.__frameBenchProduction.stop();
  const metrics=contentIntervals(observed,{canvas:1,seconds:1,startMs:0,endMs:1000});
  assert.equal(metrics.frames,60);assert.equal(metrics.achievedFps,60);assert.ok(Math.abs(metrics.medianMs-1000/60)<.001);
  assert.equal(metrics.rafFramesDuringWindow,240);
  const guarded={...observed,records:[{at:-100,canvas:1},...observed.records,{at:0,canvas:1}]};
  const sliced=contentIntervals(guarded,{canvas:1,seconds:1,startMs:0,endMs:1000});
  assert.equal(sliced.duplicateCallbacksRemoved,1);assert.equal(sliced.outsideWindowProductions,1);
  assert.throws(()=>contentIntervals({...observed,outsideRaf:[[1,10]]},{canvas:1,seconds:1,startMs:0,endMs:1000}),/outside rAF/);
});
test('duplicate native picture presents cannot masquerade as new content',()=>{
  const result=nativeProduction({unixStartMs:0,rows:[[0,1],[1000000,1],[2000000,2],[3000000,2]]},{startMs:0,endMs:5,qpcUnixOffset:0});
  assert.equal(result.freshPreparedPictures,2);assert.equal(result.duplicatePicturePresents,2);
  assert.throws(()=>nativeProduction({unixStartMs:0,rows:[]},{startMs:0,endMs:5,qpcUnixOffset:0}),/No native present identities/);
});
test('all streams require timestamps, bounded gaps and both measurement boundaries',()=>{
  assert.equal(streamCoverage([{atMs:0}],0,60000,1200),false);
  assert.equal(streamCoverage([{tick:1}],0,60000,1200),false);
  assert.equal(streamCoverage([{atMs:0},{atMs:1000},{atMs:2000}],0,2000,1200),true);
  assert.equal(streamCoverage([{atMs:0},{atMs:5000},{atMs:6000}],0,6000,1200),false);
  assert.equal(contained([1800,0,400,400],[0,0,1920,1080]),false);
});
test('remote UDP/QUIC peers fail even when TCP game socket is loopback',()=>{
  const log={constants:{logEventTypes:{TCP_CONNECT_ATTEMPT:1,UDP_CONNECT:2,QUIC_SESSION:3}},events:[{type:1,phase:1,source:{id:1},params:{address:'127.0.0.1:12345'}},{type:1,phase:2,source:{id:1}},{type:2,params:{address:'203.0.113.1:3478'}}]};
  assert.equal(netlogProof(log,'127.0.0.1:12345').verified,false);
  log.events[2]={type:3,params:{peer_address:'[2001:db8::1]:443'}};assert.equal(netlogProof(log,'127.0.0.1:12345').verified,false);
  assert.equal(profilePreferences.webrtc.ip_handling_policy,'disable_non_proxied_udp');
});
test('Windows path casing and structured CDP values never become code',async()=>{
  assert.equal(outsideRepository('c:\\Repo\\child','C:\\repo',path.win32),false);
  assert.equal(outsideRepository('D:\\scratch','C:\\repo',path.win32),true);
  const malicious="');globalThis.injected=true;//\\x\n";let params;
  const result=await CDP.prototype.call.call({context:7,send:async(_method,p)=>{params=p;return {result:{value:'ok'}};}},'function(value){return value;}',[malicious]);
  assert.equal(result,'ok');assert.equal(params.arguments[0].value,malicious);assert.equal(params.functionDeclaration,'function(value){return value;}');
});
test('failure after listeners start closes them even when evidence writing fails',async()=>{
  const tmp=await fs.mkdtemp(path.join(os.tmpdir(),'frame-bench-'));const file=path.join(tmp,'file');await fs.writeFile(file,'not a directory');
  const servers=[];const original=http.Server.prototype.listen;
  http.Server.prototype.listen=function(...args){servers.push(this);return Reflect.apply(original,this,args);};
  try{await assert.rejects(chromeSession({chrome:'unused',directory:file,adapter:{chromeLuid:'default'},register:()=>{}}));
    assert.equal(servers.length,2);assert.ok(servers.every(server=>!server.listening));
  }finally{http.Server.prototype.listen=original;await fs.rm(tmp,{recursive:true,force:true});}
  const order=[],scope=resourceScope();scope.add(()=>order.push(1));scope.add(()=>{order.push(2);throw Error('write failed');});await assert.rejects(scope.close());assert.deepEqual(order,[2,1]);
});
test('empty scene is rejected; authored image can show boundary and pellet presence only',()=>{
  const width=480,height=360,channels=3,pixels=Buffer.alloc(width*height*channels);
  assert.equal(pictureEvidence({width,height,channels,pixels}).gameCanvasVisible,false);
  const set=(x,y,r,g,b)=>{const i=(y*width+x)*3;pixels[i]=r;pixels[i+1]=g;pixels[i+2]=b;};
  for(let y=0;y<height;y++)for(let x=0;x<width;x++)if(Math.abs(Math.hypot(x-240,y-180)-140)<2)set(x,y,100,0,0);
  for(let i=0;i<10;i++)for(let x=0;x<3;x++)for(let y=0;y<3;y++)set(130+i*15+x,160+y,0,200,60);
  const evidence=pictureEvidence({width,height,channels,pixels});assert.equal(evidence.wholeMap,true);assert.ok(evidence.pelletCandidates>=10);assert.match(evidence.sceneConfidence,/exact count unverified/);
  pixels.fill(0);
  for(let y=0;y<height;y++)for(let x=0;x<width;x++)if(Math.abs(Math.hypot(x-240,y-30)-140)<2)set(x,y,100,70,0);
  const limited=pictureEvidence({width,height,channels,pixels});assert.ok(limited.boundary);assert.equal(limited.wholeMap,false);
});
