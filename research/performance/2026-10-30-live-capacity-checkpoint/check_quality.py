"""Synthetic combinations ensure only one failed gate permits its named retry."""
from pathlib import Path
import sys,tempfile,json,subprocess,hashlib
r=Path(sys.argv[1]).resolve();name=sys.argv[2];original=r/'runs'/name
sha=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
before=sha(original/'host.log');receipts=[]
with tempfile.TemporaryDirectory(dir=r,prefix='quality-controls-') as tmp:
 root=Path(tmp)
 for label,class_bad,isolation_bad,expected in [('class-only',True,False,'mature length class drift'),('isolation-only',False,True,'competing process CPU'),('both',True,True,'multiple quality gates failed: class and isolation'),('neither',False,False,None)]:
  work=root/label;work.mkdir();(work/'runs').mkdir();run=work/'runs'/name;run.mkdir()
  for n in ['base','candidate','common-client','shared248','target-cleanup.json','core-telemetry.jsonl']:(work/n).symlink_to(r/n,target_is_directory=(r/n).is_dir())
  for p in original.iterdir():
   if p.is_file() and p.name!='host.log':(run/p.name).symlink_to(p)
  lines=[]
  for line in (original/'host.log').read_text().splitlines():
   if line.startswith('CENSUS '):
    v=json.loads(line[7:]);v['p90Segments']=155 if class_bad else 176;v['living']=500;v['meanSegments']=55;v['p99Segments']=240;line='CENSUS '+json.dumps(v)
   lines.append(line)
  (run/'host.log').write_text('\n'.join(lines)+'\n')
  gate=json.loads((r/'isolation-gate.json').read_text())
  if isolation_bad:gate.update(idleEnvelopeRate=0,excessRateAllowance=0,endpointQuantumCount=0)
  (work/'isolation-gate.json').write_text(json.dumps(gate))
  p=subprocess.run([sys.executable,str(r/'select.py'),str(work),str(work/'selected.json'),name],capture_output=True,text=True)
  if expected is None:assert p.returncode==0,p.stderr
  else:assert p.returncode!=0 and p.stderr.strip().splitlines()[-1]=='AssertionError: '+expected,p.stderr
  receipts.append({'syntheticCombination':label,'correctClassification':True})
assert before==sha(original/'host.log')
(r/'quality-classification-controls.json').write_text(json.dumps({'synthetic':True,'originalUnchanged':True,'controls':receipts},indent=2)+'\n')
print('ALL FOUR QUALITY COMBINATIONS PASS')
