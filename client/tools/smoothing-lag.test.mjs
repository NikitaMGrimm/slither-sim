import test from 'node:test';
import assert from 'node:assert/strict';
import { error, station, smoothness, response, distribution, analyzeProbe, completeProbe } from './smoothing-lag.mjs';

test('truth minus draw has signed millisecond units, with undefined stationary lag',()=>{
  assert.equal(error([10,0],[9,0],0,20).lagEquivalentMs,50);
  assert.equal(error([10,0],[11,0],0,20).lagEquivalentMs,-50);
  assert.equal(error([1,0],[0,0],0,0).lagEquivalentMs,null);
  assert.equal(error([0,1],[0,0],0,20).lateral,1);
});
test('ordered arclength stations cannot jump across a nearby crossing',()=>{
  const path=[[0,0],[10,0],[10,10],[0,10],[0,0]];
  assert.deepEqual(station(path,15),[10,5]);
  assert.equal(station(path,41),null);
});
test('nonuniform derivatives preserve constant velocity and acceleration units',()=>{
  const t=[0,5,12,31,44,60,103];
  const s=smoothness(t.map(t=>({t,q:[3*t/1000,4*t/1000]})));
  assert.ok(Math.abs(s.velocity.rms-5)<1e-10);
  assert.ok(s.acceleration.max<1e-10); assert.ok(s.jerk.max<1e-7);
  const a=smoothness(t.map(t=>({t,q:[(t/1000)**2,0]})));
  assert.ok(Math.abs(a.acceleration.rms-2)<1e-9);
});
test('known delay/ramp verifies t10/t50/t90 and zero detector false positives',()=>{
  const samples=Array.from({length:101},(_,i)=>({t:i*5,y:24*Math.min(1,Math.max(0,(i*5-40)/100))}));
  const r=response(samples,0,24);
  assert.equal(r.levels.t10.estimateMs,50);assert.equal(r.levels.t50.estimateMs,90);assert.equal(r.levels.t90.estimateMs,130);
  assert.equal(response(samples.map(s=>({...s,y:0})),0,24).levels.t10,null);
  assert.equal(response(samples.map(s=>({...s,y:-s.y})),0,-24).levels.t50.estimateMs,90);
});
test('a transient first crossing cannot masquerade as sustained response',()=>{
  const samples=Array.from({length:102},(_,i)=>{const t=(i-1)*5;return {t,y:t===0?1:Math.min(1,Math.max(0,(t-40)/100))};});
  const r=response(samples,0,1);
  assert.equal(r.levels.t50.estimateMs,0);
  assert.equal(r.recrossings.t50,1);
  assert.equal(r.sustainedLevels.t50.estimateMs,90);
});
test('p99 under 1000 samples is descriptive and incomplete runs fail closed',()=>{
  assert.equal(distribution([1,2,3]).p99DescriptiveOnly,true);
  assert.equal(distribution(Array(1000).fill(1)).p99DescriptiveOnly,false);
  assert.throws(()=>analyzeProbe({format:'smoothing-lag-v1',completed:false}),/incomplete/);
});

function fixture() {
  const stations=[0,42,84,168,252],rows=[];
  for(let frame=0;frame*1000/60<=8000;frame++) {
    const atMs=frame*(1000/60),x=1000+atMs*.02;
    const snakes=[true,false].map(own=>{
      const y=own?1000:1120;
      return {id:own?7:8,token:own?'1':'2',own,drawn:[x,y],raw:[x,y],heading:0,truthHeading:0,speed:20,scale:1,truth:[x,y],
        body:[[x,y],[x-400,y]],truthBody:stations.map(d=>[x-d,y]),truthBodyHeadings:stations.map(()=>0),
        truthBodySegments:stations.map(d=>[[x-d-21,y],[x-d+21,y]]),packetHead:[1000,y],anchorId:0,poseGeneratedMs:0};
    });
    rows.push({frame,sequence:String(frame),atMs,measured:atMs>=2000,acknowledged:true,camera:[x,1000],zoom:1,snakes});
  }
  return {format:'smoothing-lag-v1',completed:true,failures:[],config:{rate:'60'},originMs:0,durationMs:8000,warmupMs:2000,stations,packets:[],rows};
}
test('complete authored rows retain world lag, normalized body errors and screen smoothness',()=>{
  const r=analyzeProbe(fixture());
  assert.equal(r.summaries.own.metrics.lagEquivalentMs.p50,0);
  assert.equal(r.summaries.enemy.metrics.body252OverReach.max,0);
  assert.ok(Math.abs(r.summaries.own.smoothness.headWorld.velocity.rms-20)<1e-9);
  assert.equal(r.summaries.own.smoothness.headScreen.velocity.max,0);
  assert.equal(r.camera.worldError.max,0);
  assert.equal(r.summaries.own.smoothness.cameraWorld,undefined);
});
test('every production control requires complete own and enemy body stations',()=>{
  for(const anchors of [false,true]) for(const own of [true,false]) {
    const r=fixture();r.config.anchors=anchors;
    r.rows[200].snakes.find(s=>s.own===own).body=[[0,0]];
    assert.ok(analyzeProbe(r).summaries[own?'own':'enemy'].failures.length>0);
    assert.throws(()=>completeProbe(r),/missing body stations/);
  }
  assert.equal(completeProbe(fixture()).summaries.own.failures.length,0);
});
test('missing frames, duplicates, missing geometry and orphan packet provenance fail completeness',()=>{
  for(const change of [r=>r.rows.splice(20,1),r=>r.rows[20].sequence='0',r=>r.rows[20].snakes.pop(),r=>r.rows[20].snakes[0].anchorId=99]) {
    const r=fixture();change(r);assert.throws(()=>analyzeProbe(r),/missing|duplicate|orphan/);
  }
  const r=fixture();r.rows[200].snakes[0].body=[[0,0]];
  assert.ok(analyzeProbe(r).summaries.own.failures.length>0);
});
