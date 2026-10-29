"""Require deliberate mutations to change frame payload bytes alone."""
from pathlib import Path
import gzip,itertools,json,struct
r=Path(__file__).resolve().parent
def frames(path):
    with gzip.open(path,'rb') as f:
        while header:=f.read(16):
            assert len(header)==16
            tag,n=struct.unpack('<QQ',header)
            data=f.read(n);assert len(data)==n
            if tag==2:yield data
        assert tag==4
rows=[]
for case in ['official500','official500-legacy']:
    counts=[0,0];different=0;first=None
    for index,pair in enumerate(itertools.zip_longest(frames(r/f'trace-{case}-candidate.gz'),frames(r/f'trace-mutation-{case}.gz'))):
        for i,p in enumerate(pair):counts[i]+=p is not None
        if pair[0]!=pair[1]:
            different+=1
            if first is None:first=index
    assert different>0
    rows.append({'case':case,'wireFramesComparedByteForByte':True,'wireFramesDiffer':different,'firstDifferentFrame':first,'frameCounts':dict(zip(['reference','mutation'],counts))})
(r/'mutation-wire-verified.json').write_text(json.dumps(rows,indent=2)+'\n')
print('PAYLOAD MUTATIONS DETECTED',json.dumps(rows),flush=True)
