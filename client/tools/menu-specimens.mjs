// Explicit finite WebGPU specimens with the existing isolated headless launcher.
// No game socket, remote art, visible window or persistent service.
import http from 'node:http';
import path from 'node:path';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { launchBrowser } from './feel-replay/browser.mjs';

const [executable, output, pkg = 'dist/wgpu-slice/pkg'] = process.argv.slice(2);
if (!executable || !output) throw new Error('Usage: node tools/menu-specimens.mjs <headless-browser> <new-output-dir> [pkg-dir]');
await mkdir(output);
const entry = `import init, {menu_specimen} from '/pkg/slither_wgpu_slice.js';
window.replayReady=init();
window.renderMenu=async(dpr,state,width=1280,height=720)=>{
  await window.replayReady;
  const canvas=document.querySelector('canvas');
  if (canvas.width!==width*dpr) canvas.width=width*dpr;
  if (canvas.height!==height*dpr) canvas.height=height*dpr;
  canvas.style.width=width+'px'; canvas.style.height=height+'px';
  if (!window.specimen) window.specimen=await menu_specimen(canvas,dpr,state);
  return new Promise((resolve,reject)=>requestAnimationFrame(()=>{
    try {
      window.specimen.render(dpr,state);
      const sample=new OffscreenCanvas(128,72), ctx=sample.getContext('2d');
      ctx.drawImage(canvas,0,0,128,72);
      const pixels=ctx.getImageData(0,0,128,72).data;
      let coverage=0;
      for(let i=4;i<pixels.length;i+=4) if(pixels[i]!==pixels[0] || pixels[i+1]!==pixels[1] || pixels[i+2]!==pixels[2]) coverage++;
      if(coverage<32) throw new Error('Blank WebGPU specimen');
      resolve({data:canvas.toDataURL('image/png').split(',')[1],coverage});
    }
    catch(e) { reject(e); }
  }));
};`;
const server = http.createServer(async (req, res) => {
  res.setHeader('Content-Security-Policy', "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' data:");
  res.setHeader('Cache-Control','no-store');
  try {
    const url=new URL(req.url,'http://127.0.0.1');
    if (url.pathname==='/') { res.setHeader('Content-Type','text/html'); res.end('<!doctype html><style>html,body{margin:0;background:#090b0e}canvas{display:block}</style><canvas></canvas><script type="module" src="/entry.mjs"></script>'); }
    else if (url.pathname==='/entry.mjs') { res.setHeader('Content-Type','text/javascript'); res.end(entry); }
    else if (/^\/pkg\/(slither_wgpu_slice(?:_bg\.wasm|\.js)|snippets\/[a-zA-Z0-9_./-]+\.mjs)$/.test(url.pathname) && !url.pathname.split('/').includes('..')) {
      const file=path.join(pkg,url.pathname.slice('/pkg/'.length));
      res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':'text/javascript'); res.end(await readFile(file));
    } else { res.writeHead(404); res.end(); }
  } catch { res.writeHead(404); res.end(); }
});
let browser, page;
try {
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  browser=await launchBrowser(executable);
  const url=`http://127.0.0.1:${server.address().port}/`;
  page=await browser.page(url,{width:1280,height:720,dpr:1},{});
  const {targetInfos}=await browser.call('Target.getTargets');
  const target=targetInfos.find(t=>t.url===url);
  const {sessionId}=await browser.call('Target.attachToTarget',{targetId:target.targetId,flatten:true});
  const results=[];
  for (const dpr of [1,1.25,2,3]) {
    await browser.call('Emulation.setDeviceMetricsOverride',{width:1280,height:720,deviceScaleFactor:dpr,mobile:false},sessionId);
    for (const [state,name] of ['normal','hover','pressed','disabled','spinner'].entries()) {
      const {data,coverage}=await page.evaluate(`window.renderMenu(${dpr},${state})`);
      const file=path.join(output,`webgpu-${name}-dpr-${dpr}.png`);
      await writeFile(file,Buffer.from(data,'base64')); results.push({file,coverage}); console.log(file);
    }
  }
  await browser.call('Emulation.setDeviceMetricsOverride',{width:960,height:540,deviceScaleFactor:1,mobile:false},sessionId);
  const {data,coverage}=await page.evaluate('window.renderMenu(1,0,960,540)');
  await writeFile(path.join(output,'webgpu-resize-missing-art.png'),Buffer.from(data,'base64'));
  results.push({file:path.join(output,'webgpu-resize-missing-art.png'),coverage});
  await writeFile(path.join(output,'result.json'),JSON.stringify({schema:1,kind:'authored menu WebGPU specimens',browser:executable,images:results,deniedRequests:page.denied()},null,2)+'\n');
} finally {
  await page?.close(); await browser?.close();
  await new Promise(resolve=>server.close(resolve));
}
