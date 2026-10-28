"""Five alternating pairs on the original four fixed-work rows."""
import importlib.util,json,os,statistics
from pathlib import Path
root=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('shared',root/'shared/reproduce.py');shared=importlib.util.module_from_spec(spec);spec.loader.exec_module(shared)
os.sched_setaffinity(0,{0})
assert len(json.loads((root/'parity.json').read_text()))==27
assert all(r['detected'] for r in json.loads((root/'mutation.json').read_text()))
assert json.loads((root/'required-checks-pass.json').read_text())['passed']
results={}
for case in ['official500','official500-legacy','official2281','client-official-500']:
 binary='perf_baseline' if case.startswith('client-') else 'study_protocols';args=[case,'24'] if binary=='perf_baseline' else [case];rows=[]
 for pair in range(-1,5):
  row={'pair':pair,'warmup':pair<0}
  for arm in (['base','candidate'] if pair<0 or pair%2==0 else ['candidate','base']):
   label=f'pair-{case}-{pair}-{arm}';shared.inventory(root,label+'-before')
   extra={'STUDY_TICK_COUNTS':str(root/(label+'-ticks.json'))} if binary=='study_protocols' else {}
   counts=shared.run(root,arm,binary,args,label,extra);shared.inventory(root,label+'-after');row[arm]=counts['instructions'][1]
   if extra:
    ticks=json.loads((root/(label+'-ticks.json')).read_text());assert len(ticks)==1080 and all(t[2]==0 or t[3]/t[2]>=.99 for t in ticks)
    row[arm+'Ticks']=sum(t[1] for t in ticks)
  if binary=='study_protocols':
   assert (root/f'pair-{case}-{pair}-base.stderr').read_bytes()==(root/f'pair-{case}-{pair}-candidate.stderr').read_bytes();row['reduction']=1-row['candidateTicks']/row['baseTicks']
  else:
   reports=[]
   for arm in ['base','candidate']:
    report=json.loads((root/f'pair-{case}-{pair}-{arm}.stdout').read_text().splitlines()[-1]);report.pop('timing')
    for key in ['admissionSeconds','admissionSecondsPerDecision']:report['details'].pop(key)
    reports.append(report)
   assert reports[0]==reports[1];row['reduction']=1-row['candidate']/row['base']
  rows.append(row);print(case,pair,row['reduction'],flush=True)
 results[case]=rows;(root/'pairs.json').write_text(json.dumps(results,indent=2)+'\n')
print('ORIGINAL FIXED ROW PAIRS COMPLETE',flush=True)
