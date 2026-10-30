"""Locate retained deferral increments; do not exclude or reclassify any ticks."""
from pathlib import Path
import argparse,json
p=argparse.ArgumentParser();p.add_argument('root',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
w=json.loads((a.root/'run-summary.json').read_text())['windows'][0];result=w['result'];first,last=result['tickRange'];start,end=w['start']['unix'],w['end']['unix']
census=[];ticks=[]
for line in (a.root/'load-run/host-stream.log').read_text().splitlines():
    if line.startswith('CENSUS '):census.append(json.loads(line[7:]))
    elif line.startswith('CAPACITY '):
        value=json.loads(line[9:])
        if first<value['tick']<=last:
            ticks.extend({'tick':value['tick']-99+i,'simulationCpuMs':row[0]/1e6,'socketCpuMs':row[1]/1e6,'bracketedBusyWallMs':row[2]/1e6} for i,row in enumerate(value['rows']))
events=[]
for before,after in zip(census,census[1:]):
    delta=after['deferred']-before['deferred']
    if delta and before['unix']<=end and after['unix']>=start:
        assert delta>0
        events.append({'beforeTick':before['tick'],'afterTick':after['tick'],'beforeOffsetSeconds':before['unix']-start,'afterOffsetSeconds':after['unix']-start,'deferredDelta':delta,'censusWallGapSeconds':after['unix']-before['unix'],'completedTickSeconds':(after['tick']-before['tick'])*.007})
assert sum(v['deferredDelta'] for v in events)==result['deferredTicks']
out={'scope':'Full requested-window covering census bracket; no exclusions','deferredTicks':result['deferredTicks'],'events':events,'largestBracketedBusyWallTicks':sorted(ticks,key=lambda v:v['bracketedBusyWallMs'],reverse=True)[:5],'clientFrameGapMaxMs':result['client']['frameGapMs']['max'],'clientFrameExcessP99Ms':result['client']['frameExcessMs']['p99'],'clientFrameExcessMaxMs':result['client']['frameExcessMs']['max'],'limits':'Busy wall brackets the tick body/handoff, not every wait between ticks. Retained counters do not identify a scheduler, kernel, storage or hypervisor cause.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
