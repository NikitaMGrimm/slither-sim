import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawn,execFileSync} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {ntl} from './browser.mjs';
import {pngEvidence} from './picture.mjs';
import {zoomPlateau} from './matrix.mjs';
import {endpointFrom,completeJsonLines,windows} from './run.mjs';
import {netlogProof,windowReasons,anchoredBoundary} from './analysis.mjs';

// Per-session preflight, before any timed round. Updated packages never reuse
// another package's zoom estimate or network receipt.
export async function calibrateNtl(config,session,provenance) {
  const directory=path.join(session,'ntl-preflight');await fs.mkdir(directory);
  let browser,hostLog='',endpoint,aborted=false;const children=[];
  const report={purpose:'untimed NTL isolation and wheel-range preflight',modTreeSha256:provenance.tree.sha256,samples:[]};
  const register=child=>{children.push(child);child.on('error',()=>{});};
  const cancel=()=>{aborted=true;browser?.close().catch(()=>{});};
  process.once('SIGINT',cancel);process.once('SIGTERM',cancel);
  try{
    const idle=(await windows({mode:'preflight'},directory,'preflight',children))[0];
    if(idle.idleMs<5000)throw Error('Owner input; NTL calibration deferred');
    const checkOwner=async label=>{
      const current=(await windows({mode:'preflight'},directory,label,children))[0];
      if(aborted||current.idleMs<5000||current.lastInputTick!==idle.lastInputTick)throw Error('Owner input or interrupt during NTL calibration; deferred');
    };
    const host=spawn(config.host,['host','--bind','127.0.0.1:0','--config',fileURLToPath(new URL('./scenarios/battledome.json',import.meta.url)),'--stats','0.5','--frame-bench'],{windowsHide:true});register(host);
    host.stdout.on('data',b=>hostLog+=b);host.stderr.on('data',b=>hostLog+=b);
    for(let i=0;i<600&&!aborted;i++){endpoint=endpointFrom(hostLog);if(endpoint)break;if(host.exitCode!==null)throw Error('Calibration host exited');await delay(100);}
    if(!endpoint||aborted)throw Error('Calibration interrupted or host readiness timed out');
    browser=await ntl({...config,directory,endpoint,scenario:'battledome',productionProbe:false,register});
    report.settings=browser.ntlSettings;
    await checkOwner('before-fullscreen');
    if(config.width===3840&&config.height===2160)await browser.fullscreen();
    else await windows({mode:'setup',clientPid:browser.child.pid,client:'browser',borderless:true,...Object.fromEntries(['width','height','x','y'].map(k=>[k,config[k]]))},directory,'setup',children);
    for(let i=0;i<600&&!aborted;i++){
      const census=completeJsonLines(hostLog,'frame-bench: ').at(-1);
      if(census?.admittedBotSlots===100&&census.players?.[0]?.alive&&census.players[0].setup){report.worldReady=census;break;}
      if(host.exitCode!==null)throw Error('Calibration host exited before full population');await delay(100);
    }
    if(!report.worldReady)throw Error('Calibration full-population/living-player readiness timed out');
    for(const notches of [80,320,640]){
      await checkOwner(`before-wheel-${notches}`);
      await browser.zoomOut(notches);
      const image=await browser.capture(`wheel-${notches}`),picture=pngEvidence(await fs.readFile(image));
      const census=completeJsonLines(hostLog,'frame-bench: ').at(-1),anchor=anchoredBoundary(picture,census);
      report.samples.push({notches,image:path.basename(image),picture,census,anchor});
    }
    const anchors=report.samples.map(s=>s.anchor);
    report.plateau=anchors.every(Boolean)&&zoomPlateau(anchors);
    if(!report.plateau)throw Error('NTL farthest view is not established by three anchored wheel-range pictures');
    if(anchors.some(a=>a.wholeMap!==anchors[0].wholeMap))throw Error('NTL calibrated extent changes between whole-map and limited view; row label is unestablished');
    Object.assign(report,anchors.at(-1),{width:config.width,height:config.height,totalWheelNotches:1040});
    const processes=await browser.browser.send('SystemInfo.getProcessInfo'),gpuPid=processes.processInfo.find(p=>p.type==='GPU')?.id;
    const snapshots=await windows({mode:'sample',clientPid:browser.child.pid,clientPids:processes.processInfo.map(p=>p.id),gpuPid,seconds:.3},directory,'display-snapshot',children);
    const first=snapshots[0],last=snapshots.at(-1);
    const reasons=windowReasons({windowSamples:snapshots,startMs:first?.qpcMs,endMs:(last?.qpcMs??NaN)+1,
      width:config.width,height:config.height,display:config.targetDisplay??first?.display,dpi:first?.dpi,refreshHz:config.targetRefreshHz??first?.refreshHz});
    const luids=[...new Set(snapshots.flatMap(s=>s.gpuEngines??[]).filter(e=>/engtype_3d/i.test(e.instance)&&e.utilization>.01).map(e=>e.instance.match(/luid_0x[0-9a-f]+_0x[0-9a-f]+/i)?.[0].toLowerCase()).filter(Boolean))];
    if(config.experiment==='primary'&&(luids.length!==1||luids[0]!==config.targetAdapterLuid))reasons.push('Preflight actual render LUID does not match the requested adapter');
    report.displayGate={scope:'untimed display snapshot only; timed runs require continuous evidence',passed:reasons.length===0,reasons,renderLuids:luids,snapshots};
  }catch(error){report.error=error.message;}
  finally{
    try{await browser?.close();}catch(error){report.cleanupError=error.message;}
    for(const child of children.reverse())if(child.exitCode===null&&child.pid)try{execFileSync('taskkill',['/PID',String(child.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});}catch{}
    process.removeListener('SIGINT',cancel);process.removeListener('SIGTERM',cancel);
    await fs.writeFile(path.join(directory,'host.txt'),hostLog);
    if(endpoint)try{report.network=netlogProof(JSON.parse(await fs.readFile(path.join(directory,'netlog.json'),'utf8')),new URL(endpoint).host);}catch(error){report.networkError=error.message;}
    await fs.writeFile(path.join(directory,'calibration.json'),JSON.stringify(report,null,2));
  }
  if(report.error||report.cleanupError||!report.network?.verified)throw Error(`NTL preflight failed; no timed rounds: ${report.error??report.cleanupError??report.network?.failure??'missing successful loopback game socket'}. Inspect ${directory}`);
  return report;
}
