// Full product pages, real Wasm and GPU, injected sockets only. Finite headless run.
import path from 'node:path';
import { readFile,mkdir,writeFile,cp } from 'node:fs/promises';
import { createSceneServer } from './scene-server.mjs';
import { createServer } from '../apps/wgpu-slice/dev-server.mjs';
import { launchBrowser } from './feel-replay/browser.mjs';

const [output,artArgument,nativeMetrics]=process.argv.slice(2);
const artDir=artArgument==='-'?undefined:artArgument;
const nativeText=nativeMetrics?JSON.parse(await readFile(nativeMetrics,'utf8')):[];
if(!output)throw new Error('node tools/login-evidence.mjs NEW_OUTPUT [PRIVATE_ART_DIR]');
await mkdir(output);
await cp('apps/wgpu-slice/web','dist/wgpu-slice',{recursive:true});
const executable='C:/Program Files/Google/Chrome/Application/chrome.exe';
const servers=[createSceneServer({simulatorPort:19099,cosmeticsDir:artDir}),createServer({webDir:path.resolve('dist/wgpu-slice'),simulatorPort:19099,cosmeticsDir:artDir})];
// Passive failure receipt: the production runtime intentionally consumes terminal
// exceptions after retiring its owners. Retain the cause in this private run.
const canvasRequest=servers[0].listeners('request')[0];servers[0].removeAllListeners('request');
servers[0].on('request',async(req,res)=>{
  if(req.url==='/game/runtime.mjs'){
    const source=await readFile('web/game-runtime.mjs','utf8');res.setHeader('Content-Type','text/javascript');
    res.end(source.replace('const fail = error => {','const fail = error => { globalThis.loginFailure=error?.stack??String(error);'));
  }else canvasRequest(req,res);
});
const browser=await launchBrowser(executable);const results=[];
try {
  for(const [index,server] of servers.entries()) {
    await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
    const origin=`http://127.0.0.1:${server.address().port}`,name=index?'webgpu':'canvas';
    const {targetId}=await browser.call('Target.createTarget',{url:'about:blank',background:true});
    const {sessionId}=await browser.call('Target.attachToTarget',{targetId,flatten:true});
    const send=(method,params={})=>browser.call(method,params,sessionId);
    await send('Page.enable');await send('Runtime.enable');
    await send('Network.enable');
    // Local HTTP is explicitly unblocked below. There is no native game socket.
    await send('Network.setBlockedURLs',{urlPatterns:[{urlPattern:`${origin}/*`,block:false},
      ...['http','https','ws','wss'].map(protocol=>({urlPattern:`${protocol}://*/*`,block:true}))]});
    await send('Page.addScriptToEvaluateOnNewDocument',{source:`
      let clock=0,serial=0;const frames=new Map();
      Object.defineProperty(performance,'now',{value:()=>clock});
      globalThis.requestAnimationFrame=callback=>{frames.set(++serial,callback);return serial;};
      globalThis.cancelAnimationFrame=id=>frames.delete(id);
      globalThis.advanceLogin=async(target)=>{while(clock<target){clock=Math.min(target,clock+16);const work=[...frames.values()];frames.clear();for(const callback of work)callback(clock);await Promise.resolve();}return clock;};
      globalThis.socketEffects=[];globalThis.sourceEvents=[];globalThis.testSockets=[];
      globalThis.WebSocket=class {static OPEN=1;readyState=0;bufferedAmount=0;
        constructor(url){socketEffects.push({kind:'connect',url,at:performance.now()});if(globalThis.failNextSocket){globalThis.failNextSocket=false;throw Error('Synthetic constructor failure');}testSockets.push(this);}
        send(bytes){socketEffects.push({kind:'send',bytes:[...bytes],at:performance.now()});}
        close(){this.readyState=3;socketEffects.push({kind:'close',at:performance.now()});}
      };
      for(const type of ['focusin','focusout','keydown','keyup','pointerdown','pointerup','click','input','compositionstart','compositionend'])
        addEventListener(type,e=>sourceEvents.push({type,role:e.target?.dataset?.role,key:e.key,value:e.target?.value,at:performance.now()}),true);
    `});
    await send('Emulation.setDeviceMetricsOverride',{width:960,height:540,deviceScaleFactor:1,mobile:false});
    await send('Emulation.setFocusEmulationEnabled',{enabled:true});
    await browser.call('Target.activateTarget',{targetId});
    const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw new Error(r.exceptionDetails.exception?.description??r.exceptionDetails.text);return r.result.value;};
    await send('Page.navigate',{url:origin+'/'});
    await evaluate(index ? `import('/local.mjs')` : `import('/game/bootstrap.mjs').then(m=>m.gamePageRuntime)`);
    await evaluate(`new Promise((resolve,reject)=>{let n=0;const timer=setInterval(()=>{if(document.querySelector('[data-login]')){clearInterval(timer);resolve(true);}else if(++n>1500||globalThis.loginFailure){clearInterval(timer);reject(Error(globalThis.loginFailure??document.body.textContent));}},10);})`);
    if(artDir) {
      await evaluate('advanceLogin(16)');
      await evaluate(`(async()=>{for(let n=0;n<500;n++){await advanceLogin(performance.now()+16);if(document.querySelectorAll('[data-login] img').length===3)return;await new Promise(resolve=>setTimeout(resolve,10));}throw Error('Private art did not settle');})()`);
    }
    await evaluate('advanceLogin(9600)');
    const checkpoints=[];
    for(const [width,height] of [[960,540],[1280,720],[640,360]]) {
      await send('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
      await evaluate('advanceLogin(performance.now()+16)');
      const measurements=await evaluate(`(async()=>{
        const wasm=await import('${index?'/pkg/slither_wgpu_slice.js':'/game/pkg/slither_client.js'}');
        const policy=new wasm.LoginMenu('en','1'), c=document.createElement('canvas').getContext('2d');c.font='bold 20px Arial';
        let view;for(let now=16;now<=performance.now();now+=16)view=policy.step(now,innerWidth,innerHeight,c.measureText('Play').width,true,false);policy.free();
        const roles={logo:'logo','nickname-holder':'holder',nickname:'input',play:'play',skin:'skin',quality:'quality_rect',spinner:'spinner'};
        if(Math.abs(Number(document.querySelector('[data-role="tips"]').style.opacity)-view.tip_alpha)>.02)throw Error('Tip phase budget');
        return Object.entries(roles).map(([role,key])=>{const n=document.querySelector('[data-role="'+role+'"]'),r=n.getBoundingClientRect(),expected=view[key];return {role,actual:[r.x,r.y,r.width,r.height],expected,error:Math.max(...[r.x,r.y,r.width,r.height].map((n,i)=>Math.abs(n-expected[i])))};});
      })()`);
      if(measurements.some(m=>m.role!=='spinner'&&m.error>2))throw new Error(`${name} rectangle budget ${JSON.stringify(measurements)}`);
      const {data}=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      await writeFile(path.join(output,`${name}-${width}x${height}.png`),Buffer.from(data,'base64'));
      checkpoints.push({width,height,measurements});
      console.log(`${name} ${width}x${height}`);
    }
    await send('Emulation.setDeviceMetricsOverride',{width:960,height:540,deviceScaleFactor:1,mobile:false});
    await evaluate('advanceLogin(performance.now()+16)');
    const before=await evaluate('socketEffects.length');if(before!==0)throw new Error('Implicit connection');
    const dispatchKey=async(key,code=key)=>{await send('Input.dispatchKeyEvent',{type:'keyDown',key,code,windowsVirtualKeyCode:key==='Tab'?9:key==='Escape'?27:13});await send('Input.dispatchKeyEvent',{type:'keyUp',key,code,windowsVirtualKeyCode:key==='Tab'?9:key==='Escape'?27:13});};
    await dispatchKey('Tab');await dispatchKey('Tab');
    const tab=await evaluate(`document.activeElement.dataset.role`);if(tab!=='quality')throw new Error(`${name} Tab route ${tab}`);
    await evaluate(`document.querySelector('[data-role="nickname"]').focus()`);
    await send('Input.insertText',{text:'A😀\u007f'});
    const normalized=await evaluate(`document.querySelector('[data-role="nickname"]').value`);
    if(normalized!=='A  \u007f')throw new Error(`${name} normalization ${normalized}`);
    await send('Input.imeSetComposition',{text:'日',selectionStart:1,selectionEnd:1});
    await send('Input.insertText',{text:'日'});
    const rect=await evaluate(`(()=>{const r=document.querySelector('[data-role="play"]').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
    for(let i=0;i<2;i++){await send('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...rect});await send('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...rect});}
    await evaluate('advanceLogin(performance.now()+304)');
    const trace=await evaluate('({events:sourceEvents,effects:socketEffects,failure:globalThis.loginFailure})');
    await writeFile(path.join(output,`${name}-trace.json`),JSON.stringify(trace,null,2));
    console.log(`${name}: ${trace.events.length} source events, ${trace.effects.length} explicit transport effects`);
    if(trace.effects.filter(e=>e.kind==='connect').length!==1)throw new Error(`${name} admission ${JSON.stringify(trace)}`);
    const {data}=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});await writeFile(path.join(output,`${name}-spinner.png`),Buffer.from(data,'base64'));
    const recovery=[];
    const clickPlay=async()=>{for(const type of ['mousePressed','mouseReleased'])await send('Input.dispatchMouseEvent',{type,button:'left',clickCount:1,...rect});await evaluate('advanceLogin(performance.now()+32)');};
    const returned=async kind=>{
      await evaluate('advanceLogin(performance.now()+32)');
      const state=await evaluate(`({disabled:document.querySelector('[data-role="play"]')?.disabled??'missing',spinner:document.querySelector('[data-role="spinner"]')?.style.display,effects:socketEffects.length,failure:globalThis.loginFailure})`);
      if(state.disabled||state.spinner!=='none'||state.failure)throw Error(`${name} ${kind} recovery ${JSON.stringify(state)}`);
      await evaluate('advanceLogin(performance.now()+64)');
      if(await evaluate('socketEffects.length')!==state.effects)throw Error('Automatic reconnect');
      recovery.push({kind,...state});console.log(`${name}: ${kind} returned`);
    };
    for(const kind of ['close','cancel','error','construction']) {
      if(kind==='cancel')await dispatchKey('Escape');
      else await evaluate(`(()=>{const socket=testSockets.at(-1);socket.readyState=3;socket.${kind==='error'?'onerror':'onclose'}?.();})()`);
      await returned(kind);
      if(kind==='construction'){
        await evaluate('globalThis.failNextSocket=true');await clickPlay();await returned('constructor-failed');
      }
      const count=await evaluate(`socketEffects.filter(e=>e.kind==='connect').length`);
      await clickPlay();
      if(await evaluate(`socketEffects.filter(e=>e.kind==='connect').length`)!==count+1)throw Error(`${name} explicit retry failed`);
    }
    await writeFile(path.join(output,`${name}-recovery.json`),JSON.stringify({recovery,trace:await evaluate('socketEffects')},null,2));
    const measured=await send('Runtime.callFunctionOn',{
      functionDeclaration:"function(metrics){const c=document.createElement('canvas').getContext('2d');return metrics.map(m=>{c.font=(m.bold?'bold ':'')+m.size+'px Arial';const advance=c.measureText(m.text).width;return {...m,browserAdvance:advance,relativeError:Math.abs(advance-m.advance)/m.advance};});}",
      objectId:(await send('Runtime.evaluate',{expression:'globalThis'})).result.objectId,
      arguments:[{value:nativeText}],returnByValue:true,
    });
    if(measured.exceptionDetails)throw new Error(measured.exceptionDetails.text);
    const textWidths=measured.result.value;
    if(textWidths.some(m=>m.relativeError>.15))throw new Error('Text width budget');
    results.push({name,checkpoints,tab,normalized,trace,recovery,textWidths});
    await browser.call('Target.closeTarget',{targetId});
  }
  await writeFile(path.join(output,'browser.json'),JSON.stringify({executable,art:artDir?'private local pack':'authored fallback',results},null,2)+'\n');
} finally {
  await browser.close();
  for(const server of servers)if(server.listening)await new Promise(resolve=>server.close(resolve));
}
