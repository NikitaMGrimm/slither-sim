// Manual browser control; importing this module never acquires or opens a window.
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {setTimeout as delay} from 'node:timers/promises';
import {chromeSession} from './browser.mjs';
import {contentIntervals} from './content-probe.mjs';
import {windows,outsideRepository} from './run.mjs';
import {checkedPresentation,windowReasons,presentMonArguments,presentMonFailure} from './analysis.mjs';
import {collectorCompletion} from './resources.mjs';
const here=path.dirname(fileURLToPath(import.meta.url));
export async function syntheticCheck(config) {
  if(!outsideRepository(path.resolve(config.output),path.resolve(here,'../../..')))throw Error('Synthetic evidence must stay outside Git');
  const directory=path.join(config.output,`synthetic-production-${Date.now()}`);await fs.mkdir(directory);
  const children=[],seconds=5;let browser,pm,completed,data;
  let report={authored:true,expectedCanvasHz:60,passed:false};
  let interrupted=false;
  const cancel=()=>{interrupted=true;browser?.close().catch(()=>{});};
  process.once('SIGINT',cancel);process.once('SIGTERM',cancel);
  try{
    report.preflight=(await windows({mode:'preflight'},directory,'preflight',children))[0];
    if(report.preflight.competingWork.length||report.preflight.idleMs<5000)throw Error('Competing work or recent owner input; synthetic acquisition deferred');
    const clock=(await windows({mode:'clock'},directory,'clock',children))[0];
    if(interrupted)throw Error('Interrupted');
    browser=await chromeSession({...config,directory,mod:null,html:await fs.readFile(path.join(here,'synthetic-content.html'),'utf8'),register:p=>{children.push(p);p.on('error',()=>{});}});
    if(interrupted)throw Error('Interrupted');
    if(config.width===3840&&config.height===2160)report.browserWindow=await browser.fullscreen();
    else await windows({mode:'setup',clientPid:browser.child.pid,client:'browser',borderless:true,width:config.width,height:config.height,x:config.x??0,y:config.y??0},directory,'setup',children);
    await windows({mode:'focus',clientPid:browser.child.pid},directory,'focus',children);
    report.presentMonArguments=presentMonArguments({session:`SlitherSynthetic-${randomUUID()}`,file:path.join(directory,'presentmon.csv'),pid:browser.gpuPid,seconds});
    pm=spawn(config.presentMon,report.presentMonArguments,{windowsHide:true});
    completed=collectorCompletion(pm);
    const early=await Promise.race([completed,delay(1000).then(()=>null)]);
    if(early)throw Error(`PresentMon failed before measurement: ${early.error??early.exitCode}; ${early.log}`);
    const transformBefore=await browser.page.evaluate(`getComputedStyle(document.querySelector('#compositor')).transform`);
    await browser.productionStart();
    const samples=await windows({mode:'sample',clientPid:browser.child.pid,clientPids:browser.processIds,gpuPid:browser.gpuPid,seconds:seconds+.5},directory,'window-load',children);
    data=await browser.productionStop();
    const first=samples[0];if(!first)throw Error('No synthetic window evidence');
    const startMs=first.atMs,endMs=startMs+seconds*1000;
    const windowEvidence={clock,startMs,endMs,width:config.width,height:config.height,display:first.display,dpi:first.dpi,refreshHz:first.refreshHz,windowSamples:samples.filter(s=>s.atMs<=endMs)};
    report={...report,windowEvidence};
    const reasons=windowReasons(windowEvidence);if(reasons.length)throw Error(reasons.join('; '));
    const browserToQpc=data.timeOrigin+clock.qpcMs-clock.unixMs;
    const metrics=contentIntervals(data,{canvas:data.canvases[0]?.canvas,seconds,startMs:startMs-browserToQpc,endMs:endMs-browserToQpc});
    const transformAfter=await browser.page.evaluate(`getComputedStyle(document.querySelector('#compositor')).transform`);
    report={...report,metrics,rafCallbacks:data.rafCallbacks,cssCompositorAnimationChanged:transformBefore!==transformAfter};
    const collector=await completed;
    if(collector.error||collector.exitCode!==0)throw Error(`PresentMon failed: ${collector.error??collector.exitCode}; ${collector.log}`);
    const display=checkedPresentation({...collector,text:await fs.readFile(path.join(directory,'presentmon.csv'),'utf8')},{pids:[browser.gpuPid],seconds,startMs,endMs});
    report.display=display;
    report.passed=metrics.achievedFps>=55&&metrics.achievedFps<=65&&metrics.rafFramesDuringWindow>metrics.frames*1.5&&transformBefore!==transformAfter&&display.metrics.achievedFps>metrics.achievedFps*1.5;
  }catch(error){report={...report,passed:false,error:error.message};}
  finally{
    try{await browser?.close();}catch(error){report={...report,passed:false,cleanupError:error.message};}
    finally{
      // Let this short, uniquely named trace terminate itself even on failure.
      if(completed){
        const collector=await completed;await fs.writeFile(path.join(directory,'presentmon-console.txt'),collector.log);
        const error=collector.error??(collector.exitCode!==0?`PresentMon exit ${collector.exitCode}`:presentMonFailure(collector.log));
        if(error)report={...report,passed:false,collectorError:error};
      }
      if(pm?.exitCode===null&&pm.pid)pm.kill();
    }
    if(data)await fs.writeFile(path.join(directory,'production.json'),JSON.stringify(data,null,2));
    if(interrupted)report={...report,passed:false,error:'Interrupted'};
    await fs.writeFile(path.join(directory,'result.json'),JSON.stringify(report,null,2));
    process.removeListener('SIGINT',cancel);process.removeListener('SIGTERM',cancel);
  }
  return {directory,...report};
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  const config=JSON.parse(await fs.readFile(process.argv[2],'utf8'));
  const result=await syntheticCheck(config);const{windowEvidence,...summary}=result;
  console.log(JSON.stringify({...summary,windowSummary:windowEvidence?{samples:windowEvidence.windowSamples.length,display:windowEvidence.display,refreshHz:windowEvidence.refreshHz,dpi:windowEvidence.dpi}:undefined}));if(!result.passed)process.exitCode=1;
}
