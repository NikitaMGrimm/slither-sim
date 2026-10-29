"""Reuse #269 symbolization; retain and aggregate every predeclared repetition."""
from pathlib import Path
import json,sys,subprocess,collections
r=Path(__file__).resolve().parent
text=(r/'donor-reduce.py').read_text()
before="names={1:'cycles',2:'branch-misses',3:'L1d-load-misses',5:'dTLB-load-misses',6:'generic-cache-misses',7:'frontend-stalled-cycles'}"
after="names={1:'cycles',3:'L1d-load-misses',5:'dTLB-load-misses'}"
assert text.count(before)==1;text=text.replace(before,after)
before="jobs=[(event,'initial',f'{protocol}-e{event}-r0-sample',r/'study_host') for event in [1,3,5,6]]"
after="jobs=[(event,'fresh',f'{protocol}-e{event}-r{rep}-sample',r/'study_host') for rep in range(3) for event in [1,3,5]]"
assert text.count(before)==1;text=text.replace(before,after)
text=text.replace("1000003 if event==1 else 10007 if series=='lower-rate' else 1009", "1000003 if event==1 else 10007 if event==3 else 1009")
text=text.replace('.most_common(16)', '.most_common()')
lines=text.splitlines(True)
for i,line in enumerate(lines):
 if line.startswith(" f['knownSnakes']={"):
  lines[i]=" f['knownSnakes']={'entryLengths':sum(x[0] for x in known),'entryCapacities':sum(x[1] for x in known),'occupied':sum(x[2] for x in known),'entryCapacityBytes':sum(x[1] for x in known)*sizes['knownSlotEntry'],'occupiedPayloadBytes':sum(x[2] for x in known)*sizes['knownSnake']}\n"
(r/'reduce.py').write_text(''.join(lines))
subprocess.run([sys.executable,str(r/'reduce.py')],check=True)
profiles=json.loads((r/'profile-results.json').read_text());aggregates=[]
for protocol in ['stock','legacy']:
 for event in ['cycles','L1d-load-misses','dTLB-load-misses']:
  for maintenance in [False,True]:
   rows=[p for p in profiles if p['protocol']==protocol and p['event']==event and p['maintenance']==maintenance]
   assert len(rows)==3
   total=sum(p['samples'] for p in rows);assert total>0
   value={'protocol':protocol,'event':event,'maintenance':maintenance,'repetitions':3,'samples':total,'lost':sum(p['lost'] for p in rows),'allProfilesPrimary':all(p['primary'] and p['lost']==0 for p in rows),'minimumSamplerRunningFraction':min(p['samplerCounter'][2]/p['samplerCounter'][1] for p in rows),'period':rows[0]['period'],'recordTypes':dict(sum((collections.Counter(p['recordTypes']) for p in rows),collections.Counter()))}
   for field,key,limit in [('sourceGroups','source',None),('leafFunctions','function',16),('sourceSites','site',16)]:
    counts=collections.Counter()
    for p in rows:counts.update({v[key]:v['samples'] for v in p[field]})
    value[field]=[{key:k,'samples':n,'share':n/total} for k,n in counts.most_common(limit)]
   aggregates.append(value)
(r/'attribution.json').write_text(json.dumps(aggregates,indent=2)+'\n')
for a in aggregates:
 if a['maintenance']:print(a['protocol'],a['event'],'maintenance',a['samples'],'samples; primary',a['allProfilesPrimary'],[(x['source'],round(x['share']*100,2)) for x in a['sourceGroups'][:6]],flush=True)
