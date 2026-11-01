import {test} from 'node:test';
import assert from 'node:assert/strict';
import {referenceCadence} from './latency-rig-poses.mjs';
test('reference channels use original RX times, deduplicate bundles and exclude other snakes and selection gaps',()=>{
  const event=(id,ns)=>[{stage:'message',generation:1,id:String(id),ns:String(ns)},{stage:'apply_end',generation:1,id:String(id),ns:String(ns+10)}];
  const events=[...event(1,10),...event(2,240000010),...event(3,300000010),...event(4,500000010),...event(5,740000010)];
  const head=(ns,id=7,token='4')=>({generation:1,trace_ns:String(ns),heads:id===null?[]:[[id,token,false]]});
  const heads=[head(0),head(400000000),head(450000000,null),head(490000000,7,'5'),head(800000000,7,'5')];
  const p=(message,id,kind)=>({generation:1,message:String(message),id,kind});
  const result=referenceCadence(events,heads,[p(1,7,2),p(2,7,2),p(2,7,2),p(2,7,3),p(3,8,2),p(4,7,2),p(5,7,2)]);
  assert.equal(result.intervals['anchor.receiveMs'].count,2);
  assert.equal(result.intervals['anchor.receiveMs'].p50,240);
  assert.equal(result.intervals['anyPose.receiveMs'].count,2);
  assert.equal(result.counts.anchor,4);
  assert.equal(result.intervals['motion.receiveMs'],undefined);
});
