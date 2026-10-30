// Composition boundary double; Rust preferences and actual DOM have separate ABI/headless checks.
import { test } from 'node:test';
import { runGameModuleTest } from './game-module-test-fixture.mjs';
test('login restores existing Canvas nickname and preserves its appearance record on edits',async()=>{
  await runGameModuleTest(async({assert,createGraph})=>{
    const graph=createGraph({armed:false,hudNative:true,ownedHudNative:true,ownedPlayNative:true,leaderboardNative:true,minimapNative:true,
      prepareWasm(api,run){api.LeaderboardScoreProjector=run('(class { project(){throw Error("No scores expected");} free(){} })');}});
    graph.stubLogin();graph.hudWasm();
    const policy=new graph.wasm.api.MenuPreferences(false,'');policy.set_nickname('Remembered');
    const saved=policy.serialize(),before=policy.view();policy.free();
    const writes=[];
    const entry=await graph.load('bootstrap.mjs');graph.arm();
    const realm=graph.nativeBoundary(native=>Object.freeze({...native,menuPreferences:Object.freeze({__proto__:null,schema:1,document:graph.f.document,read:()=>saved,write:value=>{writes.push(value);return true;},setNickname(){}})}));
    const runtime=await entry.startGamePage({realm,initialize(){},wasmApi:graph.wasm.api});
    assert.equal(graph.login.nickname,'Remembered');
    graph.login.onEdit('Edited');assert.equal(writes.length,1);
    const restored=new graph.wasm.api.MenuPreferences(false,writes[0]),after=restored.view();
    assert.equal(after.nickname,'Edited');assert.equal(after.skin,before.skin);assert.deepEqual(after.customSkin,before.customSkin);
    restored.free();assert.equal(graph.f.sockets,0);runtime.dispose();
    graph.login.onEdit('Late');assert.equal(writes.length,1);
  });
});
