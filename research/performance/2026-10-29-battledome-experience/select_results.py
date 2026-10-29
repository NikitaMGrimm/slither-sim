"""Independently rerun window selection, reconcile task clocks and bound public data."""
from pathlib import Path
import json,sys,hashlib,importlib.util,bisect,tarfile
raw=Path(sys.argv[1]);harness=Path(sys.argv[2]);destination=Path(sys.argv[3]);sys.path.insert(0,str(harness))
from reduce import reduce_window,rows,q
spec=importlib.util.spec_from_file_location('accounting',harness/'shared248/select_results.py');prior=importlib.util.module_from_spec(spec);spec.loader.exec_module(prior)
summary=json.loads((raw/'run-summary.json').read_text());pins=json.loads((raw/'pins.json').read_text());process=json.loads((raw/'host-run/process.json').read_text());config=json.loads((raw/'host-run/config.json').read_text())
recovered=[];marks=rows(raw/'host-run/marks.jsonl','')
for label in sorted({x['label'] for x in marks} - {w['label'] for w in summary['windows']}):
 starts=[x for x in marks if x['label']==label and x['active']];ends=[x for x in marks if x['label']==label and not x['active']]
 if len(starts)==len(ends)==1:
  w={'label':label,'bots':int(label.split('-')[1]),'start':starts[0],'end':ends[0]};w['result']=json.loads(json.dumps(reduce_window(raw/'load-run',w)));summary['windows'].append(w);recovered.append(label)
assert pins['runtime']==summary['runtime']=='165a5bb764aba6cbf39f42503e090f9b5e3c4d22'
assert process['binarySha256']==pins['binaries']['host']
assert config['simulation']=={'world':{'preset':'battledome'}} and config['bots']['count']==0 and config['players']['max']==641
host=[json.loads(x) for x in (raw/'host-run/telemetry.jsonl').read_text().splitlines()];client=[json.loads(x) for x in (raw/'load-run/telemetry.jsonl').read_text().splitlines()]
clients=[];sockets=rows(raw/'host-run/host.log','SOCKET ')
with (raw/'load-run/client.log').open() as stream:
 for line in stream:
  if line.startswith('CLIENT ') and line.endswith('\n'):
   value=json.loads(line[7:])
   for key in ['gapsMicros','gapResidualMicros','frameExcessMicros']:value.pop(key,None)
   clients.append(value)
assert not any(v['thin'] or v['forced'] or v['admissionRefusals'] for v in clients)
assert 'panicked at' not in (raw/'load-run/client.log').read_text()
def cpu(samples,member,a,b):return prior.cpu_delta(prior.cpu_at(samples,a,member)[0],prior.cpu_at(samples,b,member)[0])
def collector_cpu(a,b):
 values=[x['collector'] for x in host if x.get('collector')];times=[x['unix'] for x in values]
 def at(t):
  i=bisect.bisect_left(times,t);assert 0<i<len(values);first,last=values[i-1:i+1];span=last['unix']-first['unix'];assert 0<span<2.5;f=(t-first['unix'])/span
  x,y=first['process'],last['process'];assert x['pid']==y['pid'] and x['start']==y['start'] and x['cpus']==y['cpus']
  return {k:x[k]+(y[k]-x[k])*f for k in ['user','system']}
 first,last=at(a),at(b);return {'userSeconds':last['user']-first['user'],'kernelSeconds':last['system']-first['system']}
def core_at(at):
 samples=[x for x in host if 'core' in x];i=bisect.bisect_left([x['server']['unix'] for x in samples],at);assert 0<i<len(samples)
 a,b=samples[i-1:i+1];span=b['server']['unix']-a['server']['unix'];assert 0<span<2.5;f=(at-a['server']['unix'])/span
 return [(int(x)+(int(y)-int(x))*f)/100 for x,y in zip(a['core'].split()[1:],b['core'].split()[1:])]
