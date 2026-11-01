import { readFile, access } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { analyze } from './latency-trace.mjs';

const check=(ok,message)=>{if(!ok)throw Error(message);};
const angle=(a,b)=>Math.atan2(Math.sin(a-b),Math.cos(a-b));
export function distribution(values) {
  const v=values.filter(Number.isFinite).sort((a,b)=>a-b);
  if(!v.length)return {count:0,p50:null,p90:null,p99:null,sparseP99:true};
  const q=p=>v[Math.ceil(p*v.length)-1];
  return {count:v.length,p50:q(.5),p90:q(.9),p99:q(.99),min:v[0],max:v.at(-1),sparseP99:v.length<1000};
}
export function lagError(truth,head,heading,speed) {
  const x=truth[0]-head[0],y=truth[1]-head[1];
  const tangent=x*Math.cos(heading)+y*Math.sin(heading);
  return {distance:Math.hypot(x,y),tangent,lateral:-x*Math.sin(heading)+y*Math.cos(heading),lagMs:speed>1e-6?tangent/speed*1000:null};
}
function rows(text) {return text.trim().split('\n').map(l=>JSON.parse(l));}
function lower(array,t,key) {
  let lo=0,hi=array.length;
  while(lo<hi){const m=(lo+hi)>>1;if(key(array[m])<t)lo=m+1;else hi=m;}
  return lo;
}
export function interpolateTruth(ticks,ns,id) {
  const index=lower(ticks,ns,t=>t.ns);
  if(index===0||index===ticks.length)return null;
  const a=ticks[index-1],b=ticks[index],s=a.heads.get(id),e=b.heads.get(id);
  if(!s||!e||!s[2]||!e[2]||s[1]!==e[1]||b.ns-a.ns>30_000_000n)return null;
  const phase=Number(ns-a.ns)/Number(b.ns-a.ns);
  return {life:s[1],position:[s[3]+(e[3]-s[3])*phase,s[4]+(e[4]-s[4])*phase],
    heading:s[5]+angle(e[5],s[5])*phase,speed:s[6]+(e[6]-s[6])*phase,boost:s[7],
    tickGapMs:Number(b.ns-a.ns)/1e6,stepBracketMs:Math.max(a.bracket,b.bracket)};
}
export async function analyzeCapture(directory) {
  const traceText=await readFile(path.join(directory,'latency.jsonl'),'utf8');
  const validation=analyze(traceText,{cpu:true}); // Offscreen has no acquire/present. Strengthen transport/submission below.
  const trace=rows(traceText).slice(1,-1);
  check(trace.filter(r=>r.stage==='message').every(r=>r.related!=='0'&&r.detail!=='0'),'offscreen message missing raw transport reads');
  const byStage=new Map();
  for(const r of trace){r.t=BigInt(r.ns);const k=`${r.generation}/${r.id}`;if(!byStage.has(r.stage))byStage.set(r.stage,new Map());byStage.get(r.stage).set(k,r);}
  const get=(stage,r,id=r.id)=>byStage.get(stage)?.get(`${r.generation}/${id}`);
  for(const r of trace.filter(r=>r.stage==='presented'))check(get('submit_end',r),'offscreen picture missing real GPU submit');
  const headRows=rows(await readFile(path.join(directory,'client/heads.jsonl'),'utf8'));
  const headHeader=headRows.shift(),tail=headRows.pop();
  check(headHeader.format==='client-latency-heads-v1'&&headHeader.complete&&headHeader.dropped===0&&tail.end&&tail.records===headRows.length,'incomplete head journal');
  let previous;
  const maps=[];
  for(const r of headRows){
    r.t=BigInt(r.trace_ns);r.ns=(BigInt(r.unix_before_ns)+BigInt(r.unix_after_ns))/2n;
    r.mapWidthMs=Number(BigInt(r.unix_after_ns)-BigInt(r.unix_before_ns))/1e6;
    check(r.mapWidthMs>=0,'reversed client system clock');
    if(previous)check(r.t>=previous.t&&Math.abs(Number((r.ns-previous.ns)-(r.t-previous.t)))/1e6<1,'client clock jump/drift exceeds 1ms per frame');
    maps.push(r);previous=r;
  }
  const byFrame=new Map(headRows.map(r=>[`${r.generation}/${r.frame}`,r]));
  const presented=new Set(trace.filter(r=>r.stage==='presented').map(r=>`${r.generation}/${r.id}`));
  const selected=headRows.filter(r=>presented.has(`${r.generation}/${r.frame}`));
  const needed=new Set(selected.flatMap(r=>r.heads.filter(h=>!h[2]).map(h=>h[0])));
  const ticks=[];let truthMetadata=null,summary=null,clockSteps=[];
  let hasTruth=true;try{await access(path.join(directory,'truth.jsonl'));}catch{hasTruth=false;}
  if(hasTruth){
    const input=createInterface({input:createReadStream(path.join(directory,'truth.jsonl')),crlfDelay:Infinity});
    let prev;
    for await(const line of input){
      const r=JSON.parse(line);
      if(r.type==='metadata'){check(!truthMetadata,'duplicate truth header');truthMetadata=r;continue;}
      if(r.type==='summary'){summary=r;continue;}
      check(r.type==='tick'&&truthMetadata&&!summary,'invalid truth order');
      const ns=BigInt(r.unix_after_ns),mono=BigInt(r.monotonic_after_ns);
      const before=BigInt(r.unix_before_ns);
      check(ns>=before,'reversed host step');
      if(prev)check(ns>=prev.ns&&mono>=prev.mono&&Math.abs(Number((ns-prev.ns)-(mono-prev.mono)))/1e6<1,'host wall clock jump/drift');
      ticks.push({ns,mono,bracket:Number(ns-before)/1e6,heads:new Map(r.heads.filter(h=>needed.has(h[0])).map(h=>[h[0],h]))});
      clockSteps.push(Number(ns-before)/1e6);prev={ns,mono};
    }
    check(truthMetadata?.schema==='host-latency-truth-v1'&&summary?.complete===true&&summary.ticks===ticks.length,'incomplete host truth');
  }
  const controls=rows(await readFile(path.join(directory,'client/controls.jsonl'),'utf8'));
  const controlMap=new Map(controls.map(r=>[`${r.generation}/${r.effect}`,r.bytes]));
  const groups=new Map(),total=new Map(),coverage={issued:selected.length,prepared:headRows.length,nearMiddle:0,withReference:0,truthMatched:0,transition:0};
  const push=(group,metric,value)=>{if(!Number.isFinite(value))return;if(!group.has(metric))group.set(metric,[]);group.get(metric).push(value);};
  const warm=new Map(), motionHistory=new Map();let activeIdentity=null;
  for(const r of selected){
    const h=r.heads.find(h=>!h[2]);
    if(!r.middle){activeIdentity=null;continue;}coverage.nearMiddle++;
    if(!h){activeIdentity=null;continue;}coverage.withReference++;
    const identity=`${r.generation}/${h[1]}`;
    if(activeIdentity!==identity){warm.set(identity,r.t);activeIdentity=identity;}
    const submit=byStage.get('submit_end')?.get(`${r.generation}/${r.frame}`);
    check(submit&&submit.t>=r.t,'missing/reversed offscreen head submission');
    const submittedNs=r.ns+(submit.t-r.t);
    const preparedTruth=hasTruth?interpolateTruth(ticks,r.ns,h[0]):null;
    const truth=hasTruth?interpolateTruth(ticks,submittedNs,h[0]):null;
    if(hasTruth&&!truth)continue;
    const heading=truth?.heading??h[9],speed=truth?.speed??h[10],boost=truth?.boost??h[11];
    const life=truth?.life??h[1];
    const key=`${h[0]}/${life}/${identity}`;
    if(!motionHistory.has(key))motionHistory.set(key,[]);
    const history=motionHistory.get(key);history.push({ns:r.ns,heading});
    const prevIndex=lower(history,r.ns-100_000_000n,x=>x.ns);
    if(prevIndex>=history.length-1||r.t-warm.get(identity)<2_000_000_000n)continue;
    const delta=Math.abs(angle(heading,history[prevIndex].heading));
    const motion=delta<=.05?'straight':delta>=.15?'turning':'transition';
    if(motion==='transition'){coverage.transition++;continue;}
    r.stratum=`${motion}/${boost?'boost':'normal'}`;
    if(!groups.has(r.stratum))groups.set(r.stratum,new Map());const group=groups.get(r.stratum);
    const packet=lagError([h[7],h[8]],[h[3],h[4]],heading,speed);
    push(group,'packetRelativeLagEquivalentMs',packet.lagMs);push(group,'packetRelativeHeadErrorUnits',packet.distance);
    if(truth){coverage.truthMatched++;const e=lagError(truth.position,[h[3],h[4]],heading,speed);push(group,'authoritativeLagEquivalentAtSubmitMs',e.lagMs);push(group,'authoritativeHeadErrorAtSubmitUnits',e.distance);push(group,'truthInterpolationGapMs',truth.tickGapMs);if(preparedTruth)push(group,'authoritativeLagEquivalentAtPrepareMs',lagError(preparedTruth.position,[h[3],h[4]],preparedTruth.heading,preparedTruth.speed).lagMs);}
  }
  const groupAt=r=>{
    const i=lower(selected,r.t,s=>s.t);
    const s=selected[i];return s&&s.generation===r.generation&&s.t-r.t<100_000_000n&&s.stratum?groups.get(s.stratum):null;
  };
  const interval=(name,start,end,frame)=>{
    if(!start||!end)return;const value=Number(end.t-start.t)/1e6;check(value>=0,`reversed ${name}`);
    push(total,name,value);const g=frame?groups.get(frame.stratum):groupAt(end);if(g)push(g,name,value);
  };
  for(const r of trace){
    if(r.stage==='apply_end') {interval('receiveToApplyMs',get('message',r),get('apply_begin',r));interval('applyMs',get('apply_begin',r),r);}
    if(r.stage==='build_end')interval('prepareMs',get('build_begin',r),r,byFrame.get(`${r.generation}/${r.id}`));
    if(r.stage==='presented'){
      const frame=byFrame.get(`${r.generation}/${r.id}`),build=get('build_end',r),submit=get('submit_end',r);
      interval('prepareEndToSubmitMs',build,submit,frame);
      if(build?.related!=='0')interval('receiveToSubmitMs',get('message',r,build.related),submit,frame);
      interval('submitToOffscreenAckMs',submit,r,frame);
    }
    if(r.stage==='decision'&&r.related!=='0'){
      const callback=get('callback',r,r.related),flushed=get('flushed',r),bytes=controlMap.get(`${r.generation}/${r.id}`);
      if(callback&&flushed&&bytes?.length===1){const kind=bytes[0]<=250?'heading':(bytes[0]===253||bytes[0]===254)?'boost':null;if(kind)interval(`callbackToFlushed.${kind}Ms`,callback,flushed);}
    }
  }
  const format=g=>Object.fromEntries([...g].map(([k,v])=>[k,distribution(v)]));
  for(const motion of ['straight','turning'])for(const speed of ['normal','boost'])if(!groups.has(`${motion}/${speed}`))groups.set(`${motion}/${speed}`,new Map());
  const load=JSON.parse(await readFile(path.join(directory,'load.json'),'utf8'));
  check(!load.contaminated,'compiler-contaminated capture');
  return {format:'latency-rig-results-v1',sourceRevision:validation.sourceRevision,truth:hasTruth,coverage,
    clock:{source:'system-time-unix+instant',observedMinPositiveClockStepNs:headHeader.observed_min_positive_clock_step_ns,mappingBracketMs:distribution(maps.map(r=>r.mapWidthMs)),hostStepBracketMs:distribution(clockSteps),nominalWindowsSystemTimeQuantumNs:100},
    machineCpuPercent:distribution(load.samples.map(s=>s.cpuPercent).filter(v=>v!==null)),
    stages:format(total),strata:Object.fromEntries([...groups].map(([k,g])=>[k,format(g)])),
    traceValidation:{complete:validation.complete,closedUnflushed:validation.closedUnflushed,mode:'offscreen: CPU lifecycle validation plus real raw-read and submit checks'},
    limitations:['No surface acquire, present, scanout or physical input. GPU waits are serialized.','UNIX/QPC storage precision is not clock accuracy. System-time jumps over 1ms per adjacent sample reject capture.','Authoritative truth is linearly interpolated between post-step samples (gap <=30ms). Lag equivalent during turns is spatial error divided by speed, not temporal delay.','Frame quantiles are correlated descriptive observations; sparse p99 is flagged. Missing strata remain missing.']};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  try{console.log(JSON.stringify(await analyzeCapture(path.resolve(process.argv[2])),null,2));}catch(e){console.error(String(e));process.exitCode=1;}
}
