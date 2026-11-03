import test from 'node:test';
import assert from 'node:assert/strict';
import { csv, presentation, hostStats, validity, wholeMap, schedule, netlogProof, comparison, presentMonFailure,streamCoverage,contained,nativeProduction } from './compare-bench/analysis.mjs';
import { endpointFrom, completeJsonLines,outsideRepository,lineJournal } from './compare-bench/run.mjs';
import { adapterFlags,profilePreferences,chromeSession,CDP } from './compare-bench/browser.mjs';
import { installContentProbe,contentIntervals } from './compare-bench/content-probe.mjs';
import { pictureEvidence } from './compare-bench/picture.mjs';
import {resourceScope,listen,closeServer} from './compare-bench/resources.mjs';
import vm from 'node:vm';
import path from 'node:path';
import http from 'node:http';
import fs from 'node:fs/promises';
import os from 'node:os';

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
    scenario: 'battledome', networkVerified: true, width: 960, height: 540, display: 'D', dpi: 96,
    adapter: 'synthetic', renderAdapters: [{ name: 'Synthetic GPU', luid: 'luid1' }], refreshHz: 60, vsync: 'Fifo',
    windowSamples: [0,500,1000].map(atMs=>({atMs,qpcMs:atMs,foreground: true, occluded: false, minimized: false, visible: true, width: 960, height: 540, display: 'D', dpi: 96,refreshHz:60,nominalRefreshHz:60,clientRect:[0,0,960,540],displayBounds:[0,0,1920,1080],gpuEngines:[{instance:'gpu',utilization:2}]})),
    stats: [{atMs:0, ticksPerSecond: 142.85, deferredTicks: 0, lostSeconds: 0, bots: 100, players: 1 }],
    census: [{atMs:0, wholeMapFood: true, admittedBotSlots: 100, totalAliveSnakes: 100, players: [{ alive: true, setup: true, protocol: 19, wireLivingSnakes: 100,decoratedWireFood:40 }] }],
    views: [{atMs:0, wholeMap: true, visiblePellets: 40, retainedPellets: 40 }] };
  assert.equal(validity(run).valid, true);
  for (const mutation of [r => r.windowSamples[0].foreground = false, r => r.windowSamples[0].occluded = true,
    r => r.windowSamples[0].buildRunning = true, r => r.stats[0].deferredTicks = 1,
    r => r.census[0].players[0].alive = false, r => r.views[0].wholeMap = false,
    r => r.census[0].wholeMapFood = false, r => r.networkVerified = false,
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
  assert.equal(comparison(run, structuredClone(run)).classification, 'direct comparison');
  for (const change of [r => r.renderAdapters = [{ luid: 'B' }], r => r.refreshHz = 60, r => r.validity.valid = false,r=>r.metrics.presentModes=['Hardware: Independent Flip'],r=>r.metrics.presentModes=[],r=>r.metrics.presentModes=['Unknown'],r=>r.contentDisplayCorrelation='unestablished']) {
    const other = structuredClone(run); change(other); assert.equal(comparison(run, other).classification, 'labelled comparison only');
  }
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
  assert.throws(()=>contentIntervals({...observed,outsideRaf:[[1,10]]},{canvas:1,seconds:1,startMs:0,endMs:1000}),/outside rAF/);
});
test('duplicate native picture presents cannot masquerade as new content',()=>{
  const result=nativeProduction({unixStartMs:0,rows:[[0,1],[1000000,1],[2000000,2],[3000000,2]]},{startMs:0,endMs:5,qpcUnixOffset:0});
  assert.equal(result.freshPreparedPictures,2);assert.equal(result.duplicatePicturePresents,2);
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
});
