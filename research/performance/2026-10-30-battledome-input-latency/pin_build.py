from pathlib import Path
import hashlib,json,tarfile,subprocess,shutil
r=Path('/srv/slither-study');src=r/'source'
sha=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
with tarfile.open(r/'main.tar') as t:
 members=[m for m in t if m.isfile() and m.name.startswith('src/')]
 assert all(hashlib.sha256(t.extractfile(m).read()).hexdigest()==sha(src/m.name) for m in members)
manifest={p.relative_to(src).as_posix():sha(p) for p in src.rglob('*') if p.is_file() and '.git' not in p.parts}
(r/'source-manifest.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n')
pins={'runtime':'9cd6ba6922c0a63b525ba88b69f86c8558142596','runtimeArchiveSha256':sha(r/'main.tar'),'sourceManifestSha256':sha(r/'source-manifest.json'),'runtimeSrcUnchanged':True,'binaries':{p.name:sha(p) for p in (r/'bin').iterdir()},'probeSourceSha256':sha(r/'probe.rs'),'loadSourceSha256':sha(r/'aws_clients.rs'),'portable':True,'rustc':subprocess.check_output([shutil.which('rustc') or str(Path.home()/'.cargo/bin/rustc'),'--version'],text=True).strip()}
(r/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
print(json.dumps(pins))
