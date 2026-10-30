"""Guest-side replay, accounting and privacy-limited selected acceptance data."""
from pathlib import Path
import bisect,copy,hashlib,json,os,sys
from acceptance import reduce_window,verdict,mature
from metrics import prior
r=Path(__file__).resolve().parent
summary=json.loads((r/'run-summary.json').read_text());pins=json.loads((r/'pins.json').read_text())
assert summary['status']=='complete' and summary['completeHostEvidence']
assert len(summary['windows'])==1
window=summary['windows'][0];result=reduce_window(r/'load-run',window)
def non_tail_values(value):
 value=copy.deepcopy(value)
 for parent in [value,value['maintenance'],value['ordinary']]:
  for name in ['worldCpuMsPerTick','simulationCpuMsPerTick','socketCpuMsPerTick']:
   for key in ['p50','p99','p999','max']:parent[name].pop(key)
 for name in ['frameGapMs','gapResidualMs','frameExcessMs']:
  for key in ['p50','p99','p999','max']:value['client'][name].pop(key)
 value.pop('p99AtMost7ms') # Derived report-only flag, never an acceptance gate.
 return value
assert non_tail_values(result)==non_tail_values(window['result']),'Gates or non-tail replay values differ'
assert len(summary['matureScreen'])==3 and all(mature(v) for v in summary['matureScreen'])
assert [v['tick'] for v in summary['matureScreen']]==list(range(summary['matureScreen'][0]['tick'],summary['matureScreen'][0]['tick']+1500,500))
assert summary['matureScreen'][-1]['tick']>=193000
host=[json.loads(x) for x in (r/'host-run/telemetry.jsonl').read_text().splitlines()];clients=[json.loads(x) for x in (r/'load-run/telemetry.jsonl').read_text().splitlines()]
start,end=result.pop('windowUnix');n=result['ticks']
def cpu(samples,member):
 a,_=prior.cpu_at(samples,start,member);b,_=prior.cpu_at(samples,end,member);return prior.cpu_delta(a,b)
server=cpu(host,'server');client=cpu(clients,'client')
assert all(t['affinity']==[0] for t in server['threads'])
world=sum(t['userSeconds']+t['kernelSeconds'] for t in server['threads'] if t['role'].startswith(('slither-simulat','slither-sockets')))/n*1000
clock=result['worldCpuMsPerTick']['mean'];ok=abs(world-clock)<=max(.03,.02*clock)
def process_at(member,at):
 rows=[v[member] for v in host if v.get(member)];edge=bisect.bisect_left([v['unix'] for v in rows],at);assert 0<edge<len(rows)
 a,b=rows[edge-1:edge+1];span=b['unix']-a['unix'];assert 0<span<2.5;f=(at-a['unix'])/span
 return {key:a['process'][key]+(b['process'][key]-a['process'][key])*f for key in ['user','system']}
a,b=process_at('collector',start),process_at('collector',end)
result['accounting']={'reconciles':ok,'worldTaskCpuMeanMsPerTick':world,'clockMinusTaskMeanMsPerTick':clock-world,'hostProcessCpuMeanMsPerTick':(server['userSeconds']+server['kernelSeconds'])/n*1000,'collectorCpuMeanMsPerTick':sum(b[k]-a[k] for k in a)/n*1000,'threads':[{'role':t['role'],'affinity':t['affinity'],'userMsPerTick':t['userSeconds']/n*1000,'kernelMsPerTick':t['kernelSeconds']/n*1000} for t in server['threads']]}
result['client']['processCpuCoresMean']=(client['userSeconds']+client['kernelSeconds'])/(end-start)
def core_at(at):
 rows=[v for v in host if 'core' in v];edge=bisect.bisect_left([v['server']['unix'] for v in rows],at);assert 0<edge<len(rows)
 a,b=rows[edge-1:edge+1];f=(at-a['server']['unix'])/(b['server']['unix']-a['server']['unix']);aa=list(map(int,a['core'].split()[1:9]));bb=list(map(int,b['core'].split()[1:9]));return [x+(y-x)*f for x,y in zip(aa,bb)]
a,b=core_at(start),core_at(end);hz=os.sysconf('SC_CLK_TCK');delta=[(y-x)/hz for x,y in zip(a,b)]
result['hostVcpuContextMsPerTick']={key:v/n*1000 for key,v in zip(['user','nice','system','idle','iowait','irq','softirq','steal'],delta)}
result['stealPercent']=100*delta[7]/sum(delta)
result['clientValid'] &= ok
result.update(verdict(client_valid=result['clientValid'],zero_deferrals=result['deferredTicks']==0,start_mature=result['gates']['startInMatureClass'],end_mature=result['gates']['endInMatureClass'],accounting=ok))
process=json.loads((r/'host-run/process.json').read_text());assert process['binarySha256']==pins['binaries']['host']
config=json.loads((r/'host-run/config.json').read_text());assert config['players']['max']==500 and config['bots']['count']==0
for edge in ['start','end']:assert (r/'host-run'/('acceptance-500-'+edge+'-processes.txt')).exists()
assert all((r/'load-run'/('generator-counting-500-'+edge+'.txt')).exists() for edge in ['start','end'])
out={'runtime':pins['runtime'],'pins':pins,'matureScreen':[{k:x for k,x in v.items() if k not in ['unix','tickNanos']} for v in summary['matureScreen']],'result':result,'rawAccess':'Local-only raw records outside repositories; no cloud identifiers published.'}
(r/'selected-results.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'passes':result['passes'],'gates':result['gates'],'p99Ms':result['worldCpuMsPerTick']['p99'],'endMeanLength':result['endpointCensus']['fullWindowAfter']['meanSegments']}),flush=True)
