"""Reuse #263's overlay unchanged, binding it to this arm's archived runtime.

Stage prepare258.py, prepare262.py and prepare263.py plus their linked helpers
beside this script, with main.tar containing the selected arm. No local build.
"""
from pathlib import Path
import hashlib,json,runpy,tarfile
r=Path(__file__).resolve().parent
runpy.run_path(str(r/'prepare263.py'))
with tarfile.open(r/'main.tar') as archive:runtime=archive.pax_headers['comment']
assert runtime==json.loads((r/'input.json').read_text())['runtime']
source=r/'server/source'
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
manifest=r/'source-manifest.json'
manifest.write_text(json.dumps({p.relative_to(source).as_posix():sha(p) for p in source.rglob('*') if p.is_file()},sort_keys=True,indent=2)+'\n',newline='\n')
p=r/'prepared.json';pins=json.loads(p.read_text());pins['runtime']=runtime
pins['instrumentedManifestSha256']=sha(manifest)
pins['studyPrepareSha256']=sha(Path(__file__))
p.write_text(json.dumps(pins,indent=2)+'\n',newline='\n')
print('ARM PREPARED',runtime,flush=True)
