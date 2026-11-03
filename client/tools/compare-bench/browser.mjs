import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import { spawn, execFileSync } from 'node:child_process';
import WebSocket from 'ws';
import { setTimeout as delay } from 'node:timers/promises';
import { configureNTL } from './ntl-ui-adapter.mjs';
import { resourceScope, listen, closeServer } from './resources.mjs';
import { installContentProbe } from './content-probe.mjs';

export class CDP {
  constructor(ws) {
    this.ws=ws;this.id=0;this.pending=new Map();this.events=[];this.eventLimit=4096;this.droppedEvents=0;
    ws.on('error',()=>{});
    ws.on('message',bytes=>{
      const m=JSON.parse(bytes);
      if(m.id){const p=this.pending.get(m.id);this.pending.delete(m.id);clearTimeout(p?.timer);m.error?p?.reject(Error(JSON.stringify(m.error))):p?.resolve(m.result);}
      else {this.onEvent?.(m);if(this.events.length<this.eventLimit)this.events.push(m);else this.droppedEvents++;}
    });
    ws.on('close',()=>{for(const p of this.pending.values()){clearTimeout(p.timer);p.reject(Error('CDP closed'));}this.pending.clear();});
  }
  static async open(url){const ws=new WebSocket(url);await new Promise((resolve,reject)=>{ws.once('open',resolve);ws.once('error',reject);});return new CDP(ws);}
  send(method,params={}){
    const id=++this.id;
    return new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{this.pending.delete(id);reject(Error(`CDP timeout: ${method}`));},method==='Page.captureScreenshot'?30000:10000);timer.unref();
      this.pending.set(id,{resolve,reject,timer});this.ws.send(JSON.stringify({id,method,params}));
    });
  }
  async evaluate(expression){return value(await this.send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true}));}
  async call(functionDeclaration,args=[]){return value(await this.send('Runtime.callFunctionOn',{functionDeclaration,arguments:args.map(value=>({value})),executionContextId:this.context,returnByValue:true,awaitPromise:true}));}
}
function value(r){if(r.exceptionDetails)throw Error(`Browser preflight: ${r.exceptionDetails.exception?.description??r.exceptionDetails.text}`);return r.result.value;}
export function adapterFlags(adapter){
  if(!adapter||adapter==='default')return [];
  const m=adapter.match(/^luid_0x([0-9a-f]{8})_0x([0-9a-f]{8})$/i);
  if(!m)throw Error('chromeLuid must be default or an exact DXGI luid_0xHHHHHHHH_0xLLLLLLLL');
  return ['--use-gl=angle','--use-angle=d3d11',`--use-adapter-luid=${parseInt(m[1],16)|0},${parseInt(m[2],16)>>>0}`];
}
export function loopbackEndpoint(endpoint){
  if(typeof endpoint!=='string'||!/^ws:\/\/127\.0\.0\.1:[1-9]\d*\/slither$/.test(endpoint))throw Error('Exact loopback WebSocket endpoint required');
  const port=+new URL(endpoint).port;if(port>65535||[8766,18884,18885].includes(port))throw Error('Forbidden game port');return endpoint;
}
// Original local DOM shell, no official page/source/assets.
const shell=`<!doctype html><title>Loopback frame pacing</title><body><div id="login"><div id="logo"></div><div id="nick_holder"><input id="nick" maxlength="24"></div><div id="lastscore"></div><div id="playh"></div><div id="tips"></div></div>${['smh','ocho','cstx','csk','cskh','etco','etcoh','plq','clq','etcod','nbg'].map(id=>`<div id="${id}"></div>`).join('')}<script>window.getData=(url,handler)=>fetch(url).then(r=>r.text()).then(t=>handler.f(t,'success')).catch(()=>handler.f('','error'));</script>`;
export const profilePreferences={webrtc:{ip_handling_policy:'disable_non_proxied_udp'},browser:{enabled_labs_experiments:['local-network-access-check-websockets@2','local-network-access-check@2']}};

