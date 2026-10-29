"""Reuse #274 grouped observations; apply the new gate using integer CPU sums."""
from pathlib import Path
import sys,subprocess
r=Path(__file__).resolve().parent
text=(r/'compact-select-primary.py').read_text()
before="return {'ticks':n,'instructionsPerTick'"
after="return {'ticks':n,'cpuTotalNs':sums[2],'instructionsPerTick'"
assert text.count(before)==1;text=text.replace(before,after)
needle=" results.append({'name':name"
extra=""" storage['retainedSectors']={arm:{'keys':sum(x['keys'] for x in v['sectorStorage']),'times':sum(x['times'] for x in v['sectorStorage']),'keyCapacityBytes':sum(x['keyCapacityBytes'] for x in v['sectorStorage']) if all(x['keyCapacityBytes'] is not None for x in v['sectorStorage']) else None,'timeCapacityBytes':sum(x['timeCapacityBytes'] for x in v['sectorStorage']) if all(x['timeCapacityBytes'] is not None for x in v['sectorStorage']) else None} for arm,v in ready['ready'].items()}
"""
assert text.count(needle)==1;text=text.replace(needle,extra+needle)
start=text.index("mature=[x for x in results")
text=text[:start]+'''
mature=[x for x in results if x['name'].startswith('mature-')]
assert len(mature)==2
reported=lambda row:[p for p in row['pairs'] if not p['discardedAB']]
all_pairs_lower=all(p['candidate']['all']['cpuTotalNs']<p['base']['all']['cpuTotalNs'] for row in mature for p in reported(row))
maintenance=[];short=[]
for row in results:
 label='maintenance' if row in mature else 'all'
 pairs=reported(row);ticks=pairs[0]['base'][label]['ticks']
 assert all(p[arm][label]['ticks']==ticks for p in pairs for arm in ['base','candidate'])
 differences=[p['candidate'][label]['cpuTotalNs']-p['base'][label]['cpuTotalNs'] for p in pairs]
 median=statistics.median(differences)
 value={'row':row['name'],'ticksPerWindow':ticks,'pairedWindowCpuDifferenceNs':differences,'medianPairedCostUsPerTick':median/ticks/1000,'pairedCostRangeUsPerTick':[min(differences)/ticks/1000,max(differences)/ticks/1000]}
 if row in mature:
  value['qualifies']=median<=0;maintenance.append(value)
 else:
  value['qualifies']=median<=ticks*10000;short.append(value)
decision={'allTenMatureWholeWindowPairsLowerCpu':all_pairs_lower,'matureMaintenance':maintenance,'shortAbsoluteCostLimitUs':10,'shortControls':short,'runtimeQualifies':all_pairs_lower and all(v['qualifies'] for v in maintenance+short)}
out={'baseline':json.loads((r/'base/pins.json').read_text())['archiveCommit'],'candidate':json.loads((r/'candidate/pins.json').read_text())['archiveCommit'],'decision':decision,'rows':results}
(r/'selected-results.json').write_text(json.dumps(out,indent=2)+'\\n')
print(json.dumps(decision),flush=True)
for row in results:print(row['name'],json.dumps({label:row['summary'][label]['cpuMsPerTick'] for label in ['all','ordinary','maintenance']}),flush=True)
'''
(r/'select-primary.py').write_text(text)
subprocess.run([sys.executable,str(r/'select-primary.py')],check=True)
