"""Integrated-head exactness checks only; no timing request path."""
from pathlib import Path
import subprocess,sys,json,time,hashlib,shutil
r=Path(__file__).resolve().parent

def run(label,args,cwd=None):
 with (r/(label+'.log')).open('x') as out:
  p=subprocess.run(args,cwd=cwd or r,stdout=out,stderr=subprocess.STDOUT)
 assert p.returncode==0,(label,p.returncode)
 print(label,'PASS',flush=True)

run('pipeline',[sys.executable,str(r/'pipeline.py')])
run('documentation-tests',[sys.executable,'-B','-m','unittest','discover','-s','tools','-p','test_documentation.py'],r/'checks/source')
for arm in ['base','candidate','mutation']:
 pins=json.loads((r/arm/'pins.json').read_text())
 for name,digest in pins['binaries'].items():
  with (r/arm/'bin'/name).open('rb') as f:assert hashlib.file_digest(f,'sha256').hexdigest()==digest
target=Path('/tmp/slither-compact-history-target')
assert target.resolve()==target and target.is_dir() and not target.is_symlink()
shutil.rmtree(target)
(r/'target-cleanup.json').write_text(json.dumps({'allCopiedBinariesVerified':True,'targetRemoved':True})+'\n')

# Reuse the prior proof validator, removing every timing path from this copy.
p=r/'compact-cohort.py';text=p.read_text()
text=text.replace("choices=['proof','time','finish','supersede']","choices=['proof','close']")
start=text.index("elif a.phase=='time':");end=text.index('\nelse:\n',start)
text=text[:start]+text[end:]
text=text.replace("assert (directory/'TIME-DONE').exists() if a.phase=='finish' else not (directory/'TIME-DONE').exists()","assert not (directory/'TIME-DONE').exists()")
text=text.replace("'timedPairs':5 if a.phase=='finish' else 0,'discardedAB':1 if a.phase=='finish' else 0","'timedPairs':0,'discardedAB':0")
text=text.replace(" if a.phase=='supersede':outcome['reason']='Main runtime advanced before any primary timing; preserved proofs, refreshed both arms.'\n",'')
text=text.replace("('accepted.json' if a.phase=='finish' else 'superseded.json')","'spot-accepted.json'")
assert "a.phase=='time'" not in text and "range(-1,5)" not in text
p.write_text(text)

name='adoption-stock';directory=r/'sessions'/name
command=[sys.executable,str(r/'serve.py'),name,'base','candidate','--legacy','0','--warm','193000','--ticks','144']
with (r/(name+'.log')).open('x') as log:
 server=subprocess.Popen(command,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
(r/'spot-controller.json').write_text(json.dumps({'pid':server.pid,'command':command})+'\n')
while not (directory/'ready.json').exists():
 assert server.poll() is None,'warm proof failed'
 time.sleep(.5)
run('spot-child-proofs',[sys.executable,str(p),name,'proof'])
run('spot-parent-proofs',[sys.executable,str(p),name,'close'])
assert server.wait(timeout=30)==0
(r/'ADOPTION-CHECKS-DONE').write_text('No timing requests were made.\n')
print('ALL INTEGRATED ADOPTION CHECKS PASS / NO NEW TIMING',flush=True)
