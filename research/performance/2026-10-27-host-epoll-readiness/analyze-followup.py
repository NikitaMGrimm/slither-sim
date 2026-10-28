"""Reduce every diagnostic hold, with primary/post windows kept separate."""
from pathlib import Path
import bisect,collections,hashlib,json,os,statistics
if not __debug__:raise SystemExit('run without -O')
os.sched_setaffinity(0,{1});r=Path(__file__).resolve().parent
def read(p):return json.loads(p.read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def nearest(v,t):return min(v,key=lambda x:abs(x['unix']-t))
def threads(v):return {x['name']:x['cpuSeconds'] for g in v['threads'].values() for x in g.values()}
def cpu(s):return list(map(int,s.split()[1:9]))
def busy(a,b):
 d=[y-x for x,y in zip(cpu(a),cpu(b))];n=sum(d)
 return {'busy':100*(n-d[3]-d[4]-d[7])/n,'steal':100*d[7]/n}
def process(v):return sum(x['cpuSeconds'] for x in v['processes'].values() if x)
result=[]
for p in sorted((r/'raw').glob('*/summary.json')):
 h=read(p.parent/'host/host.json');t=read(p.parent/'host/telemetry.json');w=read(p.parent/'host/world.json');s=read(p)
 if s['requested']!=500:continue
 start,end=h['holdStartUnix'],h['holdEndUnix'];origin,ticks=read(p.parent/'host/ticks.json')
 # The frozen probe integrated the already-cumulative death history. Recover
 # actual deaths from conservation in this fixture: empty initial world;
 # the only live disconnect is shutdown, after the 30s post-window. Any other
 # generator error/close invalidates this recovery. Keep the original field.
 load=read(p.parent/'load/load-0.json')
 if load['errors'] or any(k not in ['local: own death','local: shutdown'] for k in load['closeReasons']):raise RuntimeError('population recovery requires death-only reconnects')
 for row in w:
  row['integratedDeathHistory']=row['deaths'];row['deaths']=row['births']-row['alive']
 live=[x for x in w if x['unix']<=end+30]
 if any(b['deaths']<a['deaths'] for a,b in zip(live,live[1:])):raise RuntimeError('non-monotone recovered deaths')
 # The load runs to primary-end+30 then keeps sockets live for two seconds.
 # Its final observed deaths bracket the last <=1s server snapshot in the observed quiet smokes; flag disagreement for diagnosis.
 if not (live[-1]['deaths']<=load['counters'].get('deaths',0)<=live[-1]['deaths']+2):raise RuntimeError('recovered count disagrees with load')
 completed=sorted(origin+(a+b)*1e-9 for a,b,c in ticks)
 own={str(x) for role in ['host','load'] for x in read(p.parent/role/'pids.json')}
 seconds=[]
 for a,b in zip(t['rows'],t['rows'][1:]):
  if b['unix']<start or b['unix']>end+30.1:continue
  elapsed=b['unix']-a['unix'];ta,tb=threads(a),threads(b);world=nearest(w,b['unix']);wa=nearest(w,a['unix'])
  n=bisect.bisect_right(completed,b['unix'])-bisect.bisect_right(completed,a['unix'])
  sim=tb.get('slither-simulat',0)-ta.get('slither-simulat',0);sock=tb.get('slither-sockets',0)-ta.get('slither-sockets',0)
  other=collections.defaultdict(float)
  for pid,x in a['allProcesses'].items():
   y=b['allProcesses'].get(pid)
   if pid not in own and y and x['startTicks']==y['startTicks']:
    other[x['comm']]+=y['userSeconds']+y['systemSeconds']-x['userSeconds']-x['systemSeconds']
  c0=busy(a['diagnostic']['cpu0Counters'],b['diagnostic']['cpu0Counters']);mach=busy(a['cpu'],b['cpu'])
  seconds.append(dict(second=b['unix']-start,seconds=elapsed,ticks=n,hostCpuPercent=100*(process(b)-process(a))/elapsed,simCpuPercent=100*sim/elapsed,socketCpuPercent=100*sock/elapsed,simCpuMsPerTick=1000*sim/n if n else None,socketCpuMsPerTick=1000*sock/n if n else None,backlog=world['backlog'],deferred=world['deferred'],worldSampleSkewSeconds=world['unix']-b['unix'],world=world,core0=c0,machine=mach,otherCpuSeconds=dict(other)))
 (p.parent/'diagnostic-seconds.json').write_text(json.dumps(seconds)+'\n')
 windows=[]
 for label,lo,hi in [('primary',0,120),('post',120,150),('early',0,90),('late',90,120)]:
  z=[x for x in seconds if lo<x['second']<=hi+.05];sec=sum(x['seconds'] for x in z);n=sum(x['ticks'] for x in z)
  ww=[x for x in w if start+lo<=x['unix']<=start+hi];other=collections.defaultdict(float)
  for x in z:
   for k,v in x['otherCpuSeconds'].items():other[k]+=v
  episodes=[]
  for i in range(1,len(z)):
   x,y=z[i-1],z[i]
   # The plan counts any newly deferred tick as an episode, as well as sustained saturation.
   if (y['deferred']>x['deferred'] or (x['hostCpuPercent']>=95 and y['hostCpuPercent']>=95 and min(x['backlog'],y['backlog'])>=4 and y['backlog']>x['backlog'])):episodes.append(round(y['second'],3))
  a,b=nearest(w,start+lo),nearest(w,start+hi)
  marginal=[{k:x[k] for k in ['second','hostCpuPercent','simCpuPercent','socketCpuPercent','simCpuMsPerTick','socketCpuMsPerTick','backlog','deferred']} for x in z if x['hostCpuPercent']>=95 or x['backlog']>=4]
  tt=[x for x in ticks if start+lo<=origin+x[0]*1e-9<start+hi]
  windows.append(dict(label=label,ticks=n,**{k:sum(x[k]*x['seconds'] for x in z)/sec for k in ['hostCpuPercent','simCpuPercent','socketCpuPercent']},simCpuMsPerTick=sum(x['simCpuPercent']*x['seconds']*.01 for x in z)*1000/n,socketCpuMsPerTick=sum(x['socketCpuPercent']*x['seconds']*.01 for x in z)*1000/n,deferred=b['deferred']-a['deferred'],backlogMax=max(x['backlog'] for x in z),saturationSamples=episodes,marginalSamples=marginal,tickBusyMaxMs=max(x[1] for x in tt)*1e-6,tickStartGapMaxMs=max(x[2] for x in tt)*1e-6,world={k:{'first':a[k],'last':b[k],'min':min(x[k] for x in ww),'max':max(x[k] for x in ww)} for k in ['alive','anchors','segments','mass','food','radius']},births=b['births']-a['births'],deaths=b['deaths']-a['deaths'],otherCpuPercent={k:100*v/sec for k,v in sorted(other.items(),key=lambda x:-x[1])[:8]},core0BusyPercent=sum(x['core0']['busy']*x['seconds'] for x in z)/sec,core0StealPercent=sum(x['core0']['steal']*x['seconds'] for x in z)/sec,machineBusyPercent=sum(x['machine']['busy']*x['seconds'] for x in z)/sec))
 result.append(dict(attempt=p.parent.name,revision=s['commit'],cpuMsPerTick=s['cpuMsPerExecutedTick'],excessP99Ms=s['excessMs']['p99'],deferred=s['deferredTicks'],validTraffic=s['validTraffic'],validAffinity=s['validAffinity'],generatorHasHeadroom=s['generatorHasHeadroom'],generatorCpuPercent=max(s['generatorCpuPercentByPinnedCore'].values()),summarySha256=sha(p),windows=windows))
(r/'followup-diagnosis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
