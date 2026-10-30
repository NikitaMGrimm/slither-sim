// Authored local files and decoder doubles only; no browser or external artwork.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { menuPaths } from '../menu-art.mjs';
import { createServer } from '../dev-server.mjs';
import { loadCosmetics } from '../web/cosmetics.mjs';

test('menu manifest admits semantic revisions and rejects malformed or escaping entries', () => {
  const bytes = s => new TextEncoder().encode(s);
  assert.deepEqual(menuPaths(bytes('menu-art-v1\nlogo:0 2 rasters/logo.png\nskin 1 menu/skin.png\n')), ['rasters/logo.png', 'menu/skin.png']);
  for (const line of ['skin 0 menu/a.png','logo:32 1 menu/a.png','skin 1 ../a.png','skin 1 /a.png',
    'skin 1 C:/a.png','skin 1 https://a/a.png','skin 1 menu\\a.png','skin 1 menu/./a.png',
    'skin 1 menu/a.png\nskin 2 menu/b.png','skin 1 menu/a.png\nbuilder 1 menu/a.png']) {
    assert.throws(() => menuPaths(bytes(`menu-art-v1\n${line}`)));
  }
  assert.throws(() => menuPaths(new Uint8Array(4097)));
});
test('served art is limited to manifest paths, defaults and contained real paths', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'menu-art-'));
  const server = createServer({ cosmeticsDir: path.join(root,'pack') });
  try {
    await mkdir(path.join(root,'pack/menu'),{recursive:true});
    await mkdir(path.join(root,'pack/rasters'));
    await mkdir(path.join(root,'outside'));
    await writeFile(path.join(root,'outside/logo.png'),'private synthetic');
    await symlink(path.join(root,'outside'),path.join(root,'pack/escape'),process.platform === 'win32' ? 'junction' : 'dir');
    await writeFile(path.join(root,'pack/menu/manifest.txt'),'menu-art-v1\nlogo:0 1 rasters/logo.png\nskin 2 escape/logo.png\n');
    await writeFile(path.join(root,'pack/rasters/logo.png'),'synthetic');
    await writeFile(path.join(root,'pack/rasters/unlisted.png'),'unlisted');
    await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
    const base=`http://127.0.0.1:${server.address().port}/cosmetics/`;
    assert.equal((await fetch(`${base}menu/manifest.txt`)).status,200);
    assert.equal(await (await fetch(`${base}rasters/logo.png`)).text(),'synthetic');
    for (const p of ['rasters/unlisted.png','escape/logo.png','menu/missing.png']) assert.equal((await fetch(base+p)).status,404);
    await writeFile(path.join(root,'pack/menu/manifest.txt'),'x'.repeat(4097));
    assert.equal((await fetch(`${base}menu/manifest.txt`)).status,404);
    assert.equal((await fetch(`${base}rasters/logo.png`)).status,404);
  } finally {
    await new Promise(resolve => server.close(resolve));
    await rm(root,{recursive:true,force:true});
  }
});
test('browser resolves manifest-selected local bytes before publishing one complete batch', async () => {
  const attempted=[], warnings=[]; let published;
  class Images {
    sources=[];
    static paths() { return ['s/a_cap.png']; }
    manifest(bytes) { this.paths=menuPaths(bytes); }
    menu_paths() { return this.paths ?? ['menu/logo.png']; }
    add(p) { this.sources.push(p); }
    free() {}
  }
  await loadCosmetics({owner:{cosmetics(images){published=images.sources;}},CosmeticImages:Images,
    baseUrl:'http://127.0.0.1/cosmetics/',warn:m=>warnings.push(m),fetch:async url=>{
      attempted.push(url.pathname);
      return new Response(url.pathname.endsWith('manifest.txt') ? 'menu-art-v1\nlogo:0 7 rasters/static.png\n' : 'synthetic');
    }});
  assert.deepEqual(published,['s/a_cap.png','rasters/static.png']);
  assert.equal(attempted.length,3); assert.equal(warnings.length,0);
});

test('browser rejects colliding menu paths before manifest publication and preserves cosmetics', async () => {
  for (const row of ['skin 1 s/a_cap.png', 'logo:0\t1\ts/a_cap.png', 'cosmetic\f1\fs/jmou3.png']) {
    const paths=['s/a_cap.png','s/jmou3.png'];
    const warnings=[]; let manifestCalls=0, published;
    class Images {
      sources=[];
      static paths() { return paths; }
      manifest() { manifestCalls++; throw new Error('Invalid manifest reached decoder'); }
      menu_paths() { return []; }
      add(p) { this.sources.push(p); }
      free() {}
    }
    await loadCosmetics({owner:{cosmetics(images,accessory){published=images.sources; assert.equal(accessory,30);}},
      CosmeticImages:Images,accessory:30,baseUrl:'http://127.0.0.1/cosmetics/',warn:m=>warnings.push(m),
      fetch:async url=>new Response(url.pathname.endsWith('manifest.txt') ? `menu-art-v1\n${row}\n` : 'synthetic')});
    assert.equal(manifestCalls,0);
    assert.deepEqual(published,paths);
    assert.equal(warnings.length,1);
    assert.match(warnings[0],/Menu manifest path collides with cosmetic/);
  }
});
