import { test } from 'node:test';
import assert from 'node:assert/strict';
import { optionsFor } from './latency-rig.mjs';
import { distribution, lagError, interpolateTruth } from './latency-rig-analysis.mjs';

test('manual rig arguments are finite and bounded without acquisition',()=>{
  const base=['--host-bin','host.exe','--client-bin','client.exe','--output','unused'];
  assert.equal(optionsFor(base).seconds,60);
  for(const args of [['--seconds','NaN'],['--seconds','121'],['--zoom','-999'],['--unknown','x'],['--zoom','0','--zoom','0']])assert.throws(()=>optionsFor([...base,...args]));
});
test('nearest-rank distributions include missing strata and sparse p99',()=>{
  assert.equal(distribution([]).count,0);assert.equal(distribution([]).p99,null);
  const d=distribution(Array.from({length:1000},(_,i)=>i+1));
  assert.equal(d.p50,500);assert.equal(d.p90,900);assert.equal(d.p99,990);assert.equal(d.sparseP99,false);
});
test('lag equivalent sign and stationary undefined value',()=>{
  assert.equal(lagError([10,0],[0,0],0,100).lagMs,100);
  assert.equal(lagError([0,0],[10,0],0,100).lagMs,-100);
  assert.equal(lagError([10,0],[0,0],0,0).lagMs,null);
});
test('truth interpolation cannot join dead/replaced lives or long gaps',()=>{
  const row=(ns,life,x,alive=true)=>({ns,bracket:.1,heads:new Map([[7,[7,life,alive,x,0,0,100,false]]])});
  assert.equal(interpolateTruth([row(0n,1,0),row(10_000_000n,1,1)],5_000_000n,7).position[0],.5);
  assert.equal(interpolateTruth([row(0n,1,0),row(10_000_000n,2,1)],5_000_000n,7),null);
  assert.equal(interpolateTruth([row(0n,1,0),row(10_000_000n,1,1,false)],5_000_000n,7),null);
  assert.equal(interpolateTruth([row(0n,1,0),row(40_000_000n,1,4)],5_000_000n,7),null);
});
