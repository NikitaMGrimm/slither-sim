// Independent reconstructions for the two latency-rig baseline ambiguities.
import {writeFile} from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {distribution,lagError} from './latency-rig-analysis.mjs';
import {analyze} from './latency-trace.mjs';
import {evidenceText} from './latency-rig-storage.mjs';
const check=(v,m)=>{if(!v)throw Error(m);};
const ms=(a,b)=>Number(BigInt(b)-BigInt(a))/1e6;
const wrap=x=>Math.atan2(Math.sin(x),Math.cos(x));
const parse=text=>text.trim().split('\n').map(JSON.parse);

/** Each message independently searches the first submitted consumed prefix.
 * No cursor, numeric-ID ordering or percentile subtraction is used. */
export function auditWatermarks(events) {
  const key=(r,id=r.id)=>`${r.thread}/${r.generation}/${id}`;
  const table=new Map();
  for(const r of events){if(!table.has(r.stage))table.set(r.stage,new Map());table.get(r.stage).set(key(r),r);}
  const get=(stage,r,id=r.id)=>table.get(stage)?.get(key(r,id));
  const frames=[];
  for(const ack of events.filter(r=>r.stage==='presented')){
    const begin=get('build_begin',ack),end=get('build_end',ack),submit=get('submit_end',ack);
    check(begin&&end&&submit,'issued picture missing build/submit');
    if(end.related==='0')continue;
    const watermark=get('apply_end',ack,end.related),message=get('message',ack,end.related);
    check(watermark&&message,'unresolved frame prefix');
    frames.push({ack,begin,end,submit,watermark,message,age:ms(message.ns,submit.ns)});
  }
  const first=[];
  for(const message of events.filter(r=>r.stage==='message')){
    const applied=get('apply_end',message),applying=get('apply_begin',message);
    check(applied&&applying,'message missing apply');
    const frame=frames.find(f=>f.ack.thread===message.thread&&f.ack.generation===message.generation&&f.watermark.event>=applied.event);
    if(!frame)continue; // Retained separately: no submitted consumer before shutdown.
    const total=ms(message.ns,frame.submit.ns);
    const c={receiveToApply:ms(message.ns,applying.ns),apply:ms(applying.ns,applied.ns),
      afterApplyToBuild:ms(applied.ns,frame.begin.ns),build:ms(frame.begin.ns,frame.end.ns),
      buildEndToSubmit:ms(frame.end.ns,frame.submit.ns)};
    const overlap=c.afterApplyToBuild<0;
    const sum=Object.values(c).reduce((a,b)=>a+b,0);
    check(Math.abs(sum-total)<1e-8,'causal component sum changed total');
    first.push({generation:message.generation,message:message.id,frame:frame.ack.id,total,overlap,components:c});
  }
  const seen=new Set();
  for(const f of frames){const k=key(f.message);f.reused=seen.has(k);seen.add(k);}
  const ordinary=first.filter(r=>!r.overlap);
  const intervals=[],gpuWait=[],idle=[];
  for(let i=1;i<frames.length;i++){
    const a=frames[i-1],b=frames[i];
    if(a.ack.generation!==b.ack.generation)continue;
    intervals.push(ms(a.begin.ns,b.begin.ns));
    gpuWait.push(ms(a.submit.ns,a.ack.ns));
    idle.push(ms(a.ack.ns,b.begin.ns));
  }
  return {submittedFramesWithWatermark:frames.length,reusedWatermarkFrames:frames.filter(f=>f.reused).length,
    watermarkAgeMs:distribution(frames.map(f=>f.age)),firstConsumerMs:distribution(first.map(f=>f.total)),
    ageOver40Ms:frames.filter(f=>f.age>40).length,ageOver40AndReused:frames.filter(f=>f.age>40&&f.reused).length,
    firstConsumerOver40Ms:first.filter(f=>f.total>40).length,
    unconsumedMessages:events.filter(r=>r.stage==='message').length-first.length,
    setupOverlappingBuilds:first.filter(f=>f.overlap).length,
    ordinaryComponentsMs:Object.fromEntries(Object.keys(ordinary[0]?.components??{}).map(k=>[k,distribution(ordinary.map(r=>r.components[k]))])),
    frameBuildIntervalMs:distribution(intervals),serializedGpuWaitMs:distribution(gpuWait),idleAfterAckMs:distribution(idle),
    longestFirstConsumers:first.sort((a,b)=>b.total-a.total).slice(0,8),
    note:'Watermark age counts repeated frames; firstConsumer counts each applied message once. Intervals and GPU waits belong to this offscreen driver; they do not measure desktop swapchain pacing.'};
}

