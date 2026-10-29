from pathlib import Path
import subprocess,sys,json,hashlib,shutil
r=Path(__file__).resolve().parent
for name in ['pipeline.py','compact-stream-control.py']:
 print('START',name,flush=True)
 p=subprocess.run([sys.executable,str(r/name)]);assert p.returncode==0,name
cmd=[sys.executable,'-B','-m','unittest','discover','-s','tools','-p','test_documentation.py']
with (r/'documentation-tests.log').open('x') as out:p=subprocess.run(cmd,cwd=r/'checks/source',stdout=out,stderr=subprocess.STDOUT)
assert p.returncode==0
for arm in ['base','candidate','mutation']:
 pins=json.loads((r/arm/'pins.json').read_text())
 for binary,digest in pins['binaries'].items():
  with (r/arm/'bin'/binary).open('rb') as f:assert hashlib.file_digest(f,'sha256').hexdigest()==digest
target=Path('/tmp/slither-retained-layout-target');assert target.resolve()==target and target.is_dir() and not target.is_symlink()
shutil.rmtree(target)
(r/'target-cleanup.json').write_text(json.dumps({'copiedBinariesVerified':True,'targetRemoved':True})+'\n')
for name in ['compact-start-mature.py','compact-proof-mature.py','compact-proof-short.py']:
 print('START',name,flush=True)
 p=subprocess.run([sys.executable,str(r/name)]);assert p.returncode==0,name
print('ALL EXACTNESS PROOFS PASS / NO PRIMARY TIMING REQUESTED',flush=True)
