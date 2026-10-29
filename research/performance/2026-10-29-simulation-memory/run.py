import subprocess,time,json,os,pwd,hashlib
from pathlib import Path
r=Path(__file__).resolve().parent
session=json.loads((r/'session.json').read_text())
def remaining():return session['measurementStopUnix']-time.time()
def inventory(name):
 commands=subprocess.check_output(['ps','-eo','pid,ppid,stat,pcpu,args','-ww'],text=True)
 (r/(name+'-processes.txt')).write_text(commands.replace(pwd.getpwuid(os.getuid()).pw_name,'<redacted-account>'))
def line(proc,log):
 text=proc.stderr.readline();assert text,(proc.poll(),log)
 with log.open('a') as f:f.write(text)
 return text
while not (r/'build.json').exists():
 assert remaining()>600,'build deadline'
 if (r/'build-driver.log').exists():assert 'Traceback' not in (r/'build-driver.log').read_text()
 time.sleep(2)
exe=r/'study_host'
results=[]
for legacy in [0,1]:
 assert remaining()>600,'warm deadline'
 label='legacy' if legacy else 'stock'
 log=r/(label+'-parent.log')
 inventory(label+'-warm-before')
 proc=subprocess.Popen([str(exe),str(legacy),'193000','1080','0'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
 (r/(label+'-parent.pid')).write_text(str(proc.pid))
 try:
  while True:
   text=line(proc,log)
   if '"ready":true' in text:break
   if remaining()<300:raise RuntimeError('warm cutoff')
  footprint=line(proc,log);assert footprint.startswith('FOOTPRINT ')
  (r/(label+'-footprint.json')).write_text(footprint.removeprefix('FOOTPRINT '))
  inventory(label+'-warm-after')
  jobs=[(event,rep,False) for rep in range(3) for event in [1,2,3,5,6,7]]+[(event,0,True) for event in [1,3,5,6]]
  for event,rep,sample in jobs:
   assert remaining()>120,'measurement cutoff'
   name=f'{label}-e{event}-r{rep}'+('-sample' if sample else '')
   inventory(name+'-before')
   path=r/(name+'.json');start=time.time()
   proc.stdin.write(f'child {path} {event}'+(' sample' if sample else '')+'\n');proc.stdin.flush()
   while not line(proc,log).startswith('DONE '):pass
   inventory(name+'-after')
   data=json.loads(path.read_text());rows=data['counts']['rows'];assert len(rows)==1080
   assert all(v[1]>0 and v[2]/v[1]>=.99 for row in rows for v in row['counters']),'multiplexing'
   result={'name':name,'seconds':time.time()-start,'sha256':hashlib.file_digest(path.open('rb'),'sha256').hexdigest(),'maintenanceTicks':sum(x['maintenance'] for x in rows)}
   results.append(result);(r/'runs.json').write_text(json.dumps(results,indent=2));print(name,flush=True)
  proc.stdin.write('quit\n');proc.stdin.flush();assert proc.wait(timeout=20)==0
 finally:
  if proc.poll() is None:proc.terminate();proc.wait(timeout=20)
(r/'DONE').write_text(str(time.time()))
