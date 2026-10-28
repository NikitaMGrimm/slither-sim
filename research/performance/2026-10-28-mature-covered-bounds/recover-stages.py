"""Bounded mature validation waves; all hosts wait at barriers before counts."""
import importlib.util,json,os,signal,subprocess,sys,time
from pathlib import Path
r=Path(__file__).resolve().parent;opt=r/'opt';sys.path.insert(0,str(opt));from compare import same_bytes
os.sched_setaffinity(0,{2})
servers=[]
def start(root,name,arms,legacy=0,warm=193000,ticks=360,cpus=None,trace=True,preload=False):
 log=(root/(name+'.log')).open('xb');cmd=['python3',str(root/'serve.py'),name,*arms,'--legacy',str(legacy),'--warm',str(warm),'--ticks',str(ticks)]
 if cpus:cmd+=['--warm-cpus',*map(str,cpus)]
 if not trace:cmd+=['--no-trace']
 if preload:cmd+=['--preload']
 proc=subprocess.Popen(cmd,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
 s={'root':root,'name':name,'path':root/'sessions'/name,'arms':arms,'proc':proc,'log':log,'seq':0};servers.append(s);return s

def ready(*items):
 while True:
  for s in items:assert s['proc'].poll() is None,(s['name'],'warm process failed')
  if all((s['path']/'ready.json').exists() for s in items):break
  time.sleep(1)
 print('WARM BARRIER READY',[s['name'] for s in items],flush=True)

def request(s,arm,label,mode='child',trace=False):
 v={'arm':arm,'label':label,'mode':mode,'trace':trace};p=s['path'];n=s['seq'];s['seq']+=1
 (p/'requests'/f'{n:03}.request').write_text(json.dumps(v))
 while not (p/(label+'.done')).exists():
  assert s['proc'].poll() is None or (p/(label+'.done')).exists(),(s['name'],label,'worker failed')
  time.sleep(.1)
 return json.loads((p/(label+'.json')).read_text())

def proofs(s):
 for arm in ['base','candidate']:
  for i in range(2):request(s,arm,f'proof-{arm}-{i}',trace=True)
 ref=s['path']/'proof-base-0.trace'
 for name in ['proof-base-1.trace','proof-candidate-0.trace','proof-candidate-1.trace']:assert same_bytes(ref,s['path']/name)
 (s['path']/'window-parity.json').write_text('{"completeWindowBytesEqual":true}\n');print('WINDOW PARITY',s['name'],flush=True)

def pairs(s):
 for pair in range(-1,5):
  for arm in (['base','candidate'] if pair<0 or pair%2==0 else ['candidate','base']):request(s,arm,f'pair-{pair}-{arm}')
 for arm in ['base','candidate']:request(s,arm,f'proof-{arm}-parent',mode='parent',trace=True)
 assert s['proc'].wait()==0
 subprocess.run(['python3',str(s['root']/'accept249.py'),str(s['path'])],check=True)
 print('PAIRS ACCEPTED',s['name'],flush=True)

def state_records(path):
 result=[]
 with path.open('rb') as f:
  while h:=f.read(16):
   assert len(h)==16;tag=int.from_bytes(h[:8],'little');n=int.from_bytes(h[8:],'little');data=f.read(n);assert len(data)==n
   if tag==0:result.append(data)
 assert len(result)==2;return result

def mutation(s,reference):
 request(s,'mutation','mutation-window',mode='parent',trace=True);assert s['proc'].wait()==0
 bad=s['path']/'mutation-window.trace';assert not same_bytes(bad,reference)
 assert state_records(bad)==state_records(reference),'coverage mutation changed physics state'
 (s['path']/'mutation.json').write_text('{"detected":true,"physicsStateUnchanged":true}\n');print('MATURE MUTATION DETECTED',s['name'],flush=True)

try:
 assert 'all 12 target-only' in (r/'preload-probes.log').read_text()
 # A supplemental failure must not discard the independent legacy result.
 legacy=start(opt,'index-legacy-retry',['base','candidate'],legacy=1,cpus=[0,1])
 mutant=start(opt,'mutation-legacy-retry',['mutation'],legacy=1,cpus=[2],trace=False)
 libm=start(r,'libm-plain-stock-retry',['base','candidate'],cpus=[2,3],trace=False,preload=True)
 while True:
  for job in [legacy,mutant]:assert job['proc'].poll() is None,(job['name'],'required warm failed')
  required=all((job['path']/'ready.json').exists() for job in [legacy,mutant])
  optional=(libm['path']/'ready.json').exists() or libm['proc'].poll() is not None
  if required and optional:break
  time.sleep(1)
 print('RECOVERY WARM BARRIER READY',flush=True)
 proofs(legacy);mutation(mutant,legacy['path']/'proof-candidate-0.trace');pairs(legacy)
 if libm['proc'].poll() is not None:
  (r/'libm-crosscheck-unavailable.json').write_text(json.dumps({'reason':'supplemental startup failed','exit':libm['proc'].returncode})+'\n')
  print('REQUIRED LEGACY RESULTS ACCEPTED; SUPPLEMENTAL LIBM UNAVAILABLE',flush=True)
 else:
  for arm in ['base','candidate']:
   request(libm,arm,arm+'-window',mode='parent',trace=True)
   assert same_bytes(libm['path']/(arm+'-window.trace'),r/'sessions/saving249-stock'/('proof-'+arm+'-0.trace'))
  assert libm['proc'].wait()==0
  subprocess.run(['python3',str(r/'attribute.py'),str(r),'libm-plain-stock-retry','--plain'],check=True)
  tagged=json.loads((r/'sessions/libm-stock/callers-selected.json').read_text())
  plain=json.loads((libm['path']/'callers-selected.json').read_text());checks=[]
  for arm in ['base','candidate']:
   a=tagged[arm+'-profile']['calls'];b=plain[arm]['calls']
   for fn in sorted({v['function'] for v in a+b}):
    x=[v for v in a if v['function']==fn];y=[v for v in b if v['function']==fn]
    assert sum(v['calls'] for v in x)==sum(v['calls'] for v in y),(arm,fn,'call count')
    ix=sum(v['calleeInstructions'] for v in x);iy=sum(v['calleeInstructions'] for v in y)
    delta=(ix-iy)/iy if iy else 0
    assert abs(delta)<.00002,(arm,fn,delta)
    checks.append({'arm':arm,'function':fn,'callsMatch':True,'calls':sum(v['calls'] for v in x),'taggedInstructions':ix,'plainInstructions':iy,'relativeInstructionDifference':delta})
  (r/'libm-crosscheck.json').write_text(json.dumps(checks,indent=2)+'\n')
  print('PLAIN/TAGGED LIBM CROSSCHECK PASS',flush=True)
 print('MATURE INDEX VALIDATION COMPLETE',flush=True)
except BaseException:
 for job in servers:
  if job['proc'].poll() is None:
   path=job['path']/'pids.json'
   if path.exists():
    for pid in json.loads(path.read_text()).values():
     try:
      if (Path('/proc')/str(pid)/'exe').resolve().is_relative_to(job['root']):os.killpg(pid,signal.SIGKILL)
     except (FileNotFoundError,ProcessLookupError):pass
   job['proc'].terminate();job['proc'].wait()
 raise
finally:
 for job in servers:job['log'].close()
