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
  if(wasm.CosmeticImages) assert.deepEqual([...wasm.CosmeticImages.paths()].sort(),[...COSMETIC_PATHS].sort());
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
}
