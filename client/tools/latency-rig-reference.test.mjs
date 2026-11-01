import {test} from 'node:test';
import assert from 'node:assert/strict';
import {ReferencePolicy,startReference} from './latency-rig-reference.mjs';
const f=(time,x=500,life=1,opponents=[])=>({time,life,rules:{pitRadius:1784},observation:{self:{head:{x,y:0},heading:0,speed:184,radius:10},arena:{radius:3584},opponents}});
test('phases begin at play and do not wait for a deep approach',()=>{
  const p=new ReferencePolicy();const a=p.decide(f(0)),b=p.decide(f(.5));
  assert.equal(a.heading,b.heading);assert.equal(a.boost,false);
  assert.notEqual(p.decide(f(2.1)).heading,p.decide(f(2.5)).heading);
  assert.equal(p.decide(f(4.1)).boost,true);assert.equal(p.decide(f(6.1)).boost,true);
  assert.equal(p.decide(f(20,3000,2)).boost,false);
});
test('received body obstructing the direct path changes the selected heading',()=>{
  const obstacle={id:'8',head:{x:100,y:0},body:[{x:100,y:0}],heading:0,speed:0,radius:10};
  const direct=new ReferencePolicy().decide(f(0,0));
  const avoided=new ReferencePolicy().decide(f(0,0,1,[obstacle]));
  assert.equal(direct.heading,0);assert.ok(Math.abs(avoided.heading)>.1);
});
test('formation target follows only the measured primary wire ID',()=>{
  const opponent=id=>({id,head:{x:500,y:0},body:[],heading:0,speed:184,radius:10});
  const other=new ReferencePolicy().decide(f(0,0,1,[opponent('9')]));
  const primary=new ReferencePolicy().decide(f(0,0,1,[opponent('1')]));
  assert.equal(other.heading,0);assert.ok(primary.heading>.2);
});
test('controlled reference rejects outside/reserved endpoints before any process starts',async()=>{
  for(const endpoint of ['ws://192.0.2.1:19090/slither','ws://127.0.0.1:8766/slither'])await assert.rejects(startReference('unused',endpoint,'unused'));
});
