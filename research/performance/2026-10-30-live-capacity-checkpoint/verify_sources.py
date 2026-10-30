"""Reconstruct archived-copy overlays, comparing complete Rust source bytes."""
from pathlib import Path
import sys,tempfile,subprocess,hashlib,json,tarfile
r=Path(sys.argv[1]).resolve();receipts=[]
sha=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
for arm in ['base','candidate','diagnostic-candidate']:
 if not (r/arm).exists():continue
 with tempfile.TemporaryDirectory(dir=r,prefix='source-check-') as tmp:
  w=Path(tmp)
  for n in ['base.tar','candidate.tar','client.rs','timing.rs','shared248','build.py','build_fine.py']:(w/n).symlink_to(r/n,target_is_directory=(r/n).is_dir()) if n not in ['build.py','build_fine.py'] else (w/n).write_bytes((r/n).read_bytes())
  command=['python3',str(w/'build_fine.py'),'--prepare-only'] if arm=='diagnostic-candidate' else ['python3',str(w/'build.py'),str(r/'shared248'),arm,'--prepare-only']
  subprocess.run(command,check=True,stdout=subprocess.DEVNULL)
  old=r/arm/'source';new=w/arm/'source'
  original={p.relative_to(old).as_posix():p.read_bytes() for p in (old/'src').rglob('*.rs')}
  generated={p.relative_to(new).as_posix():p.read_bytes() for p in (new/'src').rglob('*.rs')}
  assert original==generated,arm
  digest=hashlib.sha256(b''.join(k.encode()+b'\0'+v+b'\0' for k,v in sorted(original.items()))).hexdigest()
  receipts.append({'arm':arm,'completeRustSourcesMatch':True,'instrumentedRustSourceSha256':digest})
with tarfile.open(r/'client-base.tar') as t:
 for m in t.getmembers():
  if m.isfile() and m.name.startswith('src/') and m.name.endswith('.rs'):assert t.extractfile(m).read()==(r/'common-client/source'/m.name).read_bytes()
assert (r/'client.rs').read_bytes()==(r/'common-client/source/examples/capacity_clients.rs').read_bytes()
(r/'source-verification.json').write_text(json.dumps({'hostCopies':receipts,'commonClientProductionSourcesUnchanged':True,'commonClientHarnessUnchanged':True},indent=2)+'\n')
print('ALL SOURCE COPIES RECONSTRUCT EXACTLY')
