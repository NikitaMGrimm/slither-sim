// Manual acquisition only. Tests import pure helpers; no game connection on import.
import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, stat, readdir, copyFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { readyEndpoint } from './e2e-local-host.mjs';
import { analyzeCapture } from './latency-rig-analysis.mjs';
import { analyzePoses } from './latency-rig-poses.mjs';
import { startReference } from './latency-rig-reference.mjs';

const root = fileURLToPath(new URL('../..', import.meta.url));
const script = fileURLToPath(new URL('./latency-rig-load.ps1', import.meta.url));
const target = 'D:/CargoTargets/latency-rig';
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const check = (ok, message) => { if (!ok) throw Error(message); };
export function optionsFor(args) {
  const o = { seconds: 60, zoom: 0, truth: true, region: 'inner' };
  const seen = new Set();
  for (let i = 0; i < args.length; i++) {
    const flag = args[i];
    if (flag === '--without-truth') { o.truth = false; continue; }
    if (flag === '--scripted-reference') { check(!o.scriptedReference,'repeated reference option');o.scriptedReference=true;continue; }
    const key = { '--host-bin':'hostBin','--client-bin':'clientBin','--output':'output','--seconds':'seconds','--zoom':'zoom','--build-host-root':'hostRoot','--host-revision':'hostRevision','--region':'region' }[flag];
    check(key && !seen.has(key) && args[i+1] !== undefined, 'unknown/repeated/missing rig option');
    seen.add(key); const value = args[++i]; o[key] = ['seconds','zoom'].includes(key) ? Number(value) : ['hostRevision','region'].includes(key)?value:path.resolve(value);
  }
  check(o.output && (o.hostBin || o.hostRoot) && (o.clientBin || o.hostRoot), 'need --output, --host-bin/--client-bin or --build-host-root');
  check(Number.isFinite(o.seconds) && o.seconds >= 10 && o.seconds <= 120 && [0,-17].includes(o.zoom), 'seconds 10..120; zoom 0 or -17');
  check(!o.hostRevision||/^[a-f0-9]{40}$/i.test(o.hostRevision),'invalid host source revision');
  check(['inner','pit-edge'].includes(o.region)&&(o.region!=='pit-edge'||o.truth),'region inner or pit-edge (truth required)');
  return o;
}
export async function directoryBytes(dir) {
  let bytes = 0;
  for (const e of await readdir(dir,{withFileTypes:true})) {
    const p = path.join(dir,e.name);
    if (e.isDirectory()) bytes += await directoryBytes(p);
    else if (e.isFile()) bytes += (await stat(p)).size;
  }
  return bytes;
}
function launch(binary,args,env = process.env) {
  const p = spawn(binary,args,{windowsHide:true,env,stdio:['ignore','pipe','pipe']});
  const s = {p,stdout:'',stderr:'',done:false,code:null,error:null};
  for (const stream of ['stdout','stderr']) { p[stream].setEncoding('utf8'); p[stream].on('data',d => { s[stream]+=d; }); }
  s.finished = new Promise(resolve => {
    p.once('error',e => { s.error=e.message; });
    p.once('close',code => { s.done=true; s.code=code; resolve(); });
  });
  return s;
}
async function stop(s) {
  if (!s || s.done) return;
  s.p.kill();
  await Promise.race([s.finished,delay(2000)]);
  if (!s.done) { s.p.kill('SIGKILL'); await s.finished; }
}
async function bounded(s,seconds) {
  const timed = await Promise.race([s.finished.then(()=>false),delay(seconds*1000).then(()=>true)]);
  check(!timed, 'process deadline exceeded');
  check(s.code === 0 && !s.error, `process failed: ${s.code} ${s.error??''} ${s.stderr.slice(-2000)}`);
}
function git(args) { return execFileSync('git',args,{cwd:root,encoding:'utf8',windowsHide:true}).trim(); }
export async function run(o) {
  check(process.platform === 'win32','The load sampler and scheduled rig currently require Windows');
  await mkdir(o.output); // Fresh directory only; failures and previous runs survive.
  const relative = path.relative(root,o.output);
  check(relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative), 'captures must be outside checkout');
  check(await directoryBytes(path.dirname(o.output)) < 1.25*1024**3,'scratch acquisition budget: reserve 0.75 GiB before another capture');
  const hash = async p => createHash('sha256').update(await readFile(p)).digest('hex');
  const all = [];
  let sampler, host, client, reference, active=false, contaminated=false;
  let fatal;
  const interrupt = () => { fatal = Error('capture interrupted'); };
  process.once('SIGINT',interrupt); process.once('SIGTERM',interrupt);
  try {
    if (o.hostRoot) {
      // Per-command environment only. No persistent services or global settings.
      const env = {...process.env,CARGO_TARGET_DIR:target,CARGO_BUILD_JOBS:'2'};
      execFileSync('cargo',['build','--release','--locked','--features','latency-rig','--bin','slither-sim'],{cwd:o.hostRoot,env,stdio:'inherit',windowsHide:true});
      execFileSync('cargo',['build','--release','--locked','-p','slither-wgpu-slice','--features','latency-rig','--bin','slither-client-latency'],{cwd:root,env,stdio:'inherit',windowsHide:true});
      o.hostBin = path.join(target,'release/slither-sim.exe');
      o.clientBin = path.join(target,'release/slither-client-latency.exe');
      o.hostRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:o.hostRoot,encoding:'utf8',windowsHide:true}).trim();
    }
    await mkdir(path.join(o.output,'binaries'));
    for (const [key,name] of [['hostBin','host.exe'],['clientBin','client.exe']]) {
      const dest = path.join(o.output,'binaries',name); await copyFile(o[key],dest); o[key]=dest;
    }
    const config = {bind:'127.0.0.1:0',seed:7,players:{max:o.scriptedReference?2:1},bots:{count:120,threads:2},simulation:{world:{preset:'battledome'}}};
    await writeFile(path.join(o.output,'host.json'),JSON.stringify(config,null,2),{flag:'wx'});
    await writeFile(path.join(o.output,'plan.json'),JSON.stringify({
      format:'latency-rig-plan-v1',sourceRevision:git(['rev-parse','HEAD']),
      clientDiff:git(['diff','HEAD','--','client']),hostBinarySha256:await hash(o.hostBin),clientBinarySha256:await hash(o.clientBin),
      hostSourceRevision:o.hostRevision??null,
      seconds:o.seconds,zoomNotches:o.zoom,truth:o.truth,region:o.region,scriptedReference:Boolean(o.scriptedReference),config,
      reference:o.scriptedReference?'extra bot API client joins first; explicit reference ID only within 1200; 2s straight/turn normal/boost phases inside pit; 120 background Foragers unchanged':'nearest live received bot within 1200 units; hold until disappearance/life change/out of range; ID tie break',
      middle:o.region==='pit-edge'?'measurement head radius <=1.1 live pit; boost arm <0.7 pit; zoom on playing; target orbit0.5 pit; references warm2s from first observation; zoomout waits2s after actual zoom<0.5':'head radius <0.7 current pit; zoom on playing; target orbit0.5 pit; references warm2s from segment entry',
      strata:'reference normal/boost from host state; straight <=0.05 rad heading change over preceding 100ms, turning >=0.15 rad; remainder transitional',
      lag:'truth minus actual f32 painter head, tangent/speed lag equivalent; during turns not temporal delay',
      quantiles:'nearest rank p50/p90/p99; descriptive correlated frames; p99 under 1000 samples explicitly sparse',
      limitations:'offscreen serialized GPU, synthetic callbacks, no swapchain/physical input/present/scanout; 200ms compiler sampling plus CIM query gaps',
    },null,2),{flag:'wx'});
    sampler=launch('pwsh',['-NoProfile','-File',script]);
    let read=0, idle=0;
    const samples=() => {
      const lines=sampler.stdout.split('\n');
      while(read < lines.length-1) {
        const text=lines[read++].trim(); if(!text)continue;
        const row=JSON.parse(text); all.push(row);
        if(row.compilers.length) { idle=0; if(active) contaminated=true; } else idle++;
      }
      check(!sampler.done,`load sampler stopped: ${sampler.stderr}`);
    };
    console.log('Waiting for five compiler-free sampler observations');
    while(idle < 5) { samples(); if(fatal)throw fatal; await delay(200); }
    const args=['host','--config',path.join(o.output,'host.json'),'--stats','1'];
    if(o.truth)args.push('--latency-truth',path.join(o.output,'truth.jsonl'),'--latency-seconds',String(o.seconds+(o.scriptedReference?24:12)));
    active=true; host=launch(o.hostBin,args);
    let endpoint;
    const deadline=Date.now()+20000;
    while(!(endpoint=readyEndpoint(host.stderr)??readyEndpoint(host.stdout))) {
      samples(); if(fatal)throw fatal; check(!host.done && Date.now()<deadline,'host readiness failed'); await delay(100);
    }
    await writeFile(path.join(o.output,'endpoint.txt'),endpoint,{flag:'wx'});
    if(o.scriptedReference)reference=await startReference(o.hostBin,endpoint,path.join(o.output,'scripted-reference.json'));
    const clientArgs=['--server',new URL(endpoint).host,'--output',path.join(o.output,'client'),'--seconds',String(o.seconds),'--zoom',String(o.zoom)];
    if(reference)clientArgs.push('--reference-id',String(reference.id));
    client=launch(o.clientBin,clientArgs,{
      ...process.env,SLITHER_LATENCY_TRACE:path.join(o.output,'latency.jsonl'),SLITHER_LATENCY_REVISION:git(['rev-parse','HEAD']) });
    console.log(`Capturing ${o.seconds}s, zoom ${o.zoom}, ${endpoint}`);
    const end=Date.now()+(o.seconds+25)*1000;
    while(!client.done) { samples(); if(fatal)throw fatal; if(reference?.failure)throw reference.failure;check(Date.now()<end,'client deadline'); check(!contaminated,'compiler overlapped capture; retained invalid attempt'); await delay(100); }
    await bounded(client,1);
    if(reference){await reference.stop();reference=null;}
    if(o.truth) {
      while(!host.done) { samples(); check(Date.now()<end+10000,'host export deadline'); check(!contaminated,'compiler overlapped capture'); await delay(100); }
      await bounded(host,1);
    }
    active=false;
    await stop(host);
    await writeFile(path.join(o.output,'load.json'),JSON.stringify({contaminated,samples:all},null,2),{flag:'wx'});
    const result=await analyzeCapture(o.output,{region:o.region});
    await writeFile(path.join(o.output,'results.json'),JSON.stringify(result,null,2),{flag:'wx'});
    let hasPoseJournal=true;
    try {await stat(path.join(o.output,'client/heads.poses.jsonl'));}
    catch(error) {if(error.code!=='ENOENT')throw error;hasPoseJournal=false;}
    if(hasPoseJournal){
      await writeFile(path.join(o.output,'reference-cadence.json'),JSON.stringify(await analyzePoses(o.output),null,2),{flag:'wx'});
    }
    console.log(JSON.stringify(result,null,2));
    return result;
  } catch(error) {
    await writeFile(path.join(o.output,'failure.json'),JSON.stringify({error:String(error),contaminated,samples:all},null,2),{flag:'wx'});
    throw error;
  } finally {
    await stop(client);if(reference)await reference.stop().catch(()=>{});await stop(host);await stop(sampler);
    for(const [name,s] of [['host',host],['client',client],['load',sampler]])if(s) {
      await writeFile(path.join(o.output,`${name}.stdout.log`),s.stdout,{flag:'wx'});
      await writeFile(path.join(o.output,`${name}.stderr.log`),s.stderr,{flag:'wx'});
    }
    process.removeListener('SIGINT',interrupt);process.removeListener('SIGTERM',interrupt);
  }
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  try { await run(optionsFor(process.argv.slice(2))); } catch(e) { console.error(String(e));process.exitCode=1; }
}