/** Position-cache refreshes observed at prepared frames, not exact RX times. */
export function auditAnchorRefresh(rows) {
  let previous=null,lastChange=null;
  const gaps=[],lower=[],upper=[],steps=[],slopes=[],resets=[],seriesByToken=new Map();
  for(const r of rows){
    const h=r.heads.find(h=>!h[2]);
    if(!h){previous=null;lastChange=null;continue;}
    const identity=`${r.generation}/${h[0]}/${h[1]}`,t=BigInt(r.trace_ns);
    const value=lagError([h[7],h[8]],[h[3],h[4]],h[9],h[10]).lagMs;
    if(!seriesByToken.has(identity))seriesByToken.set(identity,[]);
    seriesByToken.get(identity).push({ms:Number(t)/1e6,packetRelativeMs:value,packetX:h[7],packetY:h[8]});
    if(previous?.identity!==identity||ms(previous.t,t)>50){previous={identity,t,h,value};lastChange=null;continue;}
    const dt=ms(previous.t,t),distance=Math.hypot(h[7]-previous.h[7],h[8]-previous.h[8]);
    if(distance>1e-6){
      steps.push(distance);
      if(lastChange&&distance<63){gaps.push(ms(lastChange.hi,t));lower.push(ms(lastChange.hi,previous.t));upper.push(ms(lastChange.lo,t));}
      if(Math.abs(wrap(h[9]-previous.h[9]))<.01&&Math.abs(h[10]-previous.h[10])<.1)resets.push(value-previous.value+dt);
      lastChange=distance<63?{lo:previous.t,hi:t}:null;
    }else if(dt>0&&Math.abs(wrap(h[9]-previous.h[9]))<.01&&Math.abs(h[10]-previous.h[10])<.1){slopes.push((value-previous.value)/dt);}
    previous={identity,t,h,value};
  }
  const longest=[...seriesByToken].sort((a,b)=>b[1].length-a[1].length)[0];
  return {endpoint:'frame-observed received-head-cache refresh (not exact per-reference wire arrival)',
    observedIntervalsMs:distribution(gaps),intervalLowerBoundsMs:distribution(lower),intervalUpperBoundsMs:distribution(upper),
    packetHeadStepUnits:distribution(steps),straightHeldAnchorSlopeMsPerMs:distribution(slopes),
    straightResetPlusFrameElapsedMs:distribution(resets),
    longestReference:longest?{identity:longest[0],samples:longest[1]}:null,
    limits:'Only contiguous same-token samples with <=50ms frame gaps; no interval bridges a missing reference. Bounds retain both frame-quantized refresh endpoints. Motion-only reports cannot be recovered from head positions.'};
}

export async function auditCapture(directory) {
  const text=await evidenceText(path.join(directory,'latency.jsonl'));
  analyze(text,{cpu:true});
  const events=parse(text).slice(1,-1);
  const head=parse(await evidenceText(path.join(directory,'client/heads.jsonl'))).slice(1,-1);
  return {format:'latency-rig-anomaly-audit-v1',watermarks:auditWatermarks(events),anchorRefresh:auditAnchorRefresh(head)};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  try{const d=path.resolve(process.argv[2]),r=await auditCapture(d);await writeFile(path.join(d,'anomaly-audit-v1.json'),JSON.stringify(r,null,2),{flag:'wx'});
    console.log(JSON.stringify({...r,anchorRefresh:{...r.anchorRefresh,longestReference:r.anchorRefresh.longestReference?.identity}},null,2));
  }catch(e){console.error(String(e));process.exitCode=1;}
}
