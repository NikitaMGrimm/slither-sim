// Optional controlled reference via the existing bot API, never authoritative state.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {writeFile} from 'node:fs/promises';
import {performance} from 'node:perf_hooks';
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));

export class ReferencePolicy {
  constructor(){this.life=null;this.entered=null;this.phase=-1;this.heading=0;}
  decide(frame){
    if(frame.life!==this.life){this.life=frame.life;this.entered=null;this.phase=-1;}
    const own=frame.observation.self,{x,y}=own.head;
    const pit=frame.rules?.pitRadius;
    if(!(pit>0))throw Error('controlled reference needs reported pit geometry');
    const radius=Math.hypot(x,y);
    if(this.entered===null&&radius<pit*.6)this.entered=frame.time;
    if(this.entered===null||radius>pit*.8){
      this.phase=-1;
      const inward=Math.max(-2,Math.min(8,(radius-pit*.4)/250));
      return {heading:Math.atan2(x/radius-inward*y/radius,-y/radius-inward*x/radius),boost:false};
    }
    const elapsed=frame.time-this.entered,phase=Math.floor(elapsed/2)%4,within=elapsed%2;
    if(phase!==this.phase){this.phase=phase;this.heading=radius>20?Math.atan2(-y,-x):own.heading;}
    return {heading:this.heading+(phase%2?within*2.5:0),boost:phase>=2};
  }
}

export async function startReference(binary,endpoint,output){
  const url=new URL(endpoint);
  if(url.protocol!=='ws:'||url.hostname!=='127.0.0.1'||[8766,18884,18885].includes(Number(url.port)))throw Error('reference is loopback-only');
  const child=spawn(binary,['client'],{windowsHide:true,stdio:['pipe','pipe','pipe']});
  const lines=createInterface({input:child.stdout})[Symbol.asyncIterator]();
  let stderr='',stopping=false,id=null,failure=null;
  child.stderr.setEncoding('utf8');child.stderr.on('data',s=>{stderr=(stderr+s).slice(-65536);});
  child.on('error',e=>{failure=e;});
  child.stdin.on('error',e=>{if(!stopping)failure=e;});
  const request=async value=>{
    if(failure)throw failure;
    child.stdin.write(JSON.stringify(value)+'\n');
    let timer;
    const next=await Promise.race([lines.next(),new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('reference API deadline')),5000);})]).finally(()=>clearTimeout(timer));
    if(next.done)throw Error(`reference API closed: ${stderr}`);
    const reply=JSON.parse(next.value);
    if(reply.protocol!=='slither-client-json-v2'||reply.error)throw Error(`reference API: ${reply.error??'contract'}`);
    if(reply.result.frame.contract!=='slither-client-v2')throw Error('reference frame contract');
    return reply.result.frame;
  };
  const journal=[],policy=new ReferencePolicy();
  let frame;
  try{
    frame=await request({op:'network',config:{url:endpoint,nickname:'Latency reference'}});
    const deadline=performance.now()+15000;
    while(frame.status!=='playing'){
      if(performance.now()>deadline)throw Error('reference join deadline');
      frame=await request({op:'step'});await sleep(20);
    }
    id=Number(frame.observation.self.id);
  }catch(e){child.kill();throw e;}
  const finished=(async()=>{
    try{
      while(!stopping){
        const start=performance.now();let action;
        if(frame.status==='dead'||frame.status==='disconnected')frame=await request({op:'reconnect'});
        if(frame.status==='playing'&&frame.observation){
          if(Number(frame.observation.self.id)!==id)throw Error('reference slot changed after reconnect');
          action=policy.decide(frame);
          if(journal.length>=8192)throw Error('reference control journal overflow');
          journal.push({time:frame.time,life:frame.life,id,head:frame.observation.self.head,action});
        }
        frame=await request({op:'step',...(action?{action}:{})});
        await sleep(Math.max(0,42-(performance.now()-start)));
      }
    }catch(e){if(!stopping)failure=e;}
  })();
  return {id,get failure(){return failure;},finished,async stop(){
    stopping=true;child.stdin.end();child.kill();await finished;
    await writeFile(output,JSON.stringify({format:'latency-rig-scripted-reference-v1',complete:!failure,id,failure:String(failure??''),journal,stderr},null,2),{flag:'wx'});
    if(failure)throw failure;
  }};
}
