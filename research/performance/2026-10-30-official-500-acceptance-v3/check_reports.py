"""Post-collection integrity check; no gate, tail or report selection changes."""
from pathlib import Path
import argparse,json,math,statistics
p=argparse.ArgumentParser();p.add_argument('root',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
w=json.loads((a.root/'run-summary.json').read_text())['windows'][0]
start,end=w['result']['windowUnix'];rows=[]
for line in (a.root/'load-run/client.log').read_text().splitlines():
    if line.startswith('CLIENT '):
        v=json.loads(line[7:])
        if start-1.1<=v['unix']<=end+1.1:rows.append(v)
keys=['decisionCpuFraction','connectorCpuFraction','publisherCpuFraction']
errors=[]
for v in rows:
    assert all(math.isfinite(v[k]) and v[k]>=0 for k in keys+['cpuFraction'])
    error=abs(v['cpuFraction']-sum(v[k] for k in keys));errors.append(error)
    # Separate binary64 divisions/JSON round-trips can differ by a few ulps.
    # This integrity tolerance never changes the strict 70%/80% gate.
    assert error<=1e-12
assert {v['worker'] for v in rows}==set(range(1,7))
assert max(v['lateP99Nanos'] for v in rows)/1e6==w['result']['client']['worstInputP99Ms']
out={'scope':'Exactly the frozen boundary-overlapping one-second reports; no gate changes','selectedReports':len(rows),'maximumComponentSumRoundingError':max(errors),'workers':{}}
for worker in range(1,7):
    selected=[v for v in rows if v['worker']==worker]
    out['workers'][str(worker)]={'reports':len(selected),'meanFractions':{k:statistics.mean(v[k] for v in selected) for k in keys+['cpuFraction']},'lateReportsAtLeast42ms':sum(v['lateP99Nanos']>=42_000_000 for v in selected)}
a.output.write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'selectedReports':len(rows),'maximumComponentSumRoundingError':max(errors),'lateReportsAtLeast42ms':sum(v['lateReportsAtLeast42ms'] for v in out['workers'].values())}))
