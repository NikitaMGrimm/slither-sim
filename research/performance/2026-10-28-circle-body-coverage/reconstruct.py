"""Reconstruct measured Rust payloads without building or rerunning a world."""
from pathlib import Path
import json,shutil,subprocess,sys,tempfile
study=Path(__file__).resolve().parent
root=Path(sys.argv[1]).resolve();fixture=Path(sys.argv[2]).resolve();rows=[]
for arm in ['base','candidate','base-profile','candidate-profile','mutation']:
 pin=root/arm/'pins.json'
 if not pin.exists():continue
 expected=json.loads(pin.read_text())['sources']
 with tempfile.TemporaryDirectory(prefix='reconstruct-',dir=root) as name:
  out=Path(name);shutil.copy2(study/'build.py',out/'build.py');shutil.copy2(fixture,out/'fixture.rs')
  (out/'shared').symlink_to(root/'shared',target_is_directory=True)
  for file in ['base.tar','candidate.tar']:(out/file).symlink_to(root/file)
  command=[sys.executable,str(out/'build.py'),arm.removesuffix('-profile'),'--prepare-only']
  if arm.endswith('-profile'):command.append('--profile')
  actual=json.loads(subprocess.check_output(command,text=True))['sources']
  assert expected==actual,(arm,[k for k in expected.keys()|actual.keys() if expected.get(k)!=actual.get(k)])
 rows.append({'arm':arm,'allRustSourceHashesMatch':True,'files':len(expected)})
 print('SOURCE HASHES MATCH',arm,len(expected),flush=True)
(root/'reconstruction.json').write_text(json.dumps(rows,indent=2)+'\n')
