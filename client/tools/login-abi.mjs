import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import path from 'node:path';
import { COSMETIC_PATHS } from '../apps/wgpu-slice/web/menu-art.mjs';

// Actual generated classes, no socket/DOM/GPU or policy double.
for(const base of ['dist/web/slither_client','dist/wgpu-slice/pkg/slither_wgpu_slice']) {
  const wasm=await import(pathToFileURL(path.resolve(`${base}.js`)));
  wasm.initSync({module:await readFile(`${base}_bg.wasm`)});
  assert.equal(typeof wasm.LoginMenu,'function',base);
  // The Wasm inventory also includes the two world backgrounds.
  if(wasm.CosmeticImages) assert.deepEqual([...wasm.CosmeticImages.paths()].sort(),[...COSMETIC_PATHS,'s/bg54.jpg','s/gbg.jpg'].sort());
  const zero=new wasm.LoginMenu('en','1');
  assert.throws(()=>zero.step(0,0,0,36,true,false),error=>!(error instanceof WebAssembly.RuntimeError));
  assert.equal(zero.step(16,960,540,36,true,false).disabled,false);zero.free();
  for(const terminal of ['failed','disconnected']) {
    const retry=new wasm.LoginMenu('en','1');
    const admission=wasm.PlayAdmission ? new wasm.PlayAdmission(true,false) : null;
    retry.step(0,960,540,36,true,false);retry.action('play',false,true);
    assert.equal(retry.observe_session(terminal),false);
    assert.ok(Object.hasOwn(retry.step(16,960,540,36,true,false),'request'));
    admission?.accept();
    assert.equal(retry.observe_session(terminal),true);
    if(admission){assert.equal(admission.attempt_return_ready(terminal),true);admission.complete_return();assert.equal(admission.accepted(),false);}
    const returned=retry.step(32,960,540,36,true,false);
    assert.equal(returned.disabled,false);assert.equal(returned.spinner_visible,false);assert.equal(Object.hasOwn(returned,'request'),false);
    retry.action('play',false,true);assert.ok(Object.hasOwn(retry.step(48,960,540,36,true,false),'request'));
    assert.equal(Object.hasOwn(retry.step(64,960,540,36,true,false),'request'),false);
    retry.free();admission?.free();
  }
  for(const [record,legacy,quality] of [['1:0','1','low'],['1:1','0','high'],[undefined,'0','low'],['2:0:native','0','low'],['1:0:extra','00','high'],['1'.repeat(65),'0','low'],[undefined,undefined,'high']]) {
    const migrated=new wasm.LoginMenu('en','1');
    migrated.restore_settings(record,legacy);
    assert.equal(migrated.step(0,960,540,36,true,false).quality,quality);
    assert.equal(migrated.settings_record(),quality==='low'?'1:0':'1:1');
    migrated.action('play',false,true);
    assert.equal(migrated.step(16,960,540,36,true,false).requested_quality,quality);
    migrated.free();
  }
  const menu=new wasm.LoginMenu('fr','0');
  assert.equal(menu.edit('A😀\u007f'),'A  \u007f');
  let view=menu.step(0,1280,720,36,true,false);
  assert.equal(view.quality,'low');assert.equal(view.play_label,'Jouer');assert.equal(view.focus,true);
  assert.deepEqual(view.input,[530,367,220,24]);
  menu.action('enter',true,true);assert.equal(menu.step(16,1280,720,36,true,false).disabled,false);
  menu.action('play',false,false);assert.equal(menu.step(32,1280,720,36,true,false).disabled,false);
  menu.action('play',false,true);menu.action('play',false,true);
  view=menu.step(48,1280,720,36,true,false);assert.equal(view.request,'A  \u007f');assert.equal(view.requested_quality,'low');assert.equal(view.spinner_visible,true);
  assert.equal(Object.hasOwn(menu.step(64,960,540,36,true,false),'request'),false);
  assert.equal(menu.step(80,960,540,36,true,true).alpha,1);
  assert.ok(menu.step(96,960,540,36,true,true).alpha<1);
  menu.free();console.log(`${base}: login ABI passed`);
  const early=new wasm.LoginMenu('en','1');
  early.edit('Stored'); early.step(0,960,540,36,true,false);early.action('play',false,true);
  early.step(16,960,540,36,true,false);
  early.observe_life('playing','',true,false,false,1,undefined,undefined);
  early.observe_life('dead','dead',true,true,false,1,1001,1n);
  early.action('play',false,false); // hidden hold is ignored
  early.observe_life('dead','dead',true,true,true,.6,1001,1n);
  let mid=early.step(2000,960,540,80,false,false);
  assert.equal(mid.disabled,true);assert.equal(mid.play_disabled,false);
  early.action('enter',false,false);
  assert.equal(early.step(2016,960,540,80,false,false).busy,false);
  early.action('play',false,false);early.action('play',false,false);
  assert.equal(early.edit('Changed'),'Stored');
  mid=early.step(2032,960,540,80,false,false);
  assert.equal(mid.busy,true);assert.equal(mid.spinner_visible,true);assert.equal(mid.alpha,.4);
  assert.equal(Object.hasOwn(mid,'request'),false);
  assert.equal(early.observe_life('disconnected','dead',false,false,false,0,1001,1n),true);
  assert.equal(Object.hasOwn(early.step(3000,960,540,80,false,false),'request'),false);
  assert.equal(early.step(3016,960,540,80,true,false).request,'Stored');
  assert.equal(Object.hasOwn(early.step(3032,960,540,80,true,false),'request'),false);
  early.free();

  for (const outcome of ['dead','victory']) {
    const returning=new wasm.LoginMenu('en','1');
    returning.step(0,960,540,36,true,false);returning.action('play',false,true);
    returning.step(16,960,540,36,true,false);
    returning.observe_life('playing','',true,false,false,1,undefined,undefined);
    returning.observe_life(outcome,outcome,true,true,false,1,1001,1n);
    let view=returning.step(1616,960,540,80,false,false);
    assert.equal(view.last_number,'1001');assert.equal(view.last_suffix,'!');assert.equal(view.last_label_alpha,.45);
    assert.equal(view.alpha,0);assert.equal(view.disabled,true);
    assert.equal(view.ordinary_controls_visible,outcome==='dead');
    returning.action('play',false,true);returning.action('enter',false,true);
    returning.observe_life(outcome,outcome,true,true,true,.5,1001,1n);
    view=returning.step(2600,960,540,80,false,false);
    assert.equal(view.alpha,.5);assert.equal(view.world_alpha,.5);assert.equal(view.play_alpha,.25);
    assert.equal(Object.hasOwn(view,'request'),false);
    returning.observe_life('disconnected',outcome,false,false,false,0,1001,1n);
    view=returning.step(3600,960,540,80,false,false);
    assert.equal(view.focus,false);
    view=returning.step(3616,960,540,80,true,false);
    assert.equal(view.focus,outcome==='dead');assert.equal(view.disabled,outcome==='victory');
    assert.equal(view.champion_pending,outcome==='victory');
    if(outcome==='dead') {
      returning.action('play',false,true);view=returning.step(3632,960,540,80,true,false);
      assert.ok(Object.hasOwn(view,'request'));assert.equal(view.last_number,'1001');
    }
    returning.free();
  }
}
