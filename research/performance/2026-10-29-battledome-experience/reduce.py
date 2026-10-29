"""Reduce retained private per-tick and real-client receipts; emit bounded aggregates."""
from pathlib import Path
import json,math,statistics,collections,re
PHASES=['movement','collisionBuild','collisionQuery','deathFood','pickupFoodCache','prey','ambient','admission','broadcast']
NESTED={16:'sharedFood',17:'coverage',18:'recipients',19:'minimap',20:'killBoard',21:'markerUpdate'}
EVENTS=['foodEpoch','minimapDue','leaderboardDue','admissionSearch','strictAccepted','fallbackAccepted','searchRefused','markerChanged']
def quantile(v,p):
 s=sorted(v);return s[math.ceil((len(s)-1)*p)] if s else None
def q(v):return {k:quantile(v,p) for k,p in [('p50',.5),('p95',.95),('p99',.99),('max',1)]}|{'mean':statistics.mean(v) if v else None,'n':len(v)}
def histq(h):
 n=sum(h.values());out={'n':n}
 if not n:return out|{k:None for k in ['p50','p95','p99','max','mean']}
 for name,p in [('p50',.5),('p95',.95),('p99',.99),('max',1)]:
  rank=math.ceil((n-1)*p);c=0
  for key,count in sorted(h.items()):
   c+=count
   if c>rank:out[name]=key/1000;break
 out['mean']=sum(k*v for k,v in h.items())/n/1000;return out
def rows(path,prefix):
 with path.open() as stream:return [json.loads(s[len(prefix):]) for s in stream if s.endswith('\n') and s.startswith(prefix)]
def client_reports(path,a,b,whole):
 previous={};chosen=[]
 with path.open() as stream:
  for line in stream:
   if not line.endswith('\n') or not line.startswith('CLIENT '):continue
   # Writer-owned numeric fields; full selected records still use the JSON
   # decoder. Avoid retaining or decoding unrelated histogram arrays.
   at=float(re.search(r'"unix"\s*:\s*([-+0-9.eE]+)',line)[1]);worker=int(re.search(r'"worker"\s*:\s*(\d+)',line)[1]);prior=previous.get(worker);previous[worker]=at
   if at<=b and (prior is not None and prior>=a if whole else a<=at):chosen.append(json.loads(line[7:]))
 return chosen
