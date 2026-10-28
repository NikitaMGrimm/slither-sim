"""Accept the mature #249 comparison only after exact trace/fork controls."""
import hashlib,json,statistics,sys
from pathlib import Path
from compare import same_bytes
root=Path(sys.argv[1]).resolve();r=json.loads((root/'ready.json').read_text())
assert r['warmTraceComparedEveryByte'] and len(set(r['traceBytes'].values()))==1
assert all(v['threads']==1 for v in r['ready'].values())
def equal(a,b):
 assert same_bytes(a,b),(a.name,b.name)
base=root/'proof-base-0.trace'
base_reference='proof-base-parent.trace' if (root/'proof-base-parent.trace').exists() else 'proof-base-third.trace'
for name in ['proof-base-1.trace',base_reference,'proof-candidate-0.trace','proof-candidate-1.trace','proof-candidate-parent.trace']:equal(base,root/name)
rows=[]
for pair in range(-1,5):
 row={'pair':pair,'warmup':pair==-1}
 for arm in ['base','candidate']:
  v=json.loads((root/f'pair-{pair}-{arm}.json').read_text());assert v['counts']['calls'][2]==r['windowTicks']
  row[arm]=v['counts']['instructions'][2]
  row[arm+'Summary']=v['summary']
 assert row.pop('baseSummary')==row.pop('candidateSummary')
 row['reduction']=1-row['candidate']/row['base'];rows.append(row)
values=[v['reduction'] for v in rows if not v['warmup']]
result={'warmTrace':r,'forkCompleteWindowBytesEqual':True,'pairs':rows,'medianReduction':statistics.median(values),'range':[min(values),max(values)],'source':f"plain current-thread instructions:u; {r['windowTicks']} host advances after fixed {r['warmTicks']} warm ticks; controls/drains/A4 excluded"}
(root/'accepted.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'medianReduction':result['medianReduction'],'range':result['range'],'warm':r['ready']['candidate']['summary']['census'],'end':json.loads((root/'pair-0-candidate.json').read_text())['summary']['census'],'warmStreamBytes':r['traceBytes'],'proofTraceSha256':hashlib.sha256(base.read_bytes()).hexdigest()},indent=2))
