"""One natural Battledome world, private-network A4 sockets, then human hold."""
from pathlib import Path
import os,json,time,subprocess,signal,urllib.request,tarfile,hashlib
from metrics import processes,prior
from reduce import reduce_window
from delivery import publish
r=Path(__file__).resolve().parent;job=json.loads((r/'job.json').read_text());root=r/'load-run';root.mkdir();os.sched_setaffinity(0,{7})
base=f"http://{job['serverPrivate']}:{job['controlPort']}"
def write(name,value):
 p=root/(name+'.tmp');p.write_text(str(value)+'\n');p.replace(root/name)
write('tick',0);write('target',80)
summary={'runtime':'165a5bb764aba6cbf39f42503e090f9b5e3c4d22','status':'running','windows':[],'clientWholeReports':True}
def save(phase,**extra):
 summary['phase']=phase;summary.update(extra);p=r/'run-status.tmp';p.write_text(json.dumps(summary)+'\n');p.replace(r/'run-status.json')
def api(path,body=None):
 data=None if body is None else json.dumps(body).encode()
 with urllib.request.urlopen(urllib.request.Request(base+path,data=data,headers={} if body is None else {'Content-Type':'application/json'}),timeout=10) as response:
  raw=response.read();return raw if path=='/bundle' else json.loads(raw)
log=(root/'client.log').open('xb');raw=(root/'host-stream.log').open('xb');telemetry=(root/'telemetry.jsonl').open('x');stats=(root/'stats.jsonl').open('x')
client=None;offset=0;pending=b'';last_sample=0;last_tick=0;last_progress=0
def pump():
 global offset,pending,last_sample,last_tick,last_progress
 assert client.poll() is None,'client exited'
 with urllib.request.urlopen(base+f'/log?offset={offset}',timeout=10) as response:
  data=response.read();assert response.headers['X-Host-Alive']=='1','host exited'
 offset+=len(data);raw.write(data);raw.flush();pending+=data;lines=pending.split(b'\n');pending=lines.pop()
 for data in lines:
  line=data.decode()
  if line.startswith('CENSUS '):
   v=json.loads(line[7:]);last_tick=v['tick'];write('tick',last_tick)
   if last_tick-last_progress>=1000:last_progress=last_tick;save(summary['phase'],tick=last_tick,census={k:v[k] for k in ['living','occupied','food','meanSegments','p90Segments','radius','boostFood','boostTotals']})
  elif 'stats: ticks/s=' in line:
   v=prior.parse_stats(line)
   if v is not None:v['unix']=time.time();stats.write(json.dumps(v)+'\n');stats.flush()
 if time.monotonic()-last_sample>=1:
  snap=processes.snapshot(client.pid);assert all(set(t['cpus'])<={0,1,2,3,4,5,7} for t in snap['threads'])
  if time.monotonic()-started>10:assert len([t for t in snap['threads'] if t['comm'].startswith('capacity-bot-')])==6
  telemetry.write(json.dumps({'client':snap})+'\n');telemetry.flush();last_sample=time.monotonic()
def wait(seconds):
 until=time.monotonic()+seconds
 while time.monotonic()<until:pump();time.sleep(.2)
def window(target,label,settle):
 write('target',target);save('settling',bots=target);wait(settle)
 (root/(label+'-generator-start.txt')).write_text(subprocess.check_output(['ps','-eo','pid,ppid,stat,pcpu,args','-ww'],text=True))
 write('RESET',label)
 w={'label':label,'bots':target,'start':api('/',{'command':'measure','label':label,'active':True})};save('counting',bots=target)
 wait(90);w['end']=api('/',{'command':'measure','label':label,'active':False});wait(2)
 (root/(label+'-generator-end.txt')).write_text(subprocess.check_output(['ps','-eo','pid,ppid,stat,pcpu,args','-ww'],text=True))
 w['result']=reduce_window(root,w,whole_client_reports=True);summary['windows'].append(w);save('ladder',lastResult=w['result']);print('WINDOW',label,json.dumps(w['result']),flush=True)
 return w['result']
try:
 started=time.monotonic();client=subprocess.Popen([str(r/'bin/clients'),str(root),str(job['gamePort']),'80',job['serverPrivate']],stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
 save('warmup',tick=0,bots=80);wait(300)
 for target in [80,120,160,240,320,480,640]:
  result=window(target,f'up-{target}',60)
  reasons=[]
  if result['deferredTicks']:reasons.append('deferred-ticks')
  if result['worldCpuMsPerTick']['p50']>=6.8:reasons.append('cpu-median-boundary')
  if result['alive']['mean']<target*.9:reasons.append('spawn-population-saturation')
  if result['spawns']['pendingMaxMs']>=30000:reasons.append('pending-login-30s')
  if result['client']['frameExcessMs']['p99']>100:reasons.append('frame-gap-p99')
  if not result['clientValid']:reasons.append('generator-or-queue-limit')
  if reasons:summary['stopReason']=reasons;break
 else:summary['stopReason']=['predeclared-640-ceiling']
 window(80,'down-80',30)
 summary.update(holdSeconds=0,holdStop='skipped-on-maintaining-agent-instruction',status='stopping',outcome='complete');save('finalizing')
except BaseException as e:
 summary.update(status='stopping',outcome='failed',failureType=type(e).__name__,failure=str(e));save('finalizing');raise
finally:
 if client is not None and client.poll() is None:
  os.killpg(client.pid,signal.SIGTERM)
  try:client.wait(timeout=10)
  except subprocess.TimeoutExpired:os.killpg(client.pid,signal.SIGKILL);client.wait()
 telemetry.close();raw.close();stats.close();log.close()
 try:
  api('/',{'command':'stop'})
  for _ in range(40):
   if api('/status').get('stopped'):break
   time.sleep(.25)
  else:raise RuntimeError('host stop not acknowledged')
  payload=api('/bundle');(r/'host-results.tar.gz').write_bytes(payload)
  with tarfile.open(r/'host-results.tar.gz') as archive:archive.extractall(r,filter='data')
  summary['hostBundleSha256']=hashlib.sha256(payload).hexdigest()
 except BaseException as e:
  summary.update(retrievalFailure=str(e),status='artifact-error');save('artifact-error');raise
 publish(r,summary,[root,r/'host-run',r/'run-summary.json',r/'pins.json'])
