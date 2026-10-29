"""Received-frame A4 ladder across private interfaces; no checkpoints."""
from pathlib import Path
import os,json,time,subprocess,signal,urllib.request,ipaddress,tarfile,hashlib
from metrics import processes,prior
from window import reduce_window
r=Path(__file__).resolve().parent;job=json.loads((r/'job.json').read_text());root=r/'load-run';root.mkdir();os.sched_setaffinity(0,{7})
assert ipaddress.ip_address(job['serverPrivate']).is_private
base=f"http://{job['serverPrivate']}:{job['controlPort']}";target=350
def write(name,value):
 p=root/(name+'.tmp');p.write_text(str(value)+'\n');p.replace(root/name)
write('tick',0);write('target',target)
summary={'runtime':'692cd561714b8af3ea95adb3adcbd807b3a48734','status':'running','initialBots':350,'maxBotSlots':750,'humanSlotReserved':True,'windows':[]}
def save(phase,**extra):
 summary['phase']=phase;summary.update(extra);(r/'run-status.json').write_text(json.dumps(summary)+'\n')
def api(path,body=None):
 data=None if body is None else json.dumps(body).encode();request=urllib.request.Request(base+path,data=data,headers={} if body is None else {'Content-Type':'application/json'})
 with urllib.request.urlopen(request,timeout=10) as response:
  raw=response.read();return json.loads(raw) if path!='/bundle' else raw
log=(root/'client.log').open('xb');raw=(root/'host-stream.log').open('xb');telemetry=(root/'telemetry.jsonl').open('x');stats=(root/'stats.jsonl').open('x')
client=None;offset=0;pending=b'';last_sample=0;last_tick=0;census=[];client_rows=[];client_offset=0;last_progress=0
def pump():
 global offset,pending,last_sample,last_tick,client_offset,last_progress
 assert client is not None and client.poll() is None,'client process exited'
 with urllib.request.urlopen(base+f'/log?offset={offset}',timeout=10) as response:
  data=response.read();assert response.headers['X-Host-Alive']=='1','host exited'
 offset+=len(data);raw.write(data);raw.flush();pending+=data
 lines=pending.split(b'\n');pending=lines.pop()
 for data in lines:
  line=data.decode()
  if line.startswith('CENSUS '):
   value=json.loads(line[7:]);value.pop('tickNanos');last_tick=value['tick'];write('tick',last_tick)
   if last_tick%500==0:census.append(value)
   if last_progress==0 or last_tick-last_progress>=5000:
    last_progress=last_tick;save(summary['phase'],tick=last_tick,census=value);print('PROGRESS',summary['phase'],last_tick,flush=True)
  elif 'stats: ticks/s=' in line:
   v=prior.parse_stats(line)
   if v is not None:v['unix']=time.time();stats.write(json.dumps(v)+'\n');stats.flush()
 with (root/'client.log').open() as stream:
  stream.seek(client_offset)
  while True:
   at=stream.tell();line=stream.readline()
   if not line.endswith('\n'):client_offset=at;break
   if line.startswith('CLIENT '):client_rows.append(json.loads(line[7:]))
 if time.monotonic()-last_sample>=1:
  snap=processes.snapshot(client.pid);assert all(set(t['cpus'])<={0,1,2,3,4,5,7} for t in snap['threads'])
  if time.monotonic()-started>8:assert len([t for t in snap['threads'] if t['comm'].startswith('capacity-bot-')])==6,'client worker exited'
  telemetry.write(json.dumps({'client':snap})+'\n');telemetry.flush();last_sample=time.monotonic()
def wait(seconds):
 until=time.monotonic()+seconds
 while time.monotonic()<until:pump();time.sleep(.2)
def screen(v):return v['living']>=343 and 45<=v['meanSegments']<=65 and 156<=v['p90Segments']<=196 and v['p99Segments']>=200
try:
 started=time.monotonic();client=subprocess.Popen([str(r/'bin/clients'),str(root),str(job['gamePort']),str(target),job['serverPrivate']],stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
 summary['clientPid']=client.pid;save('warmup',tick=0)
 while True:
  pump()
  if last_tick>=193000 and len(census)>=3 and all(screen(v) for v in census[-3:]):summary['matureScreen']=census[-3:];break
  assert time.monotonic()-started<2700,'mature screen deadline reached'
  time.sleep(.2)
 save('ladder');previous=target-50;stop_reason=None
 while True:
  write('target',target);save('settling',bots=target);wait(10)
  recent=[v for v in client_rows if v['unix']>=time.time()-3]
  assert {v['worker'] for v in recent}==set(range(1,7)) and all(v['target']==target for v in recent),'new target not admitted'
  label=f'up-{target}';window={'label':label,'bots':target,'start':api('/',{'command':'measure','label':label,'active':True})};save('counting',bots=target)
  wait(30);window['end']=api('/',{'command':'measure','label':label,'active':False});wait(1.5)
  result=reduce_window(root,window);summary['windows'].append(window|{'result':result});save('ladder',lastResult=result)
  print('WINDOW',target,'valid',result['clientValid'],'median',result['worldCpuMsPerTick']['p50'] if result['clientValid'] else 'withheld',flush=True)
  if not result['clientValid']:stop_reason='client-or-queue-or-population-limit';break
  if result['deferredTicks']:stop_reason='deferred-ticks';break
  if result['worldCpuMsPerTick']['p50']>=6.8:stop_reason='target-median-reached';break
  if target>=750:stop_reason='predeclared-slot-ceiling';break
  previous=target;target+=25 if result['worldCpuMsPerTick']['p50']>=6 else 50
 if previous is not None:
  target=previous;write('target',target);save('confirm-settling',bots=target);wait(10)
  label=f'down-{target}';window={'label':label,'bots':target,'start':api('/',{'command':'measure','label':label,'active':True})};save('confirm-counting',bots=target)
  wait(30);window['end']=api('/',{'command':'measure','label':label,'active':False});wait(1.5);result=reduce_window(root,window);summary['windows'].append(window|{'result':result})
 summary.update(status='complete',stopReason=stop_reason);save('complete')
except BaseException as e:
 summary.update(status='failed',failureType=type(e).__name__,failure=str(e));save('failed');raise
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
 except BaseException as e:summary['retrievalFailure']=type(e).__name__+': '+str(e)
 (r/'run-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
 with tarfile.open(r/'results.tar.gz','w:gz') as archive:
  for p in [root,r/'host-run',r/'run-summary.json',r/'pins.json']:
   if p.exists():archive.add(p,arcname=p.name)
 (r/'results.sha256').write_text(hashlib.file_digest((r/'results.tar.gz').open('rb'),'sha256').hexdigest()+'\n')
 (r/'run-status.json').write_text(json.dumps(summary)+'\n')
