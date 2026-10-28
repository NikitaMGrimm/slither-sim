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
 assert json.loads((opt/'required-checks-pass.json').read_text())['passed']
 assert 'CATALOG/HOST PARITY AND MUTATION PASS' in (opt/'verify-parity.log').read_text()
 # All binaries/builds/checks/traces complete before any plain pair.
 subprocess.run(['python3',str(opt/'short-pairs.py')],check=True)
 for root,name in [(r,'matched-short249'),(opt,'matched-short-index')]:
  s=start(root,name,['base','candidate'],warm=0,ticks=1080,cpus=[0,1]);ready(s);proofs(s);pairs(s)
 # Validate the exact live-stream equality routine with a known byte flip.
 for legacy in [0,1]:
  name='stream-control-'+str(legacy)
  with (opt/(name+'.log')).open('xb') as log:
   p=subprocess.run(['python3',str(opt/'serve.py'),name,'base','candidate','--legacy',str(legacy),'--warm','120','--ticks','1','--perturb-stream'],stdout=log,stderr=subprocess.STDOUT)
  assert p.returncode!=0 and json.loads((opt/'sessions'/name/'mismatch.json').read_text())['offset']==0
 print('STREAM CONTROLS DETECTED',flush=True)
 # Wave 1: four active warmers. Nothing counts until all four are ready.
 stock=start(opt,'index-stock',['base','candidate'],cpus=[0,1])
 legacy249=start(r,'saving249-legacy',['base','candidate'],legacy=1,cpus=[2,3])
 ready(stock,legacy249);proofs(stock);proofs(legacy249);pairs(legacy249)
 # Keep the index-stock parents unchanged while preparing its mature control.
 legacy=start(opt,'index-legacy',['base','candidate'],legacy=1,cpus=[0,1])
 mutant_stock=start(opt,'mutation-stock',['mutation'],cpus=[2],trace=False)
 profile=start(opt,'index-profile',['candidate-profile'],cpus=[3],trace=False)
 ready(legacy,mutant_stock,profile);proofs(legacy)
 mutation(mutant_stock,stock['path']/'proof-candidate-0.trace')
 request(profile,'candidate-profile','profile-window',mode='parent',trace=True);assert profile['proc'].wait()==0
 assert same_bytes(profile['path']/'profile-window.trace',stock['path']/'proof-candidate-0.trace')
 (profile['path']/'accepted.json').write_text('{"completeWindowBytesEqualPlainReference":true}\n')
 pairs(stock)
 # Wave 3: remaining legacy control and a plain-binary libm cross-check.
 mutant_legacy=start(opt,'mutation-legacy',['mutation'],legacy=1,cpus=[0],trace=False)
 plain_libm=start(r,'libm-plain-stock',['base','candidate'],cpus=[1,2],trace=False,preload=True)
 ready(mutant_legacy,plain_libm)
 mutation(mutant_legacy,legacy['path']/'proof-candidate-0.trace');pairs(legacy)
 for arm in ['base','candidate']:
  request(plain_libm,arm,arm+'-window',mode='parent',trace=True)
  assert same_bytes(plain_libm['path']/(arm+'-window.trace'),r/'sessions/saving249-stock'/('proof-'+arm+'-0.trace'))
 assert plain_libm['proc'].wait()==0
 subprocess.run(['python3',str(r/'attribute.py'),str(r),'libm-plain-stock','--plain'],check=True)
 print('ALL MATURE INDEX STAGES COMPLETE',flush=True)
except BaseException:
 # Kill only the dedicated task process groups, including a live fork child.
 for s in servers:
  if s['proc'].poll() is None:
   p=s['path']/'pids.json'
   if p.exists():
    for pid in json.loads(p.read_text()).values():
     try:
      exe=(Path('/proc')/str(pid)/'exe').resolve()
      if exe.is_relative_to(s['root']):os.killpg(pid,signal.SIGKILL)
     except (FileNotFoundError,ProcessLookupError):pass
   s['proc'].terminate();s['proc'].wait()
 raise
finally:
 for s in servers:s['log'].close()
