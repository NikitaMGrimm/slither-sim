"""Apply the separately planned 600-tick minimap warmup to fresh source stages.

Usage: python warm_stage.py STAGED_OUTPUT
Run after the common prepare.py stages all three arms, before native builds.
Retains original runtime archive pins and rehashes the changed overlay manifest.
"""
from pathlib import Path
import hashlib,json,sys
root=Path(sys.argv[1]).resolve()
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
changes=[]
for arm in ['base','candidate','mutation']:
    source=root/arm/'source'
    assert not (root/arm/'pins.json').exists(), 'Use fresh source-only stages'
    old='for _ in 0..300 { fixture.tick(true, None); }'
    paths=[p for p in source.rglob('*.rs') if old in p.read_text()]
    assert len(paths)==1
    p=paths[0];before=sha(p)
    p.write_text(p.read_text().replace(old,'for _ in 0..if row=="minimap" {600} else {300} { fixture.tick(true, None); }'),newline='\n')
    manifest=root/arm/'source-stage.json';data=json.loads(manifest.read_text())
    data['sources']={p.relative_to(source).as_posix():sha(p) for p in source.rglob('*.rs')}
    data['files']={p.relative_to(source).as_posix():sha(p) for p in source.rglob('*') if p.is_file()}
    manifest.write_text(json.dumps(data,indent=2)+'\n',newline='\n')
    changes.append({'arm':arm,'file':p.relative_to(source).as_posix(),'before':before,'after':sha(p)})
(root/'warm-amendment.json').write_text(json.dumps({'warmTicks':600,'countTicks':360,'changes':changes},indent=2)+'\n',newline='\n')
