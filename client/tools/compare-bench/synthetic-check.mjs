// Manual browser control, never imported by CI. No game host or mod is loaded.
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {spawn} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {chromeSession} from './browser.mjs';
import {contentIntervals} from './content-probe.mjs';
import {windows} from './run.mjs';
const config=JSON.parse(await fs.readFile(process.argv[2],'utf8'));
const directory=path.join(config.output,`synthetic-production-${Date.now()}`);await fs.mkdir(directory);
const here=path.dirname(fileURLToPath(import.meta.url)),children=[];let browser,pm;
let report;
try{
  browser=await chromeSession({...config,directory,mod:null,html:await fs.readFile(path.join(here,'synthetic-content.html'),'utf8'),register:p=>children.push(p)});
  await windows({mode:'setup',clientPid:browser.child.pid,client:'browser',borderless:true,width:config.width,height:config.height,x:config.x??0,y:config.y??0},directory,'setup',children);
  await windows({mode:'focus',clientPid:browser.child.pid},directory,'focus',children);
  let pmLog='';pm=spawn(config.presentMon,['--session_name',`SlitherSynthetic-${Date.now()}`,'--v2_metrics','--qpc_time_ms','--output_file',path.join(directory,'presentmon.csv'),'--timed','8','--terminate_after_timed','--no_console_stats'],{windowsHide:true});
  pm.stdout.on('data',b=>pmLog+=b);pm.stderr.on('data',b=>pmLog+=b);
  await delay(1000);const transformBefore=await browser.page.evaluate(`getComputedStyle(document.querySelector('#compositor')).transform`);
  const start=await browser.productionStart();await delay(5000);const data=await browser.productionStop();
  const transformAfter=await browser.page.evaluate(`getComputedStyle(document.querySelector('#compositor')).transform`);
  const metrics=contentIntervals(data,{canvas:data.canvases[0].canvas,seconds:(data.now-start.now)/1000,startMs:start.now,endMs:data.now});
  report={authored:true,expectedCanvasHz:60,metrics,rafCallbacks:data.rafCallbacks,cssCompositorAnimationChanged:transformBefore!==transformAfter,
    passed:metrics.achievedFps>=55&&metrics.achievedFps<=65&&data.rafCallbacks>metrics.frames*1.5&&transformBefore!==transformAfter};
  await fs.writeFile(path.join(directory,'production.json'),JSON.stringify({report,data},null,2));
  await new Promise(resolve=>pm.exitCode!==null?resolve():pm.once('close',resolve));await fs.writeFile(path.join(directory,'presentmon-console.txt'),pmLog);
}catch(error){report={authored:true,passed:false,error:error.message};process.exitCode=1;}
finally{
  try{await browser?.close();}catch(error){report={...report,passed:false,cleanupError:error.message};process.exitCode=1;}
  finally{if(pm?.exitCode===null)pm.kill();}
  await fs.writeFile(path.join(directory,'result.json'),JSON.stringify(report,null,2));
}
console.log(JSON.stringify({directory,...report}));
