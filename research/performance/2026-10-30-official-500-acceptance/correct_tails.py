"""Recompute report-only v1 tails from the verified archive; preserve every gate."""
from pathlib import Path
import argparse,ast,collections,copy,hashlib,json,math,statistics,tarfile
p=argparse.ArgumentParser();p.add_argument('archive',type=Path);p.add_argument('selected',type=Path);p.add_argument('receipt',type=Path);a=p.parse_args()
digest=hashlib.file_digest(a.archive.open('rb'),'sha256').hexdigest()
assert digest=='4002cf60512b63558e163e252db84f9f33782fb17efe415c35d70dda3a6323f9'
tree=ast.parse(Path(__file__).with_name('acceptance.py').read_text())
functions=[v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name in ['quantiles','hist_quantiles']]
exec(compile(ast.Module(body=functions,type_ignores=[]),'actual-nearest-rank-helpers','exec'))
data=json.loads(a.selected.read_text());before=copy.deepcopy(data)
assert len(data['runs'])==1
r=data['runs'][0]['result'];first,last=r['tickRange']
with tarfile.open(a.archive) as archive:
    rows=[]
    for line in archive.extractfile('load-run/host-stream.log'):
        if line.startswith(b'CAPACITY '):
            v=json.loads(line[9:])
            if first<v['tick']<=last:
                assert len(v['rows'])==100
                rows.extend((v['tick']-99+i,row) for i,row in enumerate(v['rows']))
    windows=[json.loads(s[14:]) for s in archive.extractfile('load-run/client.log') if s.startswith(b'CLIENT_WINDOW ')]
assert len(rows)==r['ticks'] and len(windows)==6
changes=[]
def replace(target,new,path):
    assert set(target)==set(new)
    for key,value in new.items():
        if target[key]!=value:
            assert key in ['p50','p99','p999'],(path,key,target[key],value)
            changes.append({'path':path+'.'+key,'beforeMs':target[key],'afterMs':value,'absoluteChangeMs':abs(value-target[key])})
    target.update(new)
due=lambda t:math.floor(t*.007/.504)>math.floor((t-1)*.007/.504)
for name,chosen in [('all',rows),('maintenance',[x for x in rows if due(x[0])]),('ordinary',[x for x in rows if not due(x[0])])]:
    target=r if name=='all' else r[name]
    for key,values in [('worldCpuMsPerTick',[(v[0]+v[1])/1e6 for _,v in chosen]),('simulationCpuMsPerTick',[v[0]/1e6 for _,v in chosen]),('socketCpuMsPerTick',[v[1]/1e6 for _,v in chosen])]:replace(target[key],quantiles(values),name+'.'+key)
for name,key in [('frameGapMs','gapsMicros'),('gapResidualMs','gapResidualMicros'),('frameExcessMs','frameExcessMicros')]:replace(r['client'][name],hist_quantiles(windows,key),'client.'+name)
assert r['p99AtMost7ms']==(r['worldCpuMsPerTick']['p99']<=7)
# Removing only the declared report distributions must leave the entire result identical.
def strip(value):
    v=copy.deepcopy(value);result=v['runs'][0]['result']
    for name in ['all','maintenance','ordinary']:
        target=result if name=='all' else result[name]
        for key in ['worldCpuMsPerTick','simulationCpuMsPerTick','socketCpuMsPerTick']:target.pop(key)
    for key in ['frameGapMs','gapResidualMs','frameExcessMs']:result['client'].pop(key)
    return v
assert strip(data)==strip(before)
# A rerun on already-corrected input would overwrite the receipt with no changes.
assert changes,'input is already corrected; recreate prior-selected.json from 55f29215'
receipt={'label':'Post-result nearest-rank correction matching the frozen plan; gates and outcome unchanged','archiveSha256':digest,'largestAbsoluteChangeMs':max((v['absoluteChangeMs'] for v in changes),default=0),'changes':changes,'allNonTailFieldsIdentical':True,'historicalLadderUnchanged':True}
a.selected.write_text(json.dumps(data,indent=2)+'\n',newline='\n');a.receipt.write_text(json.dumps(receipt,indent=2)+'\n',newline='\n')
print(json.dumps(receipt))
