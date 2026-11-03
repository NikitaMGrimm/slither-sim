import {schedule} from './analysis.mjs';

export function roundPlan(rounds,clients,{telemetryControls=true,limitedNtlView=false}={}) {
  return schedule(rounds,clients).flatMap(({round,client},index)=>['battledome','normal'].flatMap(scenario=>{
    const base={round,client,scenario,viewCondition:scenario==='normal'?'stock':client==='ntl'&&limitedNtlView?'ntl-farthest':'whole-map'};
    const on={...base,telemetry:'on'},off={...base,telemetry:'off'};
    const rows=round===1&&telemetryControls&&client!=='vlither'?(index%2?[off,on]:[on,off]):[on];
    if(scenario==='battledome'&&limitedNtlView&&['original','native'].includes(client))rows.push({...on,viewCondition:'ntl-farthest'});
    return rows;
  }));
}

export function anchoredBoundary(view,census) {
  const b=view.boundary,p=census?.players?.[0],radius=census?.radius;
  if(!b||!p?.alive||!p.setup||!Number.isFinite(radius)||radius<=0)return null;
  const scale=b.radius/radius,[width,height]=view.viewport;
  const predicted=[width/2-p.x*scale,height/2-p.y*scale];
  // Camera smoothing and receipt lag are allowed; a pit circle at a different
  // radius cannot also explain the projected outer-map origin at the orbit.
  const error=Math.hypot(b.x-predicted[0],b.y-predicted[1]);
  if(error>Math.max(30,b.radius*.04))return null;
  return {physicalPixelsPerWorldUnit:scale,centerErrorPixels:error,
    wholeMap:b.x-b.radius>=0&&b.y-b.radius>=0&&b.x+b.radius<=width&&b.y+b.radius<=height,
    confidence:'outer-circle candidate anchored to host radius and projected player offset; camera smoothing tolerance 4% or 30 pixels'};
}

export function zoomPlateau(samples) {
  const scales=samples.map(s=>s.physicalPixelsPerWorldUnit);
  return scales.length>=3&&scales.every(v=>Number.isFinite(v)&&v>0)
    &&Math.max(...scales)/Math.min(...scales)<=1.03;
}
