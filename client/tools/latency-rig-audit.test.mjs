import {test} from 'node:test';
import assert from 'node:assert/strict';
import {auditWatermarks,auditAnchorRefresh} from './latency-rig-audit.mjs';

function tape(stall=10) {
  const e=[];let event=0;
  const add=(stage,id,ns,related='0')=>e.push({stage,id:String(id),ns:String(ns*1e6),related:String(related),thread:0,generation:1,event:event++});
  add('message',1,0);add('apply_begin',1,.1);add('apply_end',1,1);
  add('message',2,5);add('apply_begin',2,5.1);add('apply_end',2,6);
  for(const [id,t] of [[10,stall],[11,100],[12,200]]){add('build_begin',id,t,2);add('build_end',id,t+2,2);add('submit_end',id,t+3);add('presented',id,t+5);}
  return e;
}
test('repeated watermark age cannot invent delayed first use; every prefix member counted',()=>{
  const r=auditWatermarks(tape());
  assert.equal(r.firstConsumerMs.count,2);assert.equal(r.firstConsumerMs.p50,8);assert.equal(r.firstConsumerMs.p90,13);
  assert.equal(r.watermarkAgeMs.max,198);assert.equal(r.ageOver40Ms,2);assert.equal(r.ageOver40AndReused,2);
  assert.equal(r.firstConsumerOver40Ms,0);
});
test('a genuine late first consumer stays visible instead of being discarded as stale',()=>{
  const r=auditWatermarks(tape(70));assert.equal(r.firstConsumerOver40Ms,2);assert.equal(r.firstConsumerMs.max,73);
});
test('anchor refresh cadence has frame bounds and never bridges replacement tokens',()=>{
  const rows=[];
  for(let i=0;i<40;i++){const x=Math.floor(i/14)*42;rows.push({generation:1,trace_ns:String(i*16_000_000),heads:[[7,'9',false,i*3,0,i*3,0,x,0,0,187.5,false]]});}
  const r=auditAnchorRefresh(rows);assert.equal(r.observedIntervalsMs.p50,224);assert.equal(r.packetHeadStepUnits.p50,42);
  assert.equal(r.intervalLowerBoundsMs.p50,208);assert.equal(r.intervalUpperBoundsMs.p50,240);
  assert.equal(r.straightHeldAnchorSlopeMsPerMs.p50,-1);
  rows[28].heads[0][1]='10';assert.equal(auditAnchorRefresh(rows).observedIntervalsMs.count,0);
});
