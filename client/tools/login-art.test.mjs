import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loginArt,menuImageSize } from '../apps/wgpu-slice/web/login-art.mjs';
const png=(width,height)=>{const b=new Uint8Array(24),v=new DataView(b.buffer);v.setUint32(0,0x89504e47);v.setUint32(4,0x0d0a1a0a);v.setUint32(12,0x49484452);v.setUint32(16,width);v.setUint32(20,height);return b;};
test('menu art dimensions are bounded before decode, including JPEG SOF',()=>{
  assert.deepEqual(menuImageSize(png(450,115)),[450,115]);
  assert.deepEqual(menuImageSize(Uint8Array.of(255,216,255,192,0,7,8,0,89,0,95)),[95,89]);
  for(const bytes of [png(0,1),png(1025,1),new Uint8Array(24),Uint8Array.of(255,216,255,192,0,0)])assert.throws(()=>menuImageSize(bytes));
});
test('DOM art shares a remaining pixel allowance and stale reads cannot publish',async()=>{
  for(const stale of [false,true]){
    const calls=[];let active=true;
    const realm={location:new URL('http://127.0.0.1:19090/'),Blob,
      URL:{createObjectURL(){calls.push('allocate');return 'blob:synthetic';},revokeObjectURL(){calls.push('retire');}},
      Image:class{naturalWidth=10;naturalHeight=10;async decode(){calls.push('decode');}},
      async fetch(url){if(url.pathname.endsWith('.txt'))return new Response('menu-art-v1\nlogo:0 1 menu/logo.png\nskin 1 menu/skin.png\n');if(stale)active=false;return new Response(png(10,10));}};
    const lease=loginArt({realm,current:()=>active,pixelBudget:400});await lease.ready;
    assert.equal(Object.keys(lease.art).length,stale?0:1);lease.dispose();
    assert.deepEqual(calls,stale?[]:['allocate','decode','retire']);
  }
});

for(const manifest of ['menu-art-v1\nquality-high 1 s/a_cap.png','\ufeffmenu-art-v1\nquality-high 1 custom/high.png']) {
  test(`DOM loader falls back for reserved paths or BOM: ${JSON.stringify(manifest)}`,async()=>{
    const reads=[];
    const realm={location:new URL('http://127.0.0.1:19090/'),Blob,
      URL:{createObjectURL:()=> 'blob:synthetic',revokeObjectURL(){}},
      Image:class{naturalWidth=10;naturalHeight=10;async decode(){}},
      async fetch(url){reads.push(url.pathname);return new Response(url.pathname.endsWith('.txt')?manifest:png(10,10));}};
    const lease=loginArt({realm,current:()=>true});await lease.ready;
    assert.ok(!reads.includes('/cosmetics/s/a_cap.png'));
    assert.ok(!reads.includes('/cosmetics/custom/high.png'));
    assert.ok(reads.includes('/cosmetics/menu/quality-high.png'));
    lease.dispose();
  });
}
