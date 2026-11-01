import {test} from 'node:test';
import assert from 'node:assert/strict';
import {ReferencePolicy,startReference} from './latency-rig-reference.mjs';
test('scripted reference phases include straight/turning normal/boost and approach without boost',()=>{
  const policy=new ReferencePolicy();
  const f=(time,x=500,life=1)=>({time,life,rules:{pitRadius:1784},observation:{self:{head:{x,y:0},heading:0}}});
  assert.equal(policy.decide(f(0,3000)).boost,false);
  const a=policy.decide(f(10)),b=policy.decide(f(10.5));assert.equal(a.heading,b.heading);assert.equal(a.boost,false);
  const c=policy.decide(f(12.1)),d=policy.decide(f(12.5));assert.notEqual(c.heading,d.heading);assert.equal(c.boost,false);
  assert.equal(policy.decide(f(14.1)).boost,true);assert.equal(policy.decide(f(16.1)).boost,true);
  assert.equal(policy.decide(f(20,3000,2)).boost,false);
});
test('controlled reference rejects outside/reserved endpoints before any process starts',async()=>{
  for(const endpoint of ['ws://192.0.2.1:19090/slither','ws://127.0.0.1:8766/slither'])await assert.rejects(startReference('unused',endpoint,'unused'));
});
