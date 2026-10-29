"""Reuse the common fixed-work/trace builder; mutate only marker-sector hints.

Usage: python prepare.py PRIOR_SPAWN_PREPARE PRIOR_MINIMAP_PREPARE REPO OUTPUT ARM REF
Both earlier helpers and their adjacent Rust sources stay in external staging.
"""
from pathlib import Path
import hashlib,json,os,sys
own=Path(__file__).resolve();prior=Path(sys.argv.pop(1)).resolve()
root=Path(sys.argv[3]).resolve();arm=sys.argv[4]
text=prior.read_text()
for old,new in [
    ("p=source/'src/client/minimap.rs';payload=p.read_text()","p=source/'src/client/simulation/markers.rs';payload=p.read_text()"),
    ("old='pit / world.radius() * side as f64 / 2.';assert payload.count(old)>=1","old='self.affected.contains(&sector)';assert payload.count(old)==1"),
    ("payload.replace(old,old+' * 0.98')","payload.replace(old,old+' && false')"),
]:
    assert text.count(old)==1,old
    text=text.replace(old,new)
os.environ['STUDY_MUTATION']='map'
staged=False
try:
    exec(compile(text,str(prior),'exec'),{'__file__':str(prior),'__name__':'__main__'})
except SystemExit as error:
    if error.code not in [None,0]:raise
    staged=True
p=root/arm/('source-stage.json' if staged else 'pins.json');pins=json.loads(p.read_text())
pins['markerWrapperSha256']=hashlib.sha256(own.read_bytes()).hexdigest()
pins['priorSpawnPrepareSha256']=hashlib.sha256(prior.read_bytes()).hexdigest()
p.write_text(json.dumps(pins,indent=2)+'\n',newline='\n')
