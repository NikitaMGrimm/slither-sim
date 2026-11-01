// Exact per-reference receipt cadence, independently of frame-observed cache ages.
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {distribution} from './latency-rig-analysis.mjs';
import {evidenceText} from './latency-rig-storage.mjs';
const check=(ok,message)=>{if(!ok)throw Error(message);};
const parse=text=>text.trim().split('\n').map(JSON.parse);
const ms=(a,b)=>Number(BigInt(b)-BigInt(a))/1e6;

export function referenceCadence(events,heads,poses) {
  const messages=new Map(events.filter(r=>r.stage==='message').map(r=>[`${r.generation}/${r.id}`,r]));
  const applied=new Map(events.filter(r=>r.stage==='apply_end').map(r=>[`${r.generation}/${r.id}`,r]));
  const segments=[];let current=null;
  for(const row of heads){
    const h=row.heads.find(h=>!h[2]);
    const key=h?`${row.generation}/${h[0]}/${h[1]}`:null;
    if(!key){current=null;continue;}
    if(current?.key!==key){current={key,generation:row.generation,id:h[0],begin:BigInt(row.trace_ns),end:BigInt(row.trace_ns)};segments.push(current);}
    current.end=BigInt(row.trace_ns);
  }
  const byKind=new Map(),counts={appearance:0,removal:0,anchor:0,motion:0},channels=['appearance','removal','anchor','motion'];
  const add=(key,value)=>{if(!byKind.has(key))byKind.set(key,[]);byKind.get(key).push(value);};
  const details=[];
  for(const segment of segments){
    const last=new Map(),seen=new Set(),records=[];
    for(const pose of poses){
      if(pose.generation!==segment.generation||pose.id!==segment.id)continue;
      const key=`${pose.generation}/${pose.message}`,message=messages.get(key),apply=applied.get(key);
      check(message&&apply,'pose receipt lacks original receive/apply join');
      const admitted=BigInt(apply.ns);
      if(admitted<segment.begin||admitted>segment.end)continue;
      const kind=channels[pose.kind];check(kind,'unknown pose kind');
      // One bundled WebSocket message contributes at most one timestamp per channel.
      const unique=`${pose.message}/${kind}`;if(seen.has(unique))continue;seen.add(unique);counts[kind]++;
      if(kind==='appearance'||kind==='removal'){last.clear();continue;}
      for(const channel of [kind,'anyPose']){
        const prior=last.get(channel);
        if(prior?.message===pose.message)continue;
        if(prior){add(`${channel}.receiveMs`,ms(prior.receive,message.ns));add(`${channel}.applyMs`,ms(prior.apply,apply.ns));}
        last.set(channel,{message:pose.message,receive:message.ns,apply:apply.ns});
      }
      records.push({message:pose.message,kind,receive_ns:message.ns,apply_ns:apply.ns});
    }
    if(records.length)details.push({identity:segment.key,begin_ns:String(segment.begin),end_ns:String(segment.end),records});
  }
  return {endpoint:'native completed WebSocket message receipt and committed apply; selected reference only',
    counts,intervals:Object.fromEntries([...byKind].map(([k,v])=>[k,distribution(v)])),segments:details,
    limits:'Conservative selected-reference segments only; no interval bridges selection gaps, removal or appearance. Bundled commands deduplicated per message and channel. No absent motion reports are invented.'};
}
export async function analyzePoses(directory) {
  const trace=parse(await evidenceText(path.join(directory,'latency.jsonl')));
  const heads=parse(await evidenceText(path.join(directory,'client/heads.jsonl')));
  const poses=parse(await evidenceText(path.join(directory,'client/heads.poses.jsonl')));
  const header=poses.shift(),end=poses.pop();
  check(header.format==='client-latency-poses-v1'&&header.complete&&header.dropped===0&&end.end&&end.records===poses.length,'incomplete pose journal');
  return {format:'latency-rig-reference-cadence-v1',...referenceCadence(trace.slice(1,-1),heads.slice(1,-1),poses)};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  try{console.log(JSON.stringify(await analyzePoses(path.resolve(process.argv[2])),null,2));}
  catch(e){console.error(String(e));process.exitCode=1;}
}
