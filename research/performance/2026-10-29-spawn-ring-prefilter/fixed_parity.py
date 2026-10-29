"""Check complete queued-stress bytes and exact exhausted-search budgets.

Usage: python fixed_parity.py OUTPUT
All runs have timing/PMU disabled. Keep raw outputs and errors after failures.
"""
from pathlib import Path
import hashlib,json,os,struct,subprocess,sys
root=Path(sys.argv[1]).resolve()
suffix='.exe' if os.name=='nt' else ''
ticks=int(os.environ.get('FIXED_TRACE_TICKS','360'));assert ticks>=360

def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def run(arm,protocol):
    folder=root/arm;exe=folder/'bin'/('study_fixed'+suffix)
    pins=json.loads((folder/'pins.json').read_text());assert sha(exe)==pins['binaries'][exe.name]
    out=folder/f'fixed{protocol}.bin';err=folder/f'fixed{protocol}.stderr'
    with out.open('xb') as f,err.open('xb') as e:
        subprocess.run([str(exe),str(protocol),str(ticks)],stdout=f,stderr=e,check=True,timeout=300,
                       env=dict(os.environ,RAYON_NUM_THREADS='1'))
    result=json.loads(err.read_text().splitlines()[-1]);summary=result['summary']
    assert all(r['waitingBefore']==72 and r['waitingAfter']==72 for r in result['ticks'] if r['blocked'])
    assert summary['blockedSearches']>=72 and summary['blockedExhausted']==summary['blockedSearches']
    assert summary['blockedAttempts']==4096*summary['blockedSearches']
    assert summary['releaseSearches']>0 and summary['waiting']<72
    wire=hashlib.sha256();frames=states=0;last=None
    with out.open('rb') as f:
        while header:=f.read(16):
            assert len(header)==16;tag,n=struct.unpack('<QQ',header);data=f.read(n);assert len(data)==n
            if tag in [0,3]:assert json.loads(data)['backend']['world']['state'];states+=1
            elif tag==1:assert len(json.loads(data))==4
            elif tag==2:wire.update(header);wire.update(data);frames+=1
            elif tag==4:assert json.loads(data)==summary
            else:raise AssertionError(tag)
            last=tag
    assert states==2 and frames>0 and last==4
    assert sha(exe)==pins['binaries'][exe.name]
    return {'sha256':sha(out),'bytes':out.stat().st_size,'wireSha256':wire.hexdigest(),
            'frames':frames,'summary':summary}

rows=[]
for protocol in [19,13]:
    row={'protocol':protocol}
    for arm in ['base','candidate','mutation']:
        row[arm]=run(arm,protocol)
    assert row['base']==row['candidate'],protocol
    with (root/'base'/f'fixed{protocol}.bin').open('rb') as a,(root/'candidate'/f'fixed{protocol}.bin').open('rb') as b:
        while True:
            x,y=a.read(1<<20),b.read(1<<20);assert x==y
            if not x:break
    assert row['mutation']['wireSha256']!=row['candidate']['wireSha256'],protocol
    row['wireMutationDetected']=True;rows.append(row)
    (root/'fixed-parity.json').write_text(json.dumps(rows,indent=2)+'\n',newline='\n')
    print('FIXED PARITY + MUTATION',protocol,flush=True)
