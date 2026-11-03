import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawn} from 'node:child_process';
import {isIP} from 'node:net';

const here=path.dirname(fileURLToPath(import.meta.url));
const loopback=address=>typeof address==='string'&&(isIP(address)===4?address.startsWith('127.'):isIP(address)===6&&(address==='::1'||/^::ffff:127\./i.test(address)));
export function nativeNetworkMaxSeconds({seconds,warmupSeconds}) {
  if(!Number.isFinite(warmupSeconds)||warmupSeconds<0)throw Error('warmupSeconds must be finite and nonnegative');
  // Collection starts at launch, before readiness and the configured warmup.
  return seconds+warmupSeconds+240;
}
export function nativeNetworkEvidence(samples,details={}) {
  const rejectedEndpoints=[];
  for(const sample of samples){
    for(const [protocol,endpoints] of [['tcp',sample.tcp??[]],['udp',sample.udp??[]]])for(const endpoint of endpoints){
      const remoteAbsent=endpoint.remoteAddress===null||endpoint.remoteAddress===undefined
        ||(protocol==='tcp'&&['Listen','Bound','Closed'].includes(endpoint.state)&&['0.0.0.0','::'].includes(endpoint.remoteAddress));
      if(!loopback(endpoint.localAddress)||(!remoteAbsent&&!loopback(endpoint.remoteAddress)))rejectedEndpoints.push({protocol,atUnixMs:sample.atUnixMs,...endpoint});
    }
  }
  const observed=samples.some(s=>s.pids?.length&&s.tcpCollected&&s.udpCollected);
  return {...details,method:'periodic owned-process-tree TCP and UDP endpoint samples',complete:false,
    status:rejectedEndpoints.length?'non-loopback-observed':observed?'sampled-loopback-only':'unavailable',
    limitations:['Connections and children that live between polls may be missed.','Windows UDP endpoint tables expose local bindings, not per-datagram destinations.'],
    firstSampleUnixMs:samples[0]?.atUnixMs,lastSampleUnixMs:samples.at(-1)?.atUnixMs,
    sampleErrors:samples.flatMap(s=>s.errors??[]),rejectedEndpoints,samples};
}
export async function startNativeNetwork({pid,executable,directory,maxSeconds,startedUnixMs}) {
  const stopFile=path.join(directory,'native-network-stop');
  const configFile=path.join(directory,'native-network-config.json');
  await fs.writeFile(configFile,JSON.stringify({pid,executable:path.resolve(executable),stopFile,maxSeconds,intervalMs:1000}),{flag:'wx'});
  const child=spawn('powershell.exe',['-NoProfile','-File',path.join(here,'native-network.ps1'),'-Config',configFile],{windowsHide:true});
  const samples=[],errors=[];let pending='',stderr='',overflow=false;
  child.stdout.on('data',bytes=>{
    pending+=bytes;let end;
    while((end=pending.indexOf('\n'))>=0){const line=pending.slice(0,end);pending=pending.slice(end+1);if(!line.trim())continue;
      try{const value=JSON.parse(line);if(samples.length<2048)samples.push(value);else overflow=true;}catch(error){errors.push(error.message);}
    }
  });
  child.stderr.on('data',bytes=>{if(stderr.length<65536)stderr+=bytes;});
  const done=new Promise(resolve=>{child.once('error',error=>{errors.push(error.message);resolve(null);});child.once('close',resolve);});
  return {child,async finish(){
    try{await fs.writeFile(stopFile,'stop\n',{flag:'wx'});}catch(error){errors.push(error.message);if(child.exitCode===null)child.kill();}
    let timer;
    const code=await Promise.race([done,new Promise(resolve=>{timer=setTimeout(()=>resolve('timeout'),10000);})]);clearTimeout(timer);
    if(code==='timeout'){child.kill();errors.push('Network sampler did not stop within ten seconds');}
    else if(code!==0)errors.push(`Network sampler exit ${code}`);
    if(stderr)errors.push(stderr);if(overflow)errors.push('Network sample bound exceeded');
    const observation=nativeNetworkEvidence(samples,{startedUnixMs,stoppedUnixMs:Date.now(),requestedIntervalMs:1000,errors});
    if(errors.length)observation.status=observation.rejectedEndpoints.length?'non-loopback-observed':'unavailable';
    await fs.writeFile(path.join(directory,'native-network.json'),JSON.stringify(observation,null,2));return observation;
  }};
}
