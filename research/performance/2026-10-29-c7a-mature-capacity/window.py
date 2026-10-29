"""Online ladder reduction; primary quantiles and queue helpers are linked."""
from pathlib import Path
import json,statistics,math
from metrics import prior,q,slope
def reduce_window(root,window):
 cs={};capacity={};sockets=[];stats=[]
 for line in (root/'host-stream.log').read_text().splitlines():
  if line.startswith('CENSUS '):v=json.loads(line[7:]);cs[v['tick']]=v
  elif line.startswith('CAPACITY '):v=json.loads(line[9:]);capacity[v['tick']]=v
  elif line.startswith('SOCKET '):sockets.append(json.loads(line[7:]))
 stats=[json.loads(x) for x in (root/'stats.jsonl').read_text().splitlines()]
 start=window['start']['unix'];end=window['end']['unix'];target=window['bots']
 ticks=sorted(t for t in capacity if t-100 in cs and cs[t-100]['unix']>=start and cs[t]['unix']<=end)
 assert len(ticks)>=10,'not enough complete blocks'
 blocks=[capacity[t] for t in ticks];assert all(len(b['rows'])==100 for b in blocks)
 rows=[r for b in blocks for r in b['rows']];n=len(rows);a=cs[ticks[0]-100]['unix'];b=cs[ticks[-1]]['unix']
 clients=[json.loads(x[7:]) for x in (root/'client.log').read_text().splitlines() if x.startswith('CLIENT ')]
 # Include overlapping one-second client reports at both boundaries.
 cr=[v for v in clients if a-1.1<=v['unix']<=b+1.1];assert {v['worker'] for v in cr}==set(range(1,7))
 workers={str(w):q([v['cpuFraction'] for v in cr if v['worker']==w]) for w in range(1,7)}
 cpu_headroom=all(v['p50']<.7 and v['p99']<.8 for v in workers.values());late=max(v['lateP99Nanos'] for v in cr)/1e6
 ss=[v for v in sockets if a<=v['unix']<=b];assert len(ss)>=5
 left=max((x for x in sockets if x['unix']<=a),key=lambda x:x['unix']);right=min((x for x in sockets if x['unix']>=b),key=lambda x:x['unix'])
 errors={k:right[k]-left[k] for k in ['writeBlocks','bufferFull','rejects']}
 queues={key:{'maxBytes':max(v[key] for v in ss),'slopeBytesPerSecond':slope(ss,key),'firstThirdMean':statistics.mean(v[key] for v in ss[:len(ss)//3]),'lastThirdMean':statistics.mean(v[key] for v in ss[-len(ss)//3:])} for key in ['queueBytes','kernelSendBytes']}
 stable=all(v['slopeBytesPerSecond']<8192 and v['lastThirdMean']<=v['firstThirdMean']+524288 for v in queues.values())
 census=[cs[ticks[0]-100],*[cs[t] for t in ticks]];deferred=census[-1]['deferred']-census[0]['deferred']
 actual=[v for v in stats if a<=v['unix']<=b];assert actual
 human=[v.get('externalPlayers',0) for v in sockets if a-1.1<=v['unix']<=b+1.1]
 population=min(v['players'] for v in ss)>=target-5 and statistics.mean(v['living'] for v in census)>=target*.98
 log=(root/'client.log').read_text();clean='panicked at' not in log and 'CLIENT_ERROR' not in log
 valid=cpu_headroom and late<42 and stable and clean and errors['writeBlocks']==0 and errors['bufferFull']==0 and errors['rejects']==0 and population
 world=q([(s+i)/1e6 for s,i,_ in rows]);passes=valid and deferred==0 and world['p50']<7
 result={'label':window['label'],'bots':target,'ticks':n,'tickRange':[ticks[0]-100,ticks[-1]],'windowUnix':[a,b],'wallSeconds':b-a,'ticksPerSecond':n/(b-a),'worldCpuMsPerTick':world,'simulationCpuMsPerTick':q([s/1e6 for s,_,_ in rows]),'socketCpuMsPerTick':q([i/1e6 for _,i,_ in rows]),'deferredTicks':deferred,'clientValid':valid,'passes':passes,'client':{'headroom':cpu_headroom,'workerCpuFraction':workers,'worstInputP99Ms':late},'socket':{'errors':errors,'queues':queues,'playersRange':[min(v['players'] for v in ss),max(v['players'] for v in ss)]},'human':{'externalPlayingRange':[min(human),max(human)],'affected':max(human)>0,'statsPlayersRange':[min(v['players'] for v in actual),max(v['players'] for v in actual)]},'census':{k:{'first':census[0][k],'last':census[-1][k],'min':min(v[k] for v in census),'max':max(v[k] for v in census)} for k in ['living','occupied','meanSegments','p90Segments','p99Segments','food','prey','radius']},'phaseMeanCpuMsPerTick':{name:sum(v['stages'][i] for v in blocks)/n/1e6 for i,name in enumerate(prior.STAGES[:9])}}
 # A failed generator/queue gate is a limit, not a publishable host number.
 if not valid:result['hostMetricsQualified']=False
 else:result['hostMetricsQualified']=True
 return result
