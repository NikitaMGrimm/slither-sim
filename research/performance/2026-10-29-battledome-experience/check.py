"""Authored controls for aligned CPU, histogram ranks, tail cadence and rejection."""
from pathlib import Path
import tempfile,json,math
from reduce import reduce_window,q,histq
assert q([1,2,3])['p50']==2
assert histq({7000:98,100000:1,300000:1})['p99']==300
with tempfile.TemporaryDirectory() as name:
 r=Path(name);host=[];clients=[]
 for t in range(0,2101,100):
  host.append('CENSUS '+json.dumps({'tick':t,'unix':t*.007,'deferred':0,'living':80,'occupied':80,'boostTotals':[t*80,t*40,t],'meanSegments':26,'p90Segments':30,'p99Segments':40,'food':1000,'boostFood':200,'radius':5667}))
  if t:
   rr=[]
   for tick in range(t-99,t+1):
    spike=tick%72==0;ph=[0]*32;ph[8]=5000000 if spike else 500000;ev=[0]*8;ev[0]=int(spike)
    sim=6000000 if spike else (1000000 if tick%2 else 2000000);io=3000000-sim if not spike else 1000000
    rr.append([sim,io,sim,ph,ev])
   host.append('CAPACITY '+json.dumps({'tick':t,'rows':rr}))
 for i in range(17):
  host.append('SOCKET '+json.dumps({'unix':i,'queueBytes':0,'kernelSendBytes':0,'writeBlocks':0,'bufferFull':0,'rejects':0,'externalPlayers':0}))
  for worker in range(1,7):clients.append('CLIENT '+json.dumps({'unix':i,'worker':worker,'cpuFraction':.1,'lateP99Nanos':100000,'gapsMicros':[[7000,99],[210000,1]],'gapResidualMicros':[[0,99],[5000,1]],'frameExcessMicros':[[0,99],[5000,1]],'initialWaits':[],'respawnWaits':[[worker,2000,0]],'lifetimesMicros':[2000000,5000000,20000000],'boostActions':5,'actions':10,'pendingWaitMicros':[],'frames':1000*i}))
 (r/'host-stream.log').write_text('\n'.join(host));(r/'client.log').write_text('\n'.join(clients))
 w={'label':'synthetic','bots':80,'start':{'unix':0},'end':{'unix':14.7}}
 v=reduce_window(r,w)
 assert v['worldCpuMsPerTick']['p50']==3 and v['worldCpuMsPerTick']['max']==7
 assert v['slowestCpu1Percent']['eventTicks']['foodEpoch']==v['slowestCpu1Percent']['n']
 assert set(v['slowestCpu1Percent']['tickModulo72'])=={0}
 assert v['boost']['aliveTickShare']==.5 and v['clientValid']
 assert v['spawns']['respawns']['p50']==2
 assert v['client']['gapsMs']['max']==210 and v['client']['frameExcessMs']['max']==5
 assert v['client']['residualStalls']['100']==0,'Normal 210 ms emission gaps are not transport stalls'
 authored=[json.loads(line[7:]) for line in (r/'client.log').read_text().splitlines()]
 for value in authored:
  if value['unix']==0:value['frameExcessMicros']=[[500000,100]]
 (r/'client.log').write_text('\n'.join('CLIENT '+json.dumps(value) for value in authored))
 assert reduce_window(r,w)['client']['frameExcessMs']['max']==500
 assert reduce_window(r,w,whole_client_reports=True)['client']['frameExcessMs']['max']==5
 canonical=r/'canonical-host.log';canonical.write_text('\n'.join(host)+'\n')
 (r/'host-stream.log').write_text('\n'.join(line for line in host if not line.startswith('CAPACITY ') or json.loads(line[9:])['tick']<=1000)+'\n')
 partial=reduce_window(r,w);assert partial['ticks']==1000
 complete=reduce_window(r,w,host_log=canonical);assert complete['ticks']==v['ticks'] and complete['ticks']>partial['ticks']
 replay=reduce_window(r,w,host_log=canonical,tick_range=partial['tickRange']);assert replay==partial
 (r/'host-stream.log').write_text('\n'.join(host)+'\n')
 (r/'client.log').write_text((r/'client.log').read_text().replace('"cpuFraction": 0.1','"cpuFraction": 0.9'))
 assert not reduce_window(r,w)['clientValid']
print('Synthetic aligned-clock, cadence, histogram and headroom controls passed')
with tempfile.TemporaryDirectory() as name:
 p=Path(name)/'partial.log';p.write_text('CLIENT {"ok":1}\nCLIENT {"ok":')
 from reduce import rows
 assert rows(p,'CLIENT ')==[{'ok':1}]
print('Unterminated concurrent-log tail control passed')
with tempfile.TemporaryDirectory() as name:
 from delivery import publish
 from unittest.mock import patch
 import delivery,hashlib
 r=Path(name);(r/'run-status.json').write_text('{"status":"stopping"}');(r/'payload').write_text('retained evidence')
 summary={'status':'stopping','outcome':'complete'}
 with patch.object(delivery.gzip,'open',side_effect=RuntimeError('synthetic verification failure')):
  try:publish(r,summary,[r/'payload',r/'run-summary.json'])
  except RuntimeError:pass
  else:raise AssertionError('Failure injection did not run')
 assert json.loads((r/'run-status.json').read_text())['status']=='stopping'
 assert not (r/'results.sha256').exists()
 ready=publish(r,summary,[r/'payload',r/'run-summary.json'])
 assert ready['archiveReady'] and ready['archiveSha256']==hashlib.file_digest((r/'results.tar.gz').open('rb'),'sha256').hexdigest()
 assert ready['archiveSha256']==(r/'results.sha256').read_text().strip()
print('Archive verification failure and terminal-readiness controls passed')
from delivery import cleanup_allowed
assert cleanup_allowed(False,False) and cleanup_allowed(False,True)
assert not cleanup_allowed(True,False) and cleanup_allowed(True,True)
print('Source-cleanup verification guard controls passed')
