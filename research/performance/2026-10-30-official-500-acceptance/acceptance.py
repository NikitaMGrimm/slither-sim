"""Amended acceptance gates and report-only tails on the linked ladder reducer."""
from pathlib import Path
import collections,json,math,statistics
from window import reduce_window as ladder_window

def mature(v):
 return v['living']>=490 and 45<=v['meanSegments']<=65 and 156<=v['p90Segments']<=196 and v['p99Segments']>=200

def verdict(*,client_valid,zero_deferrals,start_mature,end_mature,accounting=True):
 gates={'zeroDeferredTicks':zero_deferrals,'startInMatureClass':start_mature,'endInMatureClass':end_mature,'clientsQueuesGenerator':client_valid,'cpuClockReconciliation':accounting}
 return {'gates':gates,'passes':all(gates.values())}

def quantiles(values):
 values=sorted(values);assert values
 return {'n':len(values),'mean':statistics.mean(values),**{k:values[math.ceil((len(values)-1)*p)] for k,p in [('p50',.5),('p99',.99),('p999',.999),('max',1)]}}

def hist_quantiles(rows,key):
 h=collections.Counter()
 for row in rows:
  for value,count in row[key]:h[value]+=count
 n=sum(h.values());assert n>0
 out={'n':n,'mean':sum(k*v for k,v in h.items())/n/1000}
 for name,p in [('p50',.5),('p99',.99),('p999',.999),('max',1)]:
  rank=math.ceil((n-1)*p);count=0
  for value,multiplicity in sorted(h.items()):
   count+=multiplicity
   if count>rank:out[name]=value/1000;break
 return out

def reduce_window(root,window):
 result=ladder_window(root,window)
 cs={};blocks={};reports=[];windows=[]
 for line in (root/'host-stream.log').read_text().splitlines():
  if line.startswith('CENSUS '):v=json.loads(line[7:]);cs[v['tick']]=v
  elif line.startswith('CAPACITY '):v=json.loads(line[9:]);blocks[v['tick']]=v
 first,last=result['tickRange'];keys=list(range(first+100,last+1,100));assert all(t in blocks for t in keys)
 rr=[(t-99+i,row) for t in keys for i,row in enumerate(blocks[t]['rows'])];assert len(rr)==result['ticks']
 def describe(chosen):
  return {'ticks':len(chosen),'worldCpuMsPerTick':quantiles([(r[0]+r[1])/1e6 for _,r in chosen]),'simulationCpuMsPerTick':quantiles([r[0]/1e6 for _,r in chosen]),'socketCpuMsPerTick':quantiles([r[1]/1e6 for _,r in chosen])}
 due=lambda t:math.floor(t*.007/.504)>math.floor((t-1)*.007/.504)
 all_ticks=describe(rr);result.update({k:v for k,v in all_ticks.items() if k!='ticks'})
 result['maintenance']=describe([x for x in rr if due(x[0])]);result['ordinary']=describe([x for x in rr if not due(x[0])])
 before=max((v for v in cs.values() if v['unix']<=window['start']['unix']),key=lambda v:v['unix'])
 after=min((v for v in cs.values() if v['unix']>=window['end']['unix']),key=lambda v:v['unix'])
 clean=lambda v:{k:x for k,x in v.items() if k not in ['unix','tickNanos']}
 result['endpointCensus']={'startInterior':clean(cs[first]),'endInterior':clean(cs[last]),'fullWindowBefore':clean(before),'fullWindowAfter':clean(after),'beforeOffsetSeconds':window['start']['unix']-before['unix'],'afterOffsetSeconds':after['unix']-window['end']['unix']}
 result['deferredTicksInterior']=result['deferredTicks'];result['deferredTicks']=after['deferred']-before['deferred']
 result['requestedWallSeconds']=window['end']['unix']-window['start']['unix']
 assert result['requestedWallSeconds']>=1800,'Do not shorten an acceptance window'
 assert 0<=result['windowUnix'][0]-window['start']['unix']<=1.5 and 0<=window['end']['unix']-result['windowUnix'][1]<=1.5,'Complete interior blocks must cover the requested window, apart from boundary blocks'
 for line in (root/'client.log').read_text().splitlines():
  if line.startswith('CLIENT '):reports.append(json.loads(line[7:]))
  elif line.startswith('CLIENT_WINDOW '):windows.append(json.loads(line[14:]))
 a,b=result['windowUnix'];chosen=[v for v in reports if a-1.1<=v['unix']<=b+1.1]
 deltas={key:sum(max(v[key] for v in chosen if v['worker']==w)-min(v[key] for v in chosen if v['worker']==w) for w in range(1,7)) for key in ['frames','bytes','decisions','joins','deaths','forced','admissionRefusals']}
 progress=all(max(v['frames'] for v in chosen if v['worker']==w)>min(v['frames'] for v in chosen if v['worker']==w) for w in range(1,7))
 result['client']['counterDeltas']=deltas;result['client']['everyWorkerFrameProgress']=progress
 result['client']['maxReceiptAgeSeconds']=max(v['maxReceiptAge'] for v in chosen)
 assert len(windows)==6 and {v['worker'] for v in windows}==set(range(1,7)),'All worker metric windows required'
 result['client']['metricIntervals']=[{'worker':v['worker'],'seconds':v['endUnix']-v['startUnix'],'startOffsetSeconds':v['startUnix']-window['start']['unix'],'endOffsetSeconds':v['endUnix']-window['end']['unix']} for v in windows]
 result['client'].update({name:hist_quantiles(windows,key) for name,key in [('frameGapMs','gapsMicros'),('gapResidualMs','gapResidualMicros'),('frameExcessMs','frameExcessMicros')]})
 result['clientValid'] &= progress and not deltas['forced'] and not deltas['admissionRefusals'] and not any(v['thin'] for v in chosen)
 result.update(verdict(client_valid=result['clientValid'],zero_deferrals=result['deferredTicks']==0,start_mature=mature(cs[first]),end_mature=mature(after)))
 result['p99AtMost7ms']=result['worldCpuMsPerTick']['p99']<=7
 result['generatorInfrastructureFailure']=not result['client']['headroom'] or result['client']['worstInputP99Ms']>=42
 result['hostMetricsQualified']=result['clientValid']
 return result
