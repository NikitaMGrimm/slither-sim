import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash, randomUUID } from 'node:crypto';
import { spawn, execFileSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { checkedPresentation,presentMonArguments,presentMonFailure,hostStats, wholeMap, validity, netlogProof, comparison,nativeProduction,drawnSnakeEvidence } from './analysis.mjs';
import { contentIntervals } from './content-probe.mjs';
import { pngEvidence } from './picture.mjs';
import { ntl } from './browser.mjs';
import {collectorCompletion,recoverCollectorLog,cleanupPresentMon} from './resources.mjs';
import {startNativeNetwork,nativeNetworkMaxSeconds} from './native-network.mjs';
import {roundPlan,anchoredBoundary,settleNtlView,collectRounds,requireNtlDisplayGate} from './matrix.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repository = path.resolve(here, '../../..');
const prohibited = new Set([8766, 18884, 18885]);
let interrupted = false;
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sha = async file => hash(await fs.readFile(file));
const save = (file, value) => fs.writeFile(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
export function outsideRepository(root,repo,paths=path) {
  const relative=paths.relative(repo,root);
  return relative!==''&&(relative==='..'||relative.startsWith(`..${paths.sep}`)||paths.isAbsolute(relative));
}
export function lineJournal(accept,now) {
  let pending='';
  return bytes=>{pending+=bytes;let split;while((split=pending.indexOf('\n'))>=0){const line=pending.slice(0,split).replace(/\r$/,'');pending=pending.slice(split+1);accept(line,now());}};
}
export function completeJsonLines(text, prefix) {
  return text.split(/\r?\n/).slice(0,-1).filter(l => l.startsWith(prefix)).map(l => JSON.parse(l.slice(prefix.length)));
}
async function treeHash(directory) {
  const rows = [];
  async function walk(dir, prefix = '') {
    for (const entry of (await fs.readdir(dir, { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name, 'en'))) {
      if (entry.name === '.git') continue;
      if (entry.isSymbolicLink()) throw Error('Input tree must not contain symlinks');
      const name = prefix + entry.name;
      if (entry.isDirectory()) await walk(path.join(dir, entry.name), name + '/');
      else rows.push([name, await sha(path.join(dir, entry.name))]);
    }
  }
  await walk(directory); return { sha256: hash(JSON.stringify(rows)), files: rows };
}
export function endpointFrom(text) {
  const found = text.match(/Listening on (ws:\/\/127\.0\.0\.1:(\d+)\/slither)(?:\r?\n|$)/);
  if (!found) return null;
  const port = +found[2]; if (!port || port > 65535 || prohibited.has(port)) throw Error('Host selected forbidden port');
  return found[1];
}
function execute(command, args, options = {}) {
  return execFileSync(command, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, windowsHide: true, ...options }).trim();
}
async function waitFor(predicate, ms, label) {
  const end = performance.now() + ms;
  while (performance.now() < end) { if (interrupted) throw Error('Interrupted'); const result = await predicate(); if (result) return result; await delay(100); }
  throw Error(`${label} timeout`);
}
async function stop(child) {
  if (!child || child.exitCode !== null) return;
  // Only the process tree launched for this attempt, never owner's browser.
  try { execute('taskkill', ['/PID', String(child.pid), '/T', '/F']); } catch {}
  await Promise.race([new Promise(resolve => child.once('close', resolve)), delay(2000)]);
}
export async function windows(config, directory, label, children) {
  const file = path.join(directory, `${label}-config.json`); await save(file, {...config,callerPid:process.pid});
  const child = spawn('powershell.exe', ['-NoProfile', '-File', path.join(here, 'windows.ps1'), '-Config', file], { windowsHide: true });
  children.push(child); let out = '', err = '';
  child.stdout.on('data', b => out += b); child.stderr.on('data', b => err += b);
  const code = await new Promise((resolve, reject) => { child.once('error', reject); child.once('close', resolve); });
  await fs.writeFile(path.join(directory, `${label}.txt`), out + err);
  if (code) throw Error(`Windows ${label} failed: ${err.slice(-2000)}`);
  return out.trim().split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line));
}
async function prepare(config, root) {
  const checkout = path.join(root, 'vlither');
  try { await fs.access(path.join(checkout, '.git')); }
  catch { execute('git', ['clone', 'https://github.com/for-loop9/vlither.git', checkout]); }
  if (execute('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: checkout })) throw Error('Vlither checkout has tracked changes; preserve them and use a clean scratch root');
  execute('git', ['fetch', 'origin'], { cwd: checkout });
  const ref = config.vlitherRef || execute('git', ['symbolic-ref', 'refs/remotes/origin/HEAD'], { cwd: checkout });
  const commit = execute('git', ['rev-parse', `${ref}^{commit}`], { cwd: checkout });
  execute('git', ['checkout', '--detach', commit], { cwd: checkout });
  return { checkout, ref, commit };
}
async function inputs(config, directory) {
  const binary = path.resolve(config.binary), host = path.resolve(config.host);
  const result = { binary: { sha256: await sha(binary), path: binary }, host: { sha256: await sha(host), path: host },
    repositoryHead: execute('git', ['rev-parse', 'HEAD'], { cwd: repository }),
    sourceDirty: Boolean(execute('git', ['status', '--porcelain'], { cwd: repository })),
    presentMon: { sha256: await sha(config.presentMon) } };
  if(config.binaryReceipt){result.binary.receipt=JSON.parse(await fs.readFile(config.binaryReceipt,'utf8'));if(result.binary.receipt.binarySha256!==result.binary.sha256)throw Error('Binary build receipt hash mismatch');}
  return result;
}
async function modInput(input, session) {
  const info = await fs.stat(input);
  if (info.isDirectory()) return { directory: path.resolve(input), tree: await treeHash(input) };
  const bytes = await fs.readFile(input);
  let offset = 0;
  if (bytes.subarray(0, 4).toString() === 'Cr24') {
    const version = bytes.readUInt32LE(4);
    if (version === 3) offset = 12 + bytes.readUInt32LE(8);
    else if (version === 2) offset = 16 + bytes.readUInt32LE(8) + bytes.readUInt32LE(12);
    else throw Error(`Unsupported CRX version ${version}`);
  }
  if (bytes.subarray(offset, offset + 4).toString('hex') !== '504b0304') throw Error('NTL input must be a manifest folder, ZIP, or CRX2/3 package');
  const archive = path.join(session, 'mod-package.zip'), directory = path.join(session, 'mod-extracted');
  await fs.writeFile(archive, bytes.subarray(offset), { flag: 'wx' });
  execute('powershell.exe', ['-NoProfile', '-File', path.join(here, 'unpack-mod.ps1'), '-Archive', archive, '-Destination', directory]);
  return { directory, packageSha256: hash(bytes), tree: await treeHash(directory) };
}
async function runAttempt(config, root, item, provenance, machine) {
  const directory = path.join(root, `${item.scenario}-${item.round}-${item.client}-${randomUUID()}`);
  await fs.mkdir(directory);
  const children = []; let hostLog = '', clientLog = '', browser, pm, host, native,networkMonitor;
  let pmLog = '',pmCompleted;const browserFrames=[];
  const r = { schema: 'slither-frame-bench-v1', ...item, seconds: config.seconds,
    experiment: config.experiment, targetAdapterLuid: config.targetAdapterLuid,
    targetDisplay: config.targetDisplay, targetRefreshHz: config.targetRefreshHz,
    width: config.width, height: config.height, dt: .007, bots: item.scenario === 'battledome' ? 100 : 500,
    provenance, machine, telemetry:config.telemetry??'on', windowSamples: [], views: [], census: [], stats: [],
    ntlViewCalibration:config.ntlViewCalibration,
    browserViewSampling:item.client==='ntl'&&config.telemetry!=='off'?'CDP screenshots every 1.5 s plus capture time; decode after window; included in telemetry perturbation control':'no timed picture sampling',
    acceptance: 'qualified acquisition; content/display equivalence must be established separately',
    survival: 'host circular steering at 0.75*radius, boost off, socket player ignores body collisions; boundary/pit rules and bots unchanged; any death invalidates' };
  const register = child => { children.push(child); child.on('error', () => {}); };
  let offset=machine.clock.qpcMs-machine.clock.unixMs;
  const now=()=>Date.now()+offset;
  const telemetryErrors=[];
  const hostJournal=lineJournal((line,atMs)=>{try{
    if(line.startsWith('frame-bench: '))r.census.push({...JSON.parse(line.slice(13)),atMs});
    else for(const stats of hostStats(line))r.stats.push({...stats,atMs});
  }catch(error){telemetryErrors.push(error.message);}},now);
  const clientJournal=lineJournal((line,atMs)=>{try{
    if(line.startsWith('frame-bench-client: ')){
      const v=JSON.parse(line.slice(20));r.views.push({...v,atMs,wholeMap:wholeMap(v)});
    }
  }catch(error){telemetryErrors.push(error.message);}},now);
  const cancel = () => {
    interrupted = true;
    browser?.close().catch(() => {});
    for (const child of [...children].reverse()) stop(child).catch(() => {});
  };
  process.once('SIGINT', cancel); process.once('SIGTERM', cancel);
  try {
    r.clock=(await windows({mode:'clock'},directory,'clock',children))[0];
    offset=r.clock.qpcMs-r.clock.unixMs;
    const scenario = JSON.parse(await fs.readFile(path.join(here, 'scenarios', `${item.scenario}.json`), 'utf8'));
    const scenarioPath = path.join(directory, 'host.json'); await save(scenarioPath, scenario);
    r.scenarioSha256 = await sha(scenarioPath);
    host = spawn(config.host, ['host', '--bind', '127.0.0.1:0', '--config', scenarioPath, '--stats', '0.5', '--frame-bench'], { windowsHide: true }); register(host);
    host.stdout.on('data', b => {hostLog+=b;hostJournal(b);}); host.stderr.on('data', b => {hostLog+=b;hostJournal(b);});
    const endpoint = await waitFor(() => endpointFrom(hostLog) || (host.exitCode !== null ? Promise.reject(Error(`Host exited ${host.exitCode}: ${hostLog}`)) : null), 30000, 'Host readiness');
    r.endpoint = endpoint;
    if (item.client === 'ntl') {
      if(provenance.ntl?.error||!provenance.ntl?.tree?.sha256)throw Error('NTL mod provenance unavailable; package will not be loaded');
      browser = await ntl({ ...config, directory, endpoint, register, scenario:item.scenario,productionProbe:config.telemetry!=='off' });
      native = browser.child;
      r.adapter = browser.adapter; r.chromeVersion = browser.chromeVersion;
      r.adapterRequest = config.adapter; r.vsync = 'Chrome default compositor pacing; actual SyncInterval unmeasured until ETW';
      if(config.width===3840&&config.height===2160){
        const idle=(await windows({mode:'preflight'},directory,'before-browser-fullscreen',children))[0];
        if(idle.idleMs<5000)throw Error('Owner input before browser fullscreen; deferred');
        r.browserWindow=await browser.fullscreen();r.windowMode='Chrome app window, browser fullscreen requested through CDP';
      }else{
        await windows({ mode: 'setup', borderless:true,clientPid: native.pid, client: 'browser', scenario: item.scenario, width: config.width, height: config.height, x: config.x, y: config.y }, directory, 'setup', children);
        r.windowMode='Chrome app window, borderless task-owned HWND';
      }
      r.vsync = 'Chrome default compositor pacing; verify PresentMon SyncInterval/mode'; r.chromeVersion = browser.chromeVersion;
    } else if (item.client === 'vlither') {
      const receipt = JSON.parse(await fs.readFile(path.join(config.output, 'vlither/build/compare-build.json'), 'utf8'));
      if (!receipt.instrumented || receipt.binarySha256 !== await sha(config.vlitherBinary)
          || receipt.wrapperSha256 !== await sha(path.join(here, 'vlither-wrapper.c'))
          || receipt.commit !== provenance.vlither.commit || provenance.vlither.dirty) throw Error('Vlither wrapper/build provenance changed; prepare and rebuild before acquisition');
      r.vlitherBuild = receipt; r.vsync = 'FIFO, enabled by labelled benchmark wrapper';
      // Fresh user.dat per attempt; assets remain in external upstream checkout.
      for (const resource of ['app', 'thermite']) await fs.symlink(path.join(config.output, 'vlither', resource), path.join(directory, resource), 'junction');
      r.nativeStartedUnixMs=Date.now();
      native = spawn(config.vlitherBinary, [], { cwd: directory, env: { ...process.env,
        SLITHER_BENCH_SERVER: endpoint.replace('ws://', '').replace('/slither', ''),
        ...(item.scenario === 'battledome' ? { SLITHER_BENCH_WHOLE_MAP: '1' } : {}) }, windowsHide: false });
      register(native); native.stdout.on('data', b => {clientLog+=b;clientJournal(b);}); native.stderr.on('data', b => {clientLog+=b;clientJournal(b);});
      networkMonitor=await startNativeNetwork({pid:native.pid,executable:config.vlitherBinary,directory,maxSeconds:nativeNetworkMaxSeconds(config),startedUnixMs:r.nativeStartedUnixMs});
      await windows({ mode: 'setup',borderless:true, clientPid: native.pid, client: 'vlither', scenario: item.scenario, width: config.width, height: config.height, x: config.x, y: config.y }, directory, 'setup', children);
      r.windowMode='Vlither GLFW window, borderless task-owned HWND';
      await waitFor(() => clientLog.includes('frame-bench-client: ') || (native.exitCode !== null ? Promise.reject(Error(`Vlither exited ${native.exitCode}: ${clientLog}`)) : null), 20000, 'Vlither local join');
    } else {
      const data = path.join(directory, 'preferences'); await fs.mkdir(path.join(data, 'SlitherClient'), { recursive: true });
      await fs.writeFile(path.join(data, 'SlitherClient', 'settings.txt'), `2:1:${item.client === 'native' ? 'native' : 'original'}`);
      const clientEnv={...process.env,LOCALAPPDATA:data};
      delete clientEnv.SLITHER_FRAME_PACING;delete clientEnv.SLITHER_FRAME_PACING_JOURNAL;
      if(config.telemetry!=='off')Object.assign(clientEnv,{SLITHER_FRAME_PACING:'1',SLITHER_FRAME_PACING_JOURNAL:path.join(directory,'native-production.json')});
      r.nativeStartedUnixMs=Date.now();
      native = spawn(config.binary, ['--server', endpoint.replace('ws://', '').replace('/slither', ''), '--nickname', 'FrameBench', '--skin', '0'],
        { cwd: directory, env:clientEnv,windowsHide: false });
      register(native); native.stdout.on('data', b => {clientLog+=b;clientJournal(b);}); native.stderr.on('data', b => {clientLog+=b;clientJournal(b);});
      networkMonitor=await startNativeNetwork({pid:native.pid,executable:config.binary,directory,maxSeconds:nativeNetworkMaxSeconds(config),startedUnixMs:r.nativeStartedUnixMs});
      await windows({ mode: 'setup',borderless:true, clientPid: native.pid, client: 'ours', scenario: item.viewCondition==='ntl-farthest'?'normal':item.scenario, width: config.width, height: config.height, x: config.x, y: config.y }, directory, 'setup', children);
      r.windowMode='Native winit window, borderless task-owned HWND';
      await waitFor(() => completeJsonLines(hostLog, 'frame-bench: ').some(s => s.players?.[0]?.setup)
        || (native.exitCode !== null ? Promise.reject(Error(`Client exited ${native.exitCode}: ${clientLog}`)) : null), 60000, 'Own admission');
      r.vsync = 'Fifo (source); PresentMon records actual mode and SyncInterval';
    }
    r.worldReady = await waitFor(() => {
      const census = completeJsonLines(hostLog, 'frame-bench: ').at(-1);
      if (!census) return null;
      return census.admittedBotSlots === r.bots && census.players?.[0]?.alive && census.players[0].setup ? census : null;
    }, 60000, 'All internal bot seats and living player readiness');
    if(browser&&item.scenario==='battledome'){
      r.viewSetup=await settleNtlView(browser,config.ntlViewCalibration,item.viewCondition,async index=>{
        const file=await browser.capture(`before-warmup-${index}`),picture=pngEvidence(await fs.readFile(file));
        return {file:path.basename(file),picture,census:r.census.at(-1),atMs:now()};
      },delay);
    }
    if(item.viewCondition==='ntl-farthest'&&!browser){
      const target=config.ntlViewCalibration?.physicalPixelsPerWorldUnit;
      if(!Number.isFinite(target)||target<=0)throw Error('Matched NTL view calibration unavailable');
      let matched=false;
      for(let i=0;i<8;i++){
        const view=await waitFor(()=>r.views.at(-1),4000,'Native zoom telemetry');
        const scale=view.zoom*config.width/view.viewport?.[0];
        if(!Number.isFinite(scale)||scale<=0)throw Error('Native physical zoom unavailable');
        if(Math.abs(scale/target-1)<=.03){matched=true;break;}
        await windows({mode:'wheel',clientPid:native.pid,notches:Math.log(target/scale)/Math.log(1.12)},directory,`match-zoom-${i}`,children);
        const previous=r.views.length;await waitFor(()=>r.views.length>previous,4000,'Native post-wheel telemetry');
      }
      if(!matched)throw Error('Native wheel input did not reach the calibrated NTL view');
    }
    await delay(config.warmupSeconds * 1000);
    await windows({mode:'focus',clientPid:native.pid},directory,'foreground',children);
    if(browser){
      const before=await browser.capture('before-window');const evidence=pngEvidence(await fs.readFile(before));
      r.preflightView={...evidence,atMs:now()};r.views.push(r.preflightView);
      if(evidence.viewport[0]!==config.width||evidence.viewport[1]!==config.height)throw Error(`Browser content viewport ${evidence.viewport.join('x')} does not match requested ${config.width}x${config.height}; no timed capture`);
      const processes=await browser.browser.send('SystemInfo.getProcessInfo');
      browser.processIds=processes.processInfo.map(p=>p.id);
      browser.gpuPid=processes.processInfo.find(p=>p.type==='GPU')?.id;
      r.clientProcessSnapshot=processes;
    }
    // Start ETW before the fixed window; retain the guards before/after it too.
    const csvPath = path.join(directory, 'presentmon.csv');
    r.presentMonSession=`SlitherBench-${randomUUID()}`;
    r.presentMonArguments=presentMonArguments({session:r.presentMonSession,file:csvPath,pid:browser?browser.gpuPid:native.pid,seconds:config.seconds});
    pm = spawn(config.presentMon,r.presentMonArguments,{windowsHide:true});register(pm);
    pmCompleted=collectorCompletion(pm,log=>{r.collectorFailure??=presentMonFailure(log);});
    const early=await Promise.race([pmCompleted,delay(1000).then(()=>null)]);
    if (early) {pmLog=early.log;throw Error(`PresentMon preflight failed: ${early.error??early.exitCode}; ${pmLog}. No elevation or security changes attempted.`);}
    if(browser&&config.telemetry!=='off')r.productionStarted=await browser.productionStart();
    const samplesPromise = windows({ mode: 'sample', clientPid: native.pid,clientPids:browser?browser.processIds:[native.pid], gpuPid: browser ? browser.gpuPid : native.pid, hostPid: host.pid, seconds: config.seconds + .5 }, directory, 'window-load', children);
    let monitoring = true;
    const captureStarted=Date.now();let nextPicture=captureStarted+1500;
    samplesPromise.finally(() => { monitoring = false; }).catch(() => {});
    while (monitoring) {
      if(r.collectorFailure)throw Error(r.collectorFailure);
      if(pm.exitCode!==null)throw Error(`PresentMon collection ended early (${pm.exitCode})`);
      if(Date.now()-captureStarted>10000&&!await fs.stat(csvPath).then(s=>s.size>0,()=>false))throw Error('No presentation CSV within ten seconds; collection unavailable');
      if(browser&&config.telemetry!=='off'&&Date.now()>=nextPicture){
        const before=now(),file=await browser.capture(`view-${browserFrames.length}`);
        browserFrames.push({file,atMs:(before+now())/2});nextPicture=Date.now()+1500;
      }
      await delay(250);
    }
    r.windowSamples = await samplesPromise;
    if(r.windowSamples.some(s=>s.ownerInput||!s.foreground||s.occluded||s.minimized||!s.visible)){
      if(browser&&config.telemetry!=='off')await save(path.join(directory,'canvas-production-partial.json'),await browser.productionStop());
      throw Error('Window obscured or owner input/foreground loss; stopped without reclaiming focus');
    }
    if(browser&&config.telemetry!=='off'){
      r.production=await browser.productionStop();
      await save(path.join(directory,'canvas-production.json'),r.production);
      r.devicePixelRatio=r.production.devicePixelRatio;
      const canvases=r.production.canvases.filter(c=>c.rect[2]*r.devicePixelRatio>=config.width*.8&&c.rect[3]*r.devicePixelRatio>=config.height*.8);
      if(canvases.length!==1)throw Error('Game canvas not uniquely identified by visible viewport coverage');
      const browserToQpc=r.production.timeOrigin+offset;
      r.contentMetrics=contentIntervals(r.production,{canvas:canvases[0].canvas,seconds:config.seconds,startMs:r.windowSamples[0].atMs-browserToQpc,endMs:r.windowSamples[0].atMs+config.seconds*1000-browserToQpc});
      const after=await browser.capture('after-window');r.postflightView={...pngEvidence(await fs.readFile(after)),atMs:now()};r.views.push(r.postflightView);
      r.contentDisplayCorrelation='unestablished';
    }
    for(const frame of browserFrames)r.views.push({...pngEvidence(await fs.readFile(frame.file)),atMs:frame.atMs});
    if(browser){
      for(const view of r.views){
        const census=r.census.filter(c=>Math.abs(c.atMs-view.atMs)<=1200).sort((a,b)=>Math.abs(a.atMs-view.atMs)-Math.abs(b.atMs-view.atMs))[0];
        if(item.scenario==='battledome'){
          view.boundaryAnchor=anchoredBoundary(view,census);
          view.wholeMap=view.boundaryAnchor?.wholeMap===true;
        }
      }
      r.views.sort((a,b)=>a.atMs-b.atMs);
    }
    if (!r.windowSamples.length) throw Error('Window monitoring returned no samples');
    const first = r.windowSamples[0]; r.display = first.display; r.dpi = first.dpi;
    r.startMs = first.qpcMs; r.endMs = r.startMs + config.seconds * 1000;
    r.windowSamples = r.windowSamples.filter(s => s.qpcMs <= r.endMs);
    r.refreshHz = first.refreshHz;
    r.displayAdapters = machine.dxgiAdapters.filter(a => a.outputs?.includes(r.display));
    r.displayBindings = first.displayBinding?[first.displayBinding]:[];
    const engines = r.windowSamples.flatMap(s => s.gpuEngines ?? []);
    const renderLuids = [...new Set(engines.filter(e => /engtype_3d/i.test(e.instance) && e.utilization > .01)
      .map(e => e.instance.match(/luid_0x[0-9a-f]+_0x[0-9a-f]+/i)?.[0].toLowerCase()).filter(Boolean))];
    r.renderAdapters = renderLuids.map(luid => machine.dxgiAdapters.find(a => a.luid === luid) ?? { luid, name: 'unmapped' });
    r.presentationPath = r.renderAdapters.length && r.displayBindings.length
      ? r.renderAdapters.every(a => r.displayBindings.some(b => b.targetLuid === a.luid)) ? 'same-adapter render/output' : 'cross-adapter render/output'
      : 'unknown render/output path';
    r.adapterRequest = config.adapter ?? { chromeLuid: 'default', native: 'default', vlither: 'default' };
    r.expectedRenderLuid = config.experiment === 'primary' ? config.targetAdapterLuid
      : item.client === 'ntl' && config.adapter?.chromeLuid !== 'default' ? config.adapter?.chromeLuid : null;
    r.adapter = item.client === 'ntl' ? browser.adapter : clientLog.match(/Actual adapter (.+); MSAA/)?.[1] ?? clientLog.match(/GPU: (.+)/)?.[1];
    r.load = { hostMeanCpuCores: r.windowSamples.reduce((sum, s) => sum + s.hostCpuCores, 0) / r.windowSamples.length,
      clientMeanCpuCores:r.windowSamples.slice(1).reduce((sum,s)=>sum+s.clientCpuCores,0)/Math.max(1,r.windowSamples.length-1),
      clientCpuSeconds:r.windowSamples.slice(1).reduce((sum,s)=>sum+s.clientCpuDeltaSeconds,0),
      machineMeanCpuPercent: r.windowSamples.reduce((sum, s) => sum + s.machineCpuPercent, 0) / r.windowSamples.length,
      machineMaxCpuPercent: Math.max(...r.windowSamples.map(s => s.machineCpuPercent)),
      logicalProcessors: machine.logicalProcessors };
    const covered=rows=>rows.filter(row=>row.atMs>=r.startMs-3000&&row.atMs<=r.endMs+3000);
    r.stats=covered(r.stats);r.census=covered(r.census);r.views=covered(r.views);
    if(r.windowSamples.some(s=>s.ownerInput||!s.foreground))throw Error('Owner input or foreground loss: stopped without reclaiming focus');
    const collector=await pmCompleted;pmLog=collector.log;
    await fs.writeFile(path.join(directory, 'presentmon-console.txt'), pmLog);
    if(collector.error||collector.exitCode!==0)throw Error(`PresentMon collection failed: ${collector.error??collector.exitCode}; ${pmLog}`);
    const etwFailure=presentMonFailure(pmLog);if(etwFailure)throw Error(etwFailure);
    const text = await fs.readFile(csvPath, 'utf8');
    // Chrome GPU process owns presents. Require an explicit single active
    // surface in this dedicated process tree; never pick the fastest surface.
    const displayed=checkedPresentation({exitCode:pm.exitCode,log:pmLog,text},{pids:[browser?browser.gpuPid:native.pid],seconds:config.seconds,startMs:r.startMs,endMs:r.endMs});
    r.surfaces=displayed.surfaces;r.metrics=displayed.metrics;
    r.metrics.refreshPeriodMs = 1000 / r.refreshHz;
    r.metrics.p95RefreshPeriods = r.metrics.p95Ms / r.metrics.refreshPeriodMs;
    r.metrics.longestRefreshPeriods = r.metrics.longestStallMs / r.metrics.refreshPeriodMs;
    r.metrics.medianRefreshPeriods = r.metrics.medianMs / r.metrics.refreshPeriodMs;
    r.vsyncCondition = r.metrics.syncIntervals.length === 1 && r.metrics.syncIntervals[0] === '1' ? 'sync-interval-1' : 'recorded-modes-require-qualified-comparison';
    if(!browser&&item.client!=='vlither'&&config.telemetry!=='off'){
      await windows({mode:'close',clientPid:native.pid},directory,'close',children);
      await waitFor(()=>native.exitCode!==null,5000,'Native graceful journal export');
      r.nativeProduction=nativeProduction(JSON.parse(await fs.readFile(path.join(directory,'native-production.json'),'utf8')),{startMs:r.startMs,endMs:r.endMs,qpcUnixOffset:offset});
      r.contentDisplayCorrelation='unestablished';
    }
  } catch (error) { r.error = error.message; }
  finally {
    r.telemetryErrors=telemetryErrors;
    if(telemetryErrors.length)r.error??='Malformed timed telemetry; inspect telemetryErrors';
    try { await browser?.close(); } catch (e) { r.cleanupError = e.message; }
    if(pm&&pm.exitCode!==0&&r.presentMonSession){
      // A force-killed ETW consumer leaves a live session. Stop only the exact
      // unique session this attempt started before terminating owned children.
      try{r.traceCleanup=cleanupPresentMon(pm,r.presentMonSession,session=>execute(config.presentMon,['--session_name',session,'--terminate_existing_session'],{timeout:5000}));}
      catch(error){r.cleanupError??=`Owned ETW session cleanup failed: ${error.message}`;}
    }
    for (const child of [...children].reverse()) await stop(child);
    if(networkMonitor){
      try{r.nativeNetwork=await networkMonitor.finish();}catch(error){r.networkError=error.message;}
    }
    pmLog=await recoverCollectorLog(pmCompleted,pmLog);
    await fs.writeFile(path.join(directory, 'host.txt'), hostLog);
    await fs.writeFile(path.join(directory, 'client.txt'), clientLog);
    await fs.writeFile(path.join(directory, 'presentmon-console.txt'), pmLog);
    if (item.client === 'ntl' && r.endpoint) {
      try { r.networkProof = netlogProof(JSON.parse(await fs.readFile(path.join(directory,'netlog.json'), 'utf8')), r.endpoint.replace('ws://', '').replace('/slither', '')); r.networkVerified = r.networkProof.verified; }
      catch (e) { r.networkError = e.message; }
    }
    r.sceneAcceptance={drawnSnakes:drawnSnakeEvidence(r)};
    r.validity = validity(r); await save(path.join(directory, 'result.json'), r);
    process.removeListener('SIGINT', cancel); process.removeListener('SIGTERM', cancel);
  }
  console.log(JSON.stringify({ directory, scenario: item.scenario, client: item.client, valid: r.validity.valid, reasons: r.validity.reasons }));
  return { directory, valid: r.validity.valid, run: r };
}
export async function main(args = process.argv.slice(2)) {
  if (args.length !== 2 || !['inspect', 'prepare', 'preflight', 'smoke', 'run'].includes(args[0])) throw Error('Usage: node tools/compare-bench/run.mjs inspect|prepare|preflight|smoke|run EXTERNAL_CONFIG.json');
  if (process.platform !== 'win32') throw Error('Windows only; analysis/tests are portable');
  const config = JSON.parse(await fs.readFile(args[1], 'utf8'));
  const root = path.resolve(config.output);
  config.output = root;
  if(!outsideRepository(root,repository))throw Error('Results and inputs must stay outside Git');
  await fs.mkdir(root, { recursive: true });
  if (args[0] === 'inspect') {
    const directory = path.join(root, `machine-${randomUUID()}`); await fs.mkdir(directory);
    console.log(JSON.stringify((await windows({ mode: 'machine' }, directory, 'machine', []))[0])); return;
  }
  if (args[0] === 'prepare') { console.log(JSON.stringify(await prepare(config, root))); return; }
  Object.assign(config, { width: config.width ?? 3840, height: config.height ?? 2160, x: config.x ?? 0, y: config.y ?? 0,
    seconds: args[0] === 'smoke' ? 5 : config.seconds ?? 60, warmupSeconds: config.warmupSeconds ?? 5 });
  if (!Number.isInteger(config.seconds) || config.seconds < 3 || config.seconds > 180) throw Error('seconds must be 3..180');
  nativeNetworkMaxSeconds(config);
  if(config.telemetry&&!['on','off'].includes(config.telemetry))throw Error('telemetry must be on or off (diagnostic only)');
  const clients = config.clients ?? ['original', 'native', 'ntl', 'vlither'];
  if (!Array.isArray(clients) || !clients.length || new Set(clients).size !== clients.length || clients.some(c => !['original','native','ntl','vlither'].includes(c))) throw Error('clients must be a unique nonempty subset of original, native, ntl, vlither');
  if(args[0]==='preflight'&&!clients.includes('ntl'))throw Error('preflight requires NTL among the configured clients');
  for (const key of ['width', 'height']) if (!Number.isInteger(config[key]) || config[key] < 320 || config[key] > 3840) throw Error(`${key} must be 320..3840 physical pixels`);
  config.experiment ??= 'primary';
  if (!['primary', 'secondary', 'pixels-optional','igpu-future'].includes(config.experiment)) throw Error('Unknown experiment');
  if (config.experiment === 'primary') {
    if (!/^luid_0x[0-9a-f]{8}_0x[0-9a-f]{8}$/i.test(config.targetAdapterLuid ?? '') || !config.targetDisplay || !config.targetRefreshHz) throw Error('Primary requires the RTX render LUID and DISPLAY2 / 4K / 240 Hz target');
    config.adapter = { ...config.adapter, chromeLuid: config.targetAdapterLuid };
  }
  if (config.experiment === 'secondary' && config.adapter?.chromeLuid && config.adapter.chromeLuid !== 'default') throw Error('Secondary must use default adapter selection for every client');
  for (const client of ['native', 'vlither']) if (config.adapter?.[client] && config.adapter[client] !== 'default') throw Error(`${client} has no adapter selector in this binary/source adapter; no Windows graphics setting is changed. Use a matching Chrome LUID, or owner-managed per-app GPU preferences and verify engine evidence.`);
  const session = path.join(root, `session-${new Date().toISOString().replace(/[:.]/g, '-')}`); await fs.mkdir(session);
  const provenance = await inputs(config, session);
  try {
    const mod = await modInput(config.mod, session); config.mod = mod.directory;
    provenance.ntl = { manifest: JSON.parse(await fs.readFile(path.join(config.mod, 'manifest.json'), 'utf8')), ...mod };
  } catch (error) { provenance.ntl = { error: error.message }; }
  const checkout = path.join(root, 'vlither');
  try { provenance.vlither = { commit: execute('git', ['rev-parse', 'HEAD'], { cwd: checkout }), ref: config.vlitherRef ?? 'latest default branch at prepare',
    dirty: execute('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: checkout }), binarySha256: config.vlitherBinary ? await sha(config.vlitherBinary) : null }; }
  catch (error) { provenance.vlither = { error: error.message }; }
  await save(path.join(session, 'inputs.json'), provenance);
  const machine = (await windows({ mode: 'machine' }, session, 'machine', []))[0];
  if (config.experiment === 'primary') {
    const adapter = machine.dxgiAdapters.find(a => a.luid === config.targetAdapterLuid);
    const display = machine.screens.find(s => s.display === config.targetDisplay);
    if (adapter?.vendorId !== 0x10de || !display || display.bounds[2] !== 3840 || display.bounds[3] !== 2160 || config.targetRefreshHz !== 240||config.width!==3840||config.height!==2160) throw Error('Primary matrix requires RTX rendering and a 3840x2160 client area on the 4K / 240 Hz display');
  }
  const rounds = args[0] === 'smoke' ? 1 : config.rounds ?? 3;
  if (args[0] === 'run' && rounds < 3) throw Error('Real runs require at least three rounds');
  const maxAttempts=config.maxRoundAttempts??3;
  if(!Number.isInteger(maxAttempts)||maxAttempts<1||maxAttempts>3)throw Error('maxRoundAttempts must be 1..3');
  if(config.telemetryControls!==undefined&&typeof config.telemetryControls!=='boolean')throw Error('telemetryControls must be boolean');
  if(clients.includes('ntl')){
    if(provenance.ntl.error||!provenance.ntl.tree?.sha256)throw Error('NTL provenance unavailable; preflight will not load it');
    const {calibrateNtl}=await import('./calibrate.mjs');
    config.ntlViewCalibration=await calibrateNtl(config,session,provenance.ntl);
  }
  if(args[0]==='preflight'){
    const report=config.ntlViewCalibration;
    console.log(JSON.stringify({session,timedRoundsStarted:false,networkVerified:report.network.verified,
      noPacketReachabilityProbeCount:report.network.noPacketReachabilityProbeCount,zoomPlateau:report.plateau,
      displayGate:report.displayGate,readyForTimedRound:report.displayGate?.passed===true}));
    if(report.displayGate?.passed!==true)process.exitCode=2;
    return;
  }
  if(clients.includes('ntl'))requireNtlDisplayGate(config.ntlViewCalibration);
  const plan=roundPlan(rounds,clients,{telemetryControls:args[0]==='run'&&config.telemetryControls!==false&&!config.telemetry,
    limitedNtlView:config.ntlViewCalibration?.wholeMap===false}).map(item=>({...item,telemetry:config.telemetry??item.telemetry}));
  await save(path.join(session,'run-plan.json'),plan);
  const markInterrupted=()=>{interrupted=true;};let collection;
  process.on('SIGINT',markInterrupted);process.on('SIGTERM',markInterrupted);
  try{
    collection=await collectRounds({rounds,maxAttempts,plan,isInterrupted:()=>interrupted,
      preflight:async(round,attempt)=>(await windows({mode:'preflight'},session,`round-${round}-attempt-${attempt}-preflight`,[]))[0],
      acquire:item=>runAttempt({...config,telemetry:item.telemetry},session,item,provenance,machine),
      persist:records=>fs.writeFile(path.join(session,'round-attempts.json'),JSON.stringify(records,null,2)),
      betweenAttempts:()=>delay(5000)});
  }finally{process.removeListener('SIGINT',markInterrupted);process.removeListener('SIGTERM',markInterrupted);}
  const {results,...summary}=collection;
  await save(path.join(session,'round-summary.json'),summary);
  const comparisons = [];
  for (let i = 0; i < results.length; i++) for (let j = i + 1; j < results.length; j++) {
    const a = results[i], b = results[j];
    if (a.run.scenario === b.run.scenario && a.run.round === b.run.round) comparisons.push({ a: a.directory, b: b.directory, ...comparison(a.run, b.run) });
  }
  await save(path.join(session, 'index.json'), { mode: args[0], results: results.map(({run, ...item}) => item), comparisons });
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch(e => { console.error(e.message); process.exitCode = 1; });