windows=[];previous_target=0
for w in summary['windows']:
 online=reduce_window(raw/'load-run',w,whole_client_reports=summary.get('clientWholeReports',False),tick_range=w['result']['tickRange']);assert json.loads(json.dumps(online))==w['result'],'Offline values must exactly reproduce the retained online tick range'
 v=reduce_window(raw/'load-run',w,whole_client_reports=True,host_log=raw/'host-run/host.log')
 v['originalOnlineCpuMsPerTick']=online['worldCpuMsPerTick'];v['originalOnlineTickRange']=online['tickRange'];v['originalOnlineTicks']=online['ticks']
 v['onlineClientValid']=online['clientValid'];v['originalOnlineFrameExcessMs']=online['client'].get('frameExcessMs')
 v['client']['reportSelection']='Only worker reports whose preceding report timestamp and end are inside the CPU interval'
 a,b=v.pop('windowUnix');n=v['ticks'];server=cpu(host,'server',a,b);collector=collector_cpu(a,b);generator=cpu(client,'client',a,b)
 task=sum(t['userSeconds']+t['kernelSeconds'] for t in server['threads'] if t['role'].startswith(('slither-simulat','slither-sockets')))/n*1000
 clock=v['worldCpuMsPerTick']['mean'];reconciles=abs(task-clock)<=max(.03,.02*clock);assert reconciles,'Task counters disagree with aligned clocks'
 inventory=[(raw/'host-run'/f"{w['label']}-{edge}-processes.txt").is_file() and (raw/'load-run'/f"{w['label']}-generator-{edge}.txt").is_file() for edge in ['start','end']]
 v['inventoryComplete']=all(inventory)
 if summary['status']=='complete':assert all(inventory)
 v['accounting']={'reconciles':reconciles,'taskWorldMeanMs':task,'clockMinusTaskMs':clock-task,'collectorCpuMsPerTick':(collector['userSeconds']+collector['kernelSeconds'])/n*1000,'hostProcessMsPerTick':(server['userSeconds']+server['kernelSeconds'])/n*1000,'threads':[{'role':t['role'],'userMsPerTick':t['userSeconds']/n*1000,'kernelMsPerTick':t['kernelSeconds']/n*1000,'affinity':t['affinity']} for t in server['threads']]}
 assert all(t['affinity']==[0] for t in v['accounting']['threads'])
 v['client']['processCpuCoresMean']=(generator['userSeconds']+generator['kernelSeconds'])/(b-a)
 reports=[x for x in clients if a-1.1<=x['unix']<=b+1.1]
 v['client']['counterDeltas']={key:sum(max(x[key] for x in reports if x['worker']==i)-min(x[key] for x in reports if x['worker']==i) for i in range(1,7)) for key in ['frames','bytes','decisions','joins','deaths']}
 assert all(max(x['frames'] for x in reports if x['worker']==i)>min(x['frames'] for x in reports if x['worker']==i) for i in range(1,7))
 v['client'].pop('receiptGapHistogramMicros');v['client'].pop('frames')
 censuses=[x for x in rows(raw/'host-run/host.log','CENSUS ') if v['tickRange'][0]<=x['tick']<=v['tickRange'][1]]
 v['alive']['min']=min(x['living'] for x in censuses);v['occupied']['min']=min(x['occupied'] for x in censuses)
 for key in v['census']:v['census'][key].update({'first':censuses[0][key],'last':censuses[-1][key],'min':min(x[key] for x in censuses)})
 cohort=range(previous_target,w['bots']) if w['label'].startswith('up-') else range(0)
 cohort_waits=[row[1]/1000 for x in clients if x['unix']<=b for row in x['initialWaits'] if row[0] in cohort]
 v['spawns']['addedCohortInitial']=q(cohort_waits);v['spawns']['addedCohortSize']=len(cohort)
 previous_target=w['bots']
 # Additional offline cadence diagnostics do not change online stopping rules.
 rr=[(block['tick']-99+i,row) for block in rows(raw/'host-run/host.log','CAPACITY ') if v['tickRange'][0]<block['tick']<=v['tickRange'][1] for i,row in enumerate(block['rows'])]
 top=sorted(rr,key=lambda item:item[1][0]+item[1][1],reverse=True)[:max(1,__import__('math').ceil(len(rr)*.01))]
 events_by_tick={t:row[4] for t,row in rr}
 v['slowestCpu1Percent']['tickModulo100']=dict(__import__('collections').Counter(t%100 for t,_ in top))
 v['slowestCpu1Percent']['foodEpochOrPreviousTick']=sum(row[4][0]>0 or events_by_tick.get(t-1,[0])[0]>0 for t,row in top)
 v['eventConditionCpuMs']={name:{'present':q([(row[0]+row[1])/1e6 for _,row in rr if row[4][index]>0]),'absent':q([(row[0]+row[1])/1e6 for _,row in rr if row[4][index]==0])} for index,name in enumerate(['foodEpoch','minimapDue','leaderboardDue','admissionSearch','strictAccepted','fallbackAccepted','searchRefused','markerChanged'])}
 v['loggingConditionCpuMs']={str(mod):q([(row[0]+row[1])/1e6 for t,row in rr if t%100==mod]) for mod in [0,1,2]}
 ca,cb=core_at(a),core_at(b);v['hostCoreContextMsPerTick']={key:(cb[i]-ca[i])*1000/n for i,key in enumerate(['user','nice','system','idle','iowait','irq','softirq','steal'])}
 windows.append(v)
