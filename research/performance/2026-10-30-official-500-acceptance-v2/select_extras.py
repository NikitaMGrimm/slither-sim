"""Post-collection generator diagnostics; never alter the frozen gate selection."""
from pathlib import Path
import json, math
r=Path(__file__).resolve().parent
summary=json.loads((r/'run-summary.json').read_text())
window=summary['windows'][0]
start,end=window['result']['windowUnix']
rows=[];connections=[]
for line in (r/'load-run/client.log').read_text().splitlines():
    if line.startswith('CLIENT '):
        v=json.loads(line[7:])
        if start-1.1<=v['unix']<=end+1.1:rows.append(v)
    elif line.startswith('CLIENT_CONNECTION '):
        v=json.loads(line[18:])
        if start<=v['unix']<=end:connections.append(v)
def describe(values):
    values=sorted(values)
    if not values:return {'count':0}
    return {'count':len(values),'mean':sum(values)/len(values),'p99':values[math.ceil((len(values)-1)*.99)],'max':values[-1]}
out={'label':'Descriptive generator diagnostics; unchanged acceptance gates',
     'workers':{str(w):{'reports':sum(v['worker']==w for v in rows),'lateReportsAtLeast42ms':sum(v['worker']==w and v['lateP99Nanos']>=42_000_000 for v in rows),'maxLateP99Ms':max(v['lateP99Nanos'] for v in rows if v['worker']==w)/1e6,'maxLateMs':max(v['lateMaxNanos'] for v in rows if v['worker']==w)/1e6,'maxPassMs':max(v['maxPassNanos'] for v in rows if v['worker']==w)/1e6} for w in range(1,7)},
     'backgroundJobElapsedMs':{kind:describe([v['elapsedNanos']/1e6 for v in connections if v['kind']==kind]) for kind in ['connect','close']},
     'connectionTimingScope':'Execution through completion of the blocking job, including global connect mutex/pacing; excludes queue wait and later snake admission. Selected by completion-processing timestamp.'}
assert max(v['maxLateP99Ms'] for v in out['workers'].values())==window['result']['client']['worstInputP99Ms']
(r/'generator-selected.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out))