export async function chromeSession({chrome,mod,directory,width,height,x,y,register,adapter,html=shell,productionProbe=true}){
  const adapterArgs=adapterFlags(adapter?.chromeLuid),scope=resourceScope(),blocked=[];
  let child,browser,page,chromeOutput='',closing;
  const close=()=>closing??=(async()=>{
    try{await Promise.all([
      ...(page?[fs.writeFile(path.join(directory,'cdp-events.json'),JSON.stringify({events:page.events,dropped:page.droppedEvents}))]:[]),
      fs.writeFile(path.join(directory,'denied-requests.json'),JSON.stringify(blocked)),fs.writeFile(path.join(directory,'chrome-console.txt'),chromeOutput)]);}
    finally{await scope.close();}
  })();
  try{
    const proxy=http.createServer((req,res)=>{blocked.push(req.url);res.writeHead(403);res.end('External network denied');});scope.add(()=>closeServer(proxy));
    proxy.on('connect',(req,socket)=>{blocked.push(req.url);socket.end('HTTP/1.1 403 Forbidden\r\n\r\n');});
    proxy.on('connection',socket=>socket.on('error',error=>blocked.push(`socket-error:${error.code}`)));
    proxy.on('clientError',(_e,socket)=>socket.destroy());
    const server=http.createServer((_req,res)=>{res.writeHead(200,{'Content-Type':'text/html'});res.end(html);});scope.add(()=>closeServer(server));
    server.on('connection',socket=>socket.on('error',()=>{}));server.on('clientError',(_e,socket)=>socket.destroy());
    await listen(proxy);await listen(server);
    const profile=path.join(directory,'chrome-profile');await fs.mkdir(path.join(profile,'Default'),{recursive:true});
    await fs.writeFile(path.join(profile,'Default','Preferences'),JSON.stringify(profilePreferences));
    child=spawn(chrome,[...adapterArgs,`--user-data-dir=${profile}`,'--remote-debugging-port=0','--remote-debugging-address=127.0.0.1',
      ...(mod?[`--load-extension=${mod}`,`--disable-extensions-except=${mod}`]:[]),
      '--host-resolver-rules=MAP slither.io 127.0.0.1, MAP slither.com 127.0.0.1, MAP * ~NOTFOUND, EXCLUDE 127.0.0.1, EXCLUDE localhost',
      `--proxy-server=http://127.0.0.1:${proxy.address().port}`,'--proxy-bypass-list=127.0.0.1;localhost;slither.io;slither.com',
      '--force-webrtc-ip-handling-policy=disable_non_proxied_udp','--disable-quic','--disable-background-networking','--no-first-run','--no-default-browser-check',
      '--disable-features=LocalNetworkAccessChecks,LocalNetworkAccessChecksWebSockets',`--log-net-log=${path.join(directory,'netlog.json')}`,'--net-log-capture-mode=Default',
      // Suppress startup infobars in this throwaway GPU-test profile; otherwise
      // they shrink the web viewport even in fullscreen. Size is still verified.
      // chromium/chrome/browser/ui/startup/infobar_utils.cc (IsGpuTest).
      '--test-type=gpu',`--window-size=${width},${height}`,`--window-position=${x},${y}`,'--app=about:blank'],{stdio:['ignore','pipe','pipe'],windowsHide:false});
    register(child);scope.add(async()=>{
      try{
        try{await browser?.send('Browser.close');}catch{}
        if(child.exitCode===null)await Promise.race([new Promise(resolve=>child.once('close',resolve)),delay(1500)]);
        if(child.exitCode===null&&child.pid)try{execFileSync('taskkill',['/PID',String(child.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});}catch{}
      }finally{page?.ws.close();browser?.ws.close();}
    });
    child.stdout.on('data',b=>chromeOutput+=b);child.stderr.on('data',b=>chromeOutput+=b);
    let port;for(let i=0;i<100;i++){try{port=+(await fs.readFile(path.join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0];if(port)break;}catch{}if(child.exitCode!==null)throw Error(`Chrome exited ${child.exitCode}`);await delay(100);}
    if(!port)throw Error('Dedicated Chrome CDP startup failed');
    const version=await(await fetch(`http://127.0.0.1:${port}/json/version`)).json();
    browser=await CDP.open(version.webSocketDebuggerUrl);
    const targets=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    const target=targets.find(t=>t.type==='page');
    page=await CDP.open(target.webSocketDebuggerUrl);
    const gpu=await browser.send('SystemInfo.getInfo'),processes=await browser.send('SystemInfo.getProcessInfo');
    const gpuPid=processes.processInfo.find(p=>p.type==='GPU')?.id;
    await fs.writeFile(path.join(directory,'browser-gpu.json'),JSON.stringify({gpu,processes,gpuPid,adapterRequest:adapter??null},null,2));
    page.onEvent=m=>{if(m.method==='Runtime.executionContextCreated'&&m.params.context.auxData?.isDefault)page.context=m.params.context.id;};
    await page.send('Page.enable');await page.send('Runtime.enable');
    if(productionProbe)await page.send('Page.addScriptToEvaluateOnNewDocument',{source:`(${installContentProbe.toString()})();`});
    await page.send('Page.navigate',{url:`http://slither.io:${server.address().port}/`});
    return {child,page,browser,close,gpuPid,processIds:processes.processInfo.map(p=>p.id),chromeVersion:version.Browser,
      adapter:JSON.stringify({devices:gpu.gpu.devices,renderer:gpu.gpu.auxAttributes?.glRenderer}),
      async productionStart(){return page.evaluate('__frameBenchProduction.start()');},
      async productionStop(){return page.evaluate('__frameBenchProduction.stop()');},
      async capture(label){const{data}=await page.send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});const file=path.join(directory,`${label}.png`);await fs.writeFile(file,Buffer.from(data,'base64'));return file;},
      async fullscreen(){
        const{windowId}=await browser.send('Browser.getWindowForTarget',{targetId:target.id});
        // Let Chrome manage its frame insets; editing HWND styles can leave its
        // web viewport shorter than the nominal physical client rectangle.
        await browser.send('Browser.setWindowBounds',{windowId,bounds:{windowState:'normal'}});
        await browser.send('Browser.setWindowBounds',{windowId,bounds:{left:x??0,top:y??0,width,height}});
        value(await page.send('Runtime.evaluate',{expression:'document.documentElement.requestFullscreen({navigationUI:"hide"}).then(()=>true)',userGesture:true,awaitPromise:true,returnByValue:true}));
        let viewport;
        for(let i=0;i<50;i++){
          viewport=await page.evaluate('({width:innerWidth,height:innerHeight,devicePixelRatio,fullscreen:Boolean(document.fullscreenElement)})');
          if(Math.round(viewport.width*viewport.devicePixelRatio)===width&&Math.round(viewport.height*viewport.devicePixelRatio)===height)break;
          await delay(100);
        }
        return {...await browser.send('Browser.getWindowBounds',{windowId}),viewport};
      },
      async zoomOut(notches=80){const view=await page.evaluate('({width:innerWidth,height:innerHeight})');for(let i=0;i<notches;i++){await page.send('Input.dispatchMouseEvent',{type:'mouseWheel',x:view.width/2,y:view.height/2,deltaX:0,deltaY:120});await delay(20);}await delay(1000);},
    };
  }catch(error){try{await close();}catch(cleanup){error.cleanup=cleanup.message;}throw error;}
}
export async function ntl(options){
  const endpoint=loopbackEndpoint(options.endpoint);let session;
  try{
    session=await chromeSession(options);const{page}=session;
    let ready=false;for(let i=0;i<150;i++){ready=await page.evaluate(`Boolean(document.querySelector('#ip-server')&&document.querySelector('#connect-btn')&&document.querySelector('#nick'))`);if(ready)break;await delay(100);}
    if(!ready)throw Error('NTL local UI bootstrap changed: #nick, #ip-server and #connect-btn required');
    const settings=await configureNTL(page,{wholeMap:options.scenario==='battledome'});await fs.writeFile(path.join(options.directory,'ntl-settings.json'),JSON.stringify(settings,null,2));
    let setup=false;page.onEvent=((previous)=>m=>{previous?.(m);if(m.method==='Network.webSocketFrameReceived'&&m.params.response.opcode===2&&Buffer.from(m.params.response.payloadData,'base64')[2]===97)setup=true;})(page.onEvent);
    await page.send('Network.enable');
    // Structured arguments keep endpoint/package data out of JavaScript source.
    await page.call(`function(address){const set=(id,v)=>{const e=document.querySelector(id);e.value=v;e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));};set('#nick','FrameBench');set('#ip-server',address);document.querySelector('#connect-btn').click();}`, [new URL(endpoint).host]);
    for(let i=0;i<600&&!setup;i++)await delay(100);
    await page.send('Network.disable');
    if(!setup)throw Error('NTL protocol-13 Setup was not received from loopback');
    return session;
  }catch(error){try{await session?.close();}catch(cleanup){error.cleanup=cleanup.message;}throw error;}
}
