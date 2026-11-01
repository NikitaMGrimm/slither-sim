// Optional controlled reference via the existing bot API, never authoritative state.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {writeFile} from 'node:fs/promises';
import {performance} from 'node:perf_hooks';
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));

export class ReferencePolicy {
  constructor(){this.life=null;this.started=0;this.phase=-1;this.heading=0;}
  decide(frame){
    if(frame.life!==this.life){this.life=frame.life;this.started=frame.time;this.phase=-1;}
    const view=frame.observation,own=view.self,{x,y}=own.head;
    // The runner freezes Battledome: match the primary rig's live-wall ratio.
    const pit=view.arena.radius*1784/3584,radius=Math.hypot(x,y);
    const primary=view.opponents.find(s=>String(s.id)==='1');
    let target=primary?{
      x:primary.head.x-Math.sin(primary.heading)*250,
      y:primary.head.y+Math.cos(primary.heading)*250,
    }:{x:0,y:0};
    if(radius<pit&&Math.hypot(target.x,target.y)>pit*.85){
      const scale=pit*.85/Math.hypot(target.x,target.y);target={x:target.x*scale,y:target.y*scale};
    }
    const goal=Math.atan2(target.y-y,target.x-x),elapsed=frame.time-this.started;
    const phase=Math.floor(elapsed/2)%4,within=elapsed%2;
    if(phase!==this.phase){this.phase=phase;this.heading=goal;}
    const catching=primary&&Math.hypot(primary.head.x-x,primary.head.y-y)>950;
    let desired=radius>pit*.92||catching?goal:this.heading+(phase%2?within*2.5:0);
    let boost=phase>=2;
    const speed=boost?437.5:Math.max(184,own.speed);
    // A bounded heuristic over received obstacles; it does not guarantee survival.
    const nearby=view.opponents.map(s=>({s,d:Math.hypot(s.head.x-x,s.head.y-y)}))
      .sort((a,b)=>a.d-b.d).slice(0,24).map(r=>r.s);
    const obstacles=nearby.flatMap(s=>s.body.slice(0,32).map(p=>({...p,r:(s.radius??20)+(own.radius??20)+28})));
    const wrap=a=>Math.atan2(Math.sin(a),Math.cos(a));
    let best={score:-Infinity,heading:desired,clearance:-Infinity};
    for(const offset of [0,.25,-.25,.5,-.5,.8,-.8,1.2,-1.2]){
      const heading=desired+offset;let clearance=Infinity;
      for(const t of [.2,.4,.6]){
        const direction=own.heading+wrap(heading-own.heading)*Math.min(1,t/.4);
        const px=x+Math.cos(direction)*speed*t,py=y+Math.sin(direction)*speed*t;
        clearance=Math.min(clearance,view.arena.radius-Math.hypot(px,py)-100);
        if(radius<pit-60)clearance=Math.min(clearance,pit-Math.hypot(px,py)-60);
        for(const p of obstacles)clearance=Math.min(clearance,Math.hypot(px-p.x,py-p.y)-p.r);
        for(const s of nearby)clearance=Math.min(clearance,Math.hypot(px-s.head.x-Math.cos(s.heading)*s.speed*t,py-s.head.y-Math.sin(s.heading)*s.speed*t)-(s.radius??20)-(own.radius??20)-28);
      }
      const score=Math.min(clearance,100)-Math.abs(offset)*25;
      if(score>best.score)best={score,heading,clearance};
    }
    desired=best.heading;
    if(best.clearance<0)boost=false;
    return {heading:desired,boost};
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
          journal.push({time:frame.time,life:frame.life,id,head:frame.observation.self.head,reportedPit:frame.rules?.pitRadius,receivedWall:frame.observation.arena.radius,action});
        }
        frame=await request({op:'step',...(action?{action}:{})});
        await sleep(Math.max(0,42-(performance.now()-start)));
      }
    }catch(e){if(!stopping)failure=e;}
  })();
  return {id,get failure(){return failure;},finished,async stop(){
    stopping=true;child.stdin.end();child.kill();await finished;
    await writeFile(output,JSON.stringify({format:'latency-rig-scripted-reference-v3',complete:!failure,id,failure:String(failure??''),journal,stderr},null,2),{flag:'wx'});
    if(failure)throw failure;
  }};
}
