import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync, execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const check = (ok, message) => { if (!ok) throw Error(message); };
const sub = (a,b) => a.map((x,i)=>x-b[i]);
const norm = a => Math.hypot(...a);
const dot = (a,b) => a.reduce((s,x,i)=>s+x*b[i],0);
const wrap = a => Math.atan2(Math.sin(a),Math.cos(a));
export function distribution(values) {
  check(values.every(x=>x===null || Number.isFinite(x)), 'non-finite measurement');
  const x=values.filter(Number.isFinite).sort((a,b)=>a-b);
  if (!x.length) return {count:0,undefined:values.length,p50:null,p95:null,p99:null,max:null,rms:null,p99DescriptiveOnly:true};
  const q=p=>x[Math.ceil(p*x.length)-1];
  return {count:x.length,undefined:values.length-x.length,p50:q(.5),p95:q(.95),p99:q(.99),max:x.at(-1),rms:Math.sqrt(x.reduce((s,v)=>s+v*v,0)/x.length),p99DescriptiveOnly:x.length<1000};
}
export function error(truth, drawn, heading, speed) {
  const e=sub(truth,drawn), tangent=[Math.cos(heading),Math.sin(heading)];
  const parallel=dot(e,tangent), lateral=dot(e,[-tangent[1],tangent[0]]);
  return {euclidean:norm(e),parallel,lateral,lagEquivalentMs:Math.abs(speed)>1e-6?1000*parallel/speed:null};
}
export function station(points, distance) {
  for (let i=1;i<points.length;i++) {
    const d=norm(sub(points[i],points[i-1]));
    if (d>=distance && d>0) return points[i-1].map((x,k)=>x+(points[i][k]-x)*distance/d);
    distance-=d;
  }
  return distance<1e-7 && points.length ? points.at(-1) : null;
}
// Differencing at each derivative's own midpoint handles irregular frames.
export function smoothness(series) {
  const result={}; let current=series.map(r=>({t:r.t/1000,q:r.q}));
  for (const order of ['velocity','acceleration','jerk']) {
    const next=[];
    for(let i=1;i<current.length;i++) {
      const a=current[i-1],b=current[i],h=b.t-a.t;
      check(h>0,'non-positive derivative interval');
      next.push({t:(a.t+b.t)/2,q:sub(b.q,a.q).map(x=>x/h)});
    }
    result[order]=distribution(next.map(r=>norm(r.q))); current=next;
  }
  return result;
}
// Crossings are retained as intervals; a linear estimate is diagnostic only.
export function response(samples, trigger, amplitude) {
  check(Number.isFinite(trigger) && Number.isFinite(amplitude) && amplitude!==0,'invalid detector calibration');
  const out={triggerMs:trigger,amplitude,samples:samples.length,levels:{},sustainedLevels:{},recrossings:{}};
  const bracket=(a,b,level)=>({lowerMs:Math.max(0,a.t-trigger),upperMs:b.t-trigger,
    estimateMs:Math.max(0,a.t-trigger+(b.t-a.t)*(level*amplitude-a.y)/(b.y-a.y))});
  for(const level of [.1,.5,.9]) {
    let crossing=null;
    for(let i=1;i<samples.length;i++) {
      const a=samples[i-1],b=samples[i];
      if(b.t<trigger || a.y/amplitude>=level || b.y/amplitude<level) continue;
      crossing=bracket(a,b,level); break;
    }
    out.levels[`t${Math.round(level*100)}`]=crossing;
    const name=`t${Math.round(level*100)}`;
    // A brief packet-boundary flash can cross a threshold then fall below it.
    // Keep the first crossing, and separately measure the last such recovery.
    const lastBelow=samples.findLastIndex(s=>s.t>=trigger && s.y/amplitude<level);
    const next=lastBelow<0?samples.findIndex(s=>s.t>=trigger):lastBelow+1;
    out.sustainedLevels[name]=next>0 && next<samples.length && samples[next].y/amplitude>=level
      ? bracket(samples[next-1],samples[next],level) : null;
    out.recrossings[name]=samples.slice(1).filter((b,i)=>b.t>=trigger && samples[i].y/amplitude>=level && b.y/amplitude<level).length;
  }
  const active=samples.filter(r=>r.t>=trigger && r.t<=trigger+1000);
  out.overshoot=Math.max(0,...active.map(r=>r.y/amplitude-1));
  out.peakFraction=Math.max(0,...active.map(r=>r.y/amplitude));
  return out;
}
export function analyzeProbe(run) {
  check(run.format==='smoothing-lag-v1' && run.completed===true && run.failures.length===0,'incomplete production probe');
  const ids=new Set(),frames=new Set(),measured=run.rows.filter(r=>r.measured);
  check(measured.length>100,'insufficient measured exposure');
  const metadata=new Map([[0,{generatedMs:run.originMs,arrivalMs:run.originMs}]]);
  let lastAnchor=0;
  for(const p of run.packets) if(p.anchorId!==null) {
    check(p.anchorId===lastAnchor+1,'duplicate/reordered anchor'); lastAnchor=p.anchorId;
    const previous=metadata.get(lastAnchor-1);
    check(p.generatedMs>previous.generatedMs && p.arrivalMs>=previous.arrivalMs,'reordered delivery'); metadata.set(p.anchorId,p);
  }
  let previousTime=-Infinity;
  let expectedTime=run.originMs;
  const period=run.config.rate==='irregular'?null:1000/Number(run.config.rate);
  for(const [index,r] of run.rows.entries()) {
    check(r.frame===index && Math.abs(r.atMs-expectedTime)<1e-7 && r.measured===(r.atMs>=run.originMs+run.warmupMs),'changed/missing frozen frame');
    check(!ids.has(r.sequence) && !frames.has(r.frame) && r.atMs>previousTime && r.acknowledged,'missing/duplicate/skipped picture');
    ids.add(r.sequence); frames.add(r.frame); previousTime=r.atMs;
    check(r.snakes.length===2 && new Set(r.snakes.map(s=>s.id)).size===2,'missing snake geometry');
    for(const s of r.snakes) {
      const packet=metadata.get(s.anchorId);
      check(packet && packet.generatedMs===s.poseGeneratedMs && packet.arrivalMs<=r.atMs,'orphan or future pose provenance');
      check(![...metadata.entries()].some(([id,p])=>id>s.anchorId && p.arrivalMs<=r.atMs),'stale admitted packet prefix');
    }
    expectedTime=period===null?expectedTime+[4,7,19,5,11,23,6,9][index%8]:run.originMs+(index+1)*period;
  }
  check(expectedTime>run.originMs+run.durationMs-1e-7,'missing frozen tail');
  const maximumGap=Math.max(...run.rows.slice(1).map((r,i)=>r.atMs-run.rows[i].atMs));
  check(measured[0].atMs-run.originMs-run.warmupMs<=maximumGap+1e-6 && run.originMs+run.durationMs-measured.at(-1).atMs<=maximumGap+1e-6,'truncated measured interval');
  const summaries={};
  const camera = {
    worldError:distribution(measured.map(r=>norm(sub(r.camera,r.snakes.find(s=>s.own).truth)))),
    smoothness:{
      world:smoothness(measured.map(r=>({t:r.atMs,q:r.camera}))),
      residualWorld:smoothness(measured.map(r=>({t:r.atMs,q:sub(r.camera,r.snakes.find(s=>s.own).truth)}))),
    },
  };
  for(const own of [true,false]) {
    const role=own?'own':'enemy', channels={}, metrics={}, series={}; const failures=[];
    const add=(name,x)=>{(metrics[name]??=[]).push(x);};
    const point=(name,t,q)=>{(series[name]??=[]).push({t,q});};
    let previous=null,stale=0,longestStale=0,aboveHalfR=0,aboveR=0;
    for(const r of measured) {
      const s=r.snakes.find(s=>s.own===own), other=r.snakes.find(s=>s.own!==own);
      const radius=14.5*s.scale,reach=radius+14.5*other.scale;
      const e=error(s.truth,s.drawn,s.truthHeading,s.speed);
      for(const [name,value] of Object.entries(e)) add(name,value);
      add('errorOverRadius',e.euclidean/radius);add('errorOverReach',e.euclidean/reach);
      add('packetStateAgeMs',r.atMs-s.poseGeneratedMs);
      add('drawnVsPacket',norm(sub(s.packetHead,s.drawn)));
      add('correctionMagnitude',norm(sub(s.drawn,s.raw)));
      const screen=p=>sub(p,r.camera).map(x=>x*r.zoom);
      point('headWorld',r.atMs,s.drawn); point('headScreen',r.atMs,screen(s.drawn));
      point('headResidualWorld',r.atMs,sub(s.drawn,s.truth));point('headResidualScreen',r.atMs,sub(s.drawn,s.truth).map(x=>x*r.zoom));
      let bodyLength=0;for(let i=1;i<s.body.length;i++) bodyLength+=norm(sub(s.body[i],s.body[i-1]));
      add('bodyLength',bodyLength);
      const spacing=s.body.slice(1).map((p,i)=>norm(sub(p,s.body[i]))); for(const length of spacing)add('bodySpacing',length);
      for(let i=0;i<run.stations.length;i++) {
        const distance=run.stations[i],q=station(s.body,distance);
        if(!q) {failures.push({frame:r.frame,station:distance,reason:'short painter centerline',bodyLength});continue;}
        const b=error(s.truthBody[i],q,s.truthBodyHeadings[i],s.speed);
        add(`body${distance}Error`,b.euclidean);add(`body${distance}Lateral`,b.lateral);
        const [start,end]=s.truthBodySegments[i],edge=sub(end,start);
        const u=Math.max(0,Math.min(1,dot(sub(q,start),edge)/dot(edge,edge)));
        add(`body${distance}OrderedSegmentError`,norm(sub(q,start.map((x,k)=>x+u*edge[k]))));
        add(`body${distance}OverRadius`,b.euclidean/radius);add(`body${distance}OverReach`,b.euclidean/reach);
        point(`body${distance}World`,r.atMs,q);point(`body${distance}Screen`,r.atMs,screen(q));
        point(`body${distance}ResidualWorld`,r.atMs,sub(q,s.truthBody[i]));
        point(`body${distance}ResidualScreen`,r.atMs,sub(q,s.truthBody[i]).map(x=>x*r.zoom));
      }
      if(previous) {
        const dt=r.atMs-previous.r.atMs;add('ackGapMs',dt);
        const step=norm(sub(screen(s.drawn),previous.screen));
        if(s.anchorId!==previous.s.anchorId) {
          add('packetBoundaryScreenStep',step);
          add('packetBoundaryResidualScreenStep',norm(sub(sub(s.drawn,s.truth).map(x=>x*r.zoom),previous.residual)));
        }
        add('wrappedHeadingIncrement',Math.abs(wrap(s.heading-previous.s.heading)));
        add('directionReversal',Number(dot(sub(s.drawn,previous.s.drawn),[Math.cos(s.truthHeading),Math.sin(s.truthHeading)])< -1e-7));
        stale=norm(sub(s.drawn,previous.s.drawn))<1e-9?stale+dt:0;longestStale=Math.max(stale,longestStale);
        if(e.euclidean>.5*reach)aboveHalfR+=dt;if(e.euclidean>reach)aboveR+=dt;
      }
      previous={r,s,screen:screen(s.drawn),residual:sub(s.drawn,s.truth).map(x=>x*r.zoom)};
    }
    for(const [name,x] of Object.entries(metrics))channels[name]=distribution(x);
    summaries[role]={metrics:channels,smoothness:Object.fromEntries(Object.entries(series).map(([name,x])=>[name,smoothness(x)])),
      longestStaleMs:longestStale,timeAboveHalfReachMs:aboveHalfR,timeAboveReachMs:aboveR,
      directionReversals:(metrics.directionReversal??[]).reduce((s,v)=>s+v,0),
      baselineSafetyTargets:{p95BelowQuarterReach:channels.errorOverReach.p95<=.25,p99BelowHalfReach:channels.errorOverReach.p99<=.5},failures};
  }
  return {config:run.config,frames:run.rows.length,measuredFrames:measured.length,maximumGapMs:maximumGap,skipped:0,duplicates:0,camera,summaries};
}
// The producer retains station failures; every accepted matrix run AND control
// must pass this gate before it can contribute to a completed calibration.
export function completeProbe(run) {
  const summary=analyzeProbe(run);
  for(const role of ['own','enemy']) check(summary.summaries[role].failures.length===0,'missing body stations');
  return summary;
}
export function pairedResponse(base,impulse) {
  check(base.rows.length===impulse.rows.length,'paired frame count mismatch'); const out={};
  for(const own of [true,false]) {
    const samples=base.rows.map((r,i)=>{
      const b=impulse.rows[i]; check(r.atMs===b.atMs && r.frame===b.frame,'paired cadence mismatch');
      const s=r.snakes.find(s=>s.own===own),p=b.snakes.find(s=>s.own===own);
      return {t:r.atMs,position:p.drawn[0]-s.drawn[0],heading:wrap(p.heading-s.heading)};
    });
    const responseWindow=(field,trigger,amplitude)=>response(samples.filter(s=>s.t>=trigger-50 && s.t<=trigger+1000).map(s=>({t:s.t,y:s[field]})),trigger,amplitude);
    out[own?'own':'enemy']={position:responseWindow('position',impulse.positionImpulseMs,24),heading:responseWindow('heading',impulse.headingImpulseMs,Math.PI/8)};
  }
  return out;
}
// Higher thresholds can resolve even when lower levels lack a crossing bracket.
// Every declared level must resolve before this stratum enters a complete table.
export function completeResponse(value) {
  for(const role of ['own','enemy']) for(const kind of ['position','heading'])
    for(const group of ['levels','sustainedLevels']) for(const level of ['t10','t50','t90'])
      check(value[role][kind][group][level],`unresolved correction detector: ${role}/${kind}/${group}/${level}`);
  return value;
}
function controls() {
  const known=error([10,0],[9,0],0,20);check(known.lagEquivalentMs===50,'lag units/sign control');
  check(error([10,0],[11,0],0,20).lagEquivalentMs===-50,'ahead sign control');
  check(error([0,0],[0,0],0,0).lagEquivalentMs===null,'near-zero lag control');
  const samples=Array.from({length:101},(_,i)=>({t:i*5,y:Math.min(1,Math.max(0,(i*5-40)/100))}));
  const r=response(samples,0,1);check(Math.abs(r.levels.t50.estimateMs-90)<1e-9,'known delay/ramp t50 control');
  check(response(samples.map(s=>({...s,y:0})),0,1).levels.t10===null,'zero detector control');
  return {passed:true,knownLagMs:50,knownDetectorT50Ms:90,zeroFalsePositives:0};
}
export function markdown(results) {
  const lines=['| Rate | Speed / motion | Jitter ±ms | Own lag p50/p95 ms | Enemy lag p50/p95 ms | Position t50 own/enemy ms | Heading first→sustained t50 own/enemy ms | Status |',
    '|---|---|---:|---:|---:|---:|---:|---|'];
  const f=x=>x===null||x===undefined?'unresolved':x.toFixed(2);
  for(const r of results) {
    if(!r.baseline){lines.push(`| ${r.config.rate} | ${r.config.speed}/${r.config.motion} | ${r.config.jitterMs} | FAILED | FAILED | FAILED | FAILED | FAILED |`);continue;}
    const m=role=>r.baseline.summaries[role].metrics.lagEquivalentMs;
    const t=(role,kind)=>r.response?.[role]?.[kind]?.levels.t50?.estimateMs;
    const h=role=>`${f(t(role,'heading'))}→${f(r.response?.[role]?.heading?.sustainedLevels.t50?.estimateMs)}`;
    lines.push(`| ${r.config.rate} | ${r.config.speed}/${r.config.motion} | ${r.config.jitterMs} | ${f(m('own').p50)} / ${f(m('own').p95)} | ${f(m('enemy').p50)} / ${f(m('enemy').p95)} | ${f(t('own','position'))} / ${f(t('enemy','position'))} | ${h('own')} / ${h('enemy')} | ${r.failure?'FAILED':'complete'} |`);
  }
  return lines.join('\n')+'\n';
}
function runMatrix(binary,out) {
  fs.mkdirSync(out);const hash=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  const git=(...args)=>execFileSync('git',args,{encoding:'utf8'}).trim();
  check(!git('diff','--name-only') && !git('ls-files','--others','--exclude-standard'),'Stage all intended source before measuring');
  const sourceTree=git('write-tree');const receipt={format:'smoothing-lag-run-v1',sourceRevision:git('rev-parse','HEAD'),sourceTree,binarySha256:hash(binary),controls:controls(),results:[],failures:[]};
  fs.writeFileSync(path.join(out,'receipt-start.json'),JSON.stringify(receipt,null,2));
  for(const rate of ['60','170','240','irregular'])for(const speed of ['normal','boost'])for(const motion of ['straight','turn'])for(const jitterMs of [0,4,12]) {
    const config={rate,speed,motion,jitterMs,delayMs:20};const id=`${rate}-${speed}-${motion}-j${jitterMs}`;const runs=[];
    for(const impulse of [false,true]) {
      const label=`${id}-${impulse?'impulse':'baseline'}`;
      const p=spawnSync(binary,['--smoothing-lag-probe'],{input:JSON.stringify({...config,impulse}),encoding:'utf8',maxBuffer:128*1024*1024});
      fs.writeFileSync(path.join(out,`${label}.json`),p.stdout??'');fs.writeFileSync(path.join(out,`${label}.stderr.txt`),p.stderr??'');
      if(p.status!==0 || p.error) {receipt.failures.push({label,status:p.status,error:String(p.error??p.stderr)});runs.push(null);continue;}
      try {runs.push(JSON.parse(p.stdout));}catch(e){receipt.failures.push({label,error:String(e)});runs.push(null);}
    }
    const result={config};
    try {
      check(runs.every(Boolean),'missing run');result.baseline=completeProbe(runs[0]);result.impulse=completeProbe(runs[1]);result.response=pairedResponse(...runs);
      completeResponse(result.response);
    }catch(e){result.failure=String(e);receipt.failures.push({id,error:String(e)});}
    receipt.results.push(result);fs.writeFileSync(path.join(out,'summary.json'),JSON.stringify(receipt,null,2));
    console.log(`${id}: ${result.failure??'retained'}`);
  }
  // Separate production straight controls isolate delay from genuine turning.
  receipt.productionControls=[];
  for(const [label,delayMs,anchors] of [['no-correction',0,false],['delay-0',0,true],['delay-40',40,true]]) {
    const config={rate:'170',speed:anchors?'normal':'stationary',motion:'straight',jitterMs:0,delayMs,anchors,impulse:false};
    const p=spawnSync(binary,['--smoothing-lag-probe'],{input:JSON.stringify(config),encoding:'utf8',maxBuffer:128*1024*1024});
    fs.writeFileSync(path.join(out,`control-${label}.json`),p.stdout??'');fs.writeFileSync(path.join(out,`control-${label}.stderr.txt`),p.stderr??'');
    try {check(p.status===0,'production control failed');const s=completeProbe(JSON.parse(p.stdout));
      if(!anchors) for(const role of ['own','enemy']) {
        check(s.summaries[role].metrics.euclidean.max<1e-6 && s.summaries[role].metrics.lagEquivalentMs.count===0,'stationary zero-correction head control');
        check(s.summaries[role].metrics.correctionMagnitude.max<1e-6,'unexpected correction in zero-correction control');
      }
      receipt.productionControls.push(s);
    }catch(e){receipt.failures.push({control:label,error:String(e)});}
  }
  if(receipt.productionControls.length===3) {
    receipt.delayCalibration={};
    for(const role of ['own','enemy']) {
      const zero=receipt.productionControls[1], delayed=receipt.productionControls[2];
      const observed=delayed.summaries[role].metrics.lagEquivalentMs.p50-zero.summaries[role].metrics.lagEquivalentMs.p50;
      const uncertaintyMs=Math.max(zero.maximumGapMs,delayed.maximumGapMs);
      receipt.delayCalibration[role]={injectedMs:40,observedShiftMs:observed,framePhaseBoundMs:uncertaintyMs};
      if(Math.abs(observed-40)>uncertaintyMs) receipt.failures.push({control:'known delay shift',role,observed,uncertaintyMs});
    }
  }
  check(git('write-tree')===sourceTree && !git('diff','--name-only') && hash(binary)===receipt.binarySha256,'source/binary changed during run');
  receipt.completed=receipt.failures.length===0;
  fs.writeFileSync(path.join(out,'summary.json'),JSON.stringify(receipt,null,2));fs.writeFileSync(path.join(out,'baseline.md'),markdown(receipt.results));
  if(!receipt.completed)process.exitCode=1;
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  try {const args=process.argv.slice(2);check(args.length===4 && args[0]==='--binary' && args[2]==='--out','usage: node tools/smoothing-lag.mjs --binary path --out new-directory');runMatrix(path.resolve(args[1]),path.resolve(args[3]));}
  catch(e){console.error(e.message);process.exitCode=1;}
}
