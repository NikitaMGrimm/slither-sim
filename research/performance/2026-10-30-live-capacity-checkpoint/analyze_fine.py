"""Nested current-main CPU attribution, diagnostic-only and inclusive."""
from pathlib import Path
import sys,json,importlib.util,statistics,math
r=Path(sys.argv[1]).resolve();sys.argv=[sys.argv[0],str(r)]
spec=importlib.util.spec_from_file_location('a',r/'analyze.py');a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a)
name='current-fine-1';m,cs,bs=a.read_run(name)
labels={16:'preparation',17:'snakeVisibilityEncoding',18:'recipientFood',19:'recipientRemainderAfterFood',20:'minimap',21:'hud',22:'frameEmissionGrouping',23:'subscriptionAdvanceAndPostPhysics',24:'advanceTotal',25:'applyCommands',26:'handover',27:'subscriptionUnionSync',28:'foodMembershipRefreshOwnership'}
rows=a.rows_in(cs,bs,m['fineStartUnix'],m['diagnosticEndUnix'],True)
for x in rows:
 block=bs[((x['tick']-1)//100+1)*100];phase=block['phaseRows'][(x['tick']-1)%100]
 x['nested']={label:phase[i]/1e6 for i,label in labels.items()}
def describe(xs):
 v=a.summarize(xs);v['nestedInclusiveMeanCpuMsPerTick']={label:statistics.mean(x['nested'][label] for x in xs) for label in labels.values()};return v
out={'limits':'One fresh current-main live diagnostic; extra per-recipient timers add overhead. Nested scopes overlap and must not be summed. Not a capacity result or an optimization saving. Recipient remainder is the function remainder after food, not a pure prey scope.','sourceCommit':json.loads((r/'candidate/pins.json').read_text())['commit'],'hostSha256':m['hostSha256'],'coarseQuality':json.loads((r/'fine-coarse-validation.json').read_text())['cases'][name]['holds'][0],'fineQuality':json.loads((r/'fine-validation.json').read_text())['cases'][name]['holds'][0],'fine':{'all':describe(rows),'maintenance':describe([x for x in rows if x['food']]),'ordinary':describe([x for x in rows if not x['food']]),'slowestOnePercent':describe(sorted(rows,key=lambda x:x['world'],reverse=True)[:math.ceil(len(rows)*.01)])}}
# No per-file source maps or misleading disabled-syscall diagnostic are exported.
(r/'publication/fine-results.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out['fine'],indent=2))