def reduce_window(root,w,whole_client_reports=False,host_log=None,tick_range=None):
 host_log=host_log or root/'host-stream.log'
 cs={v['tick']:v for v in rows(host_log,'CENSUS ')};cap=rows(host_log,'CAPACITY ')
 blocks=[v for v in cap if v['tick']-100 in cs and cs[v['tick']-100]['unix']>=w['start']['unix'] and cs[v['tick']]['unix']<=w['end']['unix']]
 if tick_range is not None:blocks=[v for v in blocks if tick_range[0]<v['tick']<=tick_range[1]]
 assert len(blocks)>=10;assert all(len(v['rows'])==100 for v in blocks)
 a=cs[blocks[0]['tick']-100]['unix'];b=cs[blocks[-1]['tick']]['unix'];rr=[(v['tick']-99+i,row) for v in blocks for i,row in enumerate(v['rows'])];n=len(rr)
 cr=client_reports(root/'client.log',a,b,whole_client_reports)
 ss=[v for v in rows(host_log,'SOCKET ') if a<=v['unix']<=b];all_ss=rows(host_log,'SOCKET ')
 assert {v['worker'] for v in cr}==set(range(1,7));assert len(ss)>=5
 census=[cs[blocks[0]['tick']-100],*[cs[v['tick']] for v in blocks]]
 workers={str(i):q([v['cpuFraction'] for v in cr if v['worker']==i]) for i in range(1,7)}
 late=max(v['lateP99Nanos'] for v in cr)/1e6
 h=collections.Counter()
 for v in cr:
  for key,count in v['gapsMicros']:h[key]+=count
 residuals=collections.Counter();excess=collections.Counter()
 for v in cr:
  for key,count in v.get('gapResidualMicros',[]):residuals[key]+=count
  for key,count in v.get('frameExcessMicros',[]):excess[key]+=count
 first=max((v for v in all_ss if v['unix']<=a),key=lambda x:x['unix']);last=min((v for v in all_ss if v['unix']>=b),key=lambda x:x['unix'])
 errors={k:last[k]-first[k] for k in ['writeBlocks','bufferFull','rejects']}
 queues={}
 for key in ['queueBytes','kernelSendBytes']:
  x=[v['unix']-a for v in ss];y=[v[key] for v in ss];xm=statistics.mean(x);ym=statistics.mean(y)
  slope=sum((xx-xm)*(yy-ym) for xx,yy in zip(x,y))/sum((xx-xm)**2 for xx in x)
  third=max(1,len(y)//3);queues[key]={'max':max(y),'slopeBytesPerSecond':slope,'firstThirdMean':statistics.mean(y[:third]),'lastThirdMean':statistics.mean(y[-third:])}
 stable=all(v['slopeBytesPerSecond']<8192 and v['lastThirdMean']<=v['firstThirdMean']+524288 for v in queues.values())
 valid=all(v['p50']<.7 and v['p99']<.8 for v in workers.values()) and late<42 and stable and not any(errors.values())
 def phases(chosen):
  out={name:sum(row[3][i] for _,row in chosen)/len(chosen)/1e6 for i,name in enumerate(PHASES)}
  out['socket']=statistics.mean(row[1] for _,row in chosen)/1e6
  out['simulationOther']=statistics.mean(row[0]-sum(row[3][:9]) for _,row in chosen)/1e6
  return out
 def tail(chosen):
  return {'n':len(chosen),'worldMeanMs':statistics.mean(row[0]+row[1] for _,row in chosen)/1e6,'phasesMs':phases(chosen),'nestedMs':{name:statistics.mean(row[3][i] for _,row in chosen)/1e6 for i,name in NESTED.items()},'eventTicks':{name:sum(row[4][i]>0 for _,row in chosen) for i,name in enumerate(EVENTS)},'tickModulo72':dict(collections.Counter(t%72 for t,_ in chosen))}
 top=max(1,math.ceil(n*.01));slow=sorted(rr,key=lambda x:x[1][0]+x[1][1],reverse=True)[:top];wall=sorted(rr,key=lambda x:x[1][2],reverse=True)[:top]
 initial=[row[1]/1000 for v in cr for row in v['initialWaits']];respawn=[row[1]/1000 for v in cr for row in v['respawnWaits']];pending=[x/1000 for v in cr for x in v['pendingWaitMicros']];lifetimes=[x/1e6 for v in cr for x in v['lifetimesMicros']]
 # Initial cohort completions occur before primary windows; preserve all to this edge.
 all_initial=[]
 with (root/'client.log').open() as stream:
  for line in stream:
   if not line.endswith('\n') or not line.startswith('CLIENT '):continue
   if '"initialWaits":[]' in line:continue
   v=json.loads(line[7:])
   if v['unix']<=b:all_initial.extend(row[1]/1000 for row in v['initialWaits'] if row[0]<w['bots'])
 bt=[census[-1]['boostTotals'][i]-census[0]['boostTotals'][i] for i in range(3)]
 boost_actions=sum(v['boostActions'] for v in cr);actions=sum(v['actions'] for v in cr)
 result={'label':w['label'],'bots':w['bots'],'ticks':n,'windowUnix':[a,b],'tickRange':[rr[0][0]-1,rr[-1][0]],'wallSeconds':b-a,'worldCpuMsPerTick':q([(row[0]+row[1])/1e6 for _,row in rr]),'simulationCpuMsPerTick':q([row[0]/1e6 for _,row in rr]),'socketCpuMsPerTick':q([row[1]/1e6 for _,row in rr]),'tickBusyMs':q([row[2]/1e6 for _,row in rr]),'deferredTicks':census[-1]['deferred']-census[0]['deferred'],'clientValid':valid,'client':{'workerCpuFraction':workers,'worstInputP99Ms':late,'gapsMs':histq(h),'stalls':{str(th):sum(count for key,count in h.items() if key>th*1000) for th in [84,100,250]},'receiptGapHistogramMicros':sorted(h.items()),'frames':max(v['frames'] for v in cr),'boostActionShare':boost_actions/max(1,actions)},'socket':{'errors':errors,'queues':queues},'alive':q([v['living'] for v in census]),'occupied':q([v['occupied'] for v in census]),'census':{k:q([v[k] for v in census]) for k in ['meanSegments','p90Segments','p99Segments','food','boostFood','radius']},'boost':{'aliveTicks':bt[0],'boostingTicks':bt[1],'aliveTickShare':bt[1]/max(1,bt[0]),'foodBirths':bt[2]},'spawns':{'initialCompletedThroughWindow':q(all_initial),'newInitialCompletions':q(initial),'respawns':q(respawn),'pendingMaxMs':max(pending,default=0),'pendingSampleCount':len(pending),'pendingAtLastWorkerReports':sum(len(max((v for v in cr if v['worker']==i),key=lambda x:x['unix'])['pendingWaitMicros']) for i in range(1,7)),'lifeSeconds':q(lifetimes),'deathsUnder3s':sum(x<3 for x in lifetimes),'deathsUnder10s':sum(x<10 for x in lifetimes)},'human':{'externalPlayingRange':[min(v.get('externalPlayers',0) for v in ss),max(v.get('externalPlayers',0) for v in ss)]},'allTicks':tail(rr),'slowestCpu1Percent':tail(slow),'slowestWall1Percent':tail(wall)}
 if residuals:result['client'].update({'gapResidualMs':histq(residuals),'frameExcessMs':histq(excess),'residualStalls':{str(th):sum(count for key,count in residuals.items() if key>th*1000) for th in [20,84,100,250]}})
 return result
if __name__=='__main__':
 import sys
 root=Path(sys.argv[1]);summary=json.loads((root/'run-summary.json').read_text());out={'runtime':summary['runtime'],'stopReason':summary.get('stopReason'),'status':summary['status'],'windows':[reduce_window(root/'load-run',w) for w in summary['windows']]}
 Path(sys.argv[2]).write_text(json.dumps(out,indent=2)+'\n')