model=next(x['data'] for x in json.loads((raw/'host-run/cpu-private.json').read_text())['lscpu'] if x['field']=='Model name:')
human=[];active=[]
for v in sockets+[{'externalPlayers':0}]:
 if v.get('externalPlayers',0):active.append(v)
 elif active:
  human.append({'firstUnix':active[0]['unix'],'lastUnix':active[-1]['unix'],'sampleSpanSeconds':active[-1]['unix']-active[0]['unix'],'maximumSeats':max(x['externalPlayers'] for x in active)});active=[]
with tarfile.open(harness/'main.tar') as ar:original={p.name:hashlib.sha256(ar.extractfile(p).read()).hexdigest() for p in ar if p.isfile() and p.name.startswith('src/') and p.name.endswith('.rs')}
changed={k:v for k,v in pins['sourceOverlays'].items() if original.get(k)!=v}
out={'runtime':summary['runtime'],'status':summary['status'],'stopReason':summary.get('stopReason'),'server':{'type':'c7a.medium','vcpus':1,'cpuModel':model},'generator':{'type':'c8a.2xlarge','vcpus':8,'workers':6},'windows':windows,'humanPlayingPeriods':human,'holdSeconds':summary.get('holdSeconds'),'holdStop':summary.get('holdStop'),'pins':{k:pins[k] for k in ['archiveSha256','clientSourceSha256','cpuTarget','toolchain','binaries','priorHarnessSha256']}|{'changedSourceSha256':changed},'rawAccess':'Local-only private archives; no public download. Full receipts, commands, addresses and cloud identifiers are excluded.'}
lifetimes=[us/1e6 for x in clients for us in x['lifetimesMicros']]
out['wholeRunSpawnLifecycle']={'scope':'Warmup, transitions, counted windows, confirmation and hold; completed lives only','lifeSeconds':q(lifetimes),'deathsUnder3s':sum(x<3 for x in lifetimes),'deathsUnder10s':sum(x<10 for x in lifetimes),'initialWaitMs':q([row[1]/1000 for x in clients for row in x['initialWaits']]),'respawnWaitMs':q([row[1]/1000 for x in clients for row in x['respawnWaits']])}
census_all=rows(raw/'host-run/host.log','CENSUS ');out['totalDeferredTicks']=census_all[-1]['deferred']-census_all[0]['deferred']
first_tick=census_all[0]['tick'];out['startupCensus']=[{k:v[k] for k in ['tick','living','occupied','radius','meanSegments']} for v in census_all if v['tick']<=first_tick+3000]
out['deferredOutsideCountedWindows']=out['totalDeferredTicks']-sum(w['deferredTicks'] for w in windows)
out['recoveredCompletedWindows']=recovered;out['workflowFailureType']=summary.get('failureType');out['descending80Collected']=any(w['label']=='down-80' for w in windows)
if len(sys.argv)>4:
 collector_bundle=Path(sys.argv[4]);out['collectorBundleSha256']=hashlib.file_digest(collector_bundle.open('rb'),'sha256').hexdigest()
 with tarfile.open(collector_bundle) as bundle:out['collectorHarnessSha256']={name:hashlib.sha256(bundle.extractfile(name).read()).hexdigest() for name in ['prepare.py','prepare258.py','host_agent.py','run_ladder.py','delivery.py','reduce.py','metrics.py','build.sh']}
out['analysisSha256']={'reduce.py':hashlib.file_digest((harness/'reduce.py').open('rb'),'sha256').hexdigest(),'select_results.py':hashlib.file_digest(Path(__file__).open('rb'),'sha256').hexdigest()}
destination.write_text(json.dumps(out,indent=2)+'\n');print('Selected',len(windows),'windows; all CPU clocks reconciled')
