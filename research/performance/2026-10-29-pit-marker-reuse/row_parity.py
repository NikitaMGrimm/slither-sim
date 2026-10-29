"""Complete-byte shrinking-marker row and wire mutation controls; no timing.

Usage: python row_parity.py OUTPUT
"""
from pathlib import Path
import hashlib,json,os,struct,subprocess,sys
r=Path(sys.argv[1]);suffix='.exe' if os.name=='nt' else '';rows=[]

def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

for protocol in [19,13]:
    row={'protocol':protocol}
    for arm in ['base','candidate','mutation']:
        exe=r/arm/'bin'/('study_fixed'+suffix);pins=json.loads((r/arm/'pins.json').read_text())
        assert sha(exe)==pins['binaries'][exe.name]
        out=r/arm/f'markers-{protocol}.bin';err=out.with_suffix('.stderr')
        with out.open('xb') as f,err.open('xb') as e:
            subprocess.run([str(exe),str(protocol),'360','--trace','markers'],stdout=f,stderr=e,check=True,timeout=120)
        result=json.loads(err.read_text().splitlines()[-1]);ts=result['ticks'];assert len(ts)==360
        assert result['summary']['waiting']==0 and sum(t['marker'] for t in ts)>=3
        wire=hashlib.sha256();states=frames=0;tag=None;routing=True
        with out.open('rb') as f:
            while header:=f.read(16):
                assert len(header)==16;tag,n=struct.unpack('<QQ',header);data=f.read(n);assert len(data)==n
                if tag in [0,3]:
                    state=json.loads(data);assert state['backend']['world']['state'];assert state['backend']['living']==80
                    assert state['backend']['decoratedViewers']==80;states+=1
                    routing &= len(state['generations'])==80 and all(state['generations'])
                    assert routing
                elif tag==1:assert len(json.loads(data))==4
                elif tag==2:wire.update(header);wire.update(data);frames+=1
                elif tag==4:assert json.loads(data)==result['summary']
                else:raise AssertionError(tag)
        assert states==2 and tag==4 and frames>0
        row[arm]={'sha256':sha(out),'bytes':out.stat().st_size,'wireSha256':wire.hexdigest(),
            'summary':result['summary'],'markerTicks':sum(t['marker'] for t in ts),
            'routingGenerationsNonzero':bool(routing)}
        assert sha(exe)==pins['binaries'][exe.name]
    assert row['base']==row['candidate'],protocol
    with (r/'base'/f'markers-{protocol}.bin').open('rb') as a,(r/'candidate'/f'markers-{protocol}.bin').open('rb') as b:
        while True:
            x,y=a.read(1<<20),b.read(1<<20);assert x==y
            if not x:break
    assert row['mutation']['wireSha256']!=row['candidate']['wireSha256'],protocol
    row['wireMutationDetected']=True;rows.append(row)
    (r/'row-parity.json').write_text(json.dumps(rows,indent=2)+'\n')
    print('SHRINK PARITY + HINT MUTATION',protocol,flush=True)
