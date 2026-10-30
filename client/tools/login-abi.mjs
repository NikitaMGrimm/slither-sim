import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import path from 'node:path';

// Actual generated classes, no socket/DOM/GPU or policy double.
for(const base of ['dist/web/slither_client','dist/wgpu-slice/pkg/slither_wgpu_slice']) {
  const wasm=await import(pathToFileURL(path.resolve(`${base}.js`)));
  wasm.initSync({module:await readFile(`${base}_bg.wasm`)});
  assert.equal(typeof wasm.LoginMenu,'function',base);
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
