"""Live exact warm-stream comparison, then sequential commands to frozen parents.

Raw warm streams are compared byte for byte without storing a disk-sized trace.
Every measured child opens its own current-thread counter. Do not run builds
or other timing jobs while requests are being processed.
"""
import argparse,hashlib,importlib.util,json,os,resource,selectors,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('session');p.add_argument('arms',nargs='+');p.add_argument('--legacy',type=int,default=0);p.add_argument('--warm',type=int,default=193000);p.add_argument('--ticks',type=int,default=360);p.add_argument('--no-trace',action='store_true');p.add_argument('--preload',action='store_true');p.add_argument('--warm-cpus',type=int,nargs='+');p.add_argument('--perturb-stream',action='store_true');a=p.parse_args()
root=Path(__file__).resolve().parent;session=root/'sessions'/a.session;session.mkdir(parents=True);requests=session/'requests';requests.mkdir()
spec=importlib.util.spec_from_file_location('shared',root/'shared/reproduce.py');shared=importlib.util.module_from_spec(spec);spec.loader.exec_module(shared)
os.sched_setaffinity(0,{2});sel=selectors.DefaultSelector();procs={};logs={};buffers={};errbuf={};ready={};total={};digests={};paused=set();expected_exits=set();binary_hashes={}
trace=not a.no_trace;matched=0;perturbed=False;controller_cpu_start=time.process_time()
for i,arm in enumerate(a.arms):
 exe=root/arm/'bin/study_host';pin=json.loads((root/arm/'pins.json').read_text());assert shared.sha(exe)==pin['binaries']['study_host']
 binary_hashes[arm]=pin['binaries']['study_host']
 env=dict(os.environ,RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1')
 assert not env.get('LD_PRELOAD'), 'start the study without unrelated interposition'
 # Inject into the simulator only. Preloading taskset also runs the shim's
 # constructor in the launcher and can leave an extra PMU fd across exec.
 preload=['env','LD_PRELOAD='+str(root/'libstudy.so'),'STUDY_LIBM='+str(session/(arm+'-parent.csv'))] if a.preload else []
 cmd=['taskset','-c',str(a.warm_cpus[i] if a.warm_cpus else i),*preload,str(exe),str(a.legacy),str(a.warm),str(a.ticks),str(int(trace))]
 shared.inventory(session,arm+'-warm-before')
 proc=subprocess.Popen(cmd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,start_new_session=True)
 procs[arm]=proc;logs[arm]=(session/(arm+'.stderr')).open('xb');buffers[arm]=bytearray();errbuf[arm]=bytearray();total[arm]=0;digests[arm]=hashlib.sha256()
 for kind,pipe in [('out',proc.stdout),('err',proc.stderr)]:os.set_blocking(pipe.fileno(),False);sel.register(pipe,selectors.EVENT_READ,(arm,kind))
(session/'pids.json').write_text(json.dumps({k:v.pid for k,v in procs.items()},indent=2)+'\n')

def pump(timeout=.2):
 global matched,perturbed
 for key,_ in sel.select(timeout):
  arm,kind=key.data;data=os.read(key.fd,1<<20)
  if not data:
   sel.unregister(key.fileobj)
   if arm not in expected_exits and procs[arm].poll() is not None:raise RuntimeError((arm,'unexpected exit',procs[arm].returncode))
   continue
  if kind=='err':
   logs[arm].write(data);logs[arm].flush();errbuf[arm].extend(data)
   while b'\n' in errbuf[arm]:
    line,_,rest=errbuf[arm].partition(b'\n');errbuf[arm]=bytearray(rest)
    if line.startswith(b'{'):
     value=json.loads(line)
     if value.get('ready'):ready[arm]=value;shared.inventory(session,arm+'-warm-after')
  else:
   if a.perturb_stream and arm==a.arms[-1] and not perturbed:
    data=bytes([data[0]^1])+data[1:];perturbed=True
   buffers[arm].extend(data);total[arm]+=len(data);digests[arm].update(data)
   if len(buffers[arm])>=8<<20:sel.unregister(procs[arm].stdout);paused.add(arm)
 if len(a.arms)==2:
  left,right=(buffers[arm] for arm in a.arms);n=min(len(left),len(right))
  if memoryview(left)[:n]!=memoryview(right)[:n]:
   offset=next(i for i in range(n) if left[i]!=right[i])
   (session/'mismatch-left.bin').write_bytes(left[:n]);(session/'mismatch-right.bin').write_bytes(right[:n])
   (session/'mismatch.json').write_text(json.dumps({'offset':matched+offset,'matchedPrefixBytes':matched,'readBytes':total},indent=2)+'\n')
   raise AssertionError(('full warm trace mismatch',matched+offset))
  matched+=n
  del left[:n];del right[:n]
 else:
  for b in buffers.values():b.clear()
 for arm in list(paused):
  if len(buffers[arm])<4<<20:sel.register(procs[arm].stdout,selectors.EVENT_READ,(arm,'out'));paused.remove(arm)

try:
 while True:
  pump()
  if len(ready)==len(a.arms):
   # READY is emitted only after flushing all warm stdout writes. Drain each
   # pipe to EAGAIN, then require no unmatched bytes.
   for _ in range(3):pump(.05)
   assert not any(buffers.values()),('unmatched warm tail',total)
   break
  if any(proc.poll() is not None for proc in procs.values()):raise RuntimeError('warmup process failed')
 if len(a.arms)==2:assert len(set(total.values()))==1 and len({v.hexdigest() for v in digests.values()})==1
 receipt={'ready':ready,'warmTraceComparedEveryByte':trace and len(a.arms)==2,'traceBytes':total,'traceSha256':{k:v.hexdigest() for k,v in digests.items()},'windowTicks':a.ticks,'warmTicks':a.warm}
 for proc in procs.values():os.sched_setaffinity(proc.pid,{0})
 receipt['countCpu']=0
 (session/'ready.json').write_text(json.dumps(receipt,indent=2)+'\n');print('READY',a.session,json.dumps({k:v['summary']['census'] for k,v in ready.items()}),flush=True)
 processed=set()
 while True:
  pending=sorted(f for f in requests.glob('*.request') if f.name not in processed)
  if not pending:time.sleep(.1);continue
  for request in pending:
   v=json.loads(request.read_text());arm=v['arm'];proc=procs[arm];label=v['label'];path=session/(label+'.json');assert not path.exists()
   assert shared.sha(root/arm/'bin/study_host')==binary_hashes[arm]
   shared.inventory(session,label+'-before');start=time.monotonic()
   cmd=f"{v.get('mode','child')} {path}"+(' trace' if v.get('trace') else '')+(' libm' if v.get('libm') else '')+'\n'
   if v.get('mode')=='parent':expected_exits.add(arm)
   proc.stdin.write(cmd.encode());proc.stdin.flush()
   while not path.exists():
    pump(.05)
    if proc.poll() is not None:raise RuntimeError((arm,'window exited before result'))
    assert time.monotonic()-start<1200,'window timeout'
   # JSON is written atomically by write_all before completion marker. Wait
   # for DONE or process exit so no next request overlaps a live child.
   marker=('DONE '+str(path)).encode()
   while marker not in (session/(arm+'.stderr')).read_bytes():
    try:pump(.05)
    except RuntimeError:
     if v.get('mode')=='parent' and proc.poll()==0:break
     raise
   shared.inventory(session,label+'-after')
   assert shared.sha(root/arm/'bin/study_host')==binary_hashes[arm]
   result=json.loads(path.read_text());assert result['counts']['calls'][2]==a.ticks
   (session/(label+'.done')).write_text(json.dumps({'request':v,'wallSeconds':time.monotonic()-start})+'\n');processed.add(request.name)
   print('DONE',label,result['counts']['instructions'][2],flush=True)
   if v.get('mode')=='parent':
    assert proc.wait(timeout=10)==0,(arm,'direct parent reference failed',proc.returncode)
    for pipe in [proc.stdout,proc.stderr]:
     try:sel.unregister(pipe)
     except KeyError:pass
    del procs[arm]
  if not procs:break
except BaseException:
 for proc in procs.values():
  try:os.killpg(proc.pid,9)
  except ProcessLookupError:pass
  proc.wait()
 raise
finally:
 for log in logs.values():log.close()
 usage=resource.getrusage(resource.RUSAGE_CHILDREN)
 (session/'cpu-receipt.json').write_text(json.dumps({'waitedHostAndForkCpuSeconds':usage.ru_utime+usage.ru_stime,'controllerCpuSeconds':time.process_time()-controller_cpu_start},indent=2)+'\n')
